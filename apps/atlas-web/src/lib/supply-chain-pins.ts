/**
 * W20c — TypeScript mirror of the Rust supply-chain pin constants.
 *
 * Single source of truth is `crates/atlas-mem0g/src/embedder.rs`
 * (lines 166-244). Eleven compile-in constants:
 *   1.  HF_REVISION_SHA               — 40-char Git SHA-1 hex
 *   2-6 SHA-256 digests of 5 model files (64-char hex):
 *       - ONNX_SHA256
 *       - TOKENIZER_JSON_SHA256
 *       - CONFIG_JSON_SHA256
 *       - SPECIAL_TOKENS_MAP_SHA256
 *       - TOKENIZER_CONFIG_JSON_SHA256
 *   7-11 Full HuggingFace LFS URLs for the 5 model files (revision-pinned)
 *
 * These are mirrored here as plain TypeScript constants for V2-β-1.
 * The Rust crate remains the source of truth — a rotation lands in
 * `embedder.rs`, then the parity-smoke test in `supply-chain-pins.test.ts`
 * trips until this file is updated in the same commit. A codegen welle
 * (V2-γ) will replace the hand-mirror with a build-time pull from the
 * Rust constants.
 *
 * Threat model:
 *   * Constants are public information by definition (they're shipped
 *     in every Atlas release binary). Exposing them via
 *     `/api/atlas/system/supply-chain-pins` adds no new attack surface.
 *   * Drift between this file and `embedder.rs` is a silent build-info
 *     leak — the parity-smoke test is the defence.
 */

/** HuggingFace Git revision SHA — 40-char hex digest. */
export const HF_REVISION_SHA =
  "5c38ec7c405ec4b44b94cc5a9bb96e735b38267a";

/** SHA-256 of `model.onnx`. 64-char hex. */
export const ONNX_SHA256 =
  "828e1496d7fabb79cfa4dcd84fa38625c0d3d21da474a00f08db0f559940cf35";

/** SHA-256 of `tokenizer.json`. 64-char hex. */
export const TOKENIZER_JSON_SHA256 =
  "d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66";

/** SHA-256 of `config.json`. 64-char hex. */
export const CONFIG_JSON_SHA256 =
  "094f8e891b932f2000c92cfc663bac4c62069f5d8af5b5278c4306aef3084750";

/** SHA-256 of `special_tokens_map.json`. 64-char hex. */
export const SPECIAL_TOKENS_MAP_SHA256 =
  "b6d346be366a7d1d48332dbc9fdf3bf8960b5d879522b7799ddba59e76237ee3";

/** SHA-256 of `tokenizer_config.json`. 64-char hex. */
export const TOKENIZER_CONFIG_JSON_SHA256 =
  "9261e7d79b44c8195c1cada2b453e55b00aeb81e907a6664974b4d7776172ab3";

/** HuggingFace LFS URL for `model.onnx` (revision-pinned). */
export const MODEL_URL =
  "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/onnx/model.onnx";

/** HuggingFace LFS URL for `tokenizer.json` (revision-pinned). */
export const TOKENIZER_JSON_URL =
  "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/tokenizer.json";

/** HuggingFace LFS URL for `config.json` (revision-pinned). */
export const CONFIG_JSON_URL =
  "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/config.json";

/** HuggingFace LFS URL for `special_tokens_map.json` (revision-pinned). */
export const SPECIAL_TOKENS_MAP_URL =
  "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/special_tokens_map.json";

/** HuggingFace LFS URL for `tokenizer_config.json` (revision-pinned). */
export const TOKENIZER_CONFIG_JSON_URL =
  "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/tokenizer_config.json";

/**
 * Frozen view of all eleven pin constants. Used by the
 * `/api/atlas/system/supply-chain-pins` route and the settings UI.
 *
 * `as const` makes it a literal type so the route's response schema
 * can pick up the exact string shape without re-typing each field.
 */
export interface SupplyChainPins {
  readonly hf_revision_sha: string;
  readonly onnx_sha256: string;
  readonly tokenizer_json_sha256: string;
  readonly config_json_sha256: string;
  readonly special_tokens_map_sha256: string;
  readonly tokenizer_config_json_sha256: string;
  readonly model_url: string;
  readonly tokenizer_json_url: string;
  readonly config_json_url: string;
  readonly special_tokens_map_url: string;
  readonly tokenizer_config_json_url: string;
}

export const SUPPLY_CHAIN_PINS: Readonly<SupplyChainPins> = Object.freeze({
  hf_revision_sha: HF_REVISION_SHA,
  onnx_sha256: ONNX_SHA256,
  tokenizer_json_sha256: TOKENIZER_JSON_SHA256,
  config_json_sha256: CONFIG_JSON_SHA256,
  special_tokens_map_sha256: SPECIAL_TOKENS_MAP_SHA256,
  tokenizer_config_json_sha256: TOKENIZER_CONFIG_JSON_SHA256,
  model_url: MODEL_URL,
  tokenizer_json_url: TOKENIZER_JSON_URL,
  config_json_url: CONFIG_JSON_URL,
  special_tokens_map_url: SPECIAL_TOKENS_MAP_URL,
  tokenizer_config_json_url: TOKENIZER_CONFIG_JSON_URL,
});

/** Regex for a 40-char hex Git SHA-1. */
export const GIT_SHA1_HEX_RE = /^[0-9a-f]{40}$/;

/** Regex for a 64-char hex SHA-256. */
export const SHA256_HEX_RE = /^[0-9a-f]{64}$/;
