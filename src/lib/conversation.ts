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

/** Rename a conversation's title. */
export async function renameConversation(conversationId: number, title: string): Promise<void> {
	return invoke('rename_conversation', { conversationId, title });
}

/** Delete a conversation and its messages/corrections. */
export async function deleteConversation(conversationId: number): Promise<void> {
	return invoke('delete_conversation', { conversationId });
}

// ── Flashcards ──────────────────────────────────────────────────────

export interface FlashcardResult {
	id: number;
	word: string;
	meaning: string;
	pronunciation: string;
	exampleTarget: string;
	exampleNative: string;
	source: string;
	status: 'New' | 'Learning' | 'Mature' | 'Due';
	nextReview: string;
	dots: boolean[];
}

/** Fetch all flashcards with vocabulary data. */
export async function getFlashcards(): Promise<FlashcardResult[]> {
	return invoke('get_flashcards');
}

// ── Conversation loading ────────────────────────────────────────────

export interface LoadedMessage {
	role: string;
	content: string;
	translation: string;
}

/** Load a previous conversation's messages from the database. */
export async function loadConversation(conversationId: number): Promise<LoadedMessage[]> {
	return invoke('load_conversation', { conversationId });
}

/** Clear the in-memory conversation history (start a fresh session). */
export async function resetConversation(): Promise<void> {
	await invoke('reset_conversation');
}

/** Cancel any in-flight LLM generation. */
export async function cancelGeneration(): Promise<void> {
	await invoke('cancel_generation');
}

// ── Lessons ─────────────────────────────────────────────────────────────

export interface LessonResult {
	id: number;
	sequenceOrder: number;
	title: string;
	description: string;
	status: string;
	topic: string;
	cefrLevel: string;
	successRate: number | null;
}

/** Fetch all lessons ordered by sequence. */
export async function getLessons(): Promise<LessonResult[]> {
	return invoke('get_lessons');
}

/** Start a lesson: marks it in_progress, sets the system prompt with lesson context. */
export async function startLesson(lessonId: number): Promise<LessonResult> {
	return invoke('start_lesson', { lessonId });
}

// ── Flashcard review ────────────────────────────────────────────────────

/** Submit a flashcard review rating. Updates SRS state in the database. */
export async function reviewFlashcard(
	flashcardId: number,
	rating: string,
	responseTimeMs?: number,
): Promise<void> {
	return invoke('review_flashcard', { flashcardId, rating, responseTimeMs });
}
