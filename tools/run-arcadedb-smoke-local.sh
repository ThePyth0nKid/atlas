#!/usr/bin/env bash
# Atlas V2-β Welle 17c — local-dev runner for the ArcadeDB smoke + bench
# integration tests.
#
# Mirrors `.github/workflows/atlas-arcadedb-smoke.yml`: spins up an
# ephemeral ArcadeDB sidecar via `infra/docker-compose.arcadedb-smoke.yml`,
# waits for healthcheck, exports the env vars the integration tests
# read (`ATLAS_ARCADEDB_URL`, `ATLAS_ARCADEDB_USERNAME`,
# `ATLAS_ARCADEDB_PASSWORD`), runs `cross_backend_byte_determinism` +
# `arcadedb_benchmark`, then tears the sidecar down on exit.
#
# Usage from repo root:
#   bash tools/run-arcadedb-smoke-local.sh
#
# Prerequisites:
#   - Docker Engine + docker-compose v2 (ships with Docker Desktop).
#   - Cargo / Rust stable.
#
# Notes:
#   - Test password `playwithdata` matches the CI workflow + the test
#     file's documented expectation. Ephemeral container, no inbound
#     exposure.
#   - `down -v` in the EXIT trap removes the named volume even though
#     the compose file declares none — defensive against future
#     compose-file changes.

set -euo pipefail

COMPOSE_FILE="infra/docker-compose.arcadedb-smoke.yml"

if ! [ -f "$COMPOSE_FILE" ]; then
  echo "error: $COMPOSE_FILE not found. Run from atlas repo root." >&2
  exit 1
fi

export ATLAS_ARCADEDB_ROOT_PASSWORD="${ATLAS_ARCADEDB_ROOT_PASSWORD:-playwithdata}"

cleanup() {
  echo "tearing down ArcadeDB sidecar..."
  docker compose -f "$COMPOSE_FILE" down -v >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "starting ArcadeDB sidecar..."
docker compose -f "$COMPOSE_FILE" up -d

echo "waiting for ArcadeDB healthcheck (up to ~90 s)..."
tries=0
until docker compose -f "$COMPOSE_FILE" ps arcadedb --format json 2>/dev/null \
        | grep -q '"Health":"healthy"'; do
  tries=$((tries + 1))
  if [ "$tries" -gt 45 ]; then
    echo "error: ArcadeDB did not become healthy in time" >&2
    docker compose -f "$COMPOSE_FILE" logs arcadedb >&2
    exit 1
  fi
  sleep 2
done
echo "ArcadeDB is healthy (after $((tries * 2)) s of polling)"

export ATLAS_ARCADEDB_URL="http://localhost:2480"
export ATLAS_ARCADEDB_USERNAME="root"
export ATLAS_ARCADEDB_PASSWORD="${ATLAS_ARCADEDB_ROOT_PASSWORD}"

CARGO="${CARGO:-cargo}"

echo "running cross_backend_byte_determinism..."
"$CARGO" test -p atlas-projector \
  --test cross_backend_byte_determinism -- --ignored --nocapture

echo "running arcadedb_benchmark..."
"$CARGO" test -p atlas-projector \
  --test arcadedb_benchmark -- --ignored --nocapture

echo ""
echo "all W17c smoke + bench tests passed locally."
