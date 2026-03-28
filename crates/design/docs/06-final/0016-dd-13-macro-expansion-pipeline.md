---
number: 16
title: "DD-13: Macro Expansion Pipeline"
author: "the time"
component: All
tags: [change-me]
created: 2026-03-26
updated: 2026-03-27
state: Final
supersedes: null
superseded-by: null
version: 1.0
---

# DD-13: Macro Expansion Pipeline

**Status**: Decided
**Date**: 2026-03-26
**Session**: v0.2.0 macro system design, conversation 4

## Summary

The macro expansion pipeline uses a three-pass architecture (import → compile → expand) inserted between the reader and compiler. Expansion is recursive, top-down, fixed-point per node, driven by a dispatch table. Macro definitions within a file are order-independent via iterative fixed-point compilation. DD-12 is amended (v1.2) to use `#a(...)` / `#o(...)` dispatch syntax for consistent collection-type symmetry.

## Decisions

### Three-pass pipeline architecture

**Decision**: The expander runs three sequential passes over the top-level forms of a file. Each pass has a single responsibility.

```
reader output (s-expressions)
    │
    ▼
  Pass 0 — Process `import-macros` (load external macro modules)
    │
    ▼
  Pass 1 — Compile and register file-local `macro` definitions
    │
    ▼
  Pass 2 — Expand all remaining forms
    │
    ▼
compiler input (expanded s-expressions, macro-free)
```

- **Pass 0**: scans top-level forms for `import-macros`. Loads and compiles external macro modules, registers their macros. All other forms are untouched.
- **Pass 1**: scans top-level forms for `macro`. Compiles and registers all file-local macros. Order-independent (see "Iterative fixed-point macro compilation" below). All `macro` forms are erased from the output.
- **Pass 2**: walks all remaining forms and performs full expansion — macro calls, sugar form desugaring, quasiquote resolution, `as` pattern desugaring. Output is pure core forms ready for the compiler.

**Rationale**: Single-responsibility passes are easy to reason about and debug. By the time pass 2 runs, the macro environment is fully populated and frozen — no ordering surprises.

### Recursive top-down expansion walk (Pass 2)

**Decision**: The expansion walk is recursive, top-down, with fixed-point expansion per node. A form whose head is a known macro is expanded, and the result is re-expanded until it is no longer a macro call. Sub-forms are then expanded recursively.

```
expand(form, env):
  if form is atom → return form
  if form is empty list → return form

  head = first(form)
  strategy = dispatchTable[head] ?? "expand-all"

  dispatch on strategy:
    "none"           → return form unchanged
    "register-macro" → error (macros should be processed in pass 1)
    "desugar"        → apply transform, re-expand result
    "macro"          → call macro function, re-expand result (fixed-point)
    "expand-all"     → recur into all sub-forms (default)
```

The fixed-point is per-node — the expander does not re-walk the entire file after each expansion. A macro can expand into a call to another macro; the re-expansion loop handles this naturally.

**Rationale**: This is the standard Lisp expansion model. Fixed-point per node gives full macro composability (macro A can expand to a call to macro B). Top-down ensures macros see unexpanded arguments, which is the expected behavior.

### Table-driven dispatch

**Decision**: The expander consults a dispatch table mapping form head symbols to walk strategies. Unknown heads use the default strategy ("expand all sub-forms"). The table starts minimal and can grow.

```javascript
const dispatchTable = {
  // Don't recur
  "quote":          { walk: "none" },

  // Should already be processed — error if seen in pass 2
  "macro":          { walk: "register-macro" },

  // DD-12 sugar forms — desugar and re-expand
  "cons":           { walk: "desugar", transform: desugarCons },
  "list":           { walk: "desugar", transform: desugarList },
  "car":            { walk: "desugar", transform: desugarCar },
  "cdr":            { walk: "desugar", transform: desugarCdr },
  "cadr":           { walk: "desugar", transform: desugarCadr },
  "cddr":           { walk: "desugar", transform: desugarCddr },

  // Pattern desugaring
  "as":             { walk: "desugar", transform: desugarAs },

  // Debug utilities — expand, print to stderr, erase
  "macroexpand":    { walk: "debug-expand", mode: "full" },
  "macroexpand-1":  { walk: "debug-expand", mode: "once" },
};
```

**Rationale**: Table-driven dispatch separates data from logic. Adding a new special case means adding one table entry and possibly one handler function, without touching the core walk. Scales to 100+ entries if needed, but starts with ~12.

### Minimal special-casing

