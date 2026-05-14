---
number: 54
title: "Binary Self-Containment: Embed `packages/lang/` in the Lykn Binary"
author: "Duncan McGreggor"
component: All
tags: [change-me]
created: 2026-05-13
updated: 2026-05-13
state: Final
supersedes: null
superseded-by: null
version: 1.0
---

# DD-54 — Binary Self-Containment: Embed `packages/lang/` in the Lykn Binary

**Status:** Resolved — calls landed 2026-05-13. Ready for implementation.
**Thread:** cdc/dep-ergonomics
**Relates to:** DD-48 (V-08, the Deno subprocess infrastructure), DD-52 (surface-macros loading), DD-53 (V-08 sibling-fetch, which surfaced this issue).
**Ship-gate:** 0.6.0. DD-54 is the load-bearing fix that makes the lykn binary actually deployable to downstream consumers.

## Resolved design calls

All eight design questions resolved per Duncan's calls 2026-05-13 (deltas from proposed defaults: none — all defaults accepted).

| # | Question | Call | Detail |
|---|---|---|---|
| Q1 | Materialization location | **A — `$XDG_CACHE_HOME/lykn/embedded/<version>/`** | XDG-respecting; version-suffixed; persistent across invocations. Same parent as the existing V-08 macros cache. |
| Q2 | Materialization timing | **A — Lazy on first spawn, stat-check sentinel** | First subprocess spawn does the materialization (if not present). Subsequent spawns stat `_embedded/.lykn-version`; skip if match, rewrite otherwise. |
| Q3 | Which packages embed | **A — Whole of `packages/lang/`** | `include_dir!` of the directory. Future-proof against added files in the dep closure. |
| Q4 | In-monorepo skip optimization | **B — Always materialize** | Uniform behavior across installed-binary and dev-build invocations. Lykn devs working on `reader.js` use `cargo run` (live workspace); not a justification for runtime heuristic. |
| Q5 | Cleanup strategy | **A — Never, baseline** | Materialized JS is ~200KB per version; persisting is fine. Optional 0.7.0 enhancement: `lykn cache clean` subcommand. |
| Q6 | Versioning | **A — Sentinel file with `CARGO_PKG_VERSION`** | `_embedded/.lykn-version` contains the binary version string. |
| Q7 | Subprocess location strategy | **A — `cmd.current_dir(materialized)`** | Change `Command::current_dir` in `build_deno_command`. Embedded JS unchanged. |
| Q8 | Cross-platform paths | **Default — trust `std::path::PathBuf`** | No special-case logic unless explicit failure surfaces. |

## Refinement log

- **2026-05-13 (drafting):** DD drafted. Eight questions surfaced; Q7's "subprocess CWD change" exposed a hidden second-order interaction with `--config project.json` discovery. Explicitly called out as the load-bearing item for CC's Turn 1 diagnosis.
- **2026-05-13 (calls):** Duncan reviewed; accepted all eight defaults including the substantive Q1–Q4.

---

## Problem

The lykn binary's Deno subprocess (`crates/lykn-lang/src/expander/env.rs`) uses **CWD-relative filesystem imports**:

```javascript
// env.rs line 110 (compile action, M9)
const { read } = await import("./packages/lang/reader.js");

// env.rs line 202 (resolve-macro-source JSR branch, DD-53)
const { read: readLykn } = await import("./packages/lang/reader.js");
```

These imports resolve relative to the subprocess's CWD. When `lykn compile` is invoked from a directory that doesn't have `./packages/lang/` accessible (i.e., any directory outside the lykn monorepo or a sibling with `../lang/...` workspace pointers), the imports fail and any macro work silently breaks.

**Concrete failure scenarios:**

1. **`cargo install lykn-cli`** → user runs `lykn compile main.lykn` in their own project → `./packages/lang/reader.js` doesn't exist in their project → macro support fails.
2. **Mycelium** (canonical downstream, sibling to lykn workspace) — Duncan's `lykn compile` invocations work today because... probably because mycelium's `cargo run` against the lykn workspace binary inherits the workspace's CWD? Worth verifying as part of DD-54's acceptance gate.
3. **Any 0.6.0 downstream consumer** following the kickoff's vision (mycelium pattern, third-party adopters) will hit this immediately.

