mod audio;
mod stt;
mod tts;
mod vad;

use audio::pipeline::AudioState;
use std::path::PathBuf;
use tauri::{Manager, State};

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
        models_dir: dir.to_string_lossy().to_string(),
    })
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
    models_dir: String,
}

// ── App entry ──────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AudioState::new())
        .setup(|app| {
            let handle = app.handle().clone();
            let state: AudioState = app.state::<AudioState>().inner().clone();

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

                // STT — loaded last since it's the heaviest
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
