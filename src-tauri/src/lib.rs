mod audio;
mod db;
mod llm;
mod stt;
mod tts;
mod vad;

use audio::pipeline::AudioState;
use db::Db;
use llm::parser::{ParsedTutorResponse, StreamingJsonParser};
use llm::prompt::{build_system_prompt, ChatMessage};
use llm::{GenChunk, LlmState};
use std::path::{Path, PathBuf};
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
fn find_gemma_model(dir: &Path) -> Result<PathBuf, String> {
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
fn find_whisper_model(dir: &Path) -> Result<PathBuf, String> {
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

// ── Profile commands ──────────────────────────────────────────────────────

#[tauri::command]
async fn get_profile(db: State<'_, Db>) -> Result<Option<ProfileResult>, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let row = db.get_profile()?;
        match row {
            None => Ok(None),
            Some(r) => {
                let goals: Vec<String> =
                    serde_json::from_str(&r.goals_json).unwrap_or_default();
                Ok(Some(ProfileResult {
                    native_language: r.native_language,
                    target_language: r.target_language,
                    cefr_level: r.cefr_level,
                    goals,
                }))
            }
        }
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
async fn create_profile(
    db: State<'_, Db>,
    history: State<'_, ConversationHistory>,
    native_language: String,
    target_language: String,
    cefr_level: String,
    goals: Vec<String>,
) -> Result<ProfileResult, String> {
    let db = db.inner().clone();
    let history = history.inner().clone();
    let tl = target_language.clone();
    let nl = native_language.clone();
    let cl = cefr_level.clone();
    let g = goals.clone();
    tokio::task::spawn_blocking(move || {
        db.create_profile(&db::NewProfile {
            native_language: nl.clone(),
            target_language: tl.clone(),
            cefr_level: cl.clone(),
            goals: g.clone(),
        })?;
        // Update the system prompt for the current session.
        let goal_refs: Vec<&str> = g.iter().map(|s| s.as_str()).collect();
        let prompt = build_system_prompt(&tl, "Learner", &nl, &cl, &goal_refs, None);
        history.set_system_prompt(prompt);
        Ok(ProfileResult {
            native_language: nl,
            target_language: tl,
            cefr_level: cl,
            goals: g,
        })
    })
    .await
    .map_err(|e| format!("{e}"))?
}

// ── Conversation list commands ────────────────────────────────────────────

#[tauri::command]
async fn get_recent_conversations(
    db: State<'_, Db>,
) -> Result<Vec<RecentConversationResult>, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let rows = db.get_recent_conversations(20)?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let title = r
                    .topic
                    .unwrap_or_else(|| format!("{} conversation", r.mode));
                RecentConversationResult {
                    id: r.id.to_string(),
                    title,
                }
            })
            .collect())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

/// Reset the conversation history for the current session.
#[tauri::command]
async fn reset_conversation(history: State<'_, ConversationHistory>) -> Result<(), String> {
    history.inner().clear();
    Ok(())
}

/// Run one conversation turn: student text in → streamed tutor response out.
/// Synthesizes
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
    let user_msg = ChatMessage::user(student_text.clone());
    let mut messages = history.inner().messages_with_system();
    messages.push(user_msg.clone());

    // Record the student turn immediately.
    history.inner().push(user_msg);

    let llm = llm.inner().clone();
    let audio_handle = audio.inner().clone();
    let app_handle = app.clone();
    let cancel_for_llm = cancel_flag.clone();

    // Do the heavy work on a blocking task so we don't hog the tokio runtime.
    let result = tokio::task::spawn_blocking(move || -> Result<ConversationTurnResult, String> {
        let chunk_rx = llm.generate(messages, cancel_for_llm.clone())?;

        let mut parser = StreamingJsonParser::new();
        let mut full_text = String::new();

        // Speak + emit completed sentences as they form.
        let speak_sentences = |parser: &mut StreamingJsonParser| {
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
        };

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
                    speak_sentences(&mut parser);
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

        // Flush any tail sentence after the stream ends.
        speak_sentences(&mut parser);

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

    // ── Persist to SQLite (non-blocking, after the voice loop) ───────
    if let Some(db) = app.try_state::<Db>() {
        let db = db.inner().clone();
        let history = history.inner().clone();
        let student_text_for_db = student_text.clone();
        let result_for_db = result.clone();
        let app_for_db = app.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = persist_turn(
                &db,
                &history,
                &student_text_for_db,
                &result_for_db,
                &app_for_db,
            ) {
                eprintln!("[db] persist_turn failed: {e}");
            }
        });
    }

    Ok(result)
}

