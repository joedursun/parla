/**
 * Reactive application state. All values initialize to empty/zero defaults.
 * Once the DB layer exists (Phase 3+), these will be populated from SQLite
 * via Tauri IPC commands on app startup and after mutations.
 *
 * Uses Svelte 5 runes ($state) for fine-grained reactivity.
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

/** User profile — null until onboarding is complete. */
export let userProfile: UserProfile | null = $state(null);

/** Current lesson info — null when no lesson is active. */
export let currentLesson: LessonFocus | null = $state(null);

/** Ordered list of lessons in the learning path. */
export let lessons: Lesson[] = $state([]);

/** Recent vocabulary with strength indicators. */
export let recentVocabulary: VocabWord[] = $state([]);

/** Past conversations for the sidebar. */
export let recentConversations: RecentConversation[] = $state([]);

/** Number of flashcards due for review. */
export let flashcardsDueCount: number = $state(0);

/** Activity heatmap data (last 28 days, 0-3 intensity). */
export let activityData: number[] = $state([]);

/** Aggregated daily stats. */
export let dailyStats: DailyStats = $state({
	streak: 0,
	wordsLearned: 0,
	wordsLearnedTrend: '',
	practiceTime: '0h',
	practiceTimeTrend: '',
	flashcardAccuracy: '0%',
	flashcardAccuracyTrend: '',
});

/** Flashcard deck for the review page. */
export let flashcards: FlashcardSummary[] = $state([]);

/** Skill breakdown for progress page. */
export let skills: SkillProgress[] = $state([]);

/** Weak areas for progress page. */
export let weakAreas: WeakArea[] = $state([]);

/** Vocabulary categories for progress page. */
export let vocabCategories: VocabCategory[] = $state([]);

/** Grammar concepts for progress page. */
export let grammarConcepts: GrammarConcept[] = $state([]);

/** Overall level progress for progress page. */
export let levelProgress = $state({
	level: '',
	label: '',
	pct: 0,
	description: '',
	milestones: [] as { name: string; done: boolean }[],
	totalMastered: 0,
	totalLearning: 0,
	totalNew: 0,
});
