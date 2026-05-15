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
# WORK_DIR (renamed from TMPDIR per security-reviewer LOW-2: TMPDIR is a
# system env var consulted by mktemp/curl/python child processes; shadowing
# it is a portability hazard).
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

# python3 is universally available on modern Linux + macOS + Git Bash for
# Windows (when Python is installed). Bare `python` is missing on Ubuntu
# 22.04+ and macOS Ventura+ unless `python-is-python3` is installed
# (code-reviewer MEDIUM + security-reviewer MEDIUM-1). The script does
# require Python to parse the HuggingFace API JSON response.
PYTHON_BIN="${PYTHON_BIN:-python3}"
if ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
  if command -v python >/dev/null 2>&1; then
    PYTHON_BIN="python"
  else
    echo "ERROR: neither python3 nor python found in PATH. Install python3 or set PYTHON_BIN." >&2
    exit 1
  fi
fi

# Format-validate an sha256sum hex output (security-reviewer LOW-1).
# Defends against the case where sha256sum is missing or its output
# parsing produces a malformed value, which would otherwise propagate
# silently to the committed constants. The `pins_well_formed_after_lift`
# Rust test would catch it post-commit; this gate catches it at
# resolution time so Nelson sees the failure immediately.
validate_sha256_hex() {
  local label="$1"
  local value="$2"
  if [[ ! "$value" =~ ^[0-9a-f]{64}$ ]]; then
    echo "ERROR: $label does not look like a 64-char lowercase hex SHA-256: $value" >&2
    exit 1
  fi
}

echo "=== W18c Phase A — HuggingFace constant resolution ==="
echo "Model: $MODEL"
echo "Workdir: $WORK_DIR"
echo "Python: $PYTHON_BIN ($("$PYTHON_BIN" --version 2>&1))"
echo ""

# Step 1: HF_REVISION_SHA via API
echo "[1/3] Fetching latest commit SHA on main..."
HF_REVISION_SHA=$(curl -sf "https://huggingface.co/api/models/$MODEL" \
  | "$PYTHON_BIN" -c "import json,sys; d=json.load(sys.stdin); print(d.get('sha',''))")
if [[ ! "$HF_REVISION_SHA" =~ ^[0-9a-f]{40}$ ]]; then
  echo "ERROR: HF_REVISION_SHA does not look like a 40-char hex SHA: '$HF_REVISION_SHA'" >&2
  echo "       (blank/non-hex suggests HuggingFace API response schema changed —" >&2
  echo "        the response's 'sha' field is missing or renamed)" >&2
  exit 1
fi
echo "    -> $HF_REVISION_SHA"
echo ""

# Step 2: ONNX file download + sha256sum + size
echo "[2/3] Downloading ONNX model file (~130 MB) and computing sha256..."
ONNX_PATH="$WORK_DIR/model.onnx"
curl -sfL "https://huggingface.co/$MODEL/resolve/$HF_REVISION_SHA/onnx/model.onnx" -o "$ONNX_PATH"
ONNX_SHA256=$(sha256sum "$ONNX_PATH" | cut -d' ' -f1)
validate_sha256_hex "ONNX_SHA256" "$ONNX_SHA256"
ONNX_SIZE_BYTES=$(stat -c '%s' "$ONNX_PATH" 2>/dev/null || stat -f '%z' "$ONNX_PATH")
ONNX_SIZE_MB=$(echo "scale=2; $ONNX_SIZE_BYTES / 1048576" | bc 2>/dev/null || awk "BEGIN { printf \"%.2f\", $ONNX_SIZE_BYTES / 1048576 }")
echo "    -> sha256: $ONNX_SHA256"
echo "    -> size:   $ONNX_SIZE_BYTES bytes ($ONNX_SIZE_MB MB)"
echo ""

# Step 3: three tokenizer files
echo "[3/3] Downloading 3 tokenizer files + computing sha256..."
declare -A TOKENIZER_SHAS
for f in tokenizer.json config.json special_tokens_map.json; do
  curl -sfL "https://huggingface.co/$MODEL/resolve/$HF_REVISION_SHA/$f" -o "$WORK_DIR/$f"
  SHA=$(sha256sum "$WORK_DIR/$f" | cut -d' ' -f1)
  validate_sha256_hex "${f^^}_SHA256" "$SHA"
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
