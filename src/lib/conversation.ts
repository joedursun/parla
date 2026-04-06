import { invoke } from '@tauri-apps/api/core';

export interface TutorMessage {
	target_lang: string;
	native_lang: string;
}

export interface Correction {
	original: string;
	corrected: string;
	explanation: string;
}

export interface NewVocabulary {
	target_text: string;
	native_text: string;
	pronunciation?: string;
	part_of_speech?: string;
	example_target?: string;
	example_native?: string;
}

export interface GrammarNote {
	title: string;
	explanation: string;
}

export interface SuggestedResponse {
	target_lang: string;
	native_lang: string;
}

export interface InternalNotes {
	estimated_comprehension?: string;
	lesson_progress_pct?: number;
}

export interface ParsedTutorResponse {
	tutor_message: TutorMessage;
	correction: Correction | null;
	new_vocabulary: NewVocabulary[];
	grammar_note: GrammarNote | null;
	suggested_responses: SuggestedResponse[];
	internal_notes: InternalNotes | null;
}

export interface ConversationTurnResult {
	/** Raw JSON string the LLM emitted. */
	raw: string;
	/** Tutor's spoken line in the target language. */
	tutor_target: string;
	/** English translation (empty string if parse failed). */
	tutor_native: string;
	/** Parsed structured response, or null if JSON parse failed. */
	parsed: ParsedTutorResponse | null;
	/** Parse error message, if any. */
	parse_error: string | null;
}

/** Run one conversation turn. Emits `tutor-message-chunk`, `tutor-sentence`,
 *  and `tutor-message-done` events via Tauri's event system while streaming. */
export async function conversationTurn(studentText: string): Promise<ConversationTurnResult> {
	return invoke('conversation_turn', { studentText });
}

/** Clear the in-memory conversation history (start a fresh session). */
export async function resetConversation(): Promise<void> {
	await invoke('reset_conversation');
}

/** Cancel any in-flight LLM generation. */
export async function cancelGeneration(): Promise<void> {
	await invoke('cancel_generation');
}
