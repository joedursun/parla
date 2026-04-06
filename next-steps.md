# Next Steps — Phase 3: Data Layer & Message Persistence

The app is mock-data-free: all UI pages read from Svelte stores (`src/lib/stores.svelte.ts`) initialized to empty/zero defaults, showing appropriate empty states. The backend system prompt is parameterized via `build_system_prompt()` and the `ConversationHistory` stores its own system prompt string. The conversation voice loop works end-to-end (mic → VAD → STT → LLM → streaming JSON → TTS → speaker).

Phase 3 adds SQLite so conversations, vocabulary, and corrections persist across sessions, and wires the onboarding flow so the user's profile populates the stores and the system prompt.

---

### 3a: SQLite setup and schema ✓

- ✓ Added `rusqlite` crate with `bundled` feature
- ✓ Created `src-tauri/src/db/mod.rs` — `Db` struct wrapping `Arc<Mutex<Connection>>`
- ✓ Full 11-table schema from data.md as `CREATE TABLE IF NOT EXISTS` migration
- ✓ WAL mode + foreign keys enabled
- ✓ DB file at `<app_data_dir>/duo.db`, managed as Tauri state
- ✓ Data access functions for Phase 3 tables: `learner_profile`, `conversations`, `messages`, `corrections`, `vocabulary`, `flashcards`

### 3b: Onboarding persistence ✓

- ✓ `create_profile` Tauri command — inserts `learner_profile`, sets system prompt
- ✓ `get_profile` Tauri command — returns profile or null
- ✓ Onboarding "Start my first lesson" button calls `createProfile()` and navigates to `/`
- ✓ Layout `onMount` loads profile; redirects to `/onboarding` if none exists
- ✓ `setUserProfile()` setter in stores.svelte.ts for cross-module reactivity

### 3c: Message persistence ✓

- ✓ `ConversationHistory` now tracks `conversation_id`, `message_count`, `error_count` per session
- ✓ `persist_turn()` helper runs in a background `spawn_blocking` after each turn — never blocks voice loop
- ✓ Creates a `conversations` row on first turn, inserts student + tutor `messages`
- ✓ Persists `corrections`, upserts `vocabulary`, creates `flashcards` (status=new, due_date=now)
- ✓ Updates `conversations.message_count` and `error_count`
- ✓ Emits `flashcards-due-count` and `recent-vocabulary` events to update frontend stores
- ✓ Layout listens for these events via `setFlashcardsDueCount()` and `setRecentVocabulary()` setters

### 3d: Recent conversations in sidebar ✓

- ✓ `get_recent_conversations` Tauri command — queries last 20 conversations
- ✓ Called on app startup in layout `onMount`
- ✓ `persist_turn()` emits `recent-conversations` event after each turn
- ✓ Layout listens and updates `recentConversations` store via `setRecentConversations()`

### 3e: Conversation UI polish ✓

- ✓ Auto-scroll: `bind:this` on `.messages` div + `$effect` watches `liveMessages`, `streamingSentences`, `awaitingTutor` and scrolls to bottom via `requestAnimationFrame`
- Text input send: already wired and working

### 3f: Conversation summarization

- When conversation history exceeds ~15 turns, summarize older messages via a second LLM call (see [data.md § Model Context Window Budget](data.md#appendix-model-context-window-budget))
- Replace old messages in the LLM context with a `<conversation_summary>` block
- This prevents context overflow on long sessions — important because the system prompt alone is ~1200 tokens

**Notes:**
- This can be a simple Tauri command that calls llama-server's `/v1/chat/completions` with a "summarize this conversation" prompt and returns the summary string. Reuse the existing `LlmState.generate()` infrastructure.
- The summarization call should be non-blocking — run it after a turn completes and the user is reading/listening, not while they're waiting for a response.

---

**Exit criteria**: user can onboard (pick language, level, goals), have the profile persist to SQLite and populate the UI. Conversations, messages, corrections, and vocabulary persist across app restarts. Flashcard rows accumulate for Phase 5. The sidebar shows real recent conversations and flashcard due counts.
