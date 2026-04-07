# Next Steps — Remaining Phase 4 Work

Phase 3 loose ends (conversation titles, sidebar click handler) were already implemented. Phase 4a (grammar seeding, lesson generation) and Phase 5 (SRS engine + review UI) are done. The data layer, lesson system foundations, and flashcard review persistence are all functional.

---

### Remaining work

#### Conversation summarization (loose end from Phase 3)
When conversation history exceeds ~15 turns, summarize older messages via a second LLM call. Replace old messages in the LLM context with a `<conversation_summary>` block. This prevents context overflow on long sessions.

**Implementation notes:**
- Check turn count against `ConversationHistory.inner` length in `conversation_turn` (after the LLM response completes, before returning)
- When message count > 30 (15 turns × 2 messages each), extract older messages, call LLM with a summarization prompt
- Replace the older messages in `ConversationHistory.inner` with a single system message containing the summary
- Store the summary in `conversations.summary` column (already exists in schema)
- Run non-blocking after the turn completes — summarize in a background `spawn_blocking`

#### Lesson completion flow (4b)
When a lesson conversation ends:
- Post-conversation summarization via LLM
- Update `lessons` row: `status = 'completed'`, `success_rate`, `completed_at`
- Update `grammar_concepts` accuracy and status
- Recompute `weak_areas`
- Trigger next lesson planning if needed

**Implementation notes:**
- Add a `complete_lesson` Tauri command
- Could be triggered manually by the user (button in conversation UI) or automatically when `internal_notes.lesson_progress_pct` reaches 100
- `lesson_progress_pct` is already parsed from the LLM response (`ParsedTutorResponse.internal_notes`) but not acted on
- The summarization prompt from data.md (lines 480-551) returns `vocabulary_mastery`, `grammar_mastery`, `success_rate`, `suggested_next_focus`
- After completion, emit `lessons-updated` event so the dashboard refreshes

#### Cross-session learner model (4c + 4d)
Build the learner model from SQL queries and include it in every conversation system prompt.

**Implementation notes:**
- Aggregate data from `grammar_concepts`, `corrections`, `vocabulary`/`flashcards`, `daily_stats`
- Format as a `## Learner Model` section appended to the system prompt
- Add to `build_system_prompt()` as a new optional `learner_model: Option<&str>` parameter
- Include: overall accuracy %, strongest/weakest areas with %, vocabulary retention counts (mastered/learning/new), recent progress (last lesson, success rate)
- The SQL queries are defined in data.md § Cross-Session Memory (lines 642-661)
- Optional follow-up: mid-conversation adaptation based on `estimated_comprehension` trends (inject `<system_update>` messages). Can defer.

#### Dashboard stats (Phase 6 preview)
- `update_daily_stats()` and `get_streak()` already exist in Rust (`db/mod.rs`) — the DB is being updated on each conversation turn
- Need: a `get_dashboard_stats` Tauri command that queries streak, total words learned, recent practice time, and flashcard accuracy
- Need: frontend wiring to load stats on dashboard mount and populate the stat cards
- Need: activity heatmap data (query `daily_stats` for last 28 days, map to intensity levels)

---

**Exit criteria**: user can complete onboarding → lessons generated → start a lesson → conversation with lesson context → complete lesson → progress updates. Flashcard reviews persist with SRS scheduling. Sidebar conversations have unique titles and can be reloaded.
