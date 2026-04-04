# Phase 1 — Audio Pipeline: Status & Next Steps

## Done

- **1a: Project scaffold and audio I/O**
  - Tauri v2 + SvelteKit project, builds and runs
  - cpal mic capture (device-native rate, batch-resampled to 16kHz mono)
  - cpal speaker playback via ring buffer (resamples from any input rate to device rate)
  - Dedicated audio thread (cpal streams aren't Send on macOS) with channel-based command dispatch
  - Loopback test: hold mic button → hear your voice played back
  - Conversation UI wired to Tauri IPC (hold-to-record, visual recording state)

- **1b: Voice Activity Detection (code only — not yet runnable)**
  - SileroVad struct wrapping ort (ONNX Runtime) with correct v5 tensor shapes
  - Processes 512-sample (32ms) chunks, maintains LSTM hidden state across calls
  - Speech start/end detection with configurable thresholds
  - Wired into the audio pipeline: VAD runs over the 16kHz buffer on stop_recording
  - Speech segments returned to the frontend as start/end timestamps
  - **Missing**: need to download `silero_vad.onnx` model file and test with real audio

## Remaining

### 1b: VAD testing (finish)
- Download Silero VAD v5 ONNX model to `<app_data_dir>/models/silero_vad.onnx`
- Call `init_vad` from the frontend after `init_audio`
- Verify speech segments are returned when recording
- Tune thresholds if needed (current: activation 0.5, min silence 550ms, padding 500ms)

### 1c: Speech-to-Text
- Add `whisper-rs` crate (Rust bindings to whisper.cpp)
- Create `src-tauri/src/stt/mod.rs` module
- Load a Whisper GGUF model (start with `ggml-base.en.bin` for fast iteration, upgrade to large-v3 later)
- Add `init_stt` Tauri command to load the model
- On stop_recording, run Whisper on VAD-detected speech segments (or full buffer if no VAD)
- Return transcribed text alongside the recording result
- Show transcription as a student chat bubble in the conversation UI

### 1d: Text-to-Speech
- Integrate Kokoro 82M via ort (ONNX Runtime) — same crate already in use for VAD
- Create `src-tauri/src/tts/mod.rs` module
- Load Kokoro ONNX model + voice config
- Add `speak_text` Tauri command: text in, audio queued to playback
- Test: type text in the conversation input → hear it spoken

### 1e: End-to-end voice loop
- Wire the full pipeline: mic → VAD → STT → stub response → TTS → speaker
- Stub response: echo back the transcription or a canned phrase
- Sentence buffering: accumulate text into sentences before dispatching to TTS
- Barge-in: if VAD detects speech during TTS playback, flush playback and stop generation
- Measure and log latency at each stage
- **Exit criteria**: speak a sentence, hear a spoken response, latency under 1.5s
