---
number: 48
title: "Import-Macros JSR/npm Cache Resolution (V-08 fix)"
author: "the current"
component: All
tags: [change-me]
created: 2026-04-30
updated: 2026-04-30
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# Import-Macros JSR/npm Cache Resolution (V-08 fix)

> **Status:** open
>
> **Iteration budget:** 5 (expect 2–3)
>
> **Implementer (CC):** Claude Code, on Duncan's machine
>
> **Reviewer (CDC):** Cowork Claude (this session)
>
> **Methodology:** [LEDGER_DISCIPLINE.md](../../assets/ai/LEDGER_DISCIPLINE.md) — load before starting
>
> **Phase context:** [phase-2-plan.md](../phase-2-plan.md), [philosophy.md](../../docs/philosophy.md)
>
> **Predecessor:** M5 (toolchain bootstrap) closed `4cc1b12`
>
> **Related design records:** workbench/old/dd-34-cross-package-import-macros-resolution.md (the original three-tier scheme that this milestone amends)

---

## Background

While this doc was created as part of a larger chain of implementation tasks in the 0.5.x development cycle, it's significance is someone greater than most of its other document peers in that work. This document addresses a core issue in the first major release of the Lykn language (0.5.0), namely one regarding the fact that the language shipped that version with a broken macro import mechanism. This milestone aims to fix that. This is a user-facing, language level feature that really is, in essence, a design doc (even though it was originally created as one in a series of development milestones, expected to be released in 0.5.2).

## Why this milestone (M9) exists

V-08 from M0 (and confirmed by M5's smoke test in two distinct contexts) named a load-bearing bug in the macro-expander's specifier resolution: `jsr:` and `npm:` specifiers (and bare names that resolve through the import map to those) cannot be resolved by the current `resolveImportMacrosSpecifier` logic. The expander needs to read the macro module's *source text* to parse and evaluate macros at compile time, but Deno's `import.meta.resolve()` for registry specifiers returns a registry URL — not a filesystem path — and the expander throws.

Operational impact:

- **Scaffolded Lykn projects cannot use JSR-published macro modules** via `import-macros "jsr:@<scope>/<name>"`. M5's workaround (replacing the scaffold's `import-macros "jsr:@lykn/testing"` with direct `import "jsr:@std/assert"` + `Deno:test`) avoids the path entirely but at the cost of the testing DSL as the default new-user experience.
- **The JS expander** at `packages/lang/expander.js:1036` throws on the non-file URL.
- **The Rust expander** has the equivalent failure mode (M5 closing report's "What needs to be reflected back" #2: "JSR specifier resolution in the Rust expander is fundamentally broken").
- **Both pipelines need fixing** for downstream Lykn projects to use macros published to JSR/npm.

This milestone produces:

1. A design decision document (`docs/decisions/dd-38-import-macros-cache-resolution.md`) analyzing the options surfaced in M0's bug report (CC's options A–D), choosing one with rationale, and outlining the implementation plan.
2. Implementation of the chosen approach in **both** the JS expander (`packages/lang/expander.js`) and the Rust expander (`crates/lykn-lang/`).
3. Restoration of the scaffold's test template to use `import-macros "jsr:@lykn/testing"` (per CDC observation 2 of M5 — the workaround should not outlive the fix).
4. Integration smoke test confirming downstream Lykn projects can scaffold, test (using the testing DSL via JSR macros), build, and publish end-to-end.

Methodologically: this is the first Phase 2 milestone with a substantial DD. The DD lands in `docs/decisions/` (new tracked directory) — see CC instructions on that choice.

---

## Source materials (read in this order)

1. `assets/ai/LEDGER_DISCIPLINE.md` — protocol (mandatory)
2. `assets/ai/SUBAGENT-DELEGATION-POLICY.md` — applies; lookup subagents fine for tracing expander code paths; **DD writing in CC's main context** (judgment-heavy work).
3. `docs/philosophy.md` — ground truth. Especially Principle 2 (lykn-only tooling) and the §0.6.0 commitments. The DD's chosen approach must be philosophy-aligned.
4. `workbench/old/dd-34-cross-package-import-macros-resolution.md` — the original three-tier scheme that this milestone amends. The new DD-38 references DD-34 and updates the Tier 1 / Tier 2 logic for cache-path resolution.
5. `workbench/2026-04-25-verification-pass.md` V-08 section (reconstructed M0 closing report) — the original bug surfacing.
6. `workbench/bug-import-macros-jsr-resolution.md` — CC's M0 detailed write-up with options A–D analysis.
7. `workbench/M5-cdc-review-2026-04-29.md` — Observation 2 (scaffold workaround disclosure; explicitly names this milestone as the path to revert).
8. `packages/lang/expander.js` lines 1019–1100 — the JS expander's current resolution logic.
9. `crates/lykn-lang/` — the Rust expander source; CC traces the equivalent code path during M9-2.

