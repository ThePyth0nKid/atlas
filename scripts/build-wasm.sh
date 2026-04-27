#!/usr/bin/env bash
# Build the WASM verifier and copy artifacts into the atlas-web public/wasm directory.
#
# Run from repo root: bash scripts/build-wasm.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "→ wasm-pack build atlas-verify-wasm"
cd crates/atlas-verify-wasm
wasm-pack build --target web --release --out-dir pkg

cd "$REPO_ROOT"

DEST="apps/atlas-web/public/wasm"
echo "→ copying pkg/ → $DEST"
mkdir -p "$DEST"
cp crates/atlas-verify-wasm/pkg/atlas_verify_wasm.js "$DEST/"
cp crates/atlas-verify-wasm/pkg/atlas_verify_wasm_bg.wasm "$DEST/"
cp crates/atlas-verify-wasm/pkg/atlas_verify_wasm.d.ts "$DEST/"

echo "→ done. wasm artifacts in $DEST"
ls -lah "$DEST"
