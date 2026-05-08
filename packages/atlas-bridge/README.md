# @atlas/bridge

Internal workspace package consolidating the TypeScript bridge layer
between Atlas consumers (atlas-web, atlas-mcp-server) and the Rust
`atlas-signer` binary plus the on-disk events.jsonl DAG.

## Why this package exists

V1.19 Welle 1 stood up an independent web write surface that duplicated
8 modules from `apps/atlas-mcp-server/src/lib/`. The duplication was a
known shipping shortcut: `WriteEventResult` shapes already diverged
(MCP omits `kid`, web includes it), and any future change to event
storage, signer invocation, key derivation, or schema would have to be
applied in two places under penalty of silent drift.

Welle 2 collapses both copies into a single source of truth. Both apps
now depend on `@atlas/bridge` as a workspace package.

## Surface

| Module          | Purpose                                                     |
| --------------- | ----------------------------------------------------------- |
| `event`         | High-level "write one signed event" pipeline + workspace mutex |
| `keys`          | Per-tenant + legacy SPIFFE identity resolution              |
| `paths`         | Workspace path resolution + signer-binary discovery         |
| `schema`        | Zod runtime validation for AtlasEvent and PubkeyBundle      |
| `signer`        | Spawn-and-validate wrapper around the Rust `atlas-signer`   |
| `storage`       | Atomic append + read of events.jsonl                        |
| `types`         | Wire-format type definitions                                |
| `ulid`          | ULID generation for event IDs                               |
| `anchor-json`   | lossless-json wrapper for big-integer anchor receipt fields |

## Build

The bridge ships compiled output from `dist/`. Both apps consume the
compiled package, so after a fresh `pnpm install` the bridge MUST be
built before the apps can typecheck or run:

```bash
pnpm --filter @atlas/bridge build
```

The atlas-web `next.config.ts` declares `transpilePackages: ["@atlas/bridge"]`
so dev-mode `next dev` picks up source changes without a manual rebuild.
The atlas-mcp-server build (`tsc`) reads the bridge's emitted `.d.ts`
files directly.

## Hardening parity

The web write surface added several hardenings (existsSync guard on
signer-binary cache, child-process timeout + stdout cap, redactPaths on
500 responses, mutex Map-identity race fix). All of these now live in
the bridge so the MCP path inherits them automatically.
