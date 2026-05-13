---
number: 52
title: "DD-53 — V-08 Sibling-Fetch for Surface-macros (0.6.0 ship-gate)"
author: "Deno at"
component: All
tags: [change-me]
created: 2026-05-12
updated: 2026-05-12
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# DD-53 — V-08 Sibling-Fetch for Surface-macros (0.6.0 ship-gate)

**Status:** Resolved — calls landed 2026-05-12. Ready for implementation.
**Thread:** cdc/dep-ergonomics
**Relates to:** DD-48 (V-08 fix, M9), DD-52 (surface-macros local-path scope, just closed). Inherits DD-52's fast-follow #1.
**Ship-gate:** 0.6.0. DD-52 enables surface-macros for local-path imports; DD-53 extends to JSR-resolved imports — the canonical 0.6.0 downstream pattern.

## Resolved design calls

All eight design questions resolved per Duncan's calls 2026-05-12 (deltas from proposed defaults: none — all defaults accepted).

| # | Question | Call | Detail |
|---|---|---|---|
| Q1 | Cache layout | **A — Per-package directory** | `~/.cache/lykn/macros/{key}/mod.lykn` + sibling files co-located. Matches local-path + npm semantics. |
| Q2 | Sibling-fetch scope | **A — Surface-macros only** | `(runtime-import ...)` resolved by Deno at runtime; doesn't need lykn-cache co-location. |
| Q3 | Parsing strategy | **A — Use `packages/lang/reader.js`** | Reuse the existing reader in the subprocess; no parser duplication. |
| Q4 | Sibling-fetch failure | **A — Fail-fast** | Matches DD-52 Q3 and Principle 3 (compiler-owned output quality). |
| Q5 | Path validation | **A — Reject `..` and absolute paths** | Security; surface-macros are author-internal, no legitimate need to escape package directory. |
| Q6 | npm path | **A — Use Deno's npm cache `localDir` directly** | Asymmetric with JSR but each path internally coherent; no redundant re-caching. |
| Q7 | Cache migration | **A — Clean cutover** | lykn cache is brand new (M9 / 0.5.2); one extra round of network fetches per package is negligible. |
| Q8 | Test strategy | **D — Mock JSR server + cache inspection** | `std/http/server`-based mock + structural cache verification; deterministic, no network dependency. |

## Refinement log

- **2026-05-12 (drafting):** DD drafted. Cache-layout problem discovered during recon — flat-file layout structurally cannot hold siblings; expanded scope from "fetch siblings" to "cache layout migration + sibling fetch + npm path verification."
- **2026-05-12 (calls):** Duncan reviewed; accepted all eight defaults including the substantive Q1, Q6, Q8.

---

## Problem

DD-52 added `load-surface-macros` to the Rust expander. It works for **local-path** macro modules (where the .lykn file and its sibling JS file live on disk together). It does **not** work for **JSR-resolved** macro modules — the canonical 0.6.0 downstream pattern, e.g.:

```lykn
(import-macros "jsr:@lykn/testing" (test is-equal))
```

Two reasons it fails:

### 1. The current V-08 cache layout has no room for siblings

V-08 (DD-48 / M9) fetches the macro module's `.lykn` source via HTTP and writes it to a flat file:

```
~/.cache/lykn/macros/{cache_key}.lykn
```

When pass0 invokes `load_surface_macros(module_dir, "macros.js")`, `module_dir` is computed as `resolved.parent()` — i.e., the shared `~/.cache/lykn/macros/` directory. Surface-macros looking for `macros.js` would look at:

```
~/.cache/lykn/macros/macros.js   ← every package's `macros.js` collides
```

The flat layout structurally cannot hold siblings.

### 2. Siblings aren't fetched alongside `.lykn`

Even if the cache layout supported per-package directories, `resolve-macro-source` only fetches `mod.lykn` from the JSR base URL. The sibling JS file declared by `(surface-macros "macros.js")` inside that fetched source is never fetched.

### Downstream impact

Mycelium adopting `(import-macros "jsr:@lykn/testing" ...)` — the post-Finding-D supported pattern — would fail with a `file not found: macros.js` error. The Rust-side compile path that consumes the testing DSL is broken for any registry-pinned consumer. **0.6.0 cannot ship the full downstream story without this fix.**

---

## Scope

Three phases, all in the Rust expander:

