/**
 * W20c — GET /api/atlas/system/supply-chain-pins route tests.
 *
 * Asserts the response shape (11 keys) + hex format of all pins.
 */

import { describe, it, expect, vi } from "vitest";

vi.mock("@/lib/bootstrap", () => ({}));

import { GET } from "./route";
import {
  GIT_SHA1_HEX_RE,
  SHA256_HEX_RE,
} from "@/lib/supply-chain-pins";

describe("GET /api/atlas/system/supply-chain-pins", () => {
  it("returns 200 with all 11 pins and ok=true", async () => {
    const res = await GET();
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.ok).toBe(true);
    // 11 pin fields + 1 ok field = 12 keys
    expect(Object.keys(body)).toHaveLength(12);
  });

  it("HF_REVISION_SHA is a 40-char hex Git SHA-1", async () => {
    const body = await (await GET()).json();
    expect(body.hf_revision_sha).toMatch(GIT_SHA1_HEX_RE);
  });

  it("all five SHA-256 digests are 64-char hex", async () => {
    const body = await (await GET()).json();
    expect(body.onnx_sha256).toMatch(SHA256_HEX_RE);
    expect(body.tokenizer_json_sha256).toMatch(SHA256_HEX_RE);
    expect(body.config_json_sha256).toMatch(SHA256_HEX_RE);
    expect(body.special_tokens_map_sha256).toMatch(SHA256_HEX_RE);
    expect(body.tokenizer_config_json_sha256).toMatch(SHA256_HEX_RE);
  });

  it("all five URLs are HTTPS HuggingFace resolve URLs", async () => {
    const body = await (await GET()).json();
    for (const key of [
      "model_url",
      "tokenizer_json_url",
      "config_json_url",
      "special_tokens_map_url",
      "tokenizer_config_json_url",
    ] as const) {
      expect(body[key]).toMatch(/^https:\/\/huggingface\.co\//);
    }
  });

  it("URLs pin the HF_REVISION_SHA", async () => {
    const body = await (await GET()).json();
    expect(body.model_url).toContain(body.hf_revision_sha);
  });
});
