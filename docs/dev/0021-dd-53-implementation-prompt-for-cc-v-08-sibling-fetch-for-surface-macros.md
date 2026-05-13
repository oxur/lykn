# DD-53 Implementation Prompt for CC — V-08 Sibling-Fetch for Surface-macros

## Read this first

DD-53 extends V-08 (DD-48 / M9) so that JSR-resolved macro modules also have their sibling JS files (declared via `(surface-macros "path")`) fetched and co-located in the cache. Without this, DD-52's surface-macros loading fails for the canonical 0.6.0 downstream pattern (`(import-macros "jsr:@lykn/testing" ...)`).

**DD-53 is a 0.6.0 ship-gate.** It's the load-bearing follow-up to DD-52's local-path-only closure.

All eight design questions resolved per Duncan's calls (2026-05-12). See `workbench/dd-53-v08-sibling-fetch-2026-05-12.md` for the full design with the call rationale. Summary:

- **Q1: A** — Per-package directory cache layout (`~/.cache/lykn/macros/{key}/mod.lykn` + siblings).
- **Q2: A** — Only files declared via `(surface-macros "...")` get fetched; runtime-imports are Deno's responsibility.
- **Q3: A** — Use `packages/lang/reader.js` (already in the subprocess) to parse the fetched .lykn for surface-macros declarations.
- **Q4: A** — Fail-fast on sibling-fetch failures; whole `resolve-macro-source` call fails.
- **Q5: A** — Reject `..` and absolute paths in sibling filenames.
- **Q6: A** — npm path uses Deno's npm cache `localDir` as `module_dir`; no re-caching into lykn cache.
- **Q7: A** — Clean cache cutover; no migration code for old flat-file caches.
- **Q8: D** — Mock JSR server in tests (`std/http/server`) + cache-contents inspection.

---

## MUST framing

You MUST:

1. **Verify DD-52's cache-layout claims before changing the layout.** Read `pass0.rs` around lines 130–160 (`resolve_specifier`, `cache_path = cache_dir.join(format!("{cache_key}.lykn"))`) and confirm the current flat-file behavior. This is the spec for what's changing.

2. **Diagnose before implementing.** Write a brief diagnosis section at the top of the closing report covering: (a) every call site of `cache_path` / `cache_key` that needs updating; (b) every caller of `resolve_macro_source` (the Rust-side method, not the subprocess action) that consumes its return value; (c) confirmation that the npm-path's `localDir` is genuinely the right `module_dir` for surface-macros (read the surrounding env.rs code).