1. **Cache layout migration.** Flat-file → per-package directory.
2. **Sibling-fetch for JSR.** Parse the fetched `.lykn` for `(surface-macros "...")`, HTTP-fetch each declared sibling, co-locate in the new directory layout.
3. **npm path verification.** Surface-macros lookup for npm-resolved packages must use Deno's npm cache directory (which already has siblings), not the lykn cache.

JS expander is **not** in scope: it runs inside Deno and uses Deno's native resolution, which caches whole packages including siblings. JSR-resolved surface-macros already work on the JS side.

---

## Design questions for Duncan / CDC

Eight questions; three are substantive (Q1, Q6, Q8), the rest have clear defaults.

### Q1 (substantive) — Cache layout

**Options:**

- **A. Per-package directory.** `~/.cache/lykn/macros/{cache_key}/mod.lykn` + `~/.cache/lykn/macros/{cache_key}/macros.js`. Each macro module gets its own subdirectory. Siblings live alongside `mod.lykn` exactly as they would on a local filesystem.
- **B. Flat with sibling-name encoding.** `~/.cache/lykn/macros/{cache_key}.lykn` + `~/.cache/lykn/macros/{cache_key}__macros.js`. The subprocess `load-surface-macros` translates `macros.js` → `{cache_key}__macros.js`. Keeps the existing flat layout; awkward naming.

**Proposed default: A.** Matches local-path and npm-cache semantics. Surface-macros loading needs zero special-case translation. Cache invalidation becomes per-directory (clean `rm -rf` semantics). Slight cost: one-time cache invalidation when 0.6.0 lands (any existing flat .lykn files become stale and need re-fetch).

### Q2 — Sibling-fetch scope

**Options:**

- **A. Only files referenced by `(surface-macros "...")`.** Minimum necessary; matches DD-52's surface-macros-only scope.
- **B. All files in the JSR package.** More general but bigger fetch surface; unclear how to enumerate "all files" via JSR's HTTP API.
- **C. Surface-macros + `(runtime-import "./*.js" ...)` siblings.** Covers runtime-imports too.

**Proposed default: A.** `(runtime-import ...)` siblings (Option C) are resolved by Deno at runtime via the consuming project's import map, not by the lykn cache — they don't need to be in our cache. Option B is unbounded.

### Q3 — Parsing strategy in subprocess

How does `resolve-macro-source`'s extended logic find `(surface-macros "...")` declarations in the fetched `.lykn` source?

**Options:**

- **A. Use `packages/lang/reader.js`.** The subprocess already runs in Deno; the reader is already imported by the macro-form expansion path. Reuse it.
- **B. Regex / string scan.** `/\(\s*surface-macros\s+"([^"]+)"\s*\)/g` or similar. Avoids any dependency on the reader; simple to reason about.

**Proposed default: A.** Cleaner; doesn't duplicate parser logic; handles edge cases (comments, escaped strings, whitespace variation) correctly. The reader is already in the subprocess's load surface.

### Q4 — Sibling-fetch failure mode

**Options:**

- **A. Fail-fast.** Whole `resolve-macro-source` call fails; compilation aborts with a clear error referencing the missing sibling.
- **B. Graceful — return success with .lykn, surface-macros fails downstream.** User sees a worse error later.

**Proposed default: A.** Matches DD-52 Q3 fail-fast and Principle 3 (compiler-owned output quality — silent failure produces worse user experience downstream).

### Q5 — Path validation for sibling filenames

What if a JSR-published macro module declares `(surface-macros "../etc/passwd")` or `(surface-macros "/absolute/path")`?

**Options:**

- **A. Reject `..` and absolute paths.** Surface-macros are author-internal to the package; legitimate paths never escape the package directory.
- **B. Allow `..` (Node ecosystem convention).** Could be useful for monorepo-published packages with shared sibling JS in a parent directory.

**Proposed default: A.** Security; surface-macros' purpose is bundling JS within a package, not cross-package or cross-cache file access. Option A also produces clean errors when a poorly-formed JSR package has bad surface-macros declarations.

### Q6 (substantive) — npm path: where does `module_dir` come from?

For npm-resolved macro modules, V-08 currently reads `.lykn` text directly from Deno's npm cache (via `deno info --json`'s `mod.local`). It does **not** re-cache into `~/.cache/lykn/macros/`. So:

**Options:**

