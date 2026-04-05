---
number: 28
title: "Fix `lykn compile` Rust CLI Bugs"
author: "Duncan McGreggor"
component: All
tags: [change-me]
created: 2026-04-05
updated: 2026-04-05
state: Draft
supersedes: null
superseded-by: null
version: 1.0
---

# Fix `lykn compile` Rust CLI Bugs

## Context

The `lykn compile` Rust CLI command is wired up and runs end-to-end, but testing against all example files and a wide range of surface/kernel forms reveals several categories of bugs. The most critical is that **string literals lose their quotes** in the compiled output, making essentially all non-trivial programs produce invalid JavaScript.

## Bug Inventory

### BUG 1 — String literals lose quotes (CRITICAL)

**Severity**: Blocks all real usage
**Symptoms**: `(bind x "hello")` compiles to `const x = hello;` instead of `const x = "hello";`
**Root cause**: The JSON serializer (`emitter/json.rs`) maps `SExpr::Atom`, `SExpr::Keyword`, and `SExpr::String` all to plain JSON strings. The bridge's `fromJson()` (`bridge.rs:59`) then treats ALL JSON strings as `{type: "atom"}`. The JS compiler has no way to know which strings were originally string literals.
**Files**: `crates/lykn-lang/src/emitter/json.rs`, `crates/lykn-cli/src/bridge.rs`
**Fix**: Change JSON format to use typed objects (`{type: "string", value: "hello"}` vs `{type: "atom", value: "hello"}`), or use a distinguishing convention (e.g., prefix/wrapper). Update `fromJson()` in the bridge to reconstruct the correct types.
**Affected forms**: Every form that contains string literals — bind, obj, func error messages, template, import paths, regex patterns, switch cases, etc.

### BUG 2 — Prelude types conflict with user-defined types (CRITICAL)

**Severity**: Blocks use of Option/Result (the most common surface patterns)
**Symptoms**: `(type Option (Some :any value) None)` produces `error: duplicate constructor 'Some' (already defined in type 'Option')` — even though the user is defining Option themselves.
**Root cause**: The analyzer's prelude (`analysis/prelude.rs`) pre-registers `Option`, `Some`, `None`, `Result`, `Ok`, `Err`. When the user defines these same types (which the design docs say should shadow the prelude), the type registry rejects them as duplicates.
**Files**: `crates/lykn-lang/src/analysis/` (prelude.rs, type_registry.rs)
**Fix**: Allow user-defined types to shadow prelude types. The design doc (DD-15) explicitly says "Shadowing allowed — local definitions win, lose compiler enhancement."

### BUG 3 — Scope analysis false-positive "unused" warnings (MODERATE)

**Severity**: Noisy but non-blocking
**Symptoms**: Every top-level binding is flagged as unused, even when it's clearly used later in the file. E.g., `(bind greeting "hello") (console:log greeting)` warns that `greeting` is unused.
**Root cause**: The scope tracker doesn't properly track references across top-level forms, or doesn't resolve references to bindings defined in earlier forms.
**Files**: `crates/lykn-lang/src/analysis/scope.rs`
**Fix**: Ensure scope analysis treats the module as a single scope where all top-level bindings are visible to all top-level expressions.

### BUG 4 — User-defined macros fail to expand (MODERATE)

**Severity**: Blocks macro usage through Rust CLI
**Symptoms**: `(macro when (test (rest body)) ...)` produces `macro expansion error: extractParamNames is not a function`
**Root cause**: The Deno subprocess used for macro compilation doesn't have access to the full expander environment. The `extractParamNames` function is defined in `expander.js` but not available in the bridge's Deno eval context.
**Files**: `crates/lykn-lang/src/expander/`
**Fix**: Ensure the macro compilation script imports and exposes all necessary functions from the expander.

### BUG 5 — Template literals have broken string interpolation (MODERATE)