---

## Options analyzed

Four options were surfaced in the M0 bug report
([`workbench/bug-import-macros-jsr-resolution.md`](../../workbench/bug-import-macros-jsr-resolution.md)).
Each is analyzed below against the constraints: (1) must work for both
`jsr:` and `npm:` specifiers, (2) must not change the user-facing
`import-macros` surface, (3) must work in both JS and Rust expander
pipelines, (4) must align with philosophy Principles 2 and 3.

### Option A: `deno info --json` redirect + HTTP fetch of source text

Shell out to `deno info --json <specifier>` to obtain the registry
redirect URL (e.g., `jsr:@lykn/testing` →
`https://jsr.io/@lykn/testing/0.5.1/mod.js`). Derive the package base
URL by stripping the filename. Fetch the package's `deno.json` from the
base URL to find `lykn.macroEntry` (or fall back to `mod.lykn` per
DD-34's resolution chain). Fetch the `.lykn` source text from the
constructed URL. Cache the fetched source locally (temp dir or a
dedicated `.lykn-cache/` directory) to avoid re-fetching on subsequent
compilations.

**Pros:**
- Uses Deno's official `deno info` API for specifier resolution — the
  redirect mapping is stable across versions.
- The `fetch()` call for the `.lykn` source leverages Deno's built-in
  HTTP caching.
- No dependency on Deno's internal cache directory structure (which is
  an implementation detail that changes between versions).
- Works for both JSR and npm specifiers (both produce HTTPS redirect
  URLs).
- The existing `findMacroEntry` logic (DD-34 §2: `lykn.macroEntry` →
  `mod.lykn` → fallback chain) is preserved — it just operates on
  fetched content rather than local files.

**Cons:**
- Requires a subprocess call to `deno info` for each new registry
  specifier (first encounter only — results are cacheable).
- Requires network access on first compile (fetching `.lykn` source
  from the registry). Subsequent compiles can read from local cache.
- The `deno info --json` output format is not formally stable (though
  the `redirects` and `modules` fields have been present since Deno 1.x
  and are unlikely to change without deprecation).

### Option B: Dynamic `import()` with module-shape change

Instead of reading macro source as text and parsing it, dynamically
`import()` the resolved module at compile time. Macro modules would
export macro definitions as evaluated JavaScript functions (not as
s-expression source to be parsed).

**Pros:**
- Uses Deno's native module resolution — no custom cache-path logic.

**Cons:**
- **Fundamentally changes how macros are authored.** Currently, macro
  modules contain `(macro name ...)` forms in `.lykn` source that the
  expander parses and compiles. Option B would require macros to be
  pre-compiled JavaScript functions, changing the authoring surface.
- Breaks the current pattern where `import-macros` reads `.lykn` source
  text and `surface-macros` references pre-compiled `.js`.
- Would require every existing macro module (including `@lykn/testing`)
  to be rewritten.
- **Rejected:** too invasive for a bug fix; changes the language's
  macro authoring surface.

### Option C: Embedded source via build-step

Have `lykn build --dist` embed the macro source text (the `.lykn` file
content) as a string export in the compiled `.js` output. The expander
would `import()` the compiled module and read the embedded source
string.

**Pros:**
- Self-contained: the source text travels with the compiled module.
- No network fetches or cache-path lookups at compile time.

**Cons:**
- Changes the build pipeline to embed source in compiled output.
- Increases the published package size (source text duplicated as a
  string literal inside the JS).
- The expander would need two code paths: one for local `.lykn` files
  (read from disk), one for registry modules (extract from embedded
  string). This dual-path complexity is avoidable.
- **Rejected:** over-engineered; adds build-pipeline complexity and
  artifact-size overhead for a problem that Option A solves more
  directly.

### Option D: Ship `.lykn` source + filesystem-cache path lookup

Ensure that `lykn publish` includes `.lykn` source files in the
published artifact (both JSR and npm). Resolve the specifier to the
cached/installed package directory, find the `.lykn` entry file via
`lykn.macroEntry`, and read it from the filesystem cache.