- **A. Surface-macros looks up siblings in Deno's npm cache directly.** `load_surface_macros` receives the npm cache `localDir` as `module_dir`; siblings are already there. Asymmetric with JSR (which uses the lykn cache) but matches the existing data flow.
- **B. Re-cache npm sources into the lykn cache (per-package directory, like JSR).** Symmetric layout but redundant — the data already exists in Deno's cache.

**Proposed default: A.** Use Deno's npm cache for npm; lykn cache for JSR. Asymmetric but each path is internally coherent. Re-caching is duplicate effort with no clear benefit. **However:** Option A requires that `resolve-macro-source` return *both* the source text AND the `module_dir` path for npm (currently returns only source). Minor protocol extension.

### Q7 — Migration path for existing caches

Current users with `0.5.2` flat-file caches will need re-fetch on 0.6.0.

**Options:**

- **A. Clean cutover.** Old flat-file caches are simply ignored; new directory-layout caches get created on first 0.6.0 fetch. One extra round of network fetches per macro module, then steady state.
- **B. Detect and migrate.** Check for old flat files; move them into directory layout. Adds migration code.

**Proposed default: A.** The lykn cache is recent (M9 / 0.5.2 — only weeks old at 0.6.0 ship). Existing users have small caches; one extra fetch round is negligible cost. Migration code is over-engineering for a brand-new tool's tiny cache.

### Q8 (substantive) — Real-world test strategy

How do we verify JSR-fetched surface-macros actually work end-to-end?

**Options:**

- **A. Mock JSR server.** Use `std/http/server` in tests to serve a fake `@lykn/testing` package over HTTP, then run `resolve-macro-source` against it. Full path exercised; no network dependency.
- **B. Real JSR fetch.** Test compiles a source using `(import-macros "jsr:@lykn/testing" ...)` and verifies the macros expand. Requires network; flaky in CI.
- **C. Cache-contents inspection.** After a (mocked or real) fetch, verify the cache directory has both `mod.lykn` and `macros.js`. Indirect but practical.
- **D. Combine A + C.** Mock server + cache inspection. Comprehensive without network dependency.

**Proposed default: D.** Mock server lets us deterministically test all paths (happy / 404 / malformed / bad-form fixtures from DD-52 plus the new JSR-sibling scenarios). Cache inspection complements with structural-correctness checks. Engineering effort is modest (`std/http/server` is small).

**Fallback if D feels heavy:** Default to C alone — set up a synthetic cache directory state, run `load_surface_macros` against it. Skips the actual fetch but verifies the layout is correct. Less comprehensive but faster.

---

## Architecture (assuming defaults accepted)

### Cache layout (Q1=A)

```
~/.cache/lykn/macros/
├── {hash-of-jsr:@lykn/testing@0.5.2}/
│   ├── mod.lykn          ← fetched from JSR
│   ├── macros.js         ← fetched from JSR (sibling of mod.lykn)
│   └── deno.json         ← fetched if needed for macroEntry resolution
└── {hash-of-jsr:@other/pkg@1.0}/
    └── mod.lykn
```

`{cache_key}` = stable hash of the full specifier (`jsr:@lykn/testing@0.5.2`). Different versions → different cache keys → no version-collision.

### `resolve-macro-source` JSR path extension

After fetching the .lykn source text:

1. Parse the source with `packages/lang/reader.js` (Q3=A).
2. Walk top-level forms; collect every `(surface-macros "path")` directive's path string.
3. For each path: validate (Q5=A — no `..`, no absolute), fetch from `{baseUrl}/{path}`, write to `{cache_dir}/{path}` (relative within the package directory).
4. On any fetch failure: return `{ok: false, error: ...}` (Q4=A).
5. On success: return both source text AND the cache directory path so pass0 can use the directory for `module_dir` (subtle protocol extension — see below).

### Protocol extension

`resolve-macro-source` response shape grows from a string to an object:

**Before (DD-48 / M9):**
```json
{ "ok": true, "result": "<.lykn source text>" }
```

**After (DD-53):**
```json
{
  "ok": true,
  "result": {
    "source": "<.lykn source text>",
    "moduleDir": "/Users/foo/.cache/lykn/macros/{cache_key}"
  }
}
```

The Rust side's `resolve_macro_source` method signature changes from `Result<String, _>` to `Result<ResolvedMacroModule, _>` (or similar). Backwards compatibility: not a concern — this is all internal subprocess protocol.

