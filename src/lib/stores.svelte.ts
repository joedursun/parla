/**
 * Reactive application state. All values initialize to empty/zero defaults.
 * Populated from SQLite via Tauri IPC commands on app startup and after mutations.
 *
 * Svelte 5 rule: exported $state cannot be reassigned from outside the module.
 * So we keep all mutable state inside a single exported object (`store`) whose
 * *properties* are mutated, and provide setter functions for external callers.
 */

// ── Types ────────────────────────────────────────────────────────────────

export interface UserProfile {
	name: string;
	nativeLanguage: string;
	targetLanguage: string;
	level: string; // CEFR code, e.g. "A1"
	levelLabel: string; // e.g. "Beginner"
	goals: string[];
}

export interface Lesson {
	id: number;
	title: string;
	description: string;
	status: 'done' | 'current' | 'upcoming';
	progress: string; // "Done", "60%", "Upcoming"
}

export interface VocabWord {
	target: string;
	native: string;
	strength: number; // 0-4
}

export interface RecentConversation {
	id: string;
	title: string;
}

export interface LessonFocus {
	title: string;
	description: string;
	tags: string[];
}

export interface FlashcardSummary {
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

export interface SkillProgress {
	icon: string;
	name: string;
	pct: number;
	color: string;
}

export interface WeakArea {
	name: string;
	accuracy: string;
}

export interface VocabCategory {
	name: string;
	mastered: number;
	learning: number;
	newPct: number;
	count: number;
}

export interface GrammarConcept {
	name: string;
	desc: string;
	status: 'mastered' | 'learning' | 'upcoming';
}

export interface DailyStats {
	streak: number;
	wordsLearned: number;
	wordsLearnedTrend: string;
	practiceTime: string;
	practiceTimeTrend: string;
	flashcardAccuracy: string;
	flashcardAccuracyTrend: string;
}

// ── State ────────────────────────────────────────────────────────────────
// A single exported object whose properties are $state-ified via class fields.

class Store {
	userProfile: UserProfile | null = $state(null);
	currentLesson: LessonFocus | null = $state(null);
	lessons: Lesson[] = $state([]);
	recentVocabulary: VocabWord[] = $state([]);
	recentConversations: RecentConversation[] = $state([]);
	flashcardsDueCount: number = $state(0);
	activityData: number[] = $state([]);
	dailyStats: DailyStats = $state({
		streak: 0,
		wordsLearned: 0,
		wordsLearnedTrend: '',
		practiceTime: '0h',
		practiceTimeTrend: '',
		flashcardAccuracy: '0%',
		flashcardAccuracyTrend: '',
	});
	flashcards: FlashcardSummary[] = $state([]);
	skills: SkillProgress[] = $state([]);
	weakAreas: WeakArea[] = $state([]);
	vocabCategories: VocabCategory[] = $state([]);
	grammarConcepts: GrammarConcept[] = $state([]);
	levelProgress = $state({
		level: '',
		label: '',
		pct: 0,
		description: '',
		milestones: [] as { name: string; done: boolean }[],
		totalMastered: 0,
		totalLearning: 0,
		totalNew: 0,
	});
}

export const store = new Store();

// ── Setters (for use from layout / event listeners) ─────────────────────

export function setUserProfile(p: UserProfile | null) {
	store.userProfile = p;
}

export function setRecentVocabulary(v: VocabWord[]) {
	store.recentVocabulary = v;
}

export function setRecentConversations(c: RecentConversation[]) {
	store.recentConversations = c;
}

export function setFlashcardsDueCount(n: number) {
	store.flashcardsDueCount = n;
}

export function setFlashcards(f: FlashcardSummary[]) {
	store.flashcards = f;
}