**Pros:**
- `lykn build --dist` for `macro-module` kind packages already stages
  all `.lykn`/`.lyk` files into `dist/` (confirmed in M3). The source
  IS already published.
- Conceptually simple: "find the cached directory, read the file."

**Cons:**
- Requires knowledge of Deno's cache directory structure to find the
  cached package files. Deno uses content-hash-based filenames
  (`$DENO_DIR/remote/https/jsr.io/<hash>`) — individual files are
  not stored in a package-directory structure. This makes "find
  `mod.lykn` in the cache" unreliable without `deno info` to map
  URLs to hashes.
- For npm packages, the cache structure is different again
  (`$DENO_DIR/npm/registry.npmjs.org/@scope/name/version/`).
- **Effectively reduces to Option A** — the `.lykn` source is already
  published; the problem is finding it. Option A's `deno info` +
  fetch approach finds and reads it without depending on cache
  internals.

---

## Decision

**Option A: `deno info --json` redirect + HTTP fetch of source text.**

### Rationale

Option A is chosen because it:

1. **Solves the root cause directly.** The bug is: "the expander can't
   read source text from a registry URL." Option A gives it the source
   text — fetched from the registry URL, then cached locally.

2. **Preserves DD-34's three-tier architecture.** Tier 1 (scheme-
   prefixed) gains a real implementation instead of the current throw.
   Tier 2 and Tier 3 are unchanged. The `lykn.macroEntry` / fallback
   chain from DD-34 §2 is preserved — it operates on the fetched
   `deno.json` from the package's registry URL.

3. **Avoids dependency on Deno's cache internals.** Unlike Option D's
   direct cache-path lookup, Option A uses `deno info --json` (the
   official API for specifier resolution) and `fetch()` (standard
   web API). Both are stable interfaces.

4. **Works for both expanders.** The JS expander can call `deno info`
   via `Deno.Command` and `fetch()` natively. The Rust expander
   delegates to its existing Deno subprocess via a new protocol
   action (`"resolve-macro-source"`).

5. **Philosophy-aligned.** The user writes `(import-macros
   "jsr:@lykn/testing" ...)` and it works. The fetch/cache machinery
   is invisible (Principle 2). Error messages are compiler-grade
   diagnostics with location and suggestions (Principle 3).

### Why others rejected

- **Option B:** Changes the macro authoring surface. Macros would need
  to be pre-compiled JS functions, not `.lykn` source. Too invasive.
- **Option C:** Over-engineered. Embeds source text in compiled output,
  adding build complexity and artifact size for no user-visible benefit.
- **Option D:** Reduces to Option A — the source IS published, but
  finding it requires `deno info` anyway. Going through `fetch()` is
  more reliable than probing Deno's hash-based cache structure.

---

## Implementation outline

### JS expander changes (`packages/lang/expander.js`)

In `resolveImportMacrosSpecifier` (line ~1023), replace the throw at
line 1036 with registry-source resolution:

1. When `import.meta.resolve(specifier)` returns a non-`file://` URL:
2. Run `deno info --json <specifier>` via `Deno.Command` to get the
   redirect mapping (e.g., `jsr:@lykn/testing` →
   `https://jsr.io/@lykn/testing/0.5.1/mod.js`).
3. Derive the package base URL by stripping the filename from the
   redirect target.
