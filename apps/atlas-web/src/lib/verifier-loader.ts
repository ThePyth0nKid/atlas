/**
 * Loads the atlas-verify-wasm module from /public/wasm/ on demand and runs
 * verification fully in the browser. The server is never involved in the
 * actual cryptographic check.
 *
 * The WASM module is compiled from `crates/atlas-verify-wasm` and copied
 * into `apps/atlas-web/public/wasm/` by `scripts/build-wasm.sh`.
 */

export type VerifyEvidence = {
  check: string;
  ok: boolean;
  detail: string;
};

export type VerifyOutcome = {
  valid: boolean;
  evidence: VerifyEvidence[];
  errors: string[];
  verifier_version: string;
};

type WasmModule = {
  default: (input: { module_or_path: string | URL }) => Promise<unknown>;
  verify_trace_json: (traceJson: string, bundleJson: string) => unknown;
  verifier_version: () => string;
};

let cached: Promise<WasmModule> | null = null;

async function loadWasm(): Promise<WasmModule> {
  if (cached) return cached;
  cached = (async () => {
    // dynamic import avoids SSR import — only runs in browser.
    // /* webpackIgnore: true */ + /* @vite-ignore */ tell bundlers not to
    // try resolving this at build time. The browser fetches the wasm-pack
    // ES module from /public/wasm/ at runtime instead.
    const jsUrl = `${window.location.origin}/wasm/atlas_verify_wasm.js`;
    const mod = (await import(/* webpackIgnore: true */ /* @vite-ignore */ jsUrl)) as WasmModule;
    await mod.default({ module_or_path: `${window.location.origin}/wasm/atlas_verify_wasm_bg.wasm` });
    return mod;
  })();
  return cached;
}

/**
 * Run the verifier on a trace + pubkey bundle (both as raw JSON strings).
 * Returns the outcome with evidence list, plus the verifier version string.
 */
export async function runVerifier(
  traceJson: string,
  bundleJson: string,
): Promise<{ outcome: VerifyOutcome; verifierVersion: string }> {
  const wasm = await loadWasm();
  const raw = wasm.verify_trace_json(traceJson, bundleJson);
  const outcome = normaliseOutcome(raw);
  const verifierVersion = wasm.verifier_version();
  return { outcome, verifierVersion };
}

function normaliseOutcome(raw: unknown): VerifyOutcome {
  if (!raw || typeof raw !== "object") {
    return {
      valid: false,
      evidence: [],
      errors: ["verifier returned non-object"],
      verifier_version: "unknown",
    };
  }
  const obj = raw as Record<string, unknown>;
  return {
    valid: Boolean(obj.valid),
    evidence: Array.isArray(obj.evidence) ? (obj.evidence as VerifyEvidence[]) : [],
    errors: Array.isArray(obj.errors) ? (obj.errors as string[]) : [],
    verifier_version: typeof obj.verifier_version === "string" ? obj.verifier_version : "unknown",
  };
}
