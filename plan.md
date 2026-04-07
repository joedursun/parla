# Parla — Implementation Plan

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

## Phase 4 — Onboarding and Lesson System (partial ✓)

### 4a: Onboarding flow ✓

- ✓ Grammar concepts seeded for 9 languages × 4 CEFR levels (`db/grammar_seeds.rs`)
- ✓ LLM generates 10-lesson learning path on profile creation (background task)
- ✓ Lessons stored in DB, emitted to frontend via `lessons-updated` event

### 4b: Lesson management (partial ✓)

- ✓ Learning path UI on dashboard with real DB data
- ✓ Lesson start flow: marks in_progress, creates conversation, sets `LessonContext` system prompt
- ✓ Daily stats updated on each conversation turn
- Lesson completion flow (summarization, grammar updates, next lesson) — remaining

### 4c: Free conversation mode ✓

- ✓ Free conversation works end-to-end (from Phase 2)
- Cross-session learner model assembly — remaining

### 4d: Learner model assembly

- Cross-session learner model from SQL queries — remaining
- Mid-conversation adaptation — remaining (can defer)

**Exit criteria**: ✓ user can onboard, see generated lesson path, start lessons. Remaining: lesson completion, learner model.

---

## Phase 5 — Flashcard Review System ✓

### 5a: SRS engine ✓

- ✓ SM-2 algorithm in Rust (`db.review_flashcard()`)
- ✓ Rating → interval: Again (reset), Hard (×1.2), Good (×ease), Easy (×ease×1.3)
- ✓ Status transitions: new → learning → review → mature, with lapses
- ✓ Due card fetching with priority ordering, review log

### 5b: Review UI ✓

- ✓ Review screen with flip animation, TTS, rating buttons, response time, keyboard shortcuts

### 5c: Card browsing and management (partial)

- ✓ Browse all cards with status filtering
- Manual card creation — remaining
- Card stats — partially shown

**Exit criteria**: ✓ vocabulary → flashcards → SRS review → persistence across sessions.

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
| 3 | SQLite + persistence + onboarding | Phase 2 | ✓ Done |
| 4 | Lessons + adaptation | Phase 3 | Partial ✓ (lesson start done, completion remaining) |
| 5 | Flashcard SRS | Phase 3 | ✓ Done |
| 6 | Progress tracking + dashboard | Phases 4, 5 | — |
| 7 | Polish + optimization | All | — |
