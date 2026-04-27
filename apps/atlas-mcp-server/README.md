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
| `atlas_export_bundle` | Emit an `AtlasTrace` + matching `PubkeyBundle` for a workspace |
| `atlas_workspace_state` | Inspect current DAG tips, event count, kid roster |

All tools take a `workspace` argument (default: `ws-mcp-default`).
Workspaces are isolated on disk under `data/{workspace}/events.jsonl`.

---

## Trust property preserved

The single non-negotiable: the canonical signing-input that `atlas-mcp-server`
produces must be **bit-identical** to what `atlas-trust-core` recomputes
during verification. We preserve this structurally — not by re-implementing
CBOR canonicalisation in TypeScript, but by spawning the Rust `atlas-signer`
binary for every signed write. There is no second canonicalisation path
that can drift independently.

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

# 3. Run the smoke test (writes 3 events → exports → CLI-verifies)
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

## V1 boundaries

- **Signing keys are deterministic test keys** (matching the bank-demo
  keys in `crates/atlas-signer/examples/seed_bank_demo.rs`). V2 ships
  TPM/HSM-sealed keys; the `--secret-hex` flag is removed at the build
  level.
- **Persistence is JSONL on local disk.** V2 ships pluggable storage
  (Postgres, S3, FalkorDB).
- **No multi-tenant key isolation in V1.** V2 ships per-tenant key
  policies and bundle-rotation workers.
- **No Rekor anchoring in V1.** V1.5 ships Sigstore anchoring; until
  then exported bundles have `anchors: []`.

See [../../docs/ARCHITECTURE.md](../../docs/ARCHITECTURE.md) for the
full system context.
