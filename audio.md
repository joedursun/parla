# Building a local voice AI pipeline on Apple Silicon in 2026

**A fully local, single-process speech-to-speech system on a MacBook Pro is achievable today — but not through a single multimodal model.** The most practical architecture combines separate STT, LLM, and TTS components running concurrently via Python's asyncio. Key finding: Gemma 4 26B does not support audio input (only text+image), Voxtral spans both ASR and TTS as separate models, and llama.cpp's audio support is experimental with no Python bindings yet. The optimal stack pairs MLX-based Whisper for STT, llama.cpp for LLM inference, and Kokoro or MLX-Audio for TTS — all orchestrated through a streaming pipeline with Silero VAD for turn detection.

---

## llama.cpp now supports audio input, but it's experimental

The llama.cpp project added audio input via its **`libmtmd` (Multi-Modal) library** in May 2025, marking a significant evolution from its vision-only multimodal roots. Audio support is explicitly labeled "highly experimental and may have reduced quality." The architecture uses a separate multimodal projector (mmproj) GGUF file that encodes audio into the text model's embedding space, with audio decoding handled by the `miniaudio.h` library.

**Four audio-capable models** currently work in llama.cpp's mainline: Ultravox 0.5 (1B/8B), Qwen2-Audio, Qwen2.5-Omni (3B/7B), and Voxtral Mini 3B. The CLI tool `llama-mtmd-cli` and the `llama-server` both accept audio input. Vision support is far more mature, covering **20+ models** including Gemma 3, Qwen2.5-VL, Pixtral 12B, and Mistral Small 3.1.

Metal/Apple Silicon is a **first-class citizen** in llama.cpp, optimized via ARM NEON, Accelerate framework, and Metal GPU shaders. For a 26B parameter model at Q4_K_M quantization (~15 GB), full GPU offload via `-ngl 99` is essential. Performance is memory-bandwidth-bound at this scale — expect **8–15 tokens/second** on M2/M3 Pro chips and considerably more on M4 Max/Ultra. Georgi Gerganov demonstrated Gemma 4 26B Q8_0 running at ~300 t/s on an M2 Ultra. Flash attention (`-fa`) and partial CPU offload are available for memory-constrained setups.

At the C API level, **multiple `llama_model` and `llama_context` instances can coexist** in a single process. The API cleanly separates model loading (`llama_model_load`) from context creation (`llama_init_from_model`), enabling multiple models in memory simultaneously. However, llama.cpp's December 2025 "router mode" for multi-model serving uses a multi-process architecture for crash isolation, with LRU eviction at a configurable model limit.

A notable external fork — **`tc-mb/llama.cpp-omni`** — implements full MiniCPM-o 4.5 support with TTS, full-duplex streaming, and video+audio input by splitting the model into multiple GGUF modules. This points toward a future where llama.cpp mainline may support audio generation.

---

## Gemma 4 26B handles vision only — audio lives on the tiny edge models

**Gemma 4 26B-A4B supports text and image input exclusively.** Audio input is restricted to the smaller E2B and E4B edge models, which include a dedicated ~300M-parameter USM-style conformer audio encoder. The 26B and 31B variants have no audio encoder whatsoever — the official benchmark tables mark audio metrics as "—" for these models.

The 26B model is a Mixture-of-Experts architecture with **25.2B total parameters but only 3.8B active per forward pass** (128 experts, top-8 routing + 1 shared). It supports a **256K-token context window** with hybrid sliding-window/global attention, making it powerful for text and vision tasks. Video understanding works by processing frame sequences (up to 60 seconds at 1 fps).

GGUF files are available from multiple sources. The official `ggml-org/gemma-4-26B-A4B-it-GGUF` provides Q4_K_M at **16.8 GB**, Q8_0 at 26.9 GB, and F16 at 50.5 GB, plus a 1.19 GB mmproj file for vision. Unsloth offers dynamic quantizations. On a 32GB MacBook Pro, the Q4_K_M fits comfortably with room for other models.

llama.cpp supports Gemma 4's vision capabilities through `llama-server` and `llama-mtmd-cli`. However, even for the E2B/E4B models that do have audio encoders, **llama.cpp cannot yet process their audio** — GitHub Issue #21325 confirms "Currently llama-cpp will not parse audio with Gemma." Other engines like vLLM, HuggingFace Transformers, and mistral.rs handle E2B/E4B audio. For the 26B model specifically, the audio question is moot: it simply cannot process audio regardless of the inference engine.

---

## Voxtral is a family spanning both ASR and TTS