**Severity**: Produces invalid JS
**Symptoms**: `(template "hello " name "!")` compiles to `` `${hello }${name}${!}` `` — the literal string parts are treated as interpolated expressions instead of static template parts.
**Root cause**: Same as BUG 1 — template string parts lose their string type in JSON serialization, so the JS compiler treats them as variable references.
**Files**: Same fix as BUG 1 resolves this.

### BUG 6 — Keyword strings in obj lose quotes (MODERATE)

**Severity**: Produces invalid JS for string values
**Symptoms**: `(obj :name "Duncan")` compiles to `{ name: Duncan }` instead of `{ name: "Duncan" }`.
**Root cause**: Same as BUG 1.

### BUG 7 — `import` form fails in bridge (MODERATE)

**Severity**: Blocks module imports through Rust CLI
**Symptoms**: `(import "mod" (a b))` produces `error: import: first argument must be a module path string`
**Root cause**: Same as BUG 1 — the module path `"mod"` loses its string type and arrives as an atom.

### BUG 8 — `regex` form fails in bridge (MODERATE)

**Severity**: Blocks regex usage through Rust CLI
**Symptoms**: `(regex "^hello" "gi")` produces `error: regex pattern must be a string`
**Root cause**: Same as BUG 1.

### BUG 9 — `if` expression in value position emits invalid JS (MODERATE)

**Severity**: Produces invalid JS
**Symptoms**: `(func abs-val :args (:number x) :returns :number :post (>= ~ 0) :body (if (< x 0) (- 0 x) x))` compiles to `const result__gensym0 = if (x < 0) 0 - x; else x;;` — `if` in value position needs to be a ternary or IIFE.
**Root cause**: The Rust emitter emits `(if ...)` kernel form for the body, but when it's in value position (assigned to a const), the JS compiler generates an `if` statement where an expression is needed.
**Files**: `crates/lykn-lang/src/emitter/forms.rs` (func emission for post-condition bodies)
**Fix**: When the body is an `(if ...)` in value position, emit `(? ...)` (ternary) instead, or wrap in IIFE.

### BUG 10 — `dissoc` IIFE doesn't return value (LOW)

**Severity**: Returns `undefined` instead of the result
**Symptoms**: `(dissoc user :password)` compiles to an IIFE where `rest__gensym1;` is a bare expression statement, not a `return` statement.
**Root cause**: The IIFE body's last expression isn't wrapped with `return`.
**Files**: `crates/lykn-lang/src/emitter/forms.rs` (dissoc emission)
**Fix**: Add `return` before the final expression in the IIFE.

### BUG 11 — `some->` emits `str.toUpperCase()` instead of method call (LOW)

**Severity**: Wrong JS semantics
**Symptoms**: `(some-> user (get :name) (str:to-upper-case))` emits `str.toUpperCase(t__gensym1)` — treating `str:to-upper-case` as a function call on a `str` object, not as a method call on the threaded value.
**Root cause**: The threading macro doesn't handle method-call syntax (colon notation) differently from regular function calls.
**Files**: `crates/lykn-lang/src/emitter/forms.rs` (some-> emission)

### BUG 12 — `switch` case values lose string quotes (LOW)

**Severity**: Produces `case a:` instead of `case "a":`
**Root cause**: Same as BUG 1.

### BUG 13 — Multi-clause func orders clauses wrong (LOW)

**Severity**: Most-specific clause may not match first
**Symptoms**: 2-arg clause checked before 1-arg clause. Should be longest-first or the order the user specified.
**Root cause**: Multi-clause dispatch in the emitter may reorder clauses.
**Files**: `crates/lykn-lang/src/emitter/forms.rs` (emit_func_multi)
**Fix**: Preserve user-specified clause order, or sort by specificity (most args first).

### BUG 14 — UTF-8 em dash renders as `â` (LOW)

