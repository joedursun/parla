import { invoke } from '@tauri-apps/api/core';

export interface StopRecordingResult {
	duration_ms: number;
	sample_count: number;
}

export interface AudioStatus {
	is_recording: boolean;
	is_playing: boolean;
}

/** Initialize audio devices. Call once at app startup. */
export async function initAudio(): Promise<void> {
	await invoke('init_audio');
}

/** Start recording from the microphone. */
export async function startRecording(): Promise<void> {
	await invoke('start_recording');
}

/** Stop recording and return info about the captured audio. */
export async function stopRecording(): Promise<StopRecordingResult> {
	return invoke('stop_recording');
}

/** Stop recording and play back what was captured (loopback test). */
export async function loopbackTest(): Promise<StopRecordingResult> {
	return invoke('loopback_test');
}

/** Stop any current audio playback. */
export async function stopPlayback(): Promise<void> {
	await invoke('stop_playback');
}

/** Get current audio pipeline status. */
export async function audioStatus(): Promise<AudioStatus> {
	return invoke('audio_status');
}