3. **Stop-and-surface conditions (the methodology gate from DD-52).** STOP and surface to Duncan/CDC if diagnosis reveals any of:
   - The protocol return-shape change (`Result<String>` → `Result<{source, moduleDir}>`) breaks more than 2–3 call sites in unexpected ways.
   - `packages/lang/reader.js` can't be imported into the subprocess's evaluator context (would force Q3 fallback to regex).
   - The npm-path `localDir` isn't directly usable as `module_dir` for `load_surface_macros` (e.g., needs further normalization).
   - The mock JSR server approach (`std/http/server`) has subtle interactions with Deno's import resolver during tests.
   - The cache-layout change interacts with anything other than `resolve_macro_source` and pass0 (e.g., if there's a stable-key-derivation function used elsewhere).

   These are the surprises that change scope. DD-52 ran past a similar gate ("STOP and surface" on the V-08 sibling-fetch limitation) and we ended up shipping a partial fix. **Don't repeat that pattern.** If the diagnosis reveals architectural friction, surface BEFORE writing implementation code.

4. **Preserve DD-52's local-path semantics.** Surface-macros loading for local-path imports must continue to work unchanged. The 8 DD-52 tests gate this.

5. **Empirically verify the JSR end-to-end flow.** The closing report MUST include a test that exercises: `(import-macros "jsr:@fake/testing" ...)` → mocked JSR server returns mod.lykn declaring `(surface-macros "macros.js")` → sibling fetched and cached → surface-macros expand correctly through the Rust expander.

6. **Run the full test suite post-implementation:**
   - `cargo test -p lykn-lang` — must equal the post-DD-52 baseline (989) plus whatever new Rust unit tests DD-53 adds.
   - `./target/release/lykn build && ./target/release/lykn test` — must equal the post-DD-52 baseline (1187) plus DD-53's new JS tests.
   - `./workbench/verify-finding-e-2026-05-12.sh --lykn-bin ./target/release/lykn` — ✓ ALL CHECKS PASSED.

7. **Cover the DD-52 bad-form fixture in a test.** DD-52 left it uncovered (CDC's note in the DD-52 closing review). Add one quick test that compiles a source with `(surface-macros 42)` or `(surface-macros)` and expects validation error. One-line addition; closes the gap.

You MUST NOT:

1. **Implement migration code for old flat-file caches** (Q7=A). Existing caches simply become stale; first 0.6.0 fetch re-fetches into the new layout.

2. **Re-cache npm sources into the lykn cache** (Q6=A). Use Deno's npm cache `localDir` directly.

3. **Support `..` or absolute paths** in `(surface-macros "...")` declarations (Q5=A). Reject with a clear error message.

4. **Use regex-based parsing** for finding surface-macros declarations in the fetched .lykn source (Q3=A says use reader.js). If reader.js turns out to be unimportable in the subprocess context, surface — don't silently fall back.

5. **Auto-pass safety-bypass flags** to any tool (per CLAUDE.md "Lykn CLI safety gates").

6. **Break DD-52's existing tests.** The 8 DD-52 tests must continue to pass.

---

## Architecture (the spec)

### Protocol extension

`resolve-macro-source` response shape changes:

**Before (DD-48):**
```json
{ "ok": true, "result": "<.lykn source text>" }
```

**After (DD-53):**
```json
{
  "ok": true,
  "result": {
    "source": "<.lykn source text>",
    "moduleDir": "/Users/foo/.cache/lykn/macros/{key}"   // for JSR
                  // OR "/Users/foo/Library/Caches/deno/npm/.../"  for npm
                  // OR "/Users/foo/lab/.../packages/testing" for local
  }
}
```

The Rust-side `resolve_macro_source` method signature changes from `Result<String, _>` to `Result<ResolvedMacroModule, _>` where:

```rust
pub struct ResolvedMacroModule {
    pub source: String,
    pub module_dir: PathBuf,
}
```

### Cache layout migration

Current (V-08 / M9):
```
~/.cache/lykn/macros/{cache_key}.lykn   ← flat file
```

After DD-53:
```
~/.cache/lykn/macros/{cache_key}/
  ├── mod.lykn      ← always present (the entry .lykn source)
  ├── macros.js     ← present iff mod.lykn declares (surface-macros "macros.js")
  └── (any other sibling JS files declared by surface-macros)
```

`cache_key` derivation stays the same (whatever hash function is currently used on the specifier).

### Subprocess `resolve-macro-source` extension

In `env.rs`, after fetching `mod.lykn` from the JSR base URL:

1. Parse `source` using the imported reader (`read(source)` from `lang/reader.js`).
2. Walk top-level forms. For each `(surface-macros "path")` form:
   - Extract `path` string.
   - Validate per Q5: must be relative, no `..`, no leading `/`.
   - Fetch `{baseUrl}{path}` via HTTP.
   - If fetch fails (any non-2xx, network error, etc.), return `{ok: false, error: "surface-macros sibling fetch failed: <path>: <reason>"}`.
3. Write `mod.lykn` to `{cache_dir}/{key}/mod.lykn`.
4. Write each fetched sibling to `{cache_dir}/{key}/{path}`.
5. Return:
   ```json
   {"ok": true, "result": {"source": "<text>", "moduleDir": "{cache_dir}/{key}"}}
   ```

For npm path: return `{"source": "<text>", "moduleDir": "{npm_localDir}"}` — the `localDir` already computed from `deno info --json`.

For local-path imports (not handled by `resolve-macro-source` at all, but for clarity): the existing pass0 derivation `resolved.parent()` still applies; no changes needed there.

### `pass0.rs` updates

The `resolve_specifier` function (or wherever the cache path is constructed) needs to:

1. Build cache file path as `{cache_dir}/{cache_key}/mod.lykn` instead of `{cache_dir}/{cache_key}.lykn`.
2. Receive `module_dir` from the new `resolve_macro_source` return type and pass it to `load_surface_macros` downstream.

Other callers of `resolve_macro_source` (likely just pass0; verify in diagnosis): update their consumption of the return value.

### Path validation (Q5)

Sibling path validation function:

```rust
fn validate_sibling_path(path: &str) -> Result<(), String> {
    if path.starts_with('/') {
        return Err(format!("surface-macros: absolute paths not allowed: {}", path));
    }
    if path.split('/').any(|seg| seg == ".." || seg.is_empty()) {
        return Err(format!("surface-macros: '..' and empty segments not allowed: {}", path));
    }
    Ok(())
}
```

(Equivalent JS validation in the subprocess; same semantics.)

### Error diagnostics (Q6 from DD-52, carried forward)

Sibling-fetch errors should include source location of the `(surface-macros ...)` form. Format:

```
surface-macros: sibling fetch failed: macros.js (404 Not Found from https://jsr.io/...)
  at /Users/foo/.cache/lykn/macros/{key}/mod.lykn:3:1
```

---

## Required reading (before writing any code)

In this order:

1. `workbench/dd-53-v08-sibling-fetch-2026-05-12.md` — full DD with resolutions.
2. `workbench/2026-05-12-DD-52-closing-report.md` and `workbench/dd-52-closing-cdc-review-2026-05-12.md` — DD-52 context, especially fast-follow #1 (the gap DD-53 closes) and CDC's bad-form fixture gap note.
3. `assets/ai/LEDGER_DISCIPLINE.md` + `assets/ai/SUBAGENT-DELEGATION-POLICY.md` + `docs/philosophy.md` Principle 3 — the usual.
4. **Code paths to study before implementing:**
   - `crates/lykn-lang/src/expander/env.rs` lines 164–232 — the existing `resolve-macro-source` action (JSR + npm dispatch).
   - `crates/lykn-lang/src/expander/deno.rs` lines 178–195 (approximately) — the Rust-side `resolve_macro_source` method.
   - `crates/lykn-lang/src/expander/pass0.rs` lines 130–160 — `resolve_specifier` (cache path construction).
   - `crates/lykn-lang/src/expander/pass0.rs` lines 376–400 — the DD-52 surface-macros detection + `load_surface_macros` call site (the consumer of `module_dir`).
   - `packages/lang/reader.js` — the reader you'll import in the subprocess for Q3=A.

---

## Deliverable 1 — Diagnosis (brief; in the closing report, not a separate file)

3–5 paragraphs at the top of the closing report addressing:

- **Call-site audit:** every place that constructs `{cache_key}.lykn` or otherwise depends on the flat-file layout. List file:line for each. Confirm the migration touches all of them.
- **Caller-of-`resolve_macro_source` audit:** every Rust-side caller of `resolve_macro_source` that consumes the return value (currently a `String`). The signature change to `ResolvedMacroModule` propagates through each.
- **npm path readiness:** verify that `deno info --json`'s `mod.local` value, with its trailing-filename stripped, is genuinely usable as a directory containing the sibling JS files. If npm packages are flat-cached or otherwise structured differently from what surface-macros expects, surface.
- **Reader importability:** verify that `packages/lang/reader.js` can be imported into the subprocess's evaluator context. The subprocess already imports `lang/expander.js` etc.; the reader should be a sibling. If it requires path adjustments, document.
- **Mock JSR server feasibility:** verify `std/http/server` is usable in test context for a small mock server. Sketch the test infrastructure shape (~50 lines).

**Stop-and-surface if any audit reveals scope-relevant surprises** (see MUST item #3 above).

---

## Deliverable 2 — Implementation

### 2a. Subprocess action (env.rs)

Extend `resolve-macro-source` per the architecture above. Use the imported reader for Q3. Apply Q5 validation. Apply Q4 fail-fast on sibling fetch failures. Return the new `{source, moduleDir}` shape.

For the npm branch: in addition to returning the source text, return the `localDir` as `moduleDir`.

### 2b. Rust wrapper (deno.rs)

Update `resolve_macro_source` signature:

```rust
pub fn resolve_macro_source(&mut self, specifier: &str) -> Result<ResolvedMacroModule, LyknError>
```

Define `ResolvedMacroModule` in the appropriate location (likely `deno.rs` or a shared module).

### 2c. Pass0 updates (pass0.rs)

- Change cache file path construction: `cache_dir.join(format!("{cache_key}.lykn"))` → `cache_dir.join(&cache_key).join("mod.lykn")` (or whatever fits the codebase's idiom).
- Ensure the directory `cache_dir/{cache_key}/` exists before writing.
- Consume the new `ResolvedMacroModule` shape: thread `module_dir` through to the existing `load_surface_macros` call.
- The local-path branch (no JSR/npm) continues to use `resolved.parent()` for `module_dir`.

### 2d. Cache cutover handling (Q7=A)

When `resolve_specifier` looks for a cached file, it now looks for `{cache_dir}/{cache_key}/mod.lykn` instead of `{cache_dir}/{cache_key}.lykn`. Old flat files are simply ignored — they'll never be hit by the new lookup path. No migration code; no warning.

(Optional cleanup: if pass0 detects old flat-file caches at startup, it could `rm` them. NOT required for DD-53; logged as fast-follow if desired.)

---

## Deliverable 3 — Tests (Q8 = D)

### 3a. Mock JSR server infrastructure

A small test helper (`test/helpers/mock-jsr-server.js` or wherever fits the codebase's structure) that uses `std/http/server` to serve:

- A `mod.lykn` source on request.
- Sibling JS files on request.
- Configurable 404 / malformed responses for failure-mode testing.

`~50 lines` is the rough envelope; less if the codebase has helper patterns to reuse.

### 3b. JSR end-to-end happy path

The load-bearing real-world test. Pattern:

```javascript
Deno.test("DD-53: JSR-fetched surface-macros work end-to-end", async () => {
  // Start mock JSR server serving:
  //   /pkg/mod.lykn: '(surface-macros "macros.js")\n(macro :exported ...)\n'
  //   /pkg/macros.js: 'macroEnv.set("greet", (s) => ...);'
  // Configure subprocess to use mock URL base.
  // Compile a source using (import-macros "jsr:@fake/pkg" (greet)).
  // Verify the compiled output contains the expected macro expansion.
});
```

### 3c. Cache-contents inspection

```javascript
Deno.test("DD-53: JSR-fetched modules cache mod.lykn + siblings in per-package directory", async () => {
  // After the happy-path test above, verify:
  //   ~/.cache/lykn/macros/{key}/mod.lykn exists
  //   ~/.cache/lykn/macros/{key}/macros.js exists
  //   The directory contains nothing else (no stray flat-file)
});
```

### 3d. Failure-mode tests

- **404 on sibling:** mock server returns 404 for `macros.js`. Expected: clean fail-fast error referencing the missing sibling.
- **Path-traversal in sibling decl:** mod.lykn declares `(surface-macros "../../etc/passwd")`. Expected: validation error before fetch.
- **Absolute path:** mod.lykn declares `(surface-macros "/etc/passwd")`. Expected: validation error.
- **Sibling-fetch network error:** mock server unavailable. Expected: clean fail-fast error.

### 3e. npm path test

Add a fixture in `test/regression/surface-macros/` (or alongside) that exercises an npm-resolved macro module. Use Deno's npm cache (set up via a small test scaffold) to verify that `load_surface_macros` receives the right `module_dir` and loads siblings correctly.

If setting up a real npm fixture is heavy, an alternative: synthetic test that bypasses the subprocess but verifies the `module_dir` passed to `load_surface_macros` is the npm `localDir` for npm-resolved specifiers.

### 3f. Regression tests

- All 8 DD-52 tests still pass (local-path semantics unchanged).
- The bad-form fixture (currently uncovered per CDC's DD-52 review): add one test that compiles a source with `(surface-macros 42)` and expects validation error.
- DD-50.7 tests still pass.

---

## Deliverable 4 — Closing report

File: `workbench/2026-05-XX-DD-53-closing-report.md` (date-stamped at close time).

Required sections:

- **Summary** — what shipped
- **Diagnosis** — Deliverable 1's content (3–5 paragraphs, integrated)
- **Per-deliverable walk** — Deliverables 2 + 3 with file refs and test results
- **Empirical validation gate** — output from:
  - `cargo test -p lykn-lang` — full Rust suite
  - `./target/release/lykn build && ./target/release/lykn test` — full JS suite
  - `./workbench/verify-finding-e-2026-05-12.sh --lykn-bin ./target/release/lykn` — DD-50.7 regression check
  - The new JSR end-to-end test output specifically
- **Cache layout verification** — show the cache directory layout after a successful fetch (e.g., `tree ~/.cache/lykn/macros/` snippet)
- **Substrate-rule compliance** — Principle 3 (compiler-owned output quality), CLAUDE.md safety gates, LEDGER_DISCIPLINE
- **Cross-compiler convergence** — the JS expander already worked for JSR via Deno's native resolution; DD-53 brings Rust to parity. Confirm by running the JSR end-to-end pattern through both compilers.
- **What this milestone did NOT cover** — `(runtime-import ...)` sibling fetching, browser-side fetching, deno.json-declared sibling aliases, the `__surface_macro__` sentinel cleanup
- **Findings for fast-follow** — anything discovered during implementation

---

## Anticipated risks

- **Reader importability in the subprocess** (Q3=A). The reader at `packages/lang/reader.js` may not be directly importable into the subprocess's evaluator context. If it isn't, surface in diagnosis. Don't silently fall back to regex.

- **npm cache `localDir` semantics.** Deno's npm cache structure may not exactly mirror what surface-macros expects (e.g., maybe `localDir` points to a tarball-extracted-to dir with different layout). Verify in diagnosis.

- **Cache invalidation surprises.** When the layout changes from flat-file to directory, lykn 0.5.2 users' caches become inert. If anyone has a 0.5.2 cache they care about, they'll see one extra round of network fetches on 0.6.0 — acceptable per Q7=A but worth noting in 0.6.0 release notes.

- **`std/http/server` interactions with Deno's import resolver during testing.** If the mock server's URLs don't get treated as legitimate JSR by Deno's resolver (e.g., requires HTTPS, signed certs, etc.), the test infrastructure has more friction than the simple `~50 lines` estimate. Surface in diagnosis if discovered.

- **The `resolve-macro-source` response-shape change affects other callers.** Audit in diagnosis; if there are unexpected consumers, the protocol change has more reach.

---

## Out of scope

- **Migration code for 0.5.2-era flat-file caches.** Q7=A — clean cutover.
- **`(runtime-import ...)` sibling fetching.** Q2=A — surface-macros only.
- **JS expander parallel work.** JS expander uses Deno's native resolution; no parity work needed.
- **Removing the `__surface_macro__` sentinel mechanism.** DD-52 fast-follow #2; orthogonal.
- **Migrating npm packages to lykn-cache.** Q6=A — use Deno's npm cache directly.
- **Browser-side surface-macros.** Browser bundle is a separate consumer.

---

## Discussion points for Duncan / CDC

If diagnosis (Deliverable 1) reveals architectural surprises hitting any of the "stop and surface" conditions above, raise BEFORE writing implementation code. The DD-52 lesson is that "stop and surface" gates exist so we can re-scope before a milestone closes around a partial fix; honoring the gate is more important than charging through.

Iteration estimate: 2 iterations. Iter 1: protocol + cache + JSR fetch + npm wiring. Iter 2: tests + closing report.

If you accept this scope, proceed. The DD's questions are resolved; the protocol shape is specified; the test discipline is laid out.
