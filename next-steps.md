# Next Steps — Phase 2: LLM Integration

Phase 1 (audio pipeline) is complete: mic → VAD → STT → Kokoro TTS → speaker, all in-memory, no system dependencies. Currently uses a stub echo response.

---

### 2a: llama.cpp integration

- Add `llama-cpp-rs` crate (Rust bindings to llama.cpp)
- Create `src-tauri/src/llm/mod.rs` module
- Load Gemma 4 26B Q4_0 GGUF with full Metal GPU offload (`n_gpu_layers = -1`)
- Enable flash attention, configure 32K context window
- Implement streaming token generation, yielding tokens over a channel to the audio thread
- Add `init_llm` Tauri command to load the model on startup
- Add Gemma model download to `setup.sh`

### 2b: Structured conversation output

- Implement the conversation system prompt assembly from [data.md § The Conversation Loop](data.md#3-the-conversation-loop-core-teaching-flow)
- Use a hardcoded system prompt for now (no DB yet) — Spanish tutor, ordering food scenario
- Parse the LLM's structured JSON response into Rust types:
  - `tutor_message` (target_lang + native_lang)
  - `correction`, `new_vocabulary`, `grammar_note`
  - `suggested_responses`, `internal_notes`
- Handle malformed JSON gracefully (retry with a nudge, or extract what's parseable)
- Sentence-level streaming: as the LLM streams JSON, detect complete `tutor_message.target_lang` sentences and dispatch them to TTS immediately (don't wait for the full response)

### 2c: Full voice-to-voice conversation

- Replace the stub echo in `conversation/+page.svelte` with real LLM responses
- Pipeline: mic → VAD → STT → assemble prompt → LLM (streaming) → sentence buffer → TTS → speaker
- Maintain conversation history in memory, include in LLM context on each turn
- Stream tutor responses to the frontend via Tauri events (not just the final result):
  - `tutor-message-chunk` — partial text as it streams
  - `tutor-message-done` — final message with correction/vocab/grammar
- Update the conversation UI to render streamed messages, corrections, vocabulary cards, grammar notes, and suggested response chips from real LLM output (replacing the static mock data)
- Sentence buffering: accumulate LLM tokens into sentences before dispatching to TTS
- Barge-in: if VAD detects speech during TTS playback, cancel in-flight LLM generation and flush playback

**Exit criteria**: have a multi-turn spoken conversation with the tutor, hearing responses streamed sentence-by-sentence.
