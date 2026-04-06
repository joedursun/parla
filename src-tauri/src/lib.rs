mod audio;
mod llm;
mod stt;
mod tts;
mod vad;

use audio::pipeline::AudioState;
use llm::parser::{ParsedTutorResponse, StreamingJsonParser};
use llm::prompt::{spanish_cafe_system_prompt, ChatMessage};
use llm::{GenChunk, LlmState};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, RunEvent, State};

/// Resolve the models directory: <app_data_dir>/models/
fn models_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to get app data dir: {e}"))?;
    let dir = data_dir.join("models");
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create models dir: {e}"))?;
    Ok(dir)
}

/// Find a Gemma GGUF model file in the given directory.
/// Accepts any file matching `gemma*.gguf` (Q4_K_M, Q4_0, etc).
fn find_gemma_model(dir: &PathBuf) -> Result<PathBuf, String> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        // Prefer Q4_K_M > Q5_K_M > Q4_0, but accept any gemma .gguf.
        let mut candidates: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                name.contains("gemma") && name.ends_with(".gguf")
            })
            .collect();
        candidates.sort_by_key(|p| {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
            // Lower score = preferred.
            if n.contains("q4_k_m") { 0 }
            else if n.contains("q5_k_m") { 1 }
            else if n.contains("q4_0") { 2 }
            else { 3 }
        });
        if let Some(p) = candidates.into_iter().next() {
            return Ok(p);
        }
    }
    Err(format!(
        "No Gemma .gguf model found in {}. Run ./setup.sh to download.",
        dir.display()
    ))
}

/// Find a Whisper model file in the given directory.
fn find_whisper_model(dir: &PathBuf) -> Result<PathBuf, String> {
    let patterns = [
        "ggml-small.bin",
        "ggml-small.en.bin",
        "ggml-base.en.bin",
        "ggml-base.bin",
        "ggml-medium.bin",
        "ggml-medium.en.bin",
        "ggml-large-v3-turbo.bin",
        "ggml-large-v3.bin",
        "ggml-tiny.bin",
    ];
    for name in patterns {
        let path = dir.join(name);
        if path.exists() {
            return Ok(path);
        }
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("ggml-") && name_str.ends_with(".bin") {
                return Ok(entry.path());
            }
        }
    }
    Err(format!(
        "No Whisper model found in {}. Run ./setup.sh to download models.",
        dir.display()
    ))
}

// ── Tauri commands ─────────────────────────────────────────────────────────
// All commands that do real work are async to avoid blocking the main thread.

#[tauri::command]
async fn start_recording(state: State<'_, AudioState>) -> Result<(), String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || state.start_recording())
        .await
        .map_err(|e| format!("{e}"))?
}

#[tauri::command]
async fn stop_recording(state: State<'_, AudioState>) -> Result<StopRecordingResult, String> {
    let state = state.inner().clone();
    let result = tokio::task::spawn_blocking(move || state.stop_recording())
        .await
        .map_err(|e| format!("{e}"))??;
    Ok(make_recording_result(&result))
}

#[tauri::command]
async fn stop_recording_and_transcribe(
    state: State<'_, AudioState>,
) -> Result<TranscriptionResult, String> {
    let state = state.inner().clone();
    let result = tokio::task::spawn_blocking(move || {
        let rec = state.stop_recording()?;
        let segments: Vec<(usize, usize)> = rec
            .speech_segments
            .iter()
            .map(|s| (s.start_sample, s.end_sample))
            .collect();
        let transcription = state.transcribe(&rec.samples, &segments).unwrap_or_default();
        Ok::<_, String>((rec, transcription))
    })
    .await
    .map_err(|e| format!("{e}"))??;

    let (rec, transcription) = result;
    let recording = make_recording_result(&rec);
    Ok(TranscriptionResult {
        duration_ms: recording.duration_ms,
        sample_count: recording.sample_count,
        speech_segments: recording.speech_segments,
        transcription,
    })
}

#[tauri::command]
async fn loopback_test(state: State<'_, AudioState>) -> Result<StopRecordingResult, String> {
    let state = state.inner().clone();
    let result = tokio::task::spawn_blocking(move || {
        let rec = state.stop_recording()?;
        let recording = make_recording_result(&rec);
        state.play_audio(rec.samples, 16000)?;
        Ok::<_, String>(recording)
    })
    .await
    .map_err(|e| format!("{e}"))?;
    result
}

