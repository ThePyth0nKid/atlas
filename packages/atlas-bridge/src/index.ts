/**
 * @atlas/bridge — single source of truth for the TypeScript bridge
 * layer between Atlas consumers and the Rust `atlas-signer` binary
 * plus the on-disk `events.jsonl` DAG.
 *
 * Both `apps/atlas-web` and `apps/atlas-mcp-server` consume this
 * package. Adding a new export here lights it up for both consumers
 * automatically; the consolidation is the whole point.
 *
 * Sub-path imports are intentionally NOT supported. Consumers always
 * import from the package root (`@atlas/bridge`) — that keeps the
 * surface flat, shrinks the dependency graph for tree-shakers, and
 * means we never have to maintain a parallel `package.json#exports`
 * mapping.
 */

// ---------- Wire-format types ----------
export type {
  EventSignature,
  AtlasEvent,
  AtlasPayloadType,
  PubkeyBundle,
  AnchorKind,
  InclusionProof,
  AnchorEntry,
  AnchorBatch,
  AnchorChain,
  AtlasTrace,
} from "./types.js";
export {
  SCHEMA_VERSION,
  PUBKEY_BUNDLE_SCHEMA,
  DEFAULT_WORKSPACE,
} from "./types.js";

// ---------- Path resolution & data-root configuration ----------
export {
  isValidWorkspaceId,
  WorkspacePathError,
  setDefaultDataDir,
  dataDir,
  workspaceDir,
  eventsLogPath,
  anchorsPath,
  anchorChainPath,
  resolveSignerBinary,
  repoRoot,
  __signerBinaryCacheForTest,
} from "./paths.js";

// ---------- Identity + key derivation ----------
export type {
  LegacySignerRole,
  SignerRole,
  SignerIdentity,
} from "./keys.js";
export {
  PER_TENANT_KID_PREFIX,
  perTenantKidFor,
  workspaceIdFromKid,
  TEST_IDENTITIES,
  buildDevBundle,
  identityForKid,
  resolveIdentityForKid,
  resolvePerTenantIdentity,
  buildBundleForWorkspace,
} from "./keys.js";

// ---------- Rust signer bridge ----------
export type {
  SignArgs,
  AnchorRequest,
  AnchorBatchInput,
  AnchorOptions,
  DerivedIdentity,
  DerivedPubkey,
} from "./signer.js";
export {
  SignerError,
  redactPaths,
  signEvent,
  bundleHashViaSigner,
  anchorViaSigner,
  /**
   * @deprecated for routine use. Causes the per-tenant secret to
   * transit Node heap. Routine event signing must use
   * `signEvent({ deriveFromWorkspace })`; bundle assembly must use
   * `derivePubkeyViaSigner` (public key only). Reach for this
   * function only in explicit key-ceremony / rotation code where a
   * TS-side caller genuinely needs the derived secret.
   */
  deriveKeyViaSigner,
  derivePubkeyViaSigner,
  chainExportViaSigner,
  __signerLimitsForTest,
} from "./signer.js";

// ---------- JSONL storage ----------
export {
  StorageError,
  appendEvent,
  readAllEvents,
  computeTips,
  ensureWorkspaceDir,
} from "./storage.js";

// ---------- Schemas (Zod, runtime trust-boundary checks) ----------
export type { AtlasEventValidated } from "./schema.js";
export {
  EventSignatureSchema,
  AtlasEventSchema,
  PerTenantKidSchema,
  DerivedPubkeySchema,
  DerivedIdentitySchema,
  AnchorKindSchema,
  InclusionProofSchema,
  AnchorEntrySchema,
  AnchorEntryArraySchema,
  AnchorChainSchema,
  AnchorBatchSchema,
} from "./schema.js";

// ---------- High-level write pipeline ----------
export type { WriteEventArgs, WriteEventResult } from "./event.js";
export { writeSignedEvent } from "./event.js";

// ---------- Lossless JSON (anchor-receipt boundary) ----------
export {
  parseAnchorJson,
  stringifyAnchorJson,
  isLosslessNumber,
  LosslessNumber,
  INTEGER_LITERAL_REGEX,
} from "./anchor-json.js";

// ---------- ULID ----------
export { ulid } from "./ulid.js";