**Decision**: The expander starts with the minimal set of table entries listed above. Most core forms (`if`, `const`, `function`, `import`, etc.) do not need special handling — the default "expand all sub-forms" walk is correct for them because atoms in non-expandable positions (binding names, parameter names) pass through expansion unchanged.

**Rationale**: S-expressions make this possible. An atom that isn't a macro name returns itself from `expand()`. The compiler determines what each position means (binding vs. expression vs. key). Additional entries can be added if specific forms require special treatment, but the design doesn't presume they will.

### Quasiquote: uniform treatment regardless of context

**Decision**: The expander handles quasiquote the same way in all contexts — expression position, pattern/binding position, macro body, top level. Quasiquote resolves to plain forms (`array`, `append`, symbols), and the compiler determines semantics based on where those forms appear.

```lisp
;; In a macro body — quasiquote builds a template (construction)
(macro when (test (rest body))
  `(if ,test (do ,@body)))

;; In a binding position — quasiquote describes a shape (destructuring)
(const `(,head . ,tail) (list 1 2 3))
;; expander produces: (const (array head tail) (array 1 (array 2 (array 3 null))))
;; compiler sees array destructuring, emits:
```

```javascript
const [head, tail] = [1, [2, [3, null]]];
```

**Rationale**: Construction and destructuring are structural inverses. The quasiquote form `(,a . ,b)` maps to `(array a b)` in both cases. Whether `a` and `b` are filled in (expression) or bound (pattern) depends on context the compiler already understands. No special-casing needed in the expander.

### `as` desugaring

**Decision**: `as` has two desugaring modes depending on its arguments.

**Simple rename** — desugars to `alias` (the core form):

```lisp
;; lykn input
(import (as some-long-module slm) "./module.js")
;; after expansion
(import (alias some-long-module slm) "./module.js")
```

**Whole-and-destructure** — expands to two bindings (one-to-many):

```lisp
;; lykn input
(const (as (object (name n) (age a)) whole) person)
;; after expansion (two forms)
(const whole person)
(const (object (name n) (age a)) whole)
```

The distinction: if the first argument to `as` is an atom, it's a simple rename (→ `alias`). If it's a destructuring pattern (a list), it's whole-and-destructure (→ two bindings).

**Rationale**: Simple rename maps directly to the existing `alias` core form. Whole-and-destructure requires structural expansion that `alias` doesn't support — cleaner to expand it in the expander than to overload `alias`.

### Iterative fixed-point macro compilation (Pass 1)

**Decision**: Pass 1 compiles macro definitions in dependency order using an iterative fixed-point algorithm. Macros can call other macros at compile time without requiring any particular source order.

```
pass 1:
  pending = all macro forms from file
  max_passes = pending.length
  pass_count = 0

  loop:
    pass_count++
    if pass_count > max_passes → error

    progress = false
    still_pending = []

    for each macro in pending:
      deps = symbols in body that match other pending macro names
      if no deps:
        compile and register macro
        progress = true
      else:
        still_pending.push(macro)

    pending = still_pending
    if pending is empty → done
    if not progress → error (circular dependency)
```

```lisp
;; Order doesn't matter — both macros compile successfully

(unless (= x 0)
  (console:log "nonzero"))

(macro unless (test (rest body))
  `(when (not ,test) ,@body))

