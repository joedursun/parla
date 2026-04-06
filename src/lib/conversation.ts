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

// ── Profile ──────────────────────────────────────────────────────────────

export interface ProfileResult {
	native_language: string;
	target_language: string;
	cefr_level: string;
	goals: string[];
}

/** Get the saved learner profile, or null if none exists yet. */
export async function getProfile(): Promise<ProfileResult | null> {
	return invoke('get_profile');
}

/** Create (or replace) the learner profile. Returns the saved profile. */
export async function createProfile(
	nativeLanguage: string,
	targetLanguage: string,
	cefrLevel: string,
	goals: string[],
): Promise<ProfileResult> {
	return invoke('create_profile', {
		nativeLanguage,
		targetLanguage,
		cefrLevel,
		goals,
	});
}

// ── Conversation ─────────────────────────────────────────────────────────

/** Run one conversation turn. Emits `tutor-message-chunk`, `tutor-sentence`,
 *  and `tutor-message-done` events via Tauri's event system while streaming. */
export async function conversationTurn(studentText: string): Promise<ConversationTurnResult> {
	return invoke('conversation_turn', { studentText });
}

// ── Recent conversations ───────────────────────────���─────────────────────

export interface RecentConversationResult {
	id: string;
	title: string;
}

/** Fetch recent conversations for the sidebar. */
export async function getRecentConversations(): Promise<RecentConversationResult[]> {
	return invoke('get_recent_conversations');
}

/** Clear the in-memory conversation history (start a fresh session). */
export async function resetConversation(): Promise<void> {
	await invoke('reset_conversation');
}

/** Cancel any in-flight LLM generation. */
export async function cancelGeneration(): Promise<void> {
	await invoke('cancel_generation');
}