**Severity**: Cosmetic corruption for non-ASCII
**Symptoms**: `"Good luck — lykn"` becomes `Good luck â lykn`
**Root cause**: Encoding issue in the bridge's temp file write or Deno subprocess I/O. Likely the temp file is written without explicit UTF-8 encoding, or Deno reads it with wrong encoding.
**Files**: `crates/lykn-cli/src/bridge.rs`
**Fix**: Ensure temp file is written and read as UTF-8.

## Implementation Plan

### Phase 1 — Fix the JSON bridge format (resolves BUGs 1, 5, 6, 7, 8, 12)

This is the highest-leverage fix — one change resolves 6 bugs.

1. **`crates/lykn-lang/src/emitter/json.rs`** — Change `sexpr_to_json()`:
   - `SExpr::Atom` → `{"type": "atom", "value": "..."}`
   - `SExpr::String` → `{"type": "string", "value": "..."}`
   - `SExpr::Keyword` → `{"type": "atom", "value": "..."}` (keywords compile to their string value as an atom, matching JS reader behavior)
   - `SExpr::Number` → `{"type": "number", "value": N}`
   - `SExpr::Bool` → `{"type": "atom", "value": "true"/"false"}`
   - `SExpr::List` → `{"type": "list", "values": [...]}`

2. **`crates/lykn-cli/src/bridge.rs`** — Simplify `fromJson()`:
   - Typed objects pass through directly (already in JS reader format)
   - Only need array→list conversion for any legacy flat arrays

3. **Update tests** in `json.rs` and `bridge.rs`

### Phase 2 — Fix prelude shadowing (resolves BUG 2)

1. **`crates/lykn-lang/src/analysis/type_registry.rs`** — Allow `register_type()` to overwrite prelude-defined types
2. **`crates/lykn-lang/src/analysis/prelude.rs`** — Mark prelude types as shadowable
3. **Test**: `(type Option (Some :any value) None)` should compile without error

### Phase 3 — Fix scope analysis (resolves BUG 3)

1. **`crates/lykn-lang/src/analysis/scope.rs`** — Fix top-level binding visibility
2. Ensure bindings from earlier top-level forms are visible to later ones
3. **Test**: `(bind x 1) (console:log x)` should produce no warnings

### Phase 4 — Fix macro expansion (resolves BUG 4)

1. **`crates/lykn-lang/src/expander/`** — Ensure Deno subprocess has access to `extractParamNames` and other required functions
2. **Test**: `(macro when (test (rest body)) ...)` should expand correctly

### Phase 5 — Fix value-position if, dissoc return, UTF-8 (resolves BUGs 9, 10, 14)

1. **Emitter**: Emit `?` (ternary) instead of `if` when in value position for func post-condition body
2. **Emitter**: Add `return` to dissoc IIFE body
3. **Bridge**: Ensure UTF-8 encoding for temp file I/O

## Verification

After each phase, run:

```sh
# Compile all example files — verify no errors and correct JS output
./target/release/lykn compile examples/surface/main.lykn
./target/release/lykn compile examples/surface/showcase.lykn
./target/release/lykn compile examples/surface/browser-quotes.lykn

# Compare Rust output vs JS output for each file
deno eval "import {lykn} from './src/index.js'; ..."

# Run test suites
cargo test
deno task test
make lint
```

## Critical files

- `crates/lykn-lang/src/emitter/json.rs` — JSON serialization (Phase 1)
- `crates/lykn-cli/src/bridge.rs` — Deno bridge + fromJson (Phase 1)
- `crates/lykn-lang/src/analysis/type_registry.rs` — Type registration (Phase 2)
- `crates/lykn-lang/src/analysis/prelude.rs` — Prelude types (Phase 2)
- `crates/lykn-lang/src/analysis/scope.rs` — Scope tracking (Phase 3)
- `crates/lykn-lang/src/expander/` — Macro expansion (Phase 4)
- `crates/lykn-lang/src/emitter/forms.rs` — Form emission (Phase 5)