(macro when (test (rest body))
  `(if ,test (do ,@body)))
```

**Rationale**: Sequential (order-dependent) macro compilation is fragile — it forces dependency ordering in source files, which is a pain in practice (Clojure suffers from this). Erlang's multi-pass compiler avoids this problem. The iterative fixed-point approach is simple, handles arbitrary dependency depths, and naturally detects circular dependencies.

### Safety limits

**Decision**: Two safety limits prevent runaway expansion.

| Limit | Value | Scope | Triggers on |
|-------|-------|-------|-------------|
| Per-node expansion limit | 1000 | Pass 2, per form | Infinite macro expansion |
| Macro compilation passes | N (number of macro defs) | Pass 1, per file | Circular dependencies |

The pass 1 progress check (hard stop if a pass makes no progress) catches circular dependencies before the pass limit is reached.

**Rationale**: 1000 is a generous starting point for per-node expansion — legitimate macros should converge quickly. The pass 1 limit of N is the theoretical maximum (linear dependency chain where each pass resolves exactly one macro).

### Duplicate macro names

**Decision**: Defining two macros with the same name in a single file is a hard error.

```lisp
;; Error: duplicate macro definition: 'when'
(macro when (test body) `(if ,test ,body))
(macro when (test (rest body)) `(if ,test (do ,@body)))
```

**Rationale**: Two definitions of the same name is almost certainly a mistake. Last-definition-wins would silently discard work. Explicit error catches the problem immediately.

### `macroexpand` and `macroexpand-1` debugging utilities

**Decision**: Two debug forms and two CLI flags for macro expansion inspection.

**In-file forms** — expansion-time only, erased from output:

```lisp
;; One expansion step — prints result to stderr
(macroexpand-1 '(unless (= x 0) (console:log "nonzero")))
;; stderr: (when (not (= x 0)) (console:log "nonzero"))

;; Full fixed-point expansion — prints result to stderr
(macroexpand '(unless (= x 0) (console:log "nonzero")))
;; stderr: (if (not (= x 0)) (do (console:log "nonzero")))
```

Argument must be quoted (`'(...)`) to prevent the expander from expanding the form before the debug utility sees it.

**CLI flags**:

```bash
lykn compile --expand-1 src/app.lykn   # one-step expansion for all macro calls
lykn compile --expand src/app.lykn     # fully expanded s-expression tree
```

Output destination defaults to stderr; configurable via additional flags.

**Rationale**: Traditional Lisp names (`macroexpand`, `macroexpand-1`) are well-known and used in LFE. In-file forms give targeted inspection of specific calls. CLI flags give a file-level view. Both are needed for effective debugging.

### DD-12 v1.2 amendment: `#a(...)` and `#o(...)` dispatch syntax

**Decision**: The `#` dispatch table is amended to use letter-prefixed syntax for all collection types.

| DD-12 v1.1 | DD-12 v1.2 | Meaning |
|-------------|------------|---------|
| `#(...)` → `(array ...)` | `#a(...)` → `(array ...)` | Array literal |
| `#s(...)` → `(object ...)` | `#o(...)` → `(object ...)` | Object literal |
| — | `#(...)` → **error** | Unassigned, reserved |

```lisp
;; Array literal
#a(1 2 3)
;; reader emits: (array 1 2 3)
```

```javascript
[1, 2, 3]
```

```lisp
;; Object literal
#o((name "Duncan") (age 42))
;; reader emits: (object (name "Duncan") (age 42))
```

```javascript
({ name: "Duncan", age: 42 })
```

```lisp
;; Quasiquoted array
`#a(1 2 ,x)
;; expander produces: (array 1 2 x)
```

```javascript
[1, 2, x]
```

```lisp
;; Quasiquoted object
`#o((name ,n) (age ,a))
;; expander produces: (object (name n) (age a))
```

The remaining dispatch entries are unchanged:

| Dispatch | Meaning |
|----------|---------|
| `#;` | Expression comment |
| `#NNr` | Radix literal |
| `#\|...\|#` | Nestable block comment |

**Rationale**: `#(...)` without a letter creates broken symmetry when more collection types are added (tuples, sets, maps, vectors). LFE's experience confirms this — `#(...)` was used for tuples, and later collection types had to use `#X(...)` anyway, creating inconsistency. Starting with `#X(...)` for all collection types prevents this. `#s(...)` freed for future use (sets, structs). `#o` for object is more obvious than `#s`.

### Implicit literal keys in object patterns

**Decision**: Key positions in object destructuring patterns are implicitly literal. No quote is needed.

```lisp
;; Keys are literal — no quote needed
(const (object (name n) (age a)) person)
```

```javascript
const { name: n, age: a } = person;
```

```lisp
;; Shorthand — bare atom is both key and binding
(const (object name age) person)
```

```javascript
const { name, age } = person;
```

```lisp
;; Computed key — use `get` wrapper
(const (object ((get some-var) val)) person)
```

```javascript
const { [someVar]: val } = person;
```

The structural position determines the distinction: atom in key position → literal, list with `get` head in key position → computed. Quasiquoted object patterns use `,` to mark variable positions, which is independently unambiguous.

**Rationale**: The structure of object patterns already encodes the literal/variable distinction. Adding explicit quotes would be redundant and noisy. Three cases are cleanly distinguished by form: bare atom (shorthand), pair with atom key (rename), pair with list key (computed).

## Rejected Alternatives

### Sequential (order-dependent) macro compilation

**What**: Process macro definitions in source order. A macro at line N is available to all forms from line N+1 onward.

**Why rejected**: Fragile — forces developers to organize macros in dependency order. Clojure suffers from this. The iterative fixed-point approach eliminates ordering constraints at minimal implementation cost.

### Single-pass expansion (macros can only expand to core forms)

**What**: Macros expand once; the result is not re-expanded.

**Why rejected**: Too restrictive. Macro composability requires that macro A can expand to a call to macro B. Fixed-point per node is the standard Lisp model and essential for practical macro use.

### Expander carries full special-form knowledge

**What**: The dispatch table includes entries for every core form (`if`, `const`, `function`, `let`, `import`, etc.) specifying exactly which sub-positions to expand.

**Why rejected**: Unnecessary. The default "expand all sub-forms" walk is correct for most forms because atoms in non-expandable positions (binding names, parameters) pass through expansion unchanged. Only forms that genuinely need special treatment (`quote`, `macro`, sugar forms) go in the table. The table-driven design scales to 100+ entries if this assumption proves wrong later.

### Context-dependent quasiquote handling

**What**: The expander treats quasiquote differently in pattern position vs. expression position.

**Why rejected**: Quasiquote produces the same structural output regardless of context. `(,a . ,b)` → `(array a b)` whether constructing or destructuring. The compiler already determines semantics from position. Adding context awareness to the expander would be complexity for no benefit.

### `#(...)` for array literals (DD-12 v1.1)

**What**: Bare `#(...)` without a letter prefix for array literals.

**Why rejected**: Breaks symmetry when additional collection types are added. LFE's experience shows this — `#(...)` was used for tuples, forcing later collection types into `#X(...)` patterns. Starting with `#a(...)` ensures uniform `#X(...)` syntax from the beginning. See DD-12 v1.2 amendment.

### `#s(...)` for object literals (DD-12 v1.1)

**What**: `#s` prefix for object literals.

**Why rejected**: `#o` for object is more intuitive. Freeing `#s` keeps it available for future use (sets, structs).

### Last-definition-wins for duplicate macros

**What**: When two macros share a name, silently use the last definition.

**Why rejected**: Almost certainly a mistake. Silent override discards work and makes debugging harder. Hard error catches the problem immediately.

### Overloading `alias` for whole-and-destructure

**What**: `(alias (object ...) whole)` where the first argument is a destructuring pattern.

**Why rejected**: `alias` is a simple rename form — `(alias source target)` where both are atoms. Overloading it with structural destructuring semantics would be confusing. The `as` desugaring handles this cleanly by emitting two separate bindings.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| Macro expands to another macro call | Re-expanded via fixed-point loop | `(unless ...)` → `(when ...)` → `(if ...)` |
| Macro expands to sugar form | Re-expanded (sugar forms are in dispatch table) | macro returns `(cons x y)` → `(array x y)` |
| Expansion limit reached | Hard error with form and limit shown | `"expansion limit (1000) exceeded expanding '(bad-macro ...)'"`  |
| `macro` form encountered in pass 2 | Hard error (should be processed in pass 1) | `"unexpected macro definition in expansion pass"` |
| Circular macro dependency | Detected by progress check in pass 1 | `"circular macro dependency: 'a' needs 'b', 'b' needs 'a'"` |
| `macroexpand` of non-macro form | Prints form unchanged (no expansion to do) | `(macroexpand '(+ 1 2))` → stderr: `(+ 1 2)` |
| `as` with atom first arg | Simple rename → `alias` | `(as foo bar)` → `(alias foo bar)` |
| `as` with list first arg | Whole-and-destructure → two bindings | `(as (object ...) w)` → two `const` forms |
| `#(...)` in source | Reader error with helpful message | `"use #a(...) for array literals"` |
| One-to-many expansion (e.g., `as`) | Expander returns multiple forms; parent collects them | Context-dependent splicing into surrounding form list |

## Dependencies

- **Depends on**: DD-01 (colon syntax, camelCase), DD-06 (destructuring, `alias`, `object`/`array` patterns), DD-10 (quasiquote/unquote algorithm), DD-11 (macro definition, `as` form, hygiene, compile-time eval), DD-12 (sugar forms, dispatch table — amended to v1.2 by this document)
- **Affects**: DD-12 (v1.2 amendment: `#a`/`#o` syntax, implicit literal keys), DD-14 (macro modules — pass 0 defined here, details in DD-14)

## Open Questions

- [ ] Source location tracking through expansion (deferred — no definite milestone for source maps yet)
- [ ] Shadowing of imported macros by file-local macros: error or allowed? (deferred to DD-14)
- [ ] CLI flag syntax for `--expand` output destination (implementation detail)
- [ ] `let` form (not yet defined) — destructuring patterns in `let` bindings will follow the same expansion model as `const`
