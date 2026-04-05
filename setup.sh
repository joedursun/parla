#!/usr/bin/env bash
#
# Duo — Setup Script
#
# Downloads all required models for local development.
# Idempotent: safe to re-run — skips files that already exist.
#
# Usage:
#   ./setup.sh          # download everything
#   ./setup.sh --clean  # remove tmp/ artifacts only
#

set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_DATA_DIR="$HOME/Library/Application Support/com.duo.app"
MODELS_DIR="$APP_DATA_DIR/models"
VOICES_DIR="$MODELS_DIR/voices"
TMP_DIR="$REPO_DIR/tmp"

# Model URLs
SILERO_VAD_URL="https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx"
WHISPER_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
KOKORO_MODEL_URL="https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/main/onnx/model.onnx"
KOKORO_VOICE_URL="https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/main/voices/af_heart.bin"

# ─── Helpers ────────────────────────────────────────────────────────────────

GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
DIM='\033[2m'
RED='\033[0;31m'
RESET='\033[0m'

info()  { echo -e "  ${CYAN}[info]${RESET}  $1"; }
ok()    { echo -e "  ${GREEN}[ok]${RESET}    $1"; }
skip()  { echo -e "  ${DIM}[skip]${RESET}  $1 ${DIM}(already exists)${RESET}"; }
warn()  { echo -e "  ${YELLOW}[warn]${RESET}  $1"; }
fail()  { echo -e "  ${RED}[fail]${RESET}  $1"; }

download_if_missing() {
    local url="$1"
    local dest="$2"
    local desc="$3"

    if [ -f "$dest" ]; then
        skip "$desc"
        return 0
    fi

    info "Downloading $desc..."
    mkdir -p "$(dirname "$dest")"
    if curl -fL --progress-bar -o "$dest.part" "$url"; then
        mv "$dest.part" "$dest"
        ok "$desc ($(du -h "$dest" | cut -f1 | xargs))"
    else
        rm -f "$dest.part"
        fail "Failed to download $desc"
        return 1
    fi
}

# ─── Handle --clean ─────────────────────────────────────────────────────────

if [ "${1:-}" = "--clean" ]; then
    echo "Cleaning tmp/ directory..."
    rm -rf "$TMP_DIR"
    echo "Done."
    exit 0
fi

# ─── Main ───────────────────────────────────────────────────────────────────

echo ""
echo "=== Duo Setup ==="
echo ""
echo "  Models dir: $MODELS_DIR"
echo ""

mkdir -p "$MODELS_DIR" "$VOICES_DIR" "$TMP_DIR"

# ── 1. Silero VAD ──────────────────────────────────────────────────────────

echo "── Silero VAD ──"
download_if_missing "$SILERO_VAD_URL" "$MODELS_DIR/silero_vad.onnx" "silero_vad.onnx (~2 MB)"
echo ""

# ── 2. Whisper STT ─────────────────────────────────────────────────────────

echo "── Whisper STT ──"
download_if_missing "$WHISPER_URL" "$MODELS_DIR/ggml-small.bin" "ggml-small.bin (~466 MB)"
echo ""

# ── 3. Kokoro TTS ──────────────────────────────────────────────────────────

echo "── Kokoro TTS ──"
download_if_missing "$KOKORO_MODEL_URL" "$MODELS_DIR/kokoro-v0_19.onnx" "kokoro model (~330 MB)"
download_if_missing "$KOKORO_VOICE_URL" "$VOICES_DIR/af_heart.bin" "af_heart voice (~520 KB)"
echo ""

# ── 4. System dependencies ─────────────────────────────────────────────────

echo "── System checks ──"

if command -v espeak-ng &>/dev/null; then
    ok "espeak-ng found ($(espeak-ng --version 2>&1 | head -1))"
else
    warn "espeak-ng not found — Kokoro TTS will fall back to macOS say"
    info "Install with: brew install espeak-ng"
fi

echo ""

# ── Summary ─────────────────────────────────────────────────────────────────

echo "=== Setup Complete ==="
echo ""
echo "  Run the app:"
echo ""
echo "    cargo tauri dev"
echo ""
