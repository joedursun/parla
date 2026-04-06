import { invoke } from '@tauri-apps/api/core';

export interface SpeechSegment {
	start_ms: number;
	end_ms: number;
}

export interface StopRecordingResult {
	duration_ms: number;
	sample_count: number;
	speech_segments: SpeechSegment[];
}

export interface AudioStatus {
	is_recording: boolean;
	is_playing: boolean;
	vad_active: boolean;
	stt_ready: boolean;
	tts_ready: boolean;
	speech_detected: boolean;
}

export interface ModelStatus {
	vad: boolean;
	stt: boolean;
	llm: boolean;
	models_dir: string;
}

export interface LlmStatus {
	loaded: boolean;
}

/** Start recording from the microphone. */
export async function startRecording(): Promise<void> {
	await invoke('start_recording');
}

/** Stop recording and return info about the captured audio. */
export async function stopRecording(): Promise<StopRecordingResult> {
	return invoke('stop_recording');
}

/** Stop recording, transcribe speech, and return transcription + recording info. */
export async function stopRecordingAndTranscribe(): Promise<StopRecordingResult & { transcription: string }> {
	return invoke('stop_recording_and_transcribe');
}

/** Stop recording and play back what was captured (loopback test). */
export async function loopbackTest(): Promise<StopRecordingResult> {
	return invoke('loopback_test');
}

/** Speak text using TTS and queue for playback. */
export async function speakText(text: string): Promise<void> {
	await invoke('speak_text', { text });
}

/** Stop any current audio playback. */
export async function stopPlayback(): Promise<void> {
	await invoke('stop_playback');
}

/** Get current audio pipeline status including model readiness. */
export async function audioStatus(): Promise<AudioStatus> {
	return invoke('audio_status');
}

/** Check which models are available on disk. */
export async function checkModels(): Promise<ModelStatus> {
	return invoke('check_models');
}

/** Get LLM load status. */
export async function llmStatus(): Promise<LlmStatus> {
	return invoke('llm_status');
}
