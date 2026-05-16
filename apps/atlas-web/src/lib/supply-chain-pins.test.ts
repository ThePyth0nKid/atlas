/**
 * W20c — Parity smoke for supply-chain pin constants.
 *
 * Asserts the well-formedness of the 11 mirrored Rust constants and
 * defends against accidental blanking during refactors. Rotations
 * land in `crates/atlas-mem0g/src/embedder.rs`; this file must be
 * updated in the same commit (otherwise the hex-format checks below
 * trip on a stale value).
 */

import { describe, it, expect } from "vitest";
import {
  GIT_SHA1_HEX_RE,
  HF_REVISION_SHA,
  ONNX_SHA256,
  TOKENIZER_JSON_SHA256,
  CONFIG_JSON_SHA256,
  SPECIAL_TOKENS_MAP_SHA256,
  TOKENIZER_CONFIG_JSON_SHA256,
  MODEL_URL,
  TOKENIZER_JSON_URL,
  CONFIG_JSON_URL,
  SPECIAL_TOKENS_MAP_URL,
  TOKENIZER_CONFIG_JSON_URL,
  SHA256_HEX_RE,
  SUPPLY_CHAIN_PINS,
} from "./supply-chain-pins";

describe("supply-chain-pins constants", () => {
  it("HF_REVISION_SHA is a 40-char lowercase hex Git SHA-1", () => {
    expect(HF_REVISION_SHA).toMatch(GIT_SHA1_HEX_RE);
  });

  it("all five SHA-256 digests are 64-char lowercase hex", () => {
    const sha256s: ReadonlyArray<readonly [string, string]> = [
      ["ONNX_SHA256", ONNX_SHA256],
      ["TOKENIZER_JSON_SHA256", TOKENIZER_JSON_SHA256],
      ["CONFIG_JSON_SHA256", CONFIG_JSON_SHA256],
      ["SPECIAL_TOKENS_MAP_SHA256", SPECIAL_TOKENS_MAP_SHA256],
      ["TOKENIZER_CONFIG_JSON_SHA256", TOKENIZER_CONFIG_JSON_SHA256],
    ];
    for (const [name, value] of sha256s) {
      expect(value, `${name} must be 64-char hex`).toMatch(SHA256_HEX_RE);
    }
  });

  it("all five URLs are HTTPS HuggingFace resolve URLs containing the revision SHA", () => {
    const urls: ReadonlyArray<readonly [string, string]> = [
      ["MODEL_URL", MODEL_URL],
      ["TOKENIZER_JSON_URL", TOKENIZER_JSON_URL],
      ["CONFIG_JSON_URL", CONFIG_JSON_URL],
      ["SPECIAL_TOKENS_MAP_URL", SPECIAL_TOKENS_MAP_URL],
      ["TOKENIZER_CONFIG_JSON_URL", TOKENIZER_CONFIG_JSON_URL],
    ];
    for (const [name, url] of urls) {
      expect(url, `${name} must be HTTPS`).toMatch(/^https:\/\//);
      expect(url, `${name} must be on huggingface.co`).toContain(
        "huggingface.co",
      );
      expect(url, `${name} must pin the HF_REVISION_SHA`).toContain(
        HF_REVISION_SHA,
      );
    }
  });

  it("SUPPLY_CHAIN_PINS contains exactly 11 keys", () => {
    expect(Object.keys(SUPPLY_CHAIN_PINS)).toHaveLength(11);
  });

  it("SUPPLY_CHAIN_PINS is frozen at runtime", () => {
    expect(Object.isFrozen(SUPPLY_CHAIN_PINS)).toBe(true);
  });

  it("SUPPLY_CHAIN_PINS values match the individual exports", () => {
    expect(SUPPLY_CHAIN_PINS.hf_revision_sha).toBe(HF_REVISION_SHA);
    expect(SUPPLY_CHAIN_PINS.onnx_sha256).toBe(ONNX_SHA256);
    expect(SUPPLY_CHAIN_PINS.tokenizer_json_sha256).toBe(TOKENIZER_JSON_SHA256);
    expect(SUPPLY_CHAIN_PINS.config_json_sha256).toBe(CONFIG_JSON_SHA256);
    expect(SUPPLY_CHAIN_PINS.special_tokens_map_sha256).toBe(
      SPECIAL_TOKENS_MAP_SHA256,
    );
    expect(SUPPLY_CHAIN_PINS.tokenizer_config_json_sha256).toBe(
      TOKENIZER_CONFIG_JSON_SHA256,
    );
    expect(SUPPLY_CHAIN_PINS.model_url).toBe(MODEL_URL);
    expect(SUPPLY_CHAIN_PINS.tokenizer_json_url).toBe(TOKENIZER_JSON_URL);
    expect(SUPPLY_CHAIN_PINS.config_json_url).toBe(CONFIG_JSON_URL);
    expect(SUPPLY_CHAIN_PINS.special_tokens_map_url).toBe(
      SPECIAL_TOKENS_MAP_URL,
    );
    expect(SUPPLY_CHAIN_PINS.tokenizer_config_json_url).toBe(
      TOKENIZER_CONFIG_JSON_URL,
    );
  });
});