For **npm-resolved** packages, `moduleDir` = the npm cache `localDir` (Q6=A), avoiding duplicate caching.

### pass0 changes

Adapt the `module_dir` derivation: instead of `resolved.parent()` (which returned the global `cache_dir`), use the `moduleDir` returned from `resolve_macro_source`. For local-path imports (no JSR/npm specifier), the old behavior (parent of the resolved file path) still applies.

### What does NOT change

- `load-surface-macros` action: unchanged. Already accepts `moduleDir` + `jsRelPath` and reads sibling JS via filesystem.
- `eval-surface-macro` action: unchanged.
- pass2 dispatch: unchanged.
- JS expander: unchanged (already works for JSR via Deno's native resolution).
- DD-52's tests: should still pass (local-path semantics unchanged).

---

## Iteration estimate

**2 iterations:**

- **Iter 1:** Protocol extension + cache layout migration + JSR sibling-fetch + Q6 npm path. Includes:
  - `env.rs::resolve-macro-source`: parse source, fetch siblings, return `{source, moduleDir}`.
  - `deno.rs::resolve_macro_source`: signature change.
  - `pass0.rs`: consume `moduleDir` from the new return shape.
  - Cache layout: flat-file → per-package directory.
  - npm path: thread `localDir` through as `moduleDir`.
- **Iter 2:** Tests (mock JSR server + cache inspection per Q8=D) + closing report + cross-compiler convergence verification.

---

## Risk profile

- **Moderate-low.** Protocol change touches one action with one caller. Cache layout change is structurally clean (no migration logic per Q7=A).
- **JSR fetching is HTTP — same security profile as M9's existing JSR-source fetch.** No new attack surface.
- **Path traversal risk** (Q5) addressed by validation.
- **Test infrastructure for mock JSR server** is new engineering work (~50 lines using `std/http/server`). Modest; doesn't compound.

---

## Test gates (closing-report requirements)

1. **Existing DD-52 tests pass** (8 surface-macros tests including @lykn/testing local-path real-world).
2. **New JSR-mock test** exercises the full path: `(import-macros "jsr:@fake/testing" (test))` → mocked server returns `mod.lykn` declaring `(surface-macros "macros.js")` → sibling `macros.js` fetched and cached → surface macros expand correctly.
3. **Cache layout inspection**: after a (mocked) fetch, the directory `{cache_dir}/{key}/` contains both `mod.lykn` and `macros.js`.
4. **Failure modes** covered: missing sibling on server (404 → fail-fast), malformed sibling path (`..` rejected), surface-macros declaring non-existent file.
5. **npm path verified**: a fixture using an npm-resolved package's surface-macros loads correctly via `localDir`.
6. **DD-50.7 mycelium regression check**: `verify-finding-e-2026-05-12.sh --lykn-bin ./target/release/lykn` → ✓ ALL CHECKS PASSED.
7. **Full test suites**: `cargo test -p lykn-lang` (989+) and `lykn build && lykn test` (1187+ — DD-52's count grows by however many tests DD-53 adds).

---

## What this DD does NOT cover

- **`(runtime-import ...)` sibling fetching** (Q2 alternative C). Runtime-imports are Deno's responsibility at runtime; they don't need lykn-cache co-location.
- **JS expander parallel work.** JS expander handles JSR via Deno's native cache; no work needed.
- **Browser-side fetching.** Browser bundle is a separate consumer; not in scope.
- **`deno.json`-declared aliases for sibling paths.** If JSR packages need configurable surface-macros paths via deno.json (rare), additive extension later.
- **The `__surface_macro__` sentinel cleanup.** DD-52 fast-follow #2; orthogonal to DD-53.
- **The `bad-form` fixture test gap.** DD-52 fast-follow noted by CDC; add as a one-line test in DD-53's closing or in a separate cleanup commit.

---

## Discussion points for Duncan

Three substantive calls — Q1 (cache layout), Q6 (npm path approach), Q8 (test strategy). Five have clear defaults — Q2, Q3, Q4, Q5, Q7.

If you accept the defaults, I'll write the implementation prompt. Same pattern as DD-52: DD-53 finalized, prompt to CC, diagnose-first then implement, real-world gates, closing report with the same shape as DD-52's.

If you'd call any differently, the DD updates first.