This is the most critical clarification: **Voxtral is not one model but a family of distinct speech models** from Mistral AI, covering both recognition and generation:

- **Voxtral Mini 3B** (July 2025): ASR/speech understanding built on a Whisper large-v3 encoder + Ministral 3B decoder. Handles transcription, translation, audio Q&A, and function-calling from voice. Processes up to 30 minutes of audio. **Fully supported in llama.cpp** with official GGUF at `ggml-org/Voxtral-Mini-3B-2507-GGUF`.
- **Voxtral Small 24B** (July 2025): Same architecture scaled up with Mistral Small 3.1 as decoder. Community GGUFs available (~13.5 GB at Q4_K_S).
- **Voxtral Mini 4B Realtime** (February 2026): Streaming ASR with a custom causal encoder (not Whisper-based), achieving sub-500ms latency. Uses Delayed Streams Modeling for configurable transcription delays (80ms–2400ms). **Not supported in llama.cpp** — currently vLLM-only, though community C and Rust implementations exist (`antirez/voxtral.c`).
- **Voxtral 4B TTS** (March 2026): Text-to-speech with a 3.4B decoder backbone + 390M flow-matching acoustic transformer + 300M neural audio codec. Produces **24 kHz audio** with 20 preset voices and zero-shot voice cloning from ~3 seconds of reference audio. **Not in llama.cpp mainline** — community GGUF exists (`TrevorJS/voxtral-tts-q4-gguf`, ~2.67 GB) with standalone C implementation (`mudler/voxtral-tts.c`).

For the user's pipeline, Voxtral Mini 3B is viable as an STT component through llama.cpp, offering richer capabilities than Whisper (it can answer questions about audio, not just transcribe). Voxtral 4B TTS could serve as the speech output stage but requires either the standalone C implementation or vLLM — it's not usable through llama-cpp-python.

---

## llama-cpp-python supports vision but not audio

The Python bindings (v0.3.20, April 2026) provide a high-level `Llama` class with OpenAI-compatible APIs and low-level ctypes bindings to the full C API. **Vision/multimodal works** through chat handlers for LLaVA 1.5/1.6, Qwen2.5-VL, Moondream, and MiniCPM-V. **Audio is not supported** — Issue #2052 (August 2025) remains open with no implementation.

Streaming text generation works reliably via `stream=True`:

```python
for chunk in llm.create_chat_completion(
    messages=[{"role": "user", "content": "Hello"}],
    stream=True
):
    delta = chunk['choices'][0].get('delta', {})
    if 'content' in delta:
        process_token(delta['content'])
```

Metal acceleration requires building with `CMAKE_ARGS="-DGGML_METAL=on" pip install llama-cpp-python` or using pre-built wheels from the Metal index. Set `n_gpu_layers=-1` to offload all layers.

For **multiple simultaneous models**, you can technically instantiate multiple `Llama` objects in one process, but GPU memory is shared globally and each instance manages its own weights. The server's `LlamaProxy` class swaps models on demand rather than keeping them all resident. For a voice pipeline needing STT + LLM + TTS, **running the LLM through llama-cpp-python while using separate libraries for STT and TTS is the pragmatic path**.

---

## Kokoro and MLX-Audio lead the local TTS landscape

For TTS on Apple Silicon, the field has converged around several strong options, with **Kokoro via MLX-Audio emerging as the best balance** of quality, speed, and ease:

**Kokoro TTS** (82M parameters) ranked #1 on the HuggingFace TTS Arena despite its tiny size. It achieves **sub-300ms latency** on Apple Silicon, supports 8 languages with 54+ voices, and installs trivially (`pip install kokoro`). The ONNX variant works on even low-end hardware. Apache 2.0 licensed. No voice cloning, but preset voices are high quality.

**MLX-Audio** (`pip install mlx-audio`) is the premier TTS ecosystem for Apple Silicon, wrapping multiple models with native MLX acceleration and zero-copy unified memory. It supports Kokoro-82M, Qwen3-TTS (1.7B, with voice cloning), and Marvis-TTS (250M). A Swift package (`mlx-audio-swift`) exists for native macOS/iOS apps. Up to **40% faster than PyTorch** on Apple Silicon.

**Orpheus TTS** (150M–3B parameters) produces the most expressive and emotional speech, with support for paralinguistic tags (`<laugh>`, `<sigh>`). The 3B model is heavy but smaller variants (150M, 400M) run through GGUF quantization. Apache 2.0 licensed.

