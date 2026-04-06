# Next Steps — Phase 3 Prep: Remove All Mock Data

Phase 2 is complete and Phase 3 (conversation UI + partial audio pipeline) is partially done: mic records audio, STT transcription displays, LLM responds with streaming JSON, vocab cards appear in the context panel but aren't actionable yet. However, every page outside the live conversation flow is populated with hardcoded mock data, and the backend system prompt is locked to a single Spanish lesson scenario. Before implementing SQLite persistence, we need to strip all of this out so the app shows only real data (or appropriate empty states).

---

## 1. Backend: Replace hardcoded system prompt

### 1a. Make `spanish_cafe_system_prompt()` language- and lesson-agnostic

**File:** `src-tauri/src/llm/prompt.rs` (lines 50-143)

The entire function is a single hardcoded string for a Spanish ordering-food lesson with a student named "Joe". Replace it with a parameterized prompt builder that accepts:
- Target language name
- Student name
- Student level (CEFR code)
- Student goals
- Lesson topic, scenario, objectives (optional — `None` for free conversation)
- Vocabulary to introduce (optional list)
- Grammar focus (optional list)
- Language mix ratio (derived from level)

The function should assemble the same prompt structure but from parameters. The JSON response format section is language-agnostic and can stay as a constant. Rename the function to something like `build_system_prompt(...)`.

### 1b. Update `ConversationHistory::messages_with_system()` to accept prompt params

**File:** `src-tauri/src/lib.rs` (line 443)

Currently calls `spanish_cafe_system_prompt()` directly. Change `messages_with_system()` to accept the system prompt as a parameter (or store it when the conversation starts), so different conversations can have different prompts.

### 1c. Remove the `spanish_cafe_system_prompt` import

**File:** `src-tauri/src/lib.rs` (line 9)

After 1a/1b, the specific import `spanish_cafe_system_prompt` should no longer exist. Update the import to use the new function name.

---

## 2. Frontend: Sidebar mock data

**File:** `src/lib/components/Sidebar.svelte`

### 2a. Hardcoded user info (lines 53-57)
- Avatar initial "J", name "Joe", level "Learning Spanish" are all hardcoded
- Replace with props or a store that holds the current user profile (name, target language)
- Until onboarding/DB exists, show a generic empty state (e.g., "Set up your profile" or simply omit the user card)

### 2b. Hardcoded flashcard badge count (line 24)
- `<span class="badge">12</span>` is hardcoded
- Replace with a reactive value from a store (0 or hidden when no cards are due)

### 2c. Hardcoded recent conversations (lines 33-44)
- Three fake conversation links: "Ordering at a cafe", "Asking for directions", "Introducing yourself"
- Replace with a dynamic list from a store (empty array until conversations exist in DB)
- Show an empty state or hide the section when no conversations exist

---

## 3. Frontend: Dashboard page

**File:** `src/routes/+page.svelte`

### 3a. Hardcoded greeting (line 31)
- `"Buenos dias, Joe"` — hardcoded Spanish greeting and user name
- Replace with dynamic greeting from user profile (or generic "Welcome back" until profile exists)

### 3b. Hardcoded subtitle (line 32)
- `"Day 14 of learning Spanish · You have 12 cards to review"` — all fake
- Replace with dynamic data from stores/DB, or hide until data exists

### 3c. Mock lesson array (lines 2-10)
- 7 hardcoded Spanish lessons with titles, descriptions, statuses, and progress
- Replace with data from a store that reads from DB (empty array until lessons are generated)
- Show an empty state ("Start your first lesson" or link to onboarding) when no lessons exist

### 3d. Mock recent vocabulary (lines 12-19)
- 6 hardcoded Spanish word/translation pairs with strength ratings
- Replace with a store sourced from DB vocabulary table (empty until vocab is learned)

### 3e. Mock activity grid (lines 21-26)
- 28 hardcoded activity-level numbers
- Replace with data from daily_stats table, or show all zeros/empty until data exists

### 3f. Hardcoded stats (lines 66-86)
- Day streak: 14, Words learned: 187, Practice time: 4.2h, Flashcard accuracy: 78%, all with fake trends
- Replace with computed values from DB, showing 0/empty defaults until data exists

### 3g. Hardcoded action card text (lines 37-63)
- "Ordering food -- 60% complete" (line 40), "12 cards due today" (line 47)
- Replace with dynamic data from current lesson and flashcard due count

### 3h. Hardcoded review banner (lines 112-119)
- "12 cards ready for review", "5 min estimated"
- Replace with flashcard due count, or hide banner when count is 0

### 3i. Hardcoded level tag (line 93)
- `"A1 — Beginner"` is hardcoded
- Replace with user's current level from profile

---

## 4. Frontend: Conversation page

**File:** `src/routes/conversation/+page.svelte`

### 4a. Hardcoded tutor header (lines 212-215)
- "Your Spanish Tutor" — hardcoded language
- "Active · Lesson 4: Ordering Food" — hardcoded lesson info
- Replace "Spanish" with user's target language, lesson info with current lesson (or "Free Conversation" when no lesson is active)

### 4b. Hardcoded lesson banner (lines 229-237)
- Lesson title "Ordering Food & Drinks at a Restaurant", progress 60%
- Replace with current lesson data, or hide banner entirely when no lesson is active

