#!/usr/bin/env bash
#
# Parla — Setup Script
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
APP_DATA_DIR="$HOME/Library/Application Support/com.parla.app"
MODELS_DIR="$APP_DATA_DIR/models"
VOICES_DIR="$MODELS_DIR/voices"
TMP_DIR="$REPO_DIR/tmp"
HF_CACHE_DIR="$HOME/.cache/huggingface/hub"

# Model URLs
SILERO_VAD_URL="https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx"
WHISPER_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
KOKORO_MODEL_URL="https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/main/onnx/model.onnx"
KOKORO_VOICE_URL="https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/main/voices/af_heart.bin"
# Gemma 4 26B A4B (MoE: 25.2B total, 3.8B active) at Q4_K_M — ~15.5 GB
GEMMA_URL="https://huggingface.co/unsloth/gemma-4-26B-A4B-it-GGUF/resolve/main/gemma-4-26B-A4B-it-Q4_K_M.gguf"
GEMMA_FILE="gemma-4-26B-A4B-it-Q4_K_M.gguf"

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
echo "=== Parla Setup ==="
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

# ── 4. Gemma LLM ───────────────────────────────────────────────────────────
# 15.5 GB file. Order of operations:
#   1. If already in MODELS_DIR, skip.
#   2. If the Hugging Face cache (shared with llama-server) has it, copy it in.
#      On APFS this is a clonefile copy — near-instant, zero extra disk use.
#   3. Otherwise prompt/download, unless PARLA_SKIP_LLM=1 or --yes/--llm is set.

# Find a Gemma Q4_K_M gguf in the HF cache. Looks in several repo names
# (different uploaders publish the same weights). Returns the resolved path
# on stdout, empty if not found.
find_gemma_in_hf_cache() {
    [ -d "$HF_CACHE_DIR" ] || return 0
    local repos=(
        "models--ggml-org--gemma-4-26B-A4B-it-GGUF"
        "models--unsloth--gemma-4-26B-A4B-it-GGUF"
        "models--google--gemma-4-26b-a4b-it-gguf"
    )
    for repo in "${repos[@]}"; do
        local snapdir="$HF_CACHE_DIR/$repo/snapshots"
        [ -d "$snapdir" ] || continue
        # Iterate every snapshot revision under this repo.
        for rev in "$snapdir"/*/; do
            [ -d "$rev" ] || continue
            local candidate="$rev$GEMMA_FILE"
            if [ -e "$candidate" ]; then
                # Resolve symlinks (HF cache uses snapshot → blob symlinks).
                # -f is GNU readlink; on macOS we need to walk manually, but
                # `cp -L` follows symlinks so we can return the symlink path.
                echo "$candidate"
                return 0
            fi
        done
    done
    return 0
}

echo "── Gemma 4 26B LLM ──"
GEMMA_DEST="$MODELS_DIR/$GEMMA_FILE"
if [ -f "$GEMMA_DEST" ]; then
    skip "$GEMMA_FILE"
else
    HF_CACHED="$(find_gemma_in_hf_cache)"
    if [ -n "$HF_CACHED" ] && [ -e "$HF_CACHED" ]; then
        info "Found in Hugging Face cache: ${HF_CACHED/#$HOME/~}"
        info "Copying into app models dir (clonefile on APFS — near-instant)..."
        # cp -L dereferences the snapshot symlink so we get a regular file.
        # -c asks macOS cp to use clonefile(2) where possible (APFS only).
        if cp -cL "$HF_CACHED" "$GEMMA_DEST.part" 2>/dev/null; then
            mv "$GEMMA_DEST.part" "$GEMMA_DEST"
            ok "$GEMMA_FILE ($(du -h "$GEMMA_DEST" | cut -f1 | xargs))"
        else
            # -c not supported (non-APFS volume or older macOS); fall back to plain cp.
            rm -f "$GEMMA_DEST.part"
            info "clonefile unavailable, doing a regular copy (will use disk)..."
            if cp -L "$HF_CACHED" "$GEMMA_DEST.part"; then
                mv "$GEMMA_DEST.part" "$GEMMA_DEST"
                ok "$GEMMA_FILE ($(du -h "$GEMMA_DEST" | cut -f1 | xargs))"
            else
                rm -f "$GEMMA_DEST.part"
                fail "Failed to copy from HF cache"
            fi
        fi
    elif [ "${PARLA_SKIP_LLM:-}" = "1" ]; then
        warn "Skipping Gemma download (PARLA_SKIP_LLM=1)"
        info "Re-run without PARLA_SKIP_LLM or download manually:"
        info "  curl -L -o '$GEMMA_DEST' '$GEMMA_URL'"
    elif [ "${PARLA_YES:-}" = "1" ] || [ "${1:-}" = "--yes" ] || [ "${1:-}" = "--llm" ]; then
        download_if_missing "$GEMMA_URL" "$GEMMA_DEST" "$GEMMA_FILE (~15.5 GB)"
    else
        warn "Gemma 4 26B A4B Q4_K_M is ~15.5 GB — this is the biggest file."
        printf "  Download now? [y/N] "
        read -r answer
        case "$answer" in
            [Yy]|[Yy][Ee][Ss])
                download_if_missing "$GEMMA_URL" "$GEMMA_DEST" "$GEMMA_FILE (~15.5 GB)"
                ;;
            *)
                warn "Skipped. Re-run with --llm to download later, or:"
                info "  curl -L -o '$GEMMA_DEST' '$GEMMA_URL'"
                ;;
        esac
    fi
fi
echo ""

# ── 5. System dependencies ─────────────────────────────────────────────────

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
