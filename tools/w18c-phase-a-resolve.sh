#!/usr/bin/env bash
#
# W18c Phase A — Nelson supply-chain constant lift helper.
#
# Resolves the six HuggingFace BAAI/bge-small-en-v1.5 constants required by
# crates/atlas-mem0g/src/embedder.rs:
#
#   HF_REVISION_SHA           (40-char hex)
#   ONNX_SHA256               (64-char hex)
#   MODEL_URL                 (full LFS URL incl. revision SHA)
#   TOKENIZER_JSON_SHA256     (64-char hex)
#   CONFIG_JSON_SHA256        (64-char hex)
#   SPECIAL_TOKENS_MAP_SHA256 (64-char hex)
#
# Plus the ONNX file size (V4 verification per spike §12).
#
# Usage:  bash tools/w18c-phase-a-resolve.sh
# Output: six clearly-labeled values + one size-in-MB. Paste back to agent.

set -euo pipefail

MODEL="BAAI/bge-small-en-v1.5"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "=== W18c Phase A — HuggingFace constant resolution ==="
echo "Model: $MODEL"
echo "Tmpdir: $TMPDIR"
echo ""

# Step 1: HF_REVISION_SHA via API
echo "[1/3] Fetching latest commit SHA on main..."
HF_REVISION_SHA=$(curl -sf "https://huggingface.co/api/models/$MODEL" \
  | python -c "import json,sys; d=json.load(sys.stdin); print(d['sha'])")
if [[ ! "$HF_REVISION_SHA" =~ ^[0-9a-f]{40}$ ]]; then
  echo "ERROR: HF_REVISION_SHA does not look like a 40-char hex SHA: $HF_REVISION_SHA" >&2
  exit 1
fi
echo "    -> $HF_REVISION_SHA"
echo ""

# Step 2: ONNX file download + sha256sum + size
echo "[2/3] Downloading ONNX model file (~130 MB) and computing sha256..."
ONNX_PATH="$TMPDIR/model.onnx"
curl -sfL "https://huggingface.co/$MODEL/resolve/$HF_REVISION_SHA/onnx/model.onnx" -o "$ONNX_PATH"
ONNX_SHA256=$(sha256sum "$ONNX_PATH" | cut -d' ' -f1)
ONNX_SIZE_BYTES=$(stat -c '%s' "$ONNX_PATH" 2>/dev/null || stat -f '%z' "$ONNX_PATH")
ONNX_SIZE_MB=$(echo "scale=2; $ONNX_SIZE_BYTES / 1048576" | bc 2>/dev/null || awk "BEGIN { printf \"%.2f\", $ONNX_SIZE_BYTES / 1048576 }")
echo "    -> sha256: $ONNX_SHA256"
echo "    -> size:   $ONNX_SIZE_BYTES bytes ($ONNX_SIZE_MB MB)"
echo ""

# Step 3: three tokenizer files
echo "[3/3] Downloading 3 tokenizer files + computing sha256..."
declare -A TOKENIZER_SHAS
for f in tokenizer.json config.json special_tokens_map.json; do
  curl -sfL "https://huggingface.co/$MODEL/resolve/$HF_REVISION_SHA/$f" -o "$TMPDIR/$f"
  SHA=$(sha256sum "$TMPDIR/$f" | cut -d' ' -f1)
  TOKENIZER_SHAS[$f]=$SHA
  echo "    -> $f: $SHA"
done
echo ""

MODEL_URL="https://huggingface.co/$MODEL/resolve/$HF_REVISION_SHA/onnx/model.onnx"

echo "================================================================="
echo "PASTE THESE LINES BACK TO THE AGENT:"
echo "================================================================="
echo ""
echo "HF_REVISION_SHA           = $HF_REVISION_SHA"
echo "ONNX_SHA256               = $ONNX_SHA256"
echo "MODEL_URL                 = $MODEL_URL"
echo "TOKENIZER_JSON_SHA256     = ${TOKENIZER_SHAS[tokenizer.json]}"
echo "CONFIG_JSON_SHA256        = ${TOKENIZER_SHAS[config.json]}"
echo "SPECIAL_TOKENS_MAP_SHA256 = ${TOKENIZER_SHAS[special_tokens_map.json]}"
echo "ONNX_FILE_SIZE_BYTES      = $ONNX_SIZE_BYTES"
echo "ONNX_FILE_SIZE_MB         = $ONNX_SIZE_MB"
echo ""
echo "================================================================="
echo "Resolved at: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "================================================================="
