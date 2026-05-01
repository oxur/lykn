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

1. The design analysis in this document at `docs/design/05-active/0048-import-macros-jsrnpm-cache-resolution-v-08-fix.md` — options surfaced in M0's bug report (CC's options A–D) analyzed, chosen approach with rationale, and implementation outline.
2. Implementation of the chosen approach in **both** the JS expander (`packages/lang/expander.js`) and the Rust expander (`crates/lykn-lang/`), supporting **both** `jsr:` and `npm:` specifiers (per Duncan's 2026-04-30 decision: 0.5.2 must support both ecosystems at the same time; npm is a fast-follow within M9, not a separate release).
3. Restoration of the scaffold's test template to use `import-macros "jsr:@lykn/testing"` (per CDC observation 2 of M5 — the workaround should not outlive the fix).
4. Integration smoke test confirming downstream Lykn projects can scaffold, test (using the testing DSL via JSR macros), build, and publish end-to-end.

Methodologically: this is the first Phase 2 milestone with a substantial design analysis. The doc lives at `docs/design/05-active/0048-...md`, promoted via Duncan's `odm` flow on 2026-04-30. The workbench ledger ([`workbench/milestones/M9-import-macros-cache-resolution-ledger.md`](../../../workbench/milestones/M9-import-macros-cache-resolution-ledger.md)) tracks milestone progress (rows, evidence, dispositions); this doc is the design analysis.

---

## Source materials (read in this order)

1. `assets/ai/LEDGER_DISCIPLINE.md` — protocol (mandatory)
2. `assets/ai/SUBAGENT-DELEGATION-POLICY.md` — applies; lookup subagents fine for tracing expander code paths; **DD writing in CC's main context** (judgment-heavy work).
3. `docs/philosophy.md` — ground truth. Especially Principle 2 (lykn-only tooling) and the §0.6.0 commitments. The DD's chosen approach must be philosophy-aligned.
4. `workbench/old/dd-34-cross-package-import-macros-resolution.md` — the original three-tier scheme that this milestone amends. This design (0048) references DD-34 and updates the Tier 1 / Tier 2 logic for cache-path resolution.
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
line 1036 with registry-source resolution. The implementation
distinguishes between `jsr:` and `npm:` specifiers because their
fetch paths differ:

#### `jsr:` specifiers (HTTPS-direct file access)

1. When `import.meta.resolve(specifier)` returns a `jsr:` redirect:
2. Run `deno info --json <specifier>` via `Deno.Command` to get the
   redirect mapping (e.g., `jsr:@lykn/testing` →
   `https://jsr.io/@lykn/testing/0.5.1/mod.js`).
3. Derive the package base URL by stripping the filename from the
   redirect target.
4. Fetch `<baseUrl>/deno.json` to find `lykn.macroEntry` (falling back
   to `mod.lykn` per DD-34's resolution chain).
5. Fetch `<baseUrl>/<macroEntry>` to get the `.lykn` source text.
6. Write the source text to the persistent local cache (see "Cache
   location" below).
7. Return the cache file path (preserving the existing path-based
   interface).

#### `npm:` specifiers (tarball-based; no HTTPS-direct file access)

npm packages are distributed as tarballs and individual files are
*not* HTTPS-addressable the way JSR's are. The `jsr:` flow above
won't work directly. For `npm:` specifiers:

1. Run `deno info --json <specifier>` to get Deno's npm-cache
   resolution. Deno's npm cache extracts tarballs under
   `$DENO_DIR/npm/registry.npmjs.org/<name>/<version>/`.
2. Read the cache directory path from `deno info`'s output (the
   `local` field of the resolved module entry).
3. Locate the macro module on the cache filesystem: read
   `<cache>/package.json` for the package's `main` / `exports` and
   look for a sibling `.lykn` file (or follow `lykn.macroEntry` if
   the npm package's `package.json` has a `lykn` field).
4. Read the `.lykn` source text from the cache filesystem.
5. Write to the persistent local cache (same location as JSR path).
6. Return the cache file path.

The `findMacroEntry` logic is reused for both paths: it parses the
package metadata for `lykn.macroEntry` and falls back through
`mod.lykn`/`macros.lykn`/`index.lykn`.

#### Cache location (XDG-compliant)

Source caches live at:

- `$XDG_CACHE_HOME/lykn/macros/<specifier-hash>.lykn` if
  `XDG_CACHE_HOME` is set
- `~/.cache/lykn/macros/<specifier-hash>.lykn` otherwise

The cache must be **persistent across reboots** — `/tmp/` is not
acceptable per Duncan's 2026-04-30 decision (it can be wiped
arbitrarily). The cache directory is created on first use.

The `<specifier-hash>` includes the resolved specifier (post-`deno
info` redirect), so different versions of the same package produce
different cache keys naturally.

#### Error handling (Principle 3 — compiler-grade diagnostics)

All failure modes surface as compiler-grade diagnostics with location
and suggestion, never as raw exceptions or stack traces from the
underlying tools. Required handling:

| Failure mode | Diagnostic shape |
|---|---|
| `deno info --json` fails (network, registry down) | `cannot resolve macro module 'jsr:@scope/name': network unreachable. Try \`deno cache jsr:@scope/name\` to prefetch, or check connectivity.` |
| HTTPS fetch fails (404 — package doesn't exist) | `macro module 'jsr:@scope/name' not found on JSR. Check the specifier name and try again.` |
| HTTPS fetch fails (other HTTP error) | `cannot fetch macro module 'jsr:@scope/name@version': HTTP <code>. <human-readable reason>.` |
| Package's `deno.json`/`package.json` lacks a resolvable macro entry | `macro module 'jsr:@scope/name' has no \`lykn.macroEntry\` field and no \`mod.lykn\` / \`macros.lykn\` / \`index.lykn\` fallback. Cannot expand macros.` |
| Cache file corruption (already-cached file is invalid) | Detect on read failure; re-fetch automatically once; if re-fetch also fails, surface the underlying network/HTTP error. |
| `npm:` specifier with no `.lykn` content in cache | `macro module 'npm:<name>' does not appear to be a Lykn macro module (no .lykn files found). Verify the package was published with \`lykn build --dist\`.` |

All errors carry the source location of the originating
`import-macros` form (file:line:col), matching the existing expander
diagnostic style visible in M9-2's verify artifact.

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

3. **npm specifier support — RESOLVED 2026-04-30.** Both `jsr:` and
   `npm:` must work in 0.5.2; npm cannot be deferred. Per Duncan:
   "the npm solution is going to be a fast-follow — we will not
   release with a jsr-only solution; we need to support users of
   both ecosystems at the same time." The implementation outline
   above now spells out the npm-specific path (Deno npm-cache
   filesystem read rather than HTTPS-direct fetch). Both paths
   must pass the M9-7 integration smoke test.