**Piper TTS** remains excellent for ultra-low-latency, low-resource scenarios — models are 15–75 MB ONNX files designed for Raspberry Pi, making them nearly instant on M-series chips. Quality is good but not at Kokoro's level. Now GPL licensed.

**macOS built-in synthesis** (`say` command / AVSpeechSynthesizer) offers zero-latency, zero-setup TTS at the cost of noticeably synthetic voice quality. Premium voices are decent for notifications but not conversational AI.

For the pipeline described, **Kokoro through MLX-Audio provides the best streaming integration**: sentence-level chunked synthesis with async dispatch, sub-300ms time-to-first-byte, and negligible memory footprint alongside a 26B LLM.

---

## Designing the single-process streaming pipeline

The architecture must run audio capture, VAD, STT, LLM, TTS, and playback concurrently within one process. The proven pattern uses **asyncio as the orchestration layer with dedicated audio threads**:

```
[Mic] → sounddevice callback thread → Queue → [Silero VAD] → [STT] →
    → [LLM streaming] → sentence buffer → [TTS streaming] →
    → Queue → sounddevice playback callback thread → [Speaker]
```

**Audio I/O**: `sounddevice` (v0.5.5) is the clear winner for Python on macOS — it bundles PortAudio in its wheel (no Homebrew needed), supports callback-based non-blocking streams, and achieves **5–10ms latency** with the `'low'` setting. Use `sd.InputStream` at 16 kHz mono with blocksize=512 (32ms chunks).

**Voice Activity Detection**: Silero VAD via ONNX Runtime processes each 32ms chunk in **under 1 millisecond** on a single CPU thread. Key parameters: activation threshold 0.5, minimum silence duration 550ms, prefix padding 500ms. MIT licensed, language-agnostic across 6000+ languages.

**STT**: `lightning-whisper-mlx` claims 10x faster than whisper.cpp and 4x faster than standard MLX Whisper on Apple Silicon. The `distil-medium.en` model balances speed and accuracy. Alternative: `mlx-whisper` with `distil-large-v3` for higher accuracy. Expected latency: **200–400ms** per utterance on M2/M3/M4.

**LLM**: llama-cpp-python with Gemma 4 26B Q4_K_M (~16.8 GB). Stream tokens via `create_chat_completion(stream=True)`. Buffer tokens into sentences before dispatching to TTS — this avoids micro-synthesis overhead while maintaining streaming responsiveness.

**TTS**: Kokoro via `kokoro-onnx` async streaming API or MLX-Audio. Dispatch each complete sentence as soon as the LLM produces one. Expected time-to-first-byte: **100–200ms**.

**Inter-stage communication** uses `threading.Queue` for audio thread ↔ asyncio bridges and `asyncio.Queue` within the event loop. CPU-bound inference (Whisper, VAD) runs in `asyncio.run_in_executor()` to avoid blocking. A **circular buffer** for audio capture prevents memory allocation during real-time processing.

**Barge-in handling**: When VAD detects user speech during bot playback, immediately flush the output buffer, cancel in-flight LLM generation, and begin accumulating the new speech segment.

Expected **end-to-end voice-to-voice latency**: 600–1200ms fully local, with the LLM time-to-first-token being the dominant bottleneck at ~300–500ms for a 26B model on consumer Apple Silicon.

---

## The practical component stack and memory budget

For a 32 GB MacBook Pro, the memory budget breaks down as follows:

| Component | Library | Model | Memory |
|-----------|---------|-------|--------|
| STT | lightning-whisper-mlx | distil-medium.en | ~1.5 GB |
| VAD | Silero VAD (ONNX) | silero_vad v6 | ~2 MB |
| LLM | llama-cpp-python | Gemma 4 26B Q4_K_M | ~17 GB |
| TTS | mlx-audio / kokoro-onnx | Kokoro-82M | ~0.5 GB |
| Audio I/O | sounddevice | — | negligible |
| **Total** | | | **~19 GB** |

This leaves ~13 GB headroom for KV cache, OS, and other processes on a 32 GB machine. On a 64 GB machine, you could run the Q8_0 quantization (~27 GB) for better LLM quality, or add a larger TTS model like Qwen3-TTS (1.7 GB).

An alternative STT path: use **Voxtral Mini 3B** through llama.cpp's C API directly (since llama-cpp-python lacks audio support), gaining speech understanding capabilities beyond mere transcription. This requires writing a thin C/Python bridge or using ctypes to call `libmtmd` functions directly — feasible but adds complexity. The simpler path is Whisper via MLX.
