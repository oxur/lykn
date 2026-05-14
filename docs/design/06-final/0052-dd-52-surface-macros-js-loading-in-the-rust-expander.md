---
number: 52
title: "Surface-macros JS-loading in the Rust expander"
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

# DD-52 — Surface-macros JS-loading in the Rust expander

**Status:** Resolved — calls landed 2026-05-12. Ready for implementation.
**Thread:** cdc/dep-ergonomics
**Tracker:** [lykn-lang/lykn#2](https://github.com/lykn-lang/lykn/issues/2)
**Relates to:** DD-34 (cross-package import-macros resolution), DD-48 (V-08 fix), M9-release.

## Resolved design calls

All six design questions resolved per Duncan's calls 2026-05-12 (deltas from proposed defaults: none — all defaults accepted).

| # | Question | Call | Detail in §below |
|---|---|---|---|
| Q1 | Path resolution scope | **A — Local-only** | `(surface-macros ...)` is author-internal; consumer-facing cross-package macro loading routes through `(import-macros ...)`. Q1 §  |
| Q2 | State persistence across compilations | **A — Shared per compilation** | macroEnv carries across surface-macros loads within one compilation; reset between. Q2 §  |
| Q3 | Error model | **A — Fail-fast** | Match JS expander's synchronous throw. Q3 § |
| Q4 | Test discipline | **C — Both synthetic + @lykn/testing** | Synthetic for edge cases (empty file, throwing loader, no-name registration); @lykn/testing covers real-world shape. Q4 § |
| Q5 | Unknown-action fallback | **A — Hard-fail** | Per Principle 3: silently skipping = mysterious downstream undefined-macro errors. Q5 § |
| Q6 | Diagnostic placement | **B — Enriched with source loc** | Rust expander has span info; use it. JS expander gets symmetric enhancement as follow-up. Q6 § |

## Refinement log

- **2026-05-12 (drafting):** Pre-DD inventory written. All six questions surfaced with proposed defaults.
- **2026-05-12 (calls):** Duncan reviewed; accepted all six defaults. Q1's call benefited from CDC's explanation of the conceptual-layer distinction between `(import-macros ...)` (consumer-facing) and `(surface-macros ...)` (author-internal); the framing made A the obvious match.

## Problem

The Rust expander has no handling for `(surface-macros "path.js")` directives in macro modules. The JS expander handles them at `packages/lang/expander.js:1422–1437`. The gap is structurally observable: `grep -rnE "surface[-_]?macros" crates/` returns no matches.

Empirically: when mycelium imports macros via `(import-macros "jsr:@lykn/testing" ...)`, the Rust expander loads `@lykn/testing/mod.lykn`, sees `(surface-macros "macros.js")` (per the testing module's actual content), and fails to load the JS-defined surface macros. The downstream effect is that the testing DSL's macros (`test`, `is-equal`, `is-thrown`, etc.) are unavailable when the Rust expander runs.

Why this doesn't currently block mycelium: per the M5 + M9 context-aware split, `lykn test` routes test compilation through the JS expander, which handles surface-macros natively. The gap only fires when:
- A macro module using `surface-macros` is loaded via `import-macros` during Rust-expander-driven compilation (e.g., `lykn build --dist` or any non-test compile path that hits the macro module).
- A consumer uses `import-macros` from a surface-macros-bearing module in a context the Rust expander handles.

Per Duncan's framing (mycelium is in earliest stages; surface macros will be everywhere in the project eventually): **the gap doesn't bite today, but it will increasingly bite as mycelium and other downstream projects mature.** Closing it before that escalation is the right move.

## Architectural context

### What the JS expander does (canonical behavior)

At `packages/lang/expander.js:1422–1437`:

```javascript
} else if (form.values[0].value === 'surface-macros' &&
           form.values[1].type === 'string') {
  // (surface-macros "macros.js") → load JS companion
  const jsFile = form.values[1].value;
  const jsPath = _resolve(_dirname(resolvedPath), jsFile);
  let jsSource;
  try { jsSource = Deno.readTextFileSync(jsPath); }
  catch { throw new Error(`surface-macros: file not found: ${jsFile}`); }
  const SURFACE_PARAMS = ['macroEnv', 'sym', 'array', 'gensym', /* ... */];
  const SURFACE_VALUES = [macroEnv, sym, array, gensym, /* ... */];
  const beforeKeys = new Set(macroEnv.keys());
  try {
    const loader = new Function(...SURFACE_PARAMS, jsSource);
    loader(...SURFACE_VALUES);
  } catch (err) {
    throw new Error(`surface-macros: failed to load ${jsFile}: ${err.message}`, { cause: err });
  }
  for (const k of macroEnv.keys()) {
    if (!beforeKeys.has(k)) exportedMacroNames.add(k);
  }
}
```

Key mechanics:
- Path resolution relative to the macro module's directory (`_dirname(resolvedPath)`).
- Synchronous read of the JS source text.
- `new Function(...)` constructs an executor with macro-construction primitives as parameters.
- Executor mutates shared `macroEnv` (a Map) via `macroEnv.set('macroName', fn)`.
- Names newly added to `macroEnv` are tracked as exported macros from this module.

### What M9 already gives us (Deno subprocess infrastructure)

`crates/lykn-lang/src/expander/env.rs` runs a long-lived Deno subprocess that the Rust expander uses for:

- **Macro-form expansion** (existing): `new Function(...MACRO_API_PARAMS, request.jsBody)` — constructs and calls macro functions on demand from the Rust side.
- **`resolve-macro-source`** (M9, DD-48): fetches `.lykn` source from JSR/npm cache for `import-macros` resolution.
- **`resolve`**: `import.meta.resolve` to filesystem path.
- **`ping`**: liveness.

The subprocess is **long-lived per compilation** (`continue` loop on stdin). Per-process `macroEnv` state can be maintained across multiple subprocess actions within one compilation run — same shape the JS expander uses.

### Proposed architecture

**Extend the Deno subprocess protocol with a `load-surface-macros` action.** The Rust expander, when parsing a macro module's top-level forms, detects `(surface-macros "path")` and dispatches the new action.

**Why this shape and not embedding a JS engine in Rust:** the Deno subprocess already runs the JS execution machinery (`new Function`, macroEnv mutation, the surface-macros loader pattern). Bridging the gap is wiring the existing capability to a new action name, not adding new capability. Mirrors how M9 added `resolve-macro-source` to the same subprocess.

**Proposed protocol action shape:**

```json
{
  "action": "load-surface-macros",
  "moduleDir": "/path/to/macro/module/dir",
  "jsRelPath": "macros.js"
}
```

Response (success):

```json
{
  "ok": true,
  "result": {
    "registeredNames": ["test", "is-equal", "is-thrown", "..."]
  }
}
```

Response (failure):

```json
{
  "ok": false,
  "error": "surface-macros: file not found: macros.js"
}
```

The subprocess maintains a per-process `macroEnv` Map. After `load-surface-macros`, subsequent macro-form expansion calls (existing `MACRO_API_PARAMS` action) find the registered macros via `macroEnv.get(name)`. State persistence across requests within one compilation falls out of the long-lived process model.

## Design questions for Duncan/CDC

### Q1: Path resolution scope — local-only vs JSR/npm-aware?

The JS expander currently resolves `(surface-macros "path")` paths **relative to the macro module's directory only**. JSR/npm specifiers are not supported in the surface-macros position.

**Options:**

- **A. Local-only (mirror JS).** The Rust expander does the same: `jsRelPath` is always resolved relative to `moduleDir`. JSR/npm specifiers in `(surface-macros ...)` are an error (and remain so on the JS side).
- **B. Support JSR/npm specifiers.** `(surface-macros "jsr:@scope/pkg/macros.js")` would call the existing `resolve` or `resolve-macro-source` action to find the cached file path, then proceed as local.

**Proposed default: A (local-only).** Symmetric with JS. Surface-macros' purpose is to bundle JS macros with a `.lykn` macro module that ships them together as a single package. Loading surface macros from a *different* package is an unusual pattern; if it's needed, the consuming `.lykn` macro module can be the one that imports the package and re-exposes — the multi-step path is preferable to extending surface-macros' resolver.

### Q2: State persistence across compilations

The JS expander resets `macroEnv` between top-level compilations via `resetMacros()`. The Deno subprocess in Rust is long-lived per compilation — so it naturally has fresh state per compilation. But there's a question of **whether one compilation that loads surface-macros A and then loads surface-macros B sees A's macros when loading B**.

**Options:**

- **A. Yes — shared macroEnv per compilation.** Mirrors JS expander behavior. If B's loader reads `macroEnv` to delegate to A's macros (rare but possible), this works.
- **B. No — isolated per `load-surface-macros` call.** Each load gets a fresh macroEnv. Forces independence; might be cleaner.

**Proposed default: A (shared per compilation, reset between compilations).** Matches JS expander behavior; gives surface-macros authors the same composition options.

### Q3: Error model — fail-fast vs collect-and-continue?

If `load-surface-macros` fails (file not found, JS syntax error, runtime throw inside loader), the Rust expander has options for how to handle it.

**Options:**

- **A. Fail-fast.** Raise a `LyknError` immediately; compilation aborts. Matches JS expander (which throws synchronously).
- **B. Collect-and-continue.** Push to diagnostics; continue compilation; surface all surface-macros failures at end. Better for tooling that wants to show multiple errors but adds complexity.

**Proposed default: A (fail-fast).** Matches JS. Surface-macros failures are typically programmer errors that need immediate attention; collect-and-continue offers little value for the added complexity.

### Q4: Test discipline — `compileBoth` coverage strategy

**Options:**

- **A. Pure `compileBoth` regression tests with new fixtures.** Add a small surface-macros fixture module in `test/regression/surface-macros/`; add `compileBoth` tests that import macros from it.
- **B. Use the existing @lykn/testing module as the test fixture.** It already uses `(surface-macros "macros.js")`. Tests that import macros via `(import-macros "../packages/testing" ...)` exercise the full path.
- **C. Both.**

**Proposed default: C.** New small fixtures cover edge cases (empty surface-macros file, surface-macros that throws, surface-macros that registers no names); the existing @lykn/testing module covers the load-bearing real-world shape.

### Q5: Backward compatibility — what if the action isn't recognized?

If a future Deno subprocess version doesn't recognize `load-surface-macros`, the response is `{ ok: false, error: "unknown action: load-surface-macros" }`. The Rust side needs to handle this — either fall back to "skip the surface-macros directive (with warning)" or hard-fail.

**Proposed default: hard-fail.** Per Principle 3 (compiler-owned output quality): silently skipping surface-macros would produce code that compiles successfully but is missing macros, which manifests as "undefined macro" errors later — worse user experience than a clear "Deno subprocess doesn't support load-surface-macros" failure at the compile entry point.

### Q6: Diagnostic placement

When surface-macros fails, what's the user-facing error?

**Options:**

- **A. Mirror JS-side message format**: `surface-macros: file not found: macros.js` and `surface-macros: failed to load macros.js: <err>`.
- **B. Enrich with source location**: include the file path and line number of the `(surface-macros ...)` form.

**Proposed default: B (enriched).** The Rust expander has source-loc information; use it. JS expander can be enhanced symmetrically as a follow-up.

## Iteration estimate

2–3 iterations:

- **Iter 1: Protocol design DD finalized + Deno subprocess action implemented.**
  - Update `env.rs` with `load-surface-macros` action handler.
  - Add `DenoSubprocess::load_surface_macros(moduleDir, jsRelPath)` method.
  - JS-side action handler.
- **Iter 2: Rust pass0 integration + tests.**
  - Detect `(surface-macros "path")` in `pass0.rs::process_single_import` or equivalent.
  - Call new subprocess action.
  - Track registered macro names alongside existing macroForms tracking.
  - Write `compileBoth` tests per Q4 default.
- **Iter 3: Cleanup + edge cases.**
  - Error path coverage.
  - Cross-compiler convergence verification.
  - Closing report.

## Risk profile

- **Lower** than M10 (mechanical mirroring of an existing JS path; one Rust pass change + one protocol action).
- **Cross-cutting awareness** required: must not regress V-08 fix (DD-48); must compose cleanly with the existing macro-form expansion path; must respect philosophy.md Principle 3 (no silent failures).
- **Test fixture availability**: @lykn/testing already uses surface-macros, so real-fixture coverage is achievable without inventing test material.

## What this DD does NOT cover

- Loading surface-macros from packages published to JSR/npm where the JS file is in a sub-path that JSR's exports field doesn't expose. That's downstream of Finding D — covered separately. (If Q1 is decided A, this isn't a concern for DD-52 anyway.)
- Cross-compiler error-message format alignment in general. Tracked as a compiler-coherence finding.
- Macro hygiene refactors. Out of scope.

## Out of scope but worth noting

The DD-52 fix gives the Rust expander first-class surface-macros support. The follow-on M4 (empirical validation) work, when it lands, would exercise this code path against the @lykn/testing module as a published JSR package. That's the right empirical gate for DD-52's closure, paralleling the verify-finding-e style gate DD-50.7 establishes.

## Discussion points for Duncan

The six design questions above are the substantive calls. Each has a proposed default; flag any you'd call differently.

If you accept the defaults, an implementation prompt can be drafted similar to DD-50.6's. The prompt would scope the three iterations above with MUST framing, file pointers, anti-shortcut language, and a closing-report template that includes a surface-macros-specific gate (compile @lykn/testing's mod.lykn through both compilers; assert byte-identical macroEnv state post-load).

If you'd call any differently, the DD updates first, then the implementation prompt.