4. Fetch `<baseUrl>/deno.json` to find `lykn.macroEntry` (falling back
   to `mod.lykn` per DD-34's resolution chain).
5. Fetch `<baseUrl>/<macroEntry>` to get the `.lykn` source text.
6. Write the source text to a local cache file (e.g.,
   `/tmp/lykn-macro-cache/<specifier-hash>.lykn`) so subsequent
   compilations don't re-fetch.
7. Return the cache file path (preserving the existing path-based
   interface).

The `findMacroEntry` logic is reused: it parses `deno.json` for
`lykn.macroEntry`, falls back through `mod.lykn`/`macros.lykn`/
`index.lykn`.

### Rust expander changes (`crates/lykn-lang/`)

The Rust expander's `DenoSubprocess` (in `deno.rs`) gains a new
protocol action: `"resolve-macro-source"`. Given a registry specifier,
the Deno subprocess performs the same `deno info` + fetch sequence
internally and returns the source text (not a path).

In `pass0.rs` `resolve_specifier`, Tier 1 changes from:
```rust
return deno.resolve_specifier(module_path); // returns PathBuf (broken)
```
to:
```rust
let source = deno.resolve_macro_source(module_path)?;
// Write source to temp file, return path
```

Alternatively, the Rust pipeline's `process_single_import` can be
refactored to accept source text directly (bypassing the path step)
for registry specifiers. This is cleaner but requires a larger
refactor of the import-processing code path.

### Test strategy

- **Unit tests:** In `packages/lang/` test suite, add a test that
  calls `resolveImportMacrosSpecifier("jsr:@lykn/testing", null)` and
  verifies it returns a valid path to a `.lykn` file (or source text).
- **Integration test:** M9-7's smoke test — `lykn new` → `lykn test`
  (with `import-macros "jsr:@lykn/testing"`) → `lykn build --dist` →
  `lykn publish --dry-run`.
- **Regression:** The existing tier-3 filesystem tests must continue
  to pass (backward compatibility).

### Scaffold restoration plan (M9-6)

The scaffold test template in `crates/lykn-cli/src/main.rs`
`test_template()` reverts from M5's workaround:

```rust
// Current (M5 workaround):
r#"(import "jsr:@std/assert" (assert-equals))

(Deno:test "{name}: placeholder test"
  (fn ()
    (assert-equals (+ 1 1) 2)))
"#

// Restored:
r#"(import-macros "jsr:@lykn/testing" (test is-equal))

(test "{name}: placeholder test"
  (is-equal (+ 1 1) 2))
"#
```

---

## Relationship to DD-34

This design **amends** DD-34 §1 (three-tier resolution strategy) and
§3 (delegation protocol to Deno subprocess).

**What's preserved from DD-34:**
- The three-tier resolution order (scheme-prefixed → import-map →
  filesystem) is unchanged.
- The `lykn.macroEntry` field and fallback chain (DD-34 §2) is
  preserved and applied to fetched `deno.json` from registry URLs.
- Tier 2 (import-map lookup) and Tier 3 (filesystem paths) are
  unchanged.

**What's amended:**
- **Tier 1 implementation:** DD-34 assumed `import.meta.resolve()`
  for `jsr:`/`npm:` specifiers would return a `file://` URL pointing
  to the cached package directory. In practice, Deno returns the
  registry URL itself. The fix replaces the direct-resolve-to-path
  approach with `deno info --json` redirect lookup + HTTP fetch of
  the macro source text.
- **Deno subprocess protocol:** The `"resolve"` action (DD-34 §3)
  is supplemented by a `"resolve-macro-source"` action that returns
  source text instead of a path, for cases where path-based resolution
  isn't possible.

**What's NOT changed:**
- The user-facing `import-macros` syntax.
- The package authoring model (macro modules contain `.lykn` source,
  published via `lykn build --dist`).
- Tier 2 and Tier 3 resolution logic.

---

## Open questions

1. **Cache invalidation.** When a macro package is updated on JSR
   (e.g., `@lykn/testing@0.5.2`), the local cache file from the
   previous version may be stale. The `deno info --json` redirect
   includes the version in the URL, so different versions produce
   different cache keys. But if a specifier doesn't pin a version
   (e.g., `jsr:@lykn/testing` without `@0.5.1`), the redirect may
   change between compilations. This is acceptable — it matches
   Deno's own module resolution behavior, where un-pinned specifiers
   resolve to the latest cached version.

2. **Offline compilation.** Option A requires network access on first
   encounter with a registry specifier. If the package has never been
   fetched, compilation fails. This matches Deno's behavior for
   `import` statements (first run fetches, subsequent runs use cache).
   The `deno cache` command can pre-fetch dependencies for offline
   use — we should document this for `import-macros` as well.

3. **npm specifier support.** `npm:` specifiers resolve differently
   from `jsr:` — they use `node_modules/` or Deno's npm cache.
   The `deno info --json` approach works for both, but the URL
   structure differs. Implementation should be tested against both
   `jsr:` and `npm:` specifiers. (JSR is the priority for 0.5.2;
   npm can follow if needed.)

---

## Specifications

### Spec 1 — Verify JS expander failure (M9-1)

Reproduce V-08 cleanly against the **published** 0.5.1 (now live on JSR/npm/crates.io). From a clean directory:

```sh
cd /tmp && rm -rf m9-jsverify && lykn new m9-jsverify
cd m9-jsverify
# Restore the import-macros pattern manually so the JS expander's
# tier-1/tier-2 resolution actually triggers
cat > test/mod_test.lykn <<'EOF'
(import-macros "jsr:@lykn/testing" (test is-equal))

(test "v9 verification — jsr import-macros"
  (is-equal (+ 1 1) 2))
EOF
lykn test 2>&1 | tee /tmp/m9-js-verify.txt
echo "EXIT: $?"
```

Expected output: error from the JS expander mentioning the non-file URL (e.g., `import-macros: resolved jsr:@lykn/testing to non-file URL: jsr:@lykn/testing` or similar). Capture to `workbench/verify/m9/js-expander-prefix.txt`.

Trace the failure point in `packages/lang/expander.js`. The throw is around line 1036; verify that's still the code path triggered.

### Spec 2 — Verify Rust expander failure (M9-2)

The Rust expander's failure mode is structurally similar but uses different code. Find the Rust analog of `resolveImportMacrosSpecifier` (likely in `crates/lykn-lang/src/expander/` or similar). Construct a repro that exercises the Rust path — possibly via `lykn build --dist` on a project that uses `import-macros "jsr:@..."`. The auto-compile path from M5-4 invokes the Rust expander, so it'll trigger.

```sh
cd /tmp && rm -rf m9-rsverify && lykn new m9-rsverify
cd m9-rsverify
# Add an import-macros to the source so the Rust expander has to handle it
cat > packages/m9-rsverify/mod.lykn <<'EOF'
(import-macros "jsr:@lykn/testing" (test is-equal))

(test "v9 rust path"
  (is-equal 1 1))
EOF
rm -f packages/m9-rsverify/mod.js  # force auto-compile
lykn build --dist 2>&1 | tee /tmp/m9-rs-verify.txt
echo "EXIT: $?"
```

Capture to `workbench/verify/m9/rust-expander-prefix.txt`. Identify the Rust-side throw site.

### Spec 3 — Design decision: DD-38 (M9-3)

Produce `docs/decisions/dd-38-import-macros-cache-resolution.md`. Note: `docs/decisions/` doesn't yet exist as a tracked directory; this milestone creates it. (Existing DDs in `workbench/` are gitignored and ephemeral; durable design records belong in `docs/decisions/`.)

**Required structure:**

```markdown
# DD-38: Import-Macros Cache Resolution for jsr:/npm: Specifiers

## Status
Accepted (or Proposed pending Duncan/CDC review)

## Context
- DD-34 (workbench/old/dd-34-cross-package-import-macros-resolution.md)
  introduced the three-tier resolution scheme.
- V-08 (M0 verification) and M5's smoke-test confirmation showed that
  Tier 1 (scheme-prefixed) and Tier 2 (bare names mapping to registry
  specifiers) both throw on jsr:/npm: specifiers because the expander
  needs to *read source text* but registry URLs aren't readable as files.
- M5 worked around by removing import-macros from the scaffold; that
  workaround should not outlive this fix.

## Options analyzed

### Option A: deno info --json + cache-path read
[detailed analysis: pros, cons, implementation shape]

### Option B: dynamic import() with module-shape change
[detailed analysis]

### Option C: embedded source via build-step
[detailed analysis]

### Option D: ship .lykn source + filesystem-cache path lookup
[detailed analysis; note that lykn build --dist already ships .lykn
source for macro-module kind packages — confirmed in M3]

## Decision
[which option chosen + rationale, including why other options rejected]

## Implementation outline
- JS expander changes (packages/lang/expander.js): ...
- Rust expander changes (crates/lykn-lang/...): ...
- Test strategy: unit tests in compiler test suites; integration test
  via the M9-7 smoke test
- Scaffold restoration plan (M9-6): exact text of the reverted scaffold
  test template

## Relationship to DD-34
[explicit: amends? supersedes? specifies what's preserved from DD-34]

## Open questions
[anything that needs Duncan/CDC input before implementation]
```

**This is the load-bearing row of M9.** CC writes it in main context (no subagent). After CC closes M9-3 in the ledger, **CC pauses** for CDC review of DD-38 before proceeding to implementation rows.

The pause is explicit: M9-4 and M9-5 (implementations) block on M9-3's CDC approval. This matches the methodology's step-3-before-step-5 discipline.

### Spec 4 — JS expander implementation (M9-4)

Implement DD-38's chosen approach in `packages/lang/expander.js`. The exact changes depend on the DD's decision; common to most options is replacing the throw at line ~1036 with a path-resolution call (whether `deno info --json`, dynamic `import()`, embedded source lookup, or filesystem cache walk).

Verify: re-run M9-1's repro post-fix; expected output now succeeds (`lykn test` passes the test that uses `import-macros "jsr:@lykn/testing"`). Capture to `workbench/verify/m9/js-expander-postfix.txt`.

The fix must preserve correct behavior for:

- Tier 1 scheme-prefixed (`jsr:`, `npm:`, `https:`, `file:`)
- Tier 2 bare names mapping to either filesystem paths *or* registry specifiers
- Tier 3 filesystem paths (`./`, `../`)

### Spec 5 — Rust expander implementation (M9-5)

Implement the equivalent in `crates/lykn-lang/`. The Rust pipeline is invoked by `lykn build --dist`'s auto-compile path (M5-4) and any other Rust-side compile entry points.

If the chosen approach in DD-38 involves shelling out (e.g., `deno info --json`), the Rust implementation may use `std::process::Command` similarly. If the approach involves a Deno API that's only available in JS, DD-38 should explicitly note that and propose either a delegation pattern (Rust expander hands off macro-resolution work to a Deno subprocess) or a structural alternative.

Verify: re-run M9-2's repro post-fix; the Rust expander now resolves the jsr: specifier successfully. Capture to `workbench/verify/m9/rust-expander-postfix.txt`.

### Spec 6 — Restore scaffold test template (M9-6)

Per CDC observation 2 of M5: when V-08 is fixed, the scaffold's test template should revert from `(import "jsr:@std/assert" ...) + (Deno:test ...)` back to `(import-macros "jsr:@lykn/testing" (test is-equal)) + (test "..." ...)`.

Find the test-template generator in `crates/lykn-cli/src/main.rs` (or wherever `lykn new` produces the test scaffold — same place CC modified during M5). Revert to the import-macros form.

The reverted template should match what `lykn new` produced before M5's workaround. If the pre-M5 template isn't in git history (because it was changed in `4cc1b12` or similar), reconstruct from the testing DSL's documented forms (`docs/guides/16-testing.md` should have canonical examples).

Verify: a fresh `lykn new` produces a test file containing `import-macros` and `test`/`is-equal` macros from the testing DSL.

### Spec 7 — Integration smoke test (M9-7)

End-to-end test that the fix works for a downstream user:

```sh
mkdir -p /tmp/lykn-smoke && cd /tmp/lykn-smoke
rm -rf m9-smoke && lykn new m9-smoke
cd m9-smoke
lykn test 2>&1; echo "test EXIT: $?"
lykn build --dist 2>&1; echo "build EXIT: $?"
lykn publish --jsr --dry-run 2>&1; echo "publish-jsr-dry EXIT: $?"
lykn publish --npm --dry-run 2>&1; echo "publish-npm-dry EXIT: $?"
```

Expected post-fix:

- `lykn test` runs the testing DSL's `(test ...)` form (macro-expanded from `jsr:@lykn/testing`); 1 passed, 0 failed.
- `lykn build --dist` auto-compiles (M5-4 still works); the Rust expander successfully expands the import-macros.
- Both publish dry-runs succeed.

Capture to `workbench/verify/m9/smoke-test-postfix.txt`.

**This is the integration acceptance criterion.** If any step regresses M5's behavior or the V-08 fix doesn't work end-to-end, M9 doesn't close.

### Spec 8 — Substrate-rule compliance (M9-8)

Per Phase 2 methodology improvement #1, closing report includes a Substrate-rule compliance section. Starter rules for M9:

| Rule | Touched by | Expected evidence shape |
|------|-----------|-------------------------|
| `docs/philosophy.md` Principle 2 (lykn-only tooling) | M9-4, M9-5, M9-7 | The fix keeps the user-facing surface unchanged: `(import-macros "jsr:@..." ...)` works as documented; cache-path machinery is invisible. |
| `docs/philosophy.md` Principle 3 (compiler-owned output quality) | M9-3 (DD), M9-4, M9-5 | Whatever resolution mechanism is chosen, errors in macro expansion are reported as compiler-grade diagnostics (location + suggestion), not raw stack traces. |
| `CLAUDE.md` "Lykn CLI safety gates" | M9-4 if it shells `deno info --json` (etc.) | Any subprocess call adheres to the safety-gate discipline — no auto-passing of bypass flags. |
| `assets/ai/LEDGER_DISCIPLINE.md` "do not silently rewrite Verify commands" | All rows | Same discipline. M5's borderline disclosure-without-amendment surfaced a refinement: "verify-pattern wording flexibility within disclosure discipline" — apply if needed. |

Add others if surfaced during the work.

### Spec 9 — Commit chain (M9-9)

Single coherent commit chain naming this milestone. The DD itself can be its own commit; the implementation commits can be separate; all should reference M9 in the message.

---

## Ledger

| ID | Criterion | Verify | Significance | Origin | Status | Evidence | Notes |
|----|-----------|--------|--------------|--------|--------|----------|-------|
| M9-1 | JS expander failure on `jsr:` import-macros reproduced | `test -f workbench/verify/m9/js-expander-prefix.txt && grep -cE "non-file URL\|resolved.*jsr:\|import-macros" workbench/verify/m9/js-expander-prefix.txt` returns ≥1 match | serious | V-08 (M0); Spec 1 | open | | Reproduces against published 0.5.1. |
| M9-2 | Rust expander failure on `jsr:` import-macros reproduced; failure point traced | `test -f workbench/verify/m9/rust-expander-prefix.txt`; closing report identifies the Rust-side file:line where the equivalent throw happens | serious | M5 closing report "What needs to be reflected back" #2; Spec 2 | open | | The Rust expander's error path is different from JS; need to characterize precisely. |
| M9-3 | DD-38 written at `docs/decisions/dd-38-import-macros-cache-resolution.md` with required sections | `test -f docs/decisions/dd-38-import-macros-cache-resolution.md && grep -cE "^## (Status\|Context\|Options analyzed\|Decision\|Implementation outline\|Relationship to DD-34)" docs/decisions/dd-38-import-macros-cache-resolution.md` returns 6 (the six required sections); DD analyzes options A–D from M0 bug report; chooses one with explicit rationale | serious | Spec 3; methodology — design before implementation | open | Blocked by M9-1, M9-2 | **Hard pause point.** After M9-3 closes, CC stops and hands back for CDC review of DD-38. M9-4 and M9-5 do NOT begin until DD-38 is approved. |
| M9-4 | JS expander implements DD-38's chosen approach; M9-1's repro succeeds post-fix | `test -f workbench/verify/m9/js-expander-postfix.txt && grep -cE "1 passed\|test.*ok" workbench/verify/m9/js-expander-postfix.txt` returns ≥1; no "non-file URL" or "import-macros: resolved" errors in the post-fix output | serious | DD-38 implementation; Spec 4 | open | Blocked by M9-3 (DD approval) | The fix must preserve all three tiers' correctness. |
| M9-5 | Rust expander implements DD-38's chosen approach; M9-2's repro succeeds post-fix | `test -f workbench/verify/m9/rust-expander-postfix.txt`; `lykn build --dist` on the M9-2 repro produces a clean dist tree with no expander errors | serious | DD-38 implementation; Spec 5 | open | Blocked by M9-3 (DD approval) | Can run in parallel with M9-4 if DD-38's approach allows. |
| M9-6 | Scaffold test template restored to `import-macros "jsr:@lykn/testing"` form | A fresh `lykn new <name>` produces `test/mod_test.lykn` containing `(import-macros "jsr:@lykn/testing"` and `(test "..."` macros from the testing DSL | correctness | CDC M5 observation 2; Spec 6 | open | Blocked by M9-4, M9-5 | The workaround removed during M5 is now unnecessary; the testing DSL is restored as the default new-user experience. |
| M9-7 | Integration smoke test: scaffold → test (with testing DSL via jsr macros) → build → publish dry-runs all pass | `test -f workbench/verify/m9/smoke-test-postfix.txt && grep -cE "test EXIT: 0\|build EXIT: 0\|publish-jsr-dry EXIT: 0\|publish-npm-dry EXIT: 0" workbench/verify/m9/smoke-test-postfix.txt` returns 4 | serious | Spec 7; integration acceptance criterion | open | Blocked by M9-4, M9-5, M9-6 | If smoke test fails, M9 doesn't close. The post-fix flow must work end-to-end for a downstream user. |
| M9-8 | Substrate-rule compliance section captured in closing report | Closing report contains `## Substrate-rule compliance` section with at minimum the four starter rules (Principle 2, Principle 3, CLAUDE.md safety gates, LEDGER_DISCIPLINE) addressed | correctness | Phase 2 methodology improvement #1; Spec 8 | open | | Per the pattern established in M5-10. |
| M9-9 | Single coherent commit chain naming this milestone | `git log --grep="M9\|import-macros\|DD-38\|V-08" --oneline` returns ≥1 commit | polish | methodology — ledger evidence trail | open | | Multiple commits across DD + impl are fine; at least one names the milestone or DD. |

---

## CC instructions

1. **Read `LEDGER_DISCIPLINE.md` first.** This is M9, the load-bearing Phase 2 milestone. The protocol is the same; the stakes are higher.
2. **Read `SUBAGENT-DELEGATION-POLICY.md` second.** M9's split:
   - **Verification phase (M9-1, M9-2)** — lookup subagents fine for tracing expander code paths in both JS and Rust pipelines. Subagents return raw findings (file:line of throw sites, surrounding context); classification stays in your main context.
   - **DD writing (M9-3)** — **main context only, no subagents.** This is judgment-heavy work; option analysis and decision rationale need full context to be coherent. Read all the source materials in your main context, then write.
   - **Implementation phase (M9-4, M9-5)** — main context only with possible lookup. Editing compiler internals with semantic intent.
   - **Scaffold restoration (M9-6)** — small file edit, main context.
   - **Smoke test (M9-7)** — mechanical execution; capture output.
3. **Read `docs/philosophy.md`** before designing DD-38. Principle 2 says the user-facing surface (`import-macros "jsr:@..."`) must be preserved; the fix lives below that surface. Principle 3 implies the resolver's error reporting should be compiler-grade.
4. **Read `workbench/old/dd-34-cross-package-import-macros-resolution.md`** before writing DD-38. The new DD references and amends DD-34's three-tier scheme.
5. **Read `workbench/bug-import-macros-jsr-resolution.md`** for CC's M0 options A–D analysis. DD-38 builds on that analysis; cite it explicitly.
6. **Hard pause point after M9-3.** The DD is the load-bearing artifact of this milestone. After writing it and closing M9-3 in the ledger, **stop** and hand back to Duncan for CDC review. Do NOT begin M9-4 or M9-5 until DD-38 is approved (with possible amendment).
7. **DD-38 lives in `docs/decisions/`** — a new tracked directory. Existing DDs in `workbench/` are gitignored and ephemeral; durable design records belong in tracked locations. This milestone creates the directory.
8. **For Spec 4 / Spec 5 (implementations):** the JS expander and Rust expander may use different mechanisms for the chosen approach (the DD will spell this out). It's acceptable for them to diverge in implementation while converging in user-visible behavior — both must support `import-macros "jsr:@..."` correctly, but the *how* can differ.
9. **For Spec 6 (scaffold restoration):** find the pre-M5 form of the test template by reading git history (`git log --oneline -- crates/lykn-cli/src/main.rs` near the M5 work), or reconstruct from `docs/guides/16-testing.md` if the history is unclear.
10. **Update the ledger as you work.** Each row gets evidence at completion time.
11. **Per the protocol, walk every row in the closing report.** Plus the Substrate-rule compliance section per Phase 2 methodology.
12. **If a Verify command's pattern doesn't match your actual output's wording** (M5's borderline situation): amend the Verify column inline with a brief note (`(amended <date>: original pattern was "X"; actual wording is "Y", same root cause)`). This is the refinement from M5's CDC review — preserves the no-silent-rewrite spirit without full ledger amendment.

---

## Closing report specification

Path: `workbench/2026-04-XX-M9-closing-report.md` (use the actual close date in `YYYY-MM-DD`).

Structure: same as M5 — per-row walk plus Substrate-rule compliance plus the standard sections (What needs to be reflected back; Findings logged; What this milestone did NOT cover).

The closing report is split into two phases:

- **Pre-DD-approval phase**: M9-1, M9-2, M9-3 walked. CC pauses here for Duncan/CDC review of DD-38.
- **Post-DD-approval phase**: M9-4 through M9-9 walked. CC continues after DD-38 is approved (possibly with amendments).

The closing report can be written in two passes (matching the iteration cycles) or in one pass at full milestone close. Either is fine.

---

## What Worked

*(Filled in at milestone close. Particularly: did the explicit hard-pause-after-DD discipline produce better outcomes than the M5 implicit pattern? Did the option-analysis-in-DD format work? Worth retrospective notes for future DD-bearing milestones.)*

---

## Closure

*(CC fills in: closing commit SHA, date. CDC fills in: verification session, total rows, dispositions.)*

Closed at commit `<SHA>` on `<date>`.
CDC verification: `<session>`.
Total rows: 9. Done: _. Deferred:_. No-op: _.