**This is the canonical 0.6.0 ship-blocker.** The dep-ergo thread has been working to make downstream consumption work end-to-end; this is the foundational architectural fix the rest of the thread's work depends on.

### How the issue surfaced

DD-53 Round 3 Turn 1 (CC's test design, 2026-05-13). CC was writing a test that runs `lykn compile` from a temp project outside the workspace; the subprocess's `./packages/lang/reader.js` import failed; CC honestly surfaced this as an architectural finding rather than working around it silently.

---

## Scope

Three phases, all in the Rust binary's build + runtime path:

1. **Embed `packages/lang/`** into the lykn binary at compile time via the `include_dir!` macro (the `include_dir` crate).
2. **Materialize at runtime** to a known XDG-cache location with version-suffixed path.
3. **Point the subprocess at the materialized location** by setting `cwd` on the deno-eval `Command`.

JS expander is **not** in scope. The JS expander runs natively in Deno from within the user's project, using Deno's normal module resolution. It has no parallel issue.

---

## Design questions for Duncan / CDC

Eight questions. Q1, Q2, Q3, Q4 are substantive (have real trade-offs). Q5, Q6, Q7, Q8 have clear defaults.

### Q1 (substantive) — Materialization location

Where does the embedded `packages/lang/` get written at runtime?

**Options:**

- **A. `$XDG_CACHE_HOME/lykn/embedded/<binary-version>/packages/lang/`** (defaults to `~/.cache/lykn/embedded/<version>/...` on macOS/Linux). XDG-respecting; version-suffixed; persistent across invocations.
- **B. System temp** (`$TMPDIR/lykn-embedded-<pid>/...`). Ephemeral; recreated per invocation; ~200KB of disk I/O per `lykn compile` call. Simpler lifecycle.
- **C. Binary-co-located** (`<binary-dir>/_embedded/...`). Adjacent to the lykn binary. Often `/usr/local/bin/` or `~/.cargo/bin/` — not always writable; brittle.

**Proposed default: A.** XDG cache convention; version-suffixing solves the staleness problem; persistence eliminates per-call I/O overhead. Aligns with the existing `~/.cache/lykn/macros/` cache (V-08 / DD-53). Same parent directory; same XDG-respecting helper code.

### Q2 (substantive) — Materialization timing

When does the materialization actually happen?

**Options:**

- **A. Lazy on first subprocess spawn.** First `lykn` invocation that needs to spawn the subprocess does the materialization (if not present). Subsequent invocations skip via sentinel-file check.
- **B. Eager on every binary startup.** Materializes during `main()` setup before any command dispatch. Even `lykn --version` materializes.
- **C. Lazy + verify on every spawn** (stat-check the sentinel; rewrite if missing or wrong version).

**Proposed default: A with C's stat-check.** First time: materialize. Subsequent times: stat the sentinel file (`_embedded/.lykn-version`); if present and content matches `CARGO_PKG_VERSION`, skip materialization. If missing/mismatched, rewrite. Cost: one filesystem stat per subprocess spawn (~negligible).

### Q3 (substantive) — Which packages to embed

**Decision needed: confirm the embedded set.**

The subprocess at `env.rs` imports:
- `./packages/lang/reader.js` (compile action + resolve-macro-source JSR branch)
- `./packages/lang/expander.js` (compile action: `compileMacroBody`, `extractParamNames`)
- Any modules those transitively import within `packages/lang/`

The `packages/lang/` directory contains: `reader.js`, `expander.js`, `compiler.js`, `surface.js`, `mod.js`, `deno.json`, plus possibly `astring`-adjacent shim code.

**Options:**

- **A. Embed all of `packages/lang/` as a directory.** Includes any siblings the JS files transitively need. Future-proof: new files added to `packages/lang/` flow through `include_dir!` automatically.
- **B. Embed only the specific files the subprocess imports.** Smaller binary; more brittle (future additions need Rust-side changes).

**Proposed default: A.** `include_dir!` is designed for whole-directory embedding. The size cost is ~200KB total — negligible. The maintenance cost of "remember to embed new files" is unbounded.

Also embedded NOT in scope:
- `packages/testing/` — surface-macros + helpers; consumer-side via JSR.
- `packages/browser/` — browser bundle; separate consumer.

### Q4 (substantive) — Symlink optimization for in-monorepo development

When `lykn compile` runs inside the lykn workspace (where `./packages/lang/` already exists on disk and is the live source the dev is editing), should the binary skip materialization?

**Options:**

- **A. Detect and skip.** Heuristic: if `./packages/lang/reader.js` exists in CWD, assume in-monorepo and use those files. Lykn devs editing `reader.js` see changes immediately without rebuilding the binary.
- **B. Always materialize, even in-workspace.** Behavior is uniform across installed-binary and dev-build invocations.
- **C. Detect via Cargo build-time flag** (debug vs release). Debug builds skip materialization; release builds always materialize.

**Proposed default: B.** Uniform behavior across all invocations. The dev-convenience argument for A is weakened by: lykn devs working on reader.js would use `cargo run` (which uses the live workspace) anyway; the materialization is for the *installed-binary* case. Option A's heuristic introduces a behavioral split between installed-binary and dev-build that's exactly the kind of thing that hides bugs. (Same reasoning as DD-50.6: closure-without-empirical-validation is a methodology gap; same shape of issue.)

### Q5 — Cleanup strategy

When does the materialized directory get cleaned up?

**Options:**

- **A. Never.** Stays around indefinitely, like any normal `~/.cache/` entry.
- **B. On version-mismatch.** When a new binary version writes its materialization, old version directories are cleaned up.
- **C. On user command.** A new `lykn cache clean` subcommand for users who want to reclaim space.

**Proposed default: A as baseline, with C as a future enhancement.** Materialized JS is small (~200KB per version). Leaving it around is fine. Option C is a tiny new subcommand; can be added in 0.7.0 without affecting DD-54's scope. Option B introduces "cleanup runs from production code paths" complexity that has zero practical benefit.

### Q6 — Versioning the materialization

How does the binary know its materialization is current?

**Options:**

- **A. Sentinel file with binary version string.** `_embedded/.lykn-version` contains `env!("CARGO_PKG_VERSION")`. On startup, read the sentinel; if missing or mismatched, rewrite.
- **B. Content hash.** A digest of the embedded directory computed at compile time, embedded as a string; verified at runtime.
- **C. Mtime check.** Compare materialized files' mtimes to the binary's mtime; rewrite if older.

**Proposed default: A.** Simplest; correct in all realistic scenarios. Binary version changes → rewrite. Same version → trust existing files. Option B is more robust but has marginal practical benefit (the failure mode it protects against — manual edits to the cache — isn't a real user scenario). Option C is unreliable on systems with weird mtime semantics.

### Q7 — Subprocess location strategy

The current subprocess does `await import("./packages/lang/reader.js")`. The fix needs to make this resolve to the materialized location.

**Options:**

- **A. Change the subprocess's CWD.** Set `cwd: <materialized_dir>` in `build_deno_command` (`deno.rs:47`). Then `./packages/lang/...` resolves to `<materialized_dir>/packages/lang/...`. The embedded JS stays unchanged.
- **B. Rewrite the embedded JS to use absolute paths.** The Rust side interpolates the materialized path into the JS source at runtime: `await import("file://${MATERIALIZED}/packages/lang/reader.js")`.

**Proposed default: A.** Minimal Rust-side change (set one field on the `Command`). Embedded JS is unchanged. Subprocess errors / stack traces show the materialized path (slight readability issue but acceptable). Option B requires runtime mutation of the embedded JS, which is invasive and introduces a layer of indirection.

**Subtle consideration for A:** the subprocess's CWD becomes the materialized dir, which means any *other* CWD-relative behavior in the subprocess changes too. Need to audit env.rs for other CWD-relative code paths. If any exist, they need explicit handling.

### Q8 — Cross-platform path handling

Windows path separators, drive letters, etc.

**Default: trust `std::path::PathBuf`.** `include_dir` handles embedding cross-platform; `PathBuf::join` and `PathBuf::to_string_lossy` handle materialization. No special-case logic unless an explicit failure surfaces.

---

## Architecture (assuming defaults accepted)

### Build-time

`Cargo.toml` (workspace level) gains:
```toml
[workspace.dependencies]
include_dir = "0.7"  # or latest stable
```

`crates/lykn-lang/Cargo.toml` (or wherever the subprocess lives) gains:
```toml
[dependencies]
include_dir = { workspace = true }
```

`crates/lykn-lang/src/expander/embedded.rs` (new file):
```rust
use include_dir::{include_dir, Dir};

pub static PACKAGES_LANG: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../packages/lang");

pub const EMBEDDED_VERSION: &str = env!("CARGO_PKG_VERSION");
```

(Exact path resolution depends on workspace layout; verify in CC's diagnosis.)

### Runtime materialization

`crates/lykn-lang/src/expander/embedded.rs` (continued):
```rust
use std::path::PathBuf;
use std::fs;

pub fn materialize_packages() -> std::io::Result<PathBuf> {
    let cache_root = xdg_cache_home().join("lykn").join("embedded").join(EMBEDDED_VERSION);
    let sentinel = cache_root.join(".lykn-version");

    // Check if materialization is current
    if let Ok(existing) = fs::read_to_string(&sentinel) {
        if existing.trim() == EMBEDDED_VERSION {
            return Ok(cache_root);
        }
    }

    // Re-materialize
    fs::create_dir_all(&cache_root)?;
    let pkg_dir = cache_root.join("packages").join("lang");
    fs::create_dir_all(&pkg_dir)?;
    PACKAGES_LANG.extract(&pkg_dir)?;
    fs::write(&sentinel, EMBEDDED_VERSION)?;

    Ok(cache_root)
}

fn xdg_cache_home() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache")
    } else {
        std::env::temp_dir()  // fallback
    }
}
```

(Pseudocode — adjust to existing patterns in the codebase. The existing macros cache helper in `pass0.rs::macro_cache_dir` is the right reference pattern.)

### Subprocess spawn change

`crates/lykn-lang/src/expander/deno.rs::build_deno_command` (lines 47-56):
```rust
fn build_deno_command(script: &str, include_config: bool) -> Command {
    let mut cmd = Command::new("deno");
    let materialized = embedded::materialize_packages()
        .expect("materializing embedded packages failed");
    cmd.current_dir(&materialized);  // ← new line
    cmd.arg("eval");
    if include_config {
        cmd.args(["--config", "project.json"]);
    }
    cmd.arg("--ext=js")
        .arg(script)
        ...
}
```

The `--config project.json` flag becomes a concern: if `include_config` is true, the subprocess looks for `project.json` in its CWD — which is now the materialized dir, not the user's project. **This needs careful handling**: the subprocess wants the USER's project.json for import-map resolution (the V-08 path), but the new CWD doesn't have one. Options:

- Pass `--config /absolute/path/to/user/project.json` (explicit path before changing cwd).
- Leave the subprocess's CWD as user's project but use absolute paths for the embedded imports (Q7 Option B).
- Compose a synthesized project.json in the materialized dir that includes only what the embedded JS needs.

**This is the subtle CC must surface in diagnosis.** It's the next-most-likely surprise after the basic materialization works. The Q7-A choice doesn't fully eliminate the need to think about `project.json` discovery.

### CC's diagnosis MUST address `project.json` discovery

When the subprocess's CWD changes, the `--config project.json` flag's behavior changes. The diagnosis section in Turn 1 MUST explain how `project.json` discovery works post-fix. If the answer requires extending Q7 (e.g., passing an absolute path to the user's project.json explicitly), surface it before implementation.

---

## Mycelium acceptance gate (per Duncan)

The empirical closure gate is **mycelium running `lykn compile` against a source with `(import-macros "jsr:..." ...)` succeeding outside the lykn workspace**.

**Test fixture:** the mycelium repo at `/Users/oubiwann/lab/lykn/mycelium/` already exists. After DD-54 lands, running `lykn compile` from `mycelium/` against any source using a JSR import-macros (after Finding D's exports field fix lands, which we already did) should:

1. Succeed at the compilation step.
2. Surface-macros from `@lykn/testing` should expand correctly.
3. The cache directory `~/.cache/lykn/macros/{key}/` should have both `mod.lykn` and `macros.js`.
4. The materialized directory `~/.cache/lykn/embedded/<version>/packages/lang/` should exist and contain the embedded files.

The closing report's empirical gate MUST include this mycelium-actually-works test. Not just a synthetic temp-dir test — the real downstream consumer.

---

## Iteration estimate

**2 iterations.** Iter 1: embedding + materialization + subprocess CWD change. Iter 2: tests + mycelium acceptance gate + closing report.

(Procedural note per DD-53 learnings: two-turn structure applies. Turn 1 = diagnosis with project.json-discovery question resolved. Turn 2 = implementation.)

---

## Risk profile

- **Moderate.** The change touches build-time embedding + runtime initialization + subprocess spawn. Three layers, all small individually.
- **Pre-existing constraint (subprocess CWD-relative imports) is being remediated, not introduced.** This isn't new architecture; it's making existing architecture work for non-monorepo consumers.
- **Cross-platform considerations:** `include_dir!` is well-tested cross-platform. `std::path::PathBuf` handles separators. Should be straightforward.
- **`--config project.json` interaction:** the subtle surprise CC must surface in diagnosis. If the project.json discovery requires a more invasive change than just `current_dir()`, scope amends.

---

## Test gates (closing-report requirements)

1. **All prior tests pass.** No regression in cargo test or `lykn build && lykn test` baselines.
2. **DD-53 R-5 R3 test still passes** — with the symlink workaround removed. After DD-54, the symlink is no longer needed because the binary self-materializes.
3. **Mycelium acceptance gate** (per Duncan):
   - Run `lykn compile` from `/Users/oubiwann/lab/lykn/mycelium/` (or a fresh temp dir, after the project.json registry-pinned flip) against a source using JSR import-macros.
   - Verify it succeeds and produces the expected macro expansion.
   - Verify both `~/.cache/lykn/embedded/<version>/packages/lang/` and `~/.cache/lykn/macros/{key}/` are populated.
4. **DD-50.7 mycelium regression** (`verify-finding-e-2026-05-12.sh`): ✓ ALL CHECKS PASSED.
5. **Binary self-containment test:** delete `~/.cache/lykn/embedded/`; run `lykn compile` from outside the workspace; verify the materialization happens and the compile succeeds.

---

## What this DD does NOT cover

- **In-memory subprocess loading** (data:/Blob URLs). Memory'd as Phase 3 candidate.
- **Embedding `packages/testing/` or `packages/browser/`.** Different consumers; not in subprocess scope.
- **`lykn cache clean` subcommand.** 0.7.0 candidate.
- **The architectural finding's broader cleanup** (the surface-form-handler `Value` override from DD-50.7 fast-follow #2). Compiler-coherence territory.
- **Custom Cargo install scripts** to materialize at install time. The lazy-on-first-spawn approach (Q2) makes this unnecessary.

---

## Discussion points for Duncan

The four substantive calls (Q1, Q2, Q3, Q4) and the four small calls (Q5, Q6, Q7, Q8) above. Each has a proposed default; flag any you'd call differently.

**The hidden substantive item:** Q7 doesn't fully resolve the `project.json` discovery question. CC's Turn 1 diagnosis MUST surface and address this. If it turns out we need more than `current_dir()`, the scope amends pre-implementation (per the DD-53 R3 protocol).

If you accept the defaults, I'll write the implementation prompt next. Same shape as DD-53 R3: two-turn structure, mandatory call-path tracing, pre-solved obstacles, explicit forbidden patterns, mycelium acceptance gate as the load-bearing closure check.
