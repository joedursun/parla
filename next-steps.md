# Next Steps — Remaining Phase 4 Work + Phase 5

Phase 3 loose ends (conversation titles, sidebar click handler) were already implemented. Phase 4a (grammar seeding, lesson generation) and Phase 5a (SRS engine) are now in place. The data layer, lesson system foundations, and flashcard review persistence are all functional.

---

### Completed (this session)

#### Grammar concept seeding ✓
- Static grammar concept lists for 9 languages (ES, FR, DE, IT, PT, JA, KO, ZH, TR) at all CEFR levels (A1-B2) in `src-tauri/src/db/grammar_seeds.rs`
- Concepts seeded into `grammar_concepts` table during `create_profile`

#### DB functions for lessons and grammar ✓
- `insert_grammar_concepts`, `get_grammar_concepts` for grammar concept CRUD
- `insert_lessons`, `get_lessons`, `get_lesson`, `update_lesson_status` for lesson management
- `create_lesson_conversation` for linking conversations to lessons
- `get_due_flashcards`, `review_flashcard` implementing SM-2 SRS algorithm
- `update_daily_stats`, `get_streak` for progress tracking

#### LLM-based lesson generation ✓
- `build_lesson_generation_prompt()` in `prompt.rs` — curriculum designer prompt for 10-lesson learning paths
- `generate_initial_lessons()` in `lib.rs` — calls LLM, parses JSON array, inserts into `lessons` table
- Runs in background after `create_profile` completes, emits `lessons-updated` event when done

#### Tauri commands ✓
- `get_lessons` — fetch all lessons ordered by sequence
- `start_lesson` — marks lesson in_progress, creates linked conversation, sets system prompt with `LessonContext`
- `review_flashcard` — SM-2 algorithm: Again/Hard/Good/Easy → interval/ease/status updates, review log

#### Frontend wiring ✓
- `getLessons()`, `startLesson()`, `reviewFlashcard()` in `conversation.ts`
- `setLessons()` store setter, `lessons-updated` event listener in layout
- Dashboard learning path shows real lessons from DB, "Continue Lesson" button starts next lesson
- Lesson items clickable (current → starts lesson, upcoming → disabled)
- Flashcard review persists ratings to backend with response time tracking
- Flashcard IDs flow through the full stack (DB → Rust → TS → UI)
- Daily stats updated on each conversation turn (vocab count, corrections, messages)

---

### Remaining work

#### Conversation summarization (loose end)
When conversation history exceeds ~15 turns, summarize older messages via a second LLM call. Replace old messages in the LLM context with a `<conversation_summary>` block. This prevents context overflow on long sessions.

**Implementation notes:**
- Add a turn count check in `conversation_turn` (or after `persist_turn`)
- When history length > 15, extract older messages, call LLM with a summarization prompt
- Replace the older messages in `ConversationHistory.inner` with a single system message containing the summary
- Store the summary in `conversations.summary` column (already exists in schema)
- Run non-blocking after the turn completes

#### Lesson completion flow (4b)
When a lesson conversation ends:
- Post-conversation summarization via LLM
- Update `lessons` row: `status = 'completed'`, `success_rate`, `completed_at`
- Update `grammar_concepts` accuracy and status
- Recompute `weak_areas`
- Trigger next lesson planning if needed

**Implementation notes:**
- Add a `complete_lesson` Tauri command
- Could be triggered manually by the user (button) or automatically when `lesson_progress_pct` hits 100
- The summarization prompt from data.md returns `vocabulary_mastery`, `grammar_mastery`, `success_rate`, `suggested_next_focus`

#### Cross-session learner model (4c)
Build the learner model from SQL queries and include it in every conversation system prompt.

**Implementation notes:**
- Aggregate data from `grammar_concepts`, `corrections`, `vocabulary`/`flashcards`, `daily_stats`
- Format as a `## Learner Model` section in the system prompt
- Add to `build_system_prompt()` as a new optional parameter
- Include: overall accuracy, strongest/weakest areas, vocabulary retention stats, recent progress

#### Mid-conversation adaptation (4d — can defer)
- Track `estimated_comprehension` from `ParsedTutorResponse.internal_notes` across turns
- Inject `<system_update>` messages based on comprehension trends

#### Dashboard stats (Phase 6 preview)
- Load daily stats, streak, vocabulary counts on dashboard mount
- Populate the stats cards and activity heatmap with real data

---

**Exit criteria**: user can complete onboarding → lessons generated → start a lesson → conversation with lesson context → complete lesson → progress updates. Flashcard reviews persist with SRS scheduling. Sidebar conversations have unique titles and can be reloaded.