### 4c. Hardcoded lesson focus card in context panel (lines 372-381)
- Title "Ordering Food & Drinks", description, and tags ("Vocabulary", "Polite forms", "Numbers") are all hardcoded
- Replace with current lesson focus from lesson data, or show an empty/hidden state

### 4d. Hardcoded placeholder text (line 244)
- `"Tap the mic or type to start a conversation with your Spanish tutor."` — hardcoded "Spanish"
- Replace "Spanish" with the user's target language

### 4e. Hardcoded placeholder text (line 322)
- `placeholder="Type in Spanish (or English to translate)..."` — hardcoded "Spanish" and "English"
- Replace with user's target and native language names

### 4f. Hardcoded student avatar initial (line 254)
- `J` is hardcoded as the student avatar
- Replace with first letter of user's name from profile

---

## 5. Frontend: Flashcards page

**File:** `src/routes/flashcards/+page.svelte`

### 5a. Mock card deck (lines 4-11)
- 6 hardcoded Spanish flashcards with word, meaning, pronunciation, example sentences, SRS status, review intervals, and dot indicators
- Replace with data from a store sourced from the flashcards/vocabulary DB tables
- Show an empty state ("No flashcards yet — start a conversation to collect vocabulary") when no cards exist

### 5b. Hardcoded review progress header (lines 18-31)
- "Card 5 of 12", "42% complete", counts "3 New, 4 Learning, 5 Review" — all hardcoded
- Replace with computed values from the current review session state
- Show zeros or hide when no review session is active

### 5c. Hardcoded rating intervals (lines 65-68)
- "< 1 min", "6 min", "3 days", "7 days" — these are static
- Once SRS is implemented (Phase 5), these should be computed per-card. For now, leave them as reasonable defaults or hide them

---

## 6. Frontend: Progress page

**File:** `src/routes/progress/+page.svelte`

### 6a. Mock skills breakdown (lines 2-7)
- Reading 72%, Listening 58%, Writing 65%, Speaking 45% — all hardcoded
- Replace with computed values from DB, showing 0% defaults until data exists

### 6b. Mock weak areas (lines 9-14)
- 4 hardcoded Spanish-specific concepts with fake accuracy percentages
- Replace with data from weak_areas DB table, or show "No weak areas identified yet"

### 6c. Mock vocabulary categories (lines 16-22)
- 5 categories with hardcoded mastered/learning/new percentages and counts
- Replace with aggregated data from vocabulary DB table grouped by topic

### 6d. Mock grammar concepts (lines 24-33)
- 8 hardcoded Spanish grammar concepts with Spanish-specific descriptions
- Replace with data from grammar_concepts DB table, or show empty state

### 6e. Hardcoded level hero (lines 38-59)
- Level "A1", "65% through A1 — Beginner", encouragement text, milestone checkmarks
- Replace with user's actual CEFR level and progress from DB

### 6f. Hardcoded vocabulary summary totals (lines 98-102)
- "98 Mastered, 54 Learning, 35 New" — hardcoded counts
- Replace with aggregated counts from vocabulary/flashcard DB tables

### 6g. Hardcoded skill sub-levels (line 70)
- Each skill card says `"A1 · Beginner"` — hardcoded
- Replace with per-skill computed level or remove until skill tracking is implemented

---

## 7. Frontend: Onboarding page

**File:** `src/routes/onboarding/+page.svelte`

### 7a. Language, level, and goal lists (lines 4-28)
- These are reference data (not "mock" data per se) but are currently component-local constants with no way to persist the user's selection
- For now these can stay as constants — they represent the set of supported options
- **However**, the onboarding flow currently goes nowhere: the "Start my first lesson" button (line 125) links to `/` with no side effects. The selections are lost on navigation
- Wire the final step to persist the user's choices (will need a Tauri command + DB write in Phase 3a/4a, or at minimum a frontend store that survives navigation)

### 7b. Pre-selected defaults (line 30-32)
- `selectedLanguage = 0` (Spanish), `selectedGoals = new Set([0, 2])` (Travel, Conversation) — these are reasonable defaults, not mock data. OK to keep.

---

## Summary

| # | Area | File | Items |
|---|------|------|-------|
| 1 | Backend prompt | `prompt.rs`, `lib.rs` | Parameterize system prompt, remove Spanish-only function |
| 2 | Sidebar | `Sidebar.svelte` | User info, badge count, recent conversations |
| 3 | Dashboard | `+page.svelte` | Greeting, stats, lessons, vocabulary, activity, review banner |
| 4 | Conversation | `conversation/+page.svelte` | Tutor header, lesson banner, lesson focus, placeholders, avatar |
| 5 | Flashcards | `flashcards/+page.svelte` | Card deck, review progress, rating intervals |
| 6 | Progress | `progress/+page.svelte` | Skills, weak areas, vocab categories, grammar, level hero |
| 7 | Onboarding | `onboarding/+page.svelte` | Wire selections to persist (lists themselves are fine) |

**Approach:** For each mock data source, create a Svelte store (or extend an existing one) that will eventually be populated from Tauri IPC commands reading SQLite. In the interim, stores should initialize with empty/zero defaults and the UI should show appropriate empty states. This ensures the app compiles and runs cleanly with no fake data before we start implementing the DB layer.
