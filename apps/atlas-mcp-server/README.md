# atlas-mcp-server

The Model Context Protocol (MCP) server that lets AI agents — Claude
Desktop, Cursor, custom — write to an Atlas verifiable knowledge graph
through tool calls. Every write is signed by `atlas-signer` (Rust,
Apache-2.0) before it touches storage; every export emits a bundle that
any party can verify offline with `atlas-verify-cli`.

This package is licensed under the **Atlas Sustainable Use License**.
Self-hosting for your own internal use is free; running it as a
multi-tenant service for third parties requires a commercial licence
(nelson@ultranova.io).

---

## Tools exposed

| Tool | Purpose |
|---|---|
| `atlas_write_node` | Append a `node.create` event (dataset, model, inference, etc.) |
| `atlas_write_annotation` | Append an `annotation.add` event — used for human verification |
| `atlas_anchor_bundle` | Issue inclusion proofs over current `dag_tips` + `pubkey_bundle_hash`. Optional `rekor_url` field (or `ATLAS_REKOR_URL` env, precedence: field > env > mock) selects live Sigstore Rekor v1 submission or in-process mock-Rekor issuer. |
| `atlas_export_bundle` | Emit an `AtlasTrace` + matching `PubkeyBundle` for a workspace |
| `atlas_workspace_state` | Inspect current DAG tips, event count, kid roster |

All tools take a `workspace` argument (default: `ws-mcp-default`).
Workspaces are isolated on disk under `data/{workspace}/events.jsonl`;
issued anchors are persisted alongside as `data/{workspace}/anchors.json`
and folded into `trace.anchors` on export.

---

## Trust property preserved

The single non-negotiable: the canonical bytes the MCP server emits
(both signing-input and pubkey-bundle hash) must be **bit-identical**
to what `atlas-trust-core` recomputes during verification.

The MCP server in TypeScript owns *zero* canonical-bytes formatting:

- Event signing → spawns `atlas-signer sign` (canonical CBOR signing-input
  + Ed25519 + emit `AtlasEvent` JSON).
- Pubkey-bundle hash → spawns `atlas-signer bundle-hash` (canonical
  JSON over the bundle + blake3 + emit hex).

Both subcommands use the same Rust functions the verifier later runs.
A drift between TS and Rust is structurally impossible because TS has
no canonicalisation code path to drift.

Secret material is delivered to the signer via stdin (`--secret-stdin`),
not argv — argv values appear in OS process listings and shell history.

If the smoke test (`pnpm smoke`) ever fails to verify ✓ VALID, the bug is
real, not cosmetic.

---

## Running locally

```bash
# 1. Build the Rust signer (release for speed)
cargo build --release -p atlas-signer

# 2. Install + build the MCP server
cd apps/atlas-mcp-server
pnpm install
pnpm build

# 3. Run the smoke test (writes 3 events → issues 2 anchors → exports → CLI-verifies)
pnpm smoke
```

Expected smoke output ends with:

```
✓ atlas-verify-cli: VALID
  ✓ schema-version
  ✓ pubkey-bundle-hash
  ✓ event-hashes
  ✓ event-signatures
  ✓ parent-links
  ✓ dag-tips
  ✓ anchors
```

---

## Wiring it into Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`
(macOS) or the equivalent on Windows / Linux:

```json
{
  "mcpServers": {
    "atlas": {
      "command": "node",
      "args": ["/absolute/path/to/atlas/apps/atlas-mcp-server/dist/index.js"]
    }
  }
}
```

After restarting Claude Desktop, the four `atlas_*` tools appear in the
tool palette. Every tool call produces a signed event in the configured
workspace's `events.jsonl`.

---

## V1.5 / V1.6 boundaries

- **Signing keys are deterministic test keys** (matching the bank-demo
  keys in `crates/atlas-signer/examples/seed_bank_demo.rs`). The MCP
  server passes them to the signer via stdin (`--secret-stdin`); the
  legacy `--secret-hex` argv flag remains in the binary for the
  bank-demo example only and emits a deprecation warning. V2 ships
  TPM/HSM-sealed keys; both flags are removed at the build level.
- **Persistence is JSONL on local disk.** V2 ships pluggable storage
  (Postgres, S3, FalkorDB).
- **No multi-tenant key isolation in V1.5.** V2 ships per-tenant key
  policies and bundle-rotation workers.
- **Anchoring (V1.5 mock-Rekor, V1.6 live Sigstore).** `atlas_anchor_bundle`
  issues anchors via the in-process mock-Rekor by default (V1.5, no
  network call). Setting the `rekor_url` field or `ATLAS_REKOR_URL` env
  (precedence: field > env > mock) opts into live Sigstore Rekor v1
  submission against `https://rekor.sigstore.dev`. The verifier accepts
  both formats by log_id dispatch and validates inclusion proofs +
  checkpoint signatures against pinned log pubkeys (mock Ed25519 or
  Sigstore ECDSA P-256). Verifier path unchanged — fully offline RFC 6962
  proof recomputation in both cases.

See [../../docs/ARCHITECTURE.md](../../docs/ARCHITECTURE.md) for the
full system context.
