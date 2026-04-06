# Next Steps — Phase 4: Onboarding & Lesson System + Loose Ends

Phase 3 (SQLite, onboarding persistence, message persistence, sidebar) is complete. The data layer is in place: `rusqlite` with WAL mode, all 11 tables, `Db` managed as Tauri state. Conversations, messages, corrections, vocabulary, and flashcards persist to SQLite. The sidebar shows recent conversations and flashcard due counts. Stores use a class-based `Store` pattern (Svelte 5 runes can't export reassigned `$state`).

Phase 4 builds the lesson system, learner model, and conversation summarization.

---

### Loose ends from Phase 3

#### Conversation titles

Sidebar entries currently all show "free conversation" because `persist_turn()` creates conversations with `mode: "free"` and no topic. After the first tutor response in a new conversation, generate a short title (2-5 words) from the exchange — either via a quick LLM call or by extracting the conversation topic from `internal_notes`. Store it in `conversations.topic` so the sidebar entries are distinguishable.

**Notes:**
- The title generation should be non-blocking — fire it after the first turn completes.
- Update the `conversations.topic` column and emit a `recent-conversations` event so the sidebar refreshes.
- A lightweight approach: use the first student message as context and ask the LLM for a 3-5 word title. Alternatively, extract it from `ParsedTutorResponse.internal_notes` if a topic field is added to the prompt.

#### Sidebar conversation click handler

Clicking a recent conversation in the sidebar currently navigates to `/conversation` but doesn't load that conversation's history. Wire this up:

- Add a `load_conversation` Tauri command that takes a `conversation_id`, queries its messages from SQLite, and populates `ConversationHistory` (set the `conversation_id`, rebuild message history from DB rows).
- The sidebar link should pass the conversation id (e.g. `/conversation?id=42` or via a store).
- The conversation page should detect the id, call `load_conversation`, and populate `liveMessages` from the returned messages.
- Re-set the system prompt from the profile (the prompt won't include lesson context for free conversations, which is correct).

#### Conversation summarization

When conversation history exceeds ~15 turns, summarize older messages via a second LLM call (see [data.md § Model Context Window Budget](data.md#appendix-model-context-window-budget)). Replace old messages in the LLM context with a `<conversation_summary>` block. This prevents context overflow on long sessions.

**Notes:**
- Reuse `LlmState.generate()` with a "summarize this conversation" prompt.
- Run it non-blocking after a turn completes, not while the user is waiting.
- Store the summary in `conversations.summary` for reuse when loading old conversations.

---

### 4a: Onboarding enhancements

The onboarding flow already persists the learner profile. These additions complete it:

- Seed `grammar_concepts` rows for the selected language + CEFR level on profile creation. These are the grammar progression items the lesson system will track.
- Call the LLM to generate an initial learning path (first 8-10 lessons) — see [data.md § Initial Setup](data.md#1-initial-setup-onboarding) for the prompt template.
- Insert generated lessons into the `lessons` table with `status = 'planned'`.
- After generating, populate `store.lessons` on the frontend so the dashboard learning path shows real data.

**Notes:**
- The grammar concept seeding can be a static list per language/level (no LLM needed). Store it in Rust as a const or load from a bundled JSON file.
- The lesson generation LLM call should run in the background after the profile is created. Show a loading state on the dashboard while it runs.

### 4b: Lesson management

- Build the learning path UI on the dashboard showing lesson sequence with status (the skeleton already exists in `+page.svelte` — it just needs real data).
- Implement lesson start flow:
  - Query learner progress data from SQLite (see [data.md § Lesson Plan Generation](data.md#2-lesson-plan-generation-before-each-lesson))
  - Run adaptation check (proceed vs. review) via LLM
  - Set the conversation system prompt with lesson context via `build_system_prompt()` with `lesson: Some(LessonContext { ... })`
  - Set `store.currentLesson` so the conversation page shows the lesson banner and context panel
- Implement lesson completion:
  - Post-conversation summarization via LLM
  - Update `lessons` row: `status = 'completed'`, `success_rate`, `completed_at`
  - Update `grammar_concepts` accuracy and status
  - Recompute `weak_areas`
  - Update `daily_stats`
  - Trigger next lesson planning if needed

**Notes:**
- `build_system_prompt()` already accepts `Option<&LessonContext>` with topic, scenario, objectives, vocabulary, and grammar focus. The lesson start flow just needs to assemble this from DB data.
- The adaptation check (proceed vs. review) can be deferred — start with always proceeding to the next planned lesson.

### 4c: Free conversation mode

Free conversation already works end-to-end. The remaining work:

- Build the cross-session learner model from SQL queries (see [data.md § Cross-Session Memory](data.md#c-cross-session-memory)) and include it in the system prompt for both lesson and free conversation modes.
- This is the `## Learner Model` section in the system prompt: overall accuracy, strongest/weakest areas, vocabulary retention stats, recent progress. Assembled from `grammar_concepts`, `corrections`, `vocabulary`/`flashcards`, and `daily_stats` tables.

### 4d: Mid-conversation adaptation

- Track `estimated_comprehension` from `ParsedTutorResponse.internal_notes` across turns.
- When comprehension drops (e.g. 2+ consecutive "low" readings), inject a `<system_update>` message into the conversation context telling the tutor to adjust (more scaffolding, simpler sentences, more English).
- When comprehension is consistently high, inject the opposite (less scaffolding, more target language).

**Notes:**
- This is an enhancement that can be deferred if Phase 4 is getting large. The core lesson flow is more important.

---

**Exit criteria**: user can complete onboarding and see a generated lesson path on the dashboard. Starting a lesson sets the conversation system prompt with lesson-specific context. Completing a lesson updates progress and triggers the next lesson. Free conversations include the cross-session learner model. Sidebar conversations have unique titles and can be clicked to reload.
