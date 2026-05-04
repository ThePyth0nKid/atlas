// Atlas In-Browser Verifier — application code.
//
// Extracted from inline <script type="module"> in index.html as part of
// V1.16 Welle A (browser-runtime hardening). The extraction lets the page
// ship a strict CSP without 'unsafe-inline' on script-src, and lets us
// pin app.js with Subresource Integrity (sha384) on the loading <script>
// tag in index.html.
//
// Sink discipline: this file uses ONLY safe DOM sinks — textContent,
// className, and style.display. No innerHTML, no document.write, no
// eval, no new Function, no setTimeout(string), no setting *.src or
// *.href from user input. The CSP shipped alongside this file enforces
// this with `require-trusted-types-for 'script'; trusted-types 'none'`,
// which makes any future regression that introduces a TrustedHTML or
// TrustedScriptURL sink fail at the browser boundary rather than at
// review time.

const banner = document.getElementById('error-banner');
const traceInput = document.getElementById('trace-input');
const bundleInput = document.getElementById('bundle-input');
const verifyBtn = document.getElementById('verify-btn');
const verdictEl = document.getElementById('verdict');
const outputPanel = document.getElementById('output-panel');
const outputEl = document.getElementById('outcome-output');
const versionEl = document.getElementById('verifier-version');

function showError(msg) {
  banner.textContent = msg;
  banner.style.display = 'block';
}

function clearError() {
  banner.textContent = '';
  banner.style.display = 'none';
}

let wasmModule = null;
try {
  // The WASM module is loaded from ./pkg/, populated by `wasm-pack
  // build --out-dir ../../apps/wasm-playground/pkg`. Loading via
  // dynamic import lets us surface a friendly error if pkg/ is missing
  // (the most common dev-time foot-gun for first-time setup).
  //
  // The CSP allows this via `script-src 'self'` + `connect-src 'self'`.
  // The WASM instantiation requires `script-src 'wasm-unsafe-eval'`,
  // which is the dedicated CSP keyword for WebAssembly compilation
  // (does NOT enable JavaScript eval).
  const mod = await import('./pkg/atlas_verify_wasm.js');
  // wasm-pack emits a default export that initialises the wasm
  // binary. Calling it (with no args) loads `pkg/atlas_verify_wasm_bg.wasm`
  // sibling-relative.
  await mod.default();
  wasmModule = mod;
  // Surface the loaded verifier version in the UI footer rather than
  // console.log — auditors should see the version without needing
  // devtools open, and project style forbids console.log in shipped
  // code (see typescript/coding-style.md).
  versionEl.textContent = mod.verifier_version();
} catch (err) {
  showError(
    'Failed to load WASM module from ./pkg/. Run `wasm-pack build ' +
    'crates/atlas-verify-wasm --target web --release --out-dir ' +
    '../../apps/wasm-playground/pkg` from the repo root, then reload. ' +
    '(' + (err && err.message ? err.message : String(err)) + ')'
  );
  verifyBtn.disabled = true;
  throw err;
}

// Enable verify button only once both files are picked.
function refreshEnableState() {
  verifyBtn.disabled = !(traceInput.files.length && bundleInput.files.length);
}
traceInput.addEventListener('change', refreshEnableState);
bundleInput.addEventListener('change', refreshEnableState);

async function readFileAsText(file) {
  // FileReader returns a binary string by default; we want text
  // decoded as UTF-8 (Atlas JSON is canonical UTF-8). Using
  // file.text() is the modern shorthand and avoids a manual
  // FileReader dance.
  return await file.text();
}

verifyBtn.addEventListener('click', async () => {
  clearError();
  verdictEl.textContent = '';
  verdictEl.className = 'verdict';
  outputPanel.style.display = 'none';
  outputEl.textContent = '';

  if (!wasmModule) {
    showError('WASM module not loaded.');
    return;
  }

  try {
    const traceText = await readFileAsText(traceInput.files[0]);
    const bundleText = await readFileAsText(bundleInput.files[0]);
    const outcome = wasmModule.verify_trace_json(traceText, bundleText);
    const valid = outcome && outcome.valid === true;
    verdictEl.textContent = valid ? 'VALID — all checks passed' : 'INVALID — see outcome below';
    verdictEl.className = 'verdict ' + (valid ? 'valid' : 'invalid');
    outputEl.textContent = JSON.stringify(outcome, null, 2);
    outputPanel.style.display = 'block';
  } catch (err) {
    // The WASM bindings throw a JsValue with a string we forwarded
    // from Rust (`bundle parse: ...` or `verify: ...`); surface it
    // verbatim so the auditor sees the exact failure mode.
    showError(
      'Verification raised an error: ' +
      (err && err.message ? err.message : String(err))
    );
  }
});