#[tauri::command]
async fn speak_text(state: State<'_, AudioState>, text: String) -> Result<(), String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let samples = state.synthesize(&text)?;
        if !samples.is_empty() {
            state.play_audio(samples, 24000)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
async fn stop_playback(state: State<'_, AudioState>) -> Result<(), String> {
    state.stop_playback();
    Ok(())
}

#[tauri::command]
async fn audio_status(state: State<'_, AudioState>) -> Result<AudioStatusResult, String> {
    let status = state.status();
    Ok(AudioStatusResult {
        is_recording: status.is_recording,
        is_playing: status.is_playing,
        vad_active: status.vad_active,
        stt_ready: status.stt_ready,
        tts_ready: status.tts_ready,
        speech_detected: status.speech_detected,
    })
}

#[tauri::command]
async fn check_models(app: tauri::AppHandle) -> Result<ModelStatus, String> {
    let dir = models_dir(&app)?;
    Ok(ModelStatus {
        vad: dir.join("silero_vad.onnx").exists(),
        stt: find_whisper_model(&dir).is_ok(),
        llm: find_gemma_model(&dir).is_ok(),
        models_dir: dir.to_string_lossy().to_string(),
    })
}

#[tauri::command]
async fn llm_status(state: State<'_, LlmState>) -> Result<LlmStatusResult, String> {
    let s = state.inner().clone();
    let status = tokio::task::spawn_blocking(move || s.status())
        .await
        .map_err(|e| format!("{e}"))?;
    Ok(LlmStatusResult {
        loaded: status.loaded,
    })
}

/// Reset the conversation history for the current session.
#[tauri::command]
async fn reset_conversation(history: State<'_, ConversationHistory>) -> Result<(), String> {
    history.inner().clear();
    Ok(())
}

/// Run one conversation turn: student text in → streamed tutor response out.
/// Streams incremental text via `tutor-message-chunk` events, synthesizes
/// complete sentences to TTS as they form, and emits a final
/// `tutor-message-done` event with the parsed structured response.
#[tauri::command]
async fn conversation_turn(
    app: tauri::AppHandle,
    audio: State<'_, AudioState>,
    llm: State<'_, LlmState>,
    history: State<'_, ConversationHistory>,
    cancel: State<'_, CancelFlag>,
    student_text: String,
) -> Result<ConversationTurnResult, String> {
    let student_text = student_text.trim().to_string();
    if student_text.is_empty() {
        return Err("empty student message".into());
    }

    // Reset cancel flag for this new turn.
    let cancel_flag = cancel.inner().0.clone();
    cancel_flag.store(false, Ordering::Relaxed);

    // Build the message list from history + new user turn.
    let mut messages = history.inner().messages_with_system();
    messages.push(ChatMessage::user(student_text.clone()));

    // Record the student turn immediately.
    history.inner().push(ChatMessage::user(student_text.clone()));

    let llm = llm.inner().clone();
    let audio_handle = audio.inner().clone();
    let app_handle = app.clone();
    let cancel_for_llm = cancel_flag.clone();

    // Do the heavy work on a blocking task so we don't hog the tokio runtime.
    let result = tokio::task::spawn_blocking(move || -> Result<ConversationTurnResult, String> {
        let chunk_rx = llm.generate(messages, cancel_for_llm.clone())?;

        let mut parser = StreamingJsonParser::new();
        let mut full_text = String::new();

        // Pull chunks until Done or Error.
        loop {
            let chunk = match chunk_rx.recv() {
                Ok(c) => c,
                Err(_) => break,
            };
            match chunk {
                GenChunk::Text(t) => {
                    full_text.push_str(&t);
                    parser.push(&t);

                    // Emit raw text chunks to the UI so it can render
                    // progressively (e.g. a typing indicator or streaming bubble).
                    let _ = app_handle.emit("tutor-message-chunk", &t);

                    // Extract any newly-complete sentences and dispatch to TTS.
                    for sentence in parser.take_sentences() {
                        if cancel_for_llm.load(Ordering::Relaxed) {
                            break;
                        }
                        if let Ok(samples) = audio_handle.synthesize(&sentence) {
                            if !samples.is_empty() {
                                let _ = audio_handle.play_audio(samples, 24000);
                            }
                        }
                        let _ = app_handle.emit("tutor-sentence", &sentence);
                    }
                }
                GenChunk::Done { full_text: final_text } => {
                    full_text = final_text;
                    break;
                }
                GenChunk::Error(e) => {
                    return Err(e);
                }
            }
        }

        // Flush any final sentence (the streaming parser marks itself Finished
        // when the closing quote arrives, so take_sentences will emit the tail).
        for sentence in parser.take_sentences() {
            if cancel_for_llm.load(Ordering::Relaxed) {
                break;
            }
            if let Ok(samples) = audio_handle.synthesize(&sentence) {
                if !samples.is_empty() {
                    let _ = audio_handle.play_audio(samples, 24000);
                }
            }
            let _ = app_handle.emit("tutor-sentence", &sentence);
        }

        // Final structured parse.
        let parsed = ParsedTutorResponse::from_streamed(&full_text);
        let (tutor_target, tutor_native, structured_err) = match &parsed {
            Ok(p) => (
                p.tutor_message.target_lang.clone(),
                p.tutor_message.native_lang.clone(),
                None,
            ),
            Err(e) => {
                eprintln!("[llm] final JSON parse failed: {e}");
                // Best effort: use whatever the streaming parser captured.
                (parser.captured().to_string(), String::new(), Some(e.clone()))
            }
        };

        Ok(ConversationTurnResult {
            raw: full_text,
            tutor_target,
            tutor_native,
            parsed: parsed.ok(),
            parse_error: structured_err,
        })
    })
    .await
    .map_err(|e| format!("{e}"))??;

    // Store the assistant turn in history (use the raw JSON so the next turn
    // sees exactly what the model emitted).
    history.inner().push(ChatMessage::assistant(result.raw.clone()));

    // Emit the final done event with the structured response.
    let _ = app.emit("tutor-message-done", &result);

    Ok(result)
}

/// Cancel any in-flight LLM generation for the current turn.
#[tauri::command]
async fn cancel_generation(cancel: State<'_, CancelFlag>) -> Result<(), String> {
    cancel.inner().0.store(true, Ordering::Relaxed);
    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn make_recording_result(result: &audio::pipeline::RecordingResult) -> StopRecordingResult {
    let duration_ms = (result.samples.len() as f64 / 16000.0 * 1000.0) as u64;
    StopRecordingResult {
        duration_ms,
        sample_count: result.samples.len(),
        speech_segments: result
            .speech_segments
            .iter()
            .map(|s| SpeechSegmentResult {
                start_ms: (s.start_sample as f64 / 16.0) as u64,
                end_ms: (s.end_sample as f64 / 16.0) as u64,
            })
            .collect(),
    }
}

// ── Response types ─────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct SpeechSegmentResult {
    start_ms: u64,
    end_ms: u64,
}

#[derive(serde::Serialize)]
struct StopRecordingResult {
    duration_ms: u64,
    sample_count: usize,
    speech_segments: Vec<SpeechSegmentResult>,
}

#[derive(serde::Serialize)]
struct TranscriptionResult {
    duration_ms: u64,
    sample_count: usize,
    speech_segments: Vec<SpeechSegmentResult>,
    transcription: String,
}

#[derive(serde::Serialize)]
struct AudioStatusResult {
    is_recording: bool,
    is_playing: bool,
    vad_active: bool,
    stt_ready: bool,
    tts_ready: bool,
    speech_detected: bool,
}

#[derive(serde::Serialize)]
struct ModelStatus {
    vad: bool,
    stt: bool,
    llm: bool,
    models_dir: String,
}

#[derive(serde::Serialize)]
struct LlmStatusResult {
    loaded: bool,
}

#[derive(Clone, serde::Serialize)]
struct ConversationTurnResult {
    /// The raw text the LLM emitted (full JSON).
    raw: String,
    /// The tutor's spoken line (target language) — unescaped from raw JSON.
    tutor_target: String,
    /// English translation — empty if parse failed.
    tutor_native: String,
    /// Fully parsed structured response, or None if parse failed.
    parsed: Option<ParsedTutorResponse>,
    /// Parse error message, if any.
    parse_error: Option<String>,
}

/// Conversation history held in memory for the current session.
/// Cleared on `reset_conversation` or app restart.
#[derive(Clone)]
pub struct ConversationHistory {
    inner: Arc<Mutex<Vec<ChatMessage>>>,
}

impl ConversationHistory {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn clear(&self) {
        self.inner.lock().unwrap().clear();
    }

    fn push(&self, msg: ChatMessage) {
        self.inner.lock().unwrap().push(msg);
    }

    /// Return the full message list prefixed with the system prompt.
    fn messages_with_system(&self) -> Vec<ChatMessage> {
        let hist = self.inner.lock().unwrap();
        let mut out = Vec::with_capacity(hist.len() + 1);
        out.push(ChatMessage::system(spanish_cafe_system_prompt()));
        out.extend(hist.iter().cloned());
        out
    }
}

/// Cancel flag for in-flight LLM generation. Set to true to request abort.
#[derive(Clone)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}

// ── App entry ──────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AudioState::new())
        .manage(LlmState::new())
        .manage(ConversationHistory::new())
        .manage(CancelFlag::new())
        .setup(|app| {
            let handle = app.handle().clone();
            let state: AudioState = app.state::<AudioState>().inner().clone();
            let llm_state: LlmState = app.state::<LlmState>().inner().clone();

            // Init audio devices immediately (fast, needed for everything)
            if let Err(e) = state.init() {
                eprintln!("[setup] audio init failed: {e}");
            }

            // Load all models in a background thread — never blocks the UI
            std::thread::spawn(move || {
                println!("[duo] background model loading started");

                let data_dir = match handle.path().app_data_dir() {
                    Ok(d) => d,
                    Err(e) => {
                        println!("[duo] failed to get app data dir: {e}");
                        return;
                    }
                };
                let mdir = data_dir.join("models");
                let _ = std::fs::create_dir_all(&mdir);

                // VAD (~2 MB, loads in <1s)
                let vad_path = mdir.join("silero_vad.onnx");
                if vad_path.exists() {
                    println!("[duo] loading VAD...");
                    match state.init_vad(vad_path) {
                        Ok(()) => println!("[duo] VAD loaded"),
                        Err(e) => println!("[duo] VAD failed: {e}"),
                    }
                } else {
                    println!("[duo] VAD model not found, skipping");
                }

                // TTS (macOS say is instant; Kokoro ONNX ~1-2s)
                println!("[duo] loading TTS...");
                let tts_dir = data_dir.join("tts");
                match state.init_tts(tts_dir) {
                    Ok(()) => println!("[duo] TTS loaded"),
                    Err(e) => println!("[duo] TTS failed: {e}"),
                }

                // STT
                println!("[duo] loading STT...");
                match find_whisper_model(&mdir) {
                    Ok(stt_path) => {
                        println!("[duo] found whisper model: {}", stt_path.display());
                        match state.init_stt(stt_path) {
                            Ok(()) => println!("[duo] STT loaded"),
                            Err(e) => println!("[duo] STT failed: {e}"),
                        }
                    }
                    Err(e) => println!("[duo] STT model not found: {e}"),
                }

                // LLM — heaviest, loaded last (can take 10-30s for a 26B GGUF)
                println!("[duo] loading LLM...");
                match find_gemma_model(&mdir) {
                    Ok(llm_path) => {
                        println!("[duo] found gemma model: {}", llm_path.display());
                        match llm_state.load_model(llm_path) {
                            Ok(()) => println!("[duo] LLM loaded"),
                            Err(e) => println!("[duo] LLM failed: {e}"),
                        }
                    }
                    Err(e) => println!("[duo] LLM model not found: {e}"),
                }

                println!("[duo] all models loaded");
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            stop_recording_and_transcribe,
            loopback_test,
            speak_text,
            stop_playback,
            audio_status,
            check_models,
            llm_status,
            conversation_turn,
            reset_conversation,
            cancel_generation,
        ])
        .build(tauri::generate_context!())
        .expect("error building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Shut down the llama-server subprocess before the process exits.
                // This is the primary cleanup path. LlmState::Drop is the backup.
                if let Some(llm) = app.try_state::<LlmState>() {
                    eprintln!("[duo] RunEvent::Exit — shutting down llama-server");
                    llm.shutdown();
                }
            }
        });
}
