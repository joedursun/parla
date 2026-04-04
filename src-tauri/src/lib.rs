mod audio;
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

#[tauri::command]
fn init_audio(state: State<AudioState>) -> Result<(), String> {
    state.init()
}

#[tauri::command]
fn init_vad(app: tauri::AppHandle, state: State<AudioState>) -> Result<(), String> {
    let model_path = models_dir(&app)?.join("silero_vad.onnx");
    if !model_path.exists() {
        return Err(format!(
            "VAD model not found at {}. Download silero_vad.onnx to this path.",
            model_path.display()
        ));
    }
    state.init_vad(model_path)
}

#[tauri::command]
fn start_recording(state: State<AudioState>) -> Result<(), String> {
    state.start_recording()
}

#[tauri::command]
fn stop_recording(state: State<AudioState>) -> Result<StopRecordingResult, String> {
    let result = state.stop_recording()?;
    let duration_ms = (result.samples.len() as f64 / 16000.0 * 1000.0) as u64;
    Ok(StopRecordingResult {
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
    })
}

/// Stop recording and immediately play back the captured audio (loopback test).
#[tauri::command]
fn loopback_test(state: State<AudioState>) -> Result<StopRecordingResult, String> {
    let result = state.stop_recording()?;
    let duration_ms = (result.samples.len() as f64 / 16000.0 * 1000.0) as u64;
    let sample_count = result.samples.len();
    let speech_segments: Vec<SpeechSegmentResult> = result
        .speech_segments
        .iter()
        .map(|s| SpeechSegmentResult {
            start_ms: (s.start_sample as f64 / 16.0) as u64,
            end_ms: (s.end_sample as f64 / 16.0) as u64,
        })
        .collect();
    state.play_audio(result.samples, 16000)?;
    Ok(StopRecordingResult {
        duration_ms,
        sample_count,
        speech_segments,
    })
}

#[tauri::command]
fn stop_playback(state: State<AudioState>) {
    state.stop_playback();
}

#[tauri::command]
fn audio_status(state: State<AudioState>) -> AudioStatusResult {
    let status = state.status();
    AudioStatusResult {
        is_recording: status.is_recording,
        is_playing: status.is_playing,
        vad_active: status.vad_active,
        speech_detected: status.speech_detected,
    }
}

/// Check which models are available locally.
#[tauri::command]
fn check_models(app: tauri::AppHandle) -> Result<ModelStatus, String> {
    let dir = models_dir(&app)?;
    Ok(ModelStatus {
        vad: dir.join("silero_vad.onnx").exists(),
        models_dir: dir.to_string_lossy().to_string(),
    })
}

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
struct AudioStatusResult {
    is_recording: bool,
    is_playing: bool,
    vad_active: bool,
    speech_detected: bool,
}

#[derive(serde::Serialize)]
struct ModelStatus {
    vad: bool,
    models_dir: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AudioState::new())
        .invoke_handler(tauri::generate_handler![
            init_audio,
            init_vad,
            start_recording,
            stop_recording,
            loopback_test,
            stop_playback,
            audio_status,
            check_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