/// Cancel any in-flight LLM generation for the current turn.
#[tauri::command]
async fn cancel_generation(cancel: State<'_, CancelFlag>) -> Result<(), String> {
    cancel.inner().0.store(true, Ordering::Relaxed);
    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Persist one conversation turn (student + tutor messages, corrections,
/// vocabulary, flashcards) to SQLite. Called in a background task so it
/// never blocks the voice loop.
fn persist_turn(
    db: &Db,
    history: &ConversationHistory,
    student_text: &str,
    result: &ConversationTurnResult,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    // Ensure we have a conversation row.
    let conv_id = match history.get_conversation_id() {
        Some(id) => id,
        None => {
            let id = db.create_conversation("free", None)?;
            history.set_conversation_id(id);
            id
        }
    };

    // Insert student message.
    history.increment_messages();
    let student_msg_id = db.insert_message(&db::NewMessage {
        conversation_id: conv_id,
        role: "student".into(),
        content: student_text.to_string(),
        translation: None,
        input_method: "text".into(),
    })?;

    // Insert tutor message.
    history.increment_messages();
    let tutor_msg_id = db.insert_message(&db::NewMessage {
        conversation_id: conv_id,
        role: "tutor".into(),
        content: result.tutor_target.clone(),
        translation: if result.tutor_native.is_empty() {
            None
        } else {
            Some(result.tutor_native.clone())
        },
        input_method: "text".into(),
    })?;

    // Process parsed structured response.
    if let Some(parsed) = &result.parsed {
        // Correction
        if let Some(correction) = &parsed.correction {
            history.increment_errors();
            db.insert_correction(&db::NewCorrection {
                message_id: student_msg_id,
                conversation_id: conv_id,
                original_text: correction.original.clone(),
                corrected_text: correction.corrected.clone(),
                explanation: correction.explanation.clone(),
                error_type: "grammar".into(),
            })?;
        }

        // Vocabulary + flashcards
        for vocab in &parsed.new_vocabulary {
            let vocab_id = db.upsert_vocabulary(&db::NewVocabulary {
                target_text: vocab.target_text.clone(),
                native_text: vocab.native_text.clone(),
                pronunciation: vocab.pronunciation.clone(),
                part_of_speech: vocab.part_of_speech.clone(),
                topic: "conversation".into(),
                example_target: vocab.example_target.clone(),
                example_native: vocab.example_native.clone(),
                conversation_id: Some(conv_id),
            })?;
            db.ensure_flashcard(vocab_id)?;
        }

        // Update conversation counts.
        let (msg_count, err_count) = history.counts();
        db.update_conversation_counts(conv_id, msg_count, err_count)?;

        // Emit updated counts to the frontend.
        let due = db.flashcards_due_count().unwrap_or(0);
        let _ = app.emit("flashcards-due-count", due);

        let recent = db.recent_vocabulary(10).unwrap_or_default();
        let vocab_list: Vec<serde_json::Value> = recent
            .iter()
            .map(|v| {
                serde_json::json!({
                    "target": v.target_text,
                    "native": v.native_text,
                    "strength": match v.status.as_str() {
                        "mature" => 4,
                        "review" => 3,
                        "learning" => 2,
                        "new" => 1,
                        _ => 0,
                    }
                })
            })
            .collect();
        let _ = app.emit("recent-vocabulary", vocab_list);
    }

    // Emit updated recent conversations for the sidebar.
    if let Ok(convs) = db.get_recent_conversations(20) {
        let list: Vec<serde_json::Value> = convs
            .into_iter()
            .map(|r| {
                let title = r
                    .topic
                    .unwrap_or_else(|| format!("{} conversation", r.mode));
                serde_json::json!({ "id": r.id.to_string(), "title": title })
            })
            .collect();
        let _ = app.emit("recent-conversations", list);
    }

    // Suppress unused variable warning for tutor_msg_id.
    let _ = tutor_msg_id;

    Ok(())
}

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
struct ProfileResult {
    native_language: String,
    target_language: String,
    cefr_level: String,
    goals: Vec<String>,
}

#[derive(Clone, serde::Serialize)]
struct RecentConversationResult {
    id: String,
    title: String,
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

#[derive(Clone)]
struct ConversationHistory {
    inner: Arc<Mutex<Vec<ChatMessage>>>,
    system_prompt: Arc<Mutex<String>>,
    /// Current conversation's DB id (set on first turn, cleared on reset).
    conversation_id: Arc<Mutex<Option<i64>>>,
    /// Running message count for the current conversation.
    message_count: Arc<Mutex<i32>>,
    /// Running error count for the current conversation.
    error_count: Arc<Mutex<i32>>,
}

impl ConversationHistory {
    fn new() -> Self {
        // Default to a free conversation prompt until the DB layer provides
        // real learner profile and lesson data.
        let default_prompt = build_system_prompt(
            "Spanish",
            "Learner",
            "English",
            "A1 (Beginner)",
            &["Conversation"],
            None,
        );
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
            system_prompt: Arc::new(Mutex::new(default_prompt)),
            conversation_id: Arc::new(Mutex::new(None)),
            message_count: Arc::new(Mutex::new(0)),
            error_count: Arc::new(Mutex::new(0)),
        }
    }

    fn set_system_prompt(&self, prompt: String) {
        *self.system_prompt.lock().unwrap() = prompt;
    }

    fn clear(&self) {
        self.inner.lock().unwrap().clear();
        *self.conversation_id.lock().unwrap() = None;
        *self.message_count.lock().unwrap() = 0;
        *self.error_count.lock().unwrap() = 0;
    }

    fn push(&self, msg: ChatMessage) {
        self.inner.lock().unwrap().push(msg);
    }

    fn get_conversation_id(&self) -> Option<i64> {
        *self.conversation_id.lock().unwrap()
    }

    fn set_conversation_id(&self, id: i64) {
        *self.conversation_id.lock().unwrap() = Some(id);
    }

    fn increment_messages(&self) -> i32 {
        let mut c = self.message_count.lock().unwrap();
        *c += 1;
        *c
    }

    fn increment_errors(&self) -> i32 {
        let mut c = self.error_count.lock().unwrap();
        *c += 1;
        *c
    }

    fn counts(&self) -> (i32, i32) {
        (*self.message_count.lock().unwrap(), *self.error_count.lock().unwrap())
    }

    /// Return the full message list prefixed with the system prompt.
    fn messages_with_system(&self) -> Vec<ChatMessage> {
        let hist = self.inner.lock().unwrap();
        let prompt = self.system_prompt.lock().unwrap();
        let mut out = Vec::with_capacity(hist.len() + 1);
        out.push(ChatMessage::system(prompt.clone()));
        out.extend(hist.iter().cloned());
        out
    }
}

#[derive(Clone)]
struct CancelFlag(Arc<AtomicBool>);

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

            // Open SQLite database.
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("failed to get app data dir: {e}"))?;
            std::fs::create_dir_all(&data_dir)
                .map_err(|e| format!("failed to create data dir: {e}"))?;
            let db_path = data_dir.join("duo.db");
            println!("[duo] opening database at {}", db_path.display());
            let db =
                Db::open(&db_path).map_err(|e| Box::<dyn std::error::Error>::from(e))?;
            app.manage(db.clone());

            // If a profile exists, set the system prompt from it.
            if let Ok(Some(profile)) = db.get_profile() {
                let goals: Vec<String> =
                    serde_json::from_str(&profile.goals_json).unwrap_or_default();
                let goal_refs: Vec<&str> = goals.iter().map(|s| s.as_str()).collect();
                let prompt = build_system_prompt(
                    &profile.target_language,
                    "Learner",
                    &profile.native_language,
                    &profile.cefr_level,
                    &goal_refs,
                    None,
                );
                let history: tauri::State<ConversationHistory> = app.state();
                history.set_system_prompt(prompt);
                println!("[duo] loaded profile: {} -> {}", profile.native_language, profile.target_language);
            }

            // Init audio devices immediately (fast, needed for everything)
            if let Err(e) = state.init() {
                eprintln!("[setup] audio init failed: {e}");
            }

            // Load all models in a background thread — never blocks the UI
            std::thread::spawn(move || {
                println!("[duo] background model loading started");

                let mdir = match models_dir(&handle) {
                    Ok(d) => d,
                    Err(e) => {
                        println!("[duo] {e}");
                        return;
                    }
                };
                let data_dir = mdir.parent().unwrap().to_path_buf();

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
            get_profile,
            create_profile,
            get_recent_conversations,
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
