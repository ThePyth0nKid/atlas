//! WASM bindings for `atlas-trust-core`.
//!
//! Exports a single function `verify_trace_json(trace_str, bundle_str)`
//! that the browser calls. Returns the `VerifyOutcome` as a JS object.
//!
//! Build with: `wasm-pack build --target web --release`

#![allow(clippy::unused_unit)]

use atlas_trust_core::{pubkey_bundle::PubkeyBundle, verify::verify_trace_json as core_verify};
use wasm_bindgen::prelude::*;

/// Verify a trace JSON string against a pubkey-bundle JSON string.
/// Returns the `VerifyOutcome` serialised as a JS value.
#[wasm_bindgen]
pub fn verify_trace_json(trace_json: &str, bundle_json: &str) -> Result<JsValue, JsValue> {
    let bundle = PubkeyBundle::from_json(bundle_json.as_bytes())
        .map_err(|e| JsValue::from_str(&format!("bundle parse: {e}")))?;
    let outcome = core_verify(trace_json.as_bytes(), &bundle)
        .map_err(|e| JsValue::from_str(&format!("verify: {e}")))?;
    serde_wasm_bindgen::to_value(&outcome).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Return verifier-version string.
#[wasm_bindgen]
pub fn verifier_version() -> String {
    atlas_trust_core::VERIFIER_VERSION.to_string()
}
