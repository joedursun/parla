# Duo — Implementation Plan

This plan targets a single-process Tauri + Rust desktop app running exclusively on Apple Silicon with 128 GB unified memory. Audio (speech-to-speech) is the primary interaction mode and drives architectural decisions throughout.

See [app.md](app.md) for product vision, [data.md](data.md) for schema and LLM prompts, [audio.md](audio.md) for model research and pipeline design.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│  Tauri App (single process)                         │
│                                                     │
│  ┌───────────┐   ┌───────────┐   ┌───────────────┐ │
│  │  WebView   │   │ Rust Core │   │  C/C++ Libs   │ │
│  │  (UI)      │◄─►│  (async)  │◄─►│  via FFI      │ │
│  │  HTML/CSS  │   │  Tokio    │   │               │ │
│  │  JS/TS     │   │           │   │  llama.cpp    │ │
│  └───────────┘   │  SQLite   │   │  whisper.cpp  │ │
│                  │  Audio I/O │   │  Kokoro/ONNX  │ │
│                  │  Pipeline  │   │  Silero VAD   │ │
│                  └───────────┘   └───────────────┘ │
└─────────────────────────────────────────────────────┘
```

**Rust side**: Tokio async runtime orchestrates audio capture, VAD, STT, LLM inference, TTS, and playback as concurrent tasks communicating over channels. Native C/C++ libraries are called via FFI bindings (llama.cpp via `llama-cpp-rs`, whisper.cpp via `whisper-rs`, ONNX Runtime via `ort`).

**Frontend**: Tauri webview with a TypeScript UI. Communicates with the Rust backend through Tauri's IPC commands and event system. All inference and audio processing stays in Rust — the frontend is purely presentation.

**Memory budget** (128 GB available):

| Component | Model | Est. Memory |
|-----------|-------|-------------|
| LLM | Gemma 4 26B Q4_0 | ~14 GB |
| LLM KV cache | 32K context | ~8 GB |
| STT | Whisper large-v3 (GGUF) | ~3 GB |
| VAD | Silero VAD v6 (ONNX) | ~2 MB |
| TTS | Kokoro 82M (ONNX) | ~0.5 GB |
| App + OS + headroom | — | ~10 GB |
| **Total** | | **~37 GB** |

With 128 GB we have massive headroom. Q4_0 works well in practice and keeps the footprint small (~37 GB total), leaving ~91 GB free for OS and other apps.

---

## Phase 1 — Audio Pipeline (the critical path) ✓

The voice loop is the core experience and the hardest integration work. Get this working end-to-end before building any learning features.

### 1a: Project scaffold and audio I/O

- Initialize Tauri project with Rust backend and TypeScript frontend
- Set up `cpal` (Rust audio library) for microphone capture and speaker playback
  - 16 kHz mono input stream for STT
  - 24 kHz output stream for TTS playback
  - Callback-based non-blocking I/O on dedicated audio threads
- Wire up a minimal UI: a single mic button that captures audio and plays it back (loopback test)
- Tokio channels (`mpsc`) bridging audio threads ↔ async tasks

### 1b: Voice Activity Detection

- Integrate Silero VAD v6 via `ort` (ONNX Runtime Rust bindings)
- Process 32ms audio chunks from the mic input stream
- Detect speech start/end with configurable thresholds (activation: 0.5, min silence: 550ms, prefix padding: 500ms)
- Output complete utterances as audio segments to an async channel
- Test: speak into mic, verify VAD correctly segments utterances

### 1c: Speech-to-Text

- Integrate whisper.cpp via `whisper-rs` crate
- Load Whisper large-v3 GGUF model (we have memory for the largest model)
- Run transcription on complete utterance segments from VAD
- Use `whisper_full()` with Metal acceleration
- Output transcribed text to a channel
- Test: speak into mic → see transcription printed in console/UI

### 1d: Text-to-Speech

- Integrate Kokoro 82M via ONNX Runtime (`ort` crate)
- Load model, configure voice preset
- Accept text input, produce 24 kHz audio samples
- Stream audio to the playback output
- Test: type text → hear it spoken

### 1e: End-to-end voice loop with stub LLM

- Connect the full pipeline: mic → VAD → STT → [stub echo response] → TTS → speaker
- The "stub" just echoes back or responds with a canned phrase
- Measure and log end-to-end latency at each stage
- Implement sentence buffering: accumulate LLM tokens into sentences before dispatching to TTS
- Implement barge-in: when VAD detects speech during TTS playback, flush output buffer and cancel in-flight generation

**Exit criteria**: speak a sentence, hear a spoken response, with latency under 1.5 seconds end-to-end.

---

## Phase 2 — LLM Integration ✓

### 2a: llama-server integration ✓

- Integrated Gemma 4 26B via llama-server (HTTP API) instead of llama-cpp-rs (the latter doesn't support Gemma 4 yet)
- Full Metal GPU offload, flash attention, 32K context
- Streaming token generation via SSE, yielded over async channels

### 2b: Structured conversation output ✓

- Parameterized system prompt builder (`build_system_prompt()`) accepting target language, student profile, level, goals, and optional lesson context — language-agnostic, supports free conversation mode
- Streaming JSON parser extracts `tutor_message`, `correction`, `new_vocabulary`, `grammar_note`, `suggested_responses`, `internal_notes`
- Sentence-level streaming dispatches complete sentences to TTS as they form

### 2c: Full voice-to-voice conversation ✓

- Pipeline working: mic → VAD → STT → parameterized prompt → LLM (streaming) → sentence buffer → TTS → speaker
- Conversation history maintained in memory via `ConversationHistory` (stores its own system prompt)
- Conversation UI renders live messages, corrections, vocab cards, grammar notes, and suggested responses
- All UI pages read from Svelte stores (`src/lib/stores.svelte.ts`) with empty/zero defaults — no mock data anywhere

**Exit criteria**: ✓ multi-turn spoken conversation with the tutor, streamed sentence-by-sentence. All UI shows real data or appropriate empty states.

---

## Phase 3 — Data Layer and Persistence ✓

### 3a: SQLite database ✓

- ✓ Set up SQLite via `rusqlite` with WAL mode, all 11 tables from data.md schema
- ✓ `Db` struct wrapping `Arc<Mutex<Connection>>`, managed as Tauri state
- ✓ Data access functions for `learner_profile`, `conversations`, `messages`, `corrections`, `vocabulary`, `flashcards`

### 3b: Onboarding persistence ✓

- ✓ `create_profile` / `get_profile` Tauri commands
- ✓ Onboarding button persists profile to SQLite and sets system prompt
- ✓ Layout loads profile on startup, redirects to `/onboarding` if none

### 3c: Conversation UI ✓

- ✓ Chat bubbles, corrections, vocabulary cards, grammar notes, suggested responses
- ✓ Mic button, text input, streaming events
- ✓ Auto-scroll on new messages

### 3d: Message persistence ✓

- ✓ `persist_turn()` runs in background after each turn — stores messages, corrections, vocabulary, flashcards
- ✓ `ConversationHistory` tracks `conversation_id`, `message_count`, `error_count` per session
- ✓ Emits events to update frontend stores (flashcard due count, recent vocab, recent conversations)
- ✓ `get_recent_conversations` command for sidebar

### 3e: Conversation summarization

- When history exceeds ~15 turns, summarize older messages via a second LLM call
- Replace old messages in the LLM context with a `<conversation_summary>` block

**Exit criteria**: ✓ user can onboard (pick language, level, goals), have the profile persist to SQLite and populate the UI. Conversations, messages, corrections, and vocabulary persist across app restarts. Flashcard rows accumulate for Phase 5. The sidebar shows real recent conversations and flashcard due counts.

---

## Phase 4 — Onboarding and Lesson System

### 4a: Onboarding flow

- Build the onboarding UI: language selection, self-assessment, goals
- Create `learner_profile` on completion
- Seed `grammar_concepts` for the selected language and level
- Call LLM to generate initial learning path (see [data.md § Initial Setup](data.md#1-initial-setup-onboarding))
- Store generated lessons in `lessons` table

### 4b: Lesson management

- Build the learning path UI showing lesson sequence with status
- Implement lesson start flow:
  - Query learner progress data from SQLite (the queries in [data.md § Lesson Plan Generation](data.md#2-lesson-plan-generation-before-each-lesson))
  - Run adaptation check (proceed vs. review) via LLM
  - Assemble the full conversation system prompt from DB data
- Implement lesson completion:
  - Post-conversation summarization via LLM
  - Update lesson status, success rate
  - Update grammar concept accuracy and status
  - Recompute weak areas
  - Update daily stats
  - Trigger next lesson planning if needed

### 4c: Free conversation mode

- Implement the free conversation system prompt variant (see [data.md § Free Conversation Mode](data.md#free-conversation-mode))
- Same conversation pipeline but no lesson objectives
- Still track vocabulary, corrections, and create flashcards from new words

### 4d: Learner model assembly

- Build the cross-session learner model from SQL queries (see [data.md § Cross-Session Memory](data.md#c-cross-session-memory))
- Include it in every conversation system prompt
- Implement mid-conversation adaptation: inject `<system_update>` messages based on `estimated_comprehension` (see [data.md § Conversation-Level Adaptation](data.md#b-conversation-level-adaptation))

**Exit criteria**: user can onboard, follow a generated lesson path, have lessons adapt to their progress, and hold free conversations — all via voice.

---

## Phase 5 — Flashcard Review System

### 5a: SRS engine

- Implement the FSRS/SM-2 algorithm in Rust (see [data.md § Flashcard Review Flow](data.md#5-flashcard-review-flow))
- Rating → interval calculation (Again/Hard/Good/Easy)
- Status transitions (new → learning → review → mature, with lapses)
- Due card fetching query with priority ordering

### 5b: Review UI

- Flashcard review screen: show card front (target language), reveal back on tap
- Audio playback of pronunciation
- Rating buttons (Again / Hard / Good / Easy)
- Session summary at end of review (cards reviewed, accuracy)
- Voice mode: hear the word, speak the translation (or vice versa)

### 5c: Card browsing and management

- Browse all cards with filtering (by topic, status, lesson)
- Manual card creation
- Card stats (review count, ease, next due date)

**Exit criteria**: vocabulary from conversations automatically becomes flashcards; user can review due cards with SRS scheduling; cards persist and schedule correctly across sessions.

---

## Phase 6 — Progress Tracking and Dashboard

### 6a: Progress views

- CEFR level progress bar
- Skill breakdown (reading, listening, writing, speaking) computed from activity data (see [data.md § Progress Computation](data.md#7-progress-computation))
- Vocabulary by topic with mastery distribution
- Grammar concepts list with status and accuracy
- Weak areas with suggestions
- Streak calendar

### 6b: Dashboard

- Home screen as described in [app.md § The Dashboard](app.md#the-dashboard):
  - Greeting in target language
  - Quick actions: continue lesson, review flashcards, free conversation
  - Stats at a glance (streak, words learned, practice time, accuracy)
  - Learning path preview
  - Recent vocabulary with strength indicators
  - Due flashcard reminder

**Exit criteria**: user has a complete picture of their learning progress; dashboard provides one-tap access to all activities.

---

## Phase 7 — Polish and Optimization

### 7a: Audio pipeline optimization

- Profile and optimize end-to-end voice latency — target under 800ms
- Tune VAD parameters for natural turn-taking
- Implement smarter barge-in (distinguish backchannels like "uh huh" from actual interruptions)
- Add audio level metering in the UI
- Handle edge cases: background noise, multiple speakers, silence timeouts

### 7b: LLM response quality

- Tune system prompts for Gemma 4 26B's specific strengths and formatting tendencies
- Add JSON schema validation and graceful recovery for malformed responses
- Optimize context window usage: measure actual token counts per section, tune conversation history length
- Implement conversation summarization when history exceeds ~15 turns

### 7c: App packaging and distribution

- Bundle all model files with the app or implement first-run model download
- Code-sign and notarize for macOS
- Auto-update mechanism via Tauri's updater
- Crash reporting and local error logging

### 7d: UX refinements

- Animations and transitions for conversation flow
- Keyboard shortcuts
- Dark/light mode
- Settings screen (daily goal, voice preference, language mix ratio)
- Conversation history browser (past lessons and free conversations)

---

## Sequencing Summary

| Phase | Focus | Depends On | Status |
|-------|-------|------------|--------|
| 1 | Audio pipeline (VAD → STT → TTS) | — | ✓ Done |
| 2 | LLM integration + voice-to-voice | Phase 1 | ✓ Done |
| 3 | SQLite + persistence + onboarding | Phase 2 | ✓ Done (summarization pending) |
| 4 | Lessons + adaptation | Phase 3 | — |
| 5 | Flashcard SRS | Phase 3 | — |
| 6 | Progress tracking + dashboard | Phases 4, 5 | — |
| 7 | Polish + optimization | All | — |

Phases 4 and 5 can be worked in parallel once Phase 3 is complete.
