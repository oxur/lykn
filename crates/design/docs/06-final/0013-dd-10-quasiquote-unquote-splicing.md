---
number: 13
title: "DD-10: Quasiquote / Unquote / Splicing"
author: "Duncan McGreggor"
component: All
tags: [change-me]
created: 2026-03-26
updated: 2026-03-27
state: Final
supersedes: null
superseded-by: null
version: 1.1
---


# DD-10: Quasiquote / Unquote / Splicing

**Status**: Decided
**Date**: 2026-03-26
**Session**: v0.2.0 macro system design — topic 1 of 5

## Summary

Quasiquote (`` ` ``), unquote (`,`), and unquote-splicing (`,@`) are
expansion-time forms in the sugar language. They are fully resolved during
macro expansion and never reach the compiler. The compiler only receives
core forms (DD-01 through DD-09). Expansion follows Bawden's algorithm
using `append` and `array` over internal AST nodes.

## Decisions

### Core forms vs sugar language

**Decision**: v0.1.0 forms (DD-01 through DD-09) constitute lykn's "core
forms." All macro expansion, quasiquote resolution, reader macros, and
literal forms reduce to core forms before the compiler sees them. The
sugar language — which includes `quote`, `quasiquote`, `unquote`,
`unquote-splicing`, and all user-defined macros — is what most users
write. Core forms are the compilation target, not the authoring surface.

**Rationale**: Clean separation of concerns. The compiler is the only
component that knows about JavaScript. The expansion pass is the only
component that knows about macros and quasiquote. Neither needs to
understand the other's domain.

### Quote and quasiquote are expansion-time only

**Decision**: `quote`, `quasiquote`, `unquote`, and `unquote-splicing`
are not core forms. They are resolved entirely during macro expansion.
If any of these forms reaches the compiler, it is a compile error.
There is no runtime representation of quoted data in emitted JS.

**Syntax**:

```lisp
;; Inside a macro body — resolved at expansion time
`(if ,test (do ,@body))

;; The compiler never sees quasiquote.
;; It receives the expanded core forms, e.g.:
(if someCondition (do (console:log "a") (console:log "b")))
```

**ESTree nodes**: None — these forms produce no ESTree nodes directly.
They produce s-expression AST nodes (arrays, symbols, numbers, strings)
that subsequently compile to whatever ESTree nodes the core forms require.

**Rationale**: No runtime representation of quoted data is needed. Reader
macros and literal forms also expand at read/expansion time into core
forms. This preserves the no-runtime-dependency principle.

### Reader desugaring

**Decision**: The reader mechanically wraps shorthand characters into
long-form s-expressions. No validation at the reader level — the reader
is dumb. Validation (e.g., unquote outside quasiquote) happens in the
expansion pass.

| Character(s) | Reader expansion | Example |
|---|---|---|
| `` `expr `` | `(quasiquote expr)` | `` `(a b) `` → `(quasiquote (a b))` |
| `,expr` | `(unquote expr)` | `,x` → `(unquote x)` |
| `,@expr` | `(unquote-splicing expr)` | `,@xs` → `(unquote-splicing xs)` |

**Rationale**: Keeping the reader dumb simplifies implementation. The
expansion pass already needs to understand quasiquote semantics for
Bawden's algorithm, so it is the natural place for validation. Source
location metadata (tracked from the reader) enables good error messages
at the expansion level. Full source-map tracing (reader → expansion →
compiler) can be layered on later without design changes.

### Bawden's algorithm for expansion

**Decision**: Quasiquote expansion uses Bawden's `append`/`array` algorithm
(Bawden 1999, "Quasiquotation in Lisp", PEPM). The expansion operates on
internal AST nodes (the same symbol, array, number, and string structures
the reader produces). The macro environment API functions (`array`, `sym`,
`append`, `quote`, etc.) construct these AST nodes.

**Element expansion rules** — each element of a quasiquoted list produces
one argument to `append`:

| Element pattern | Expansion | Notes |
|---|---|---|
| Literal `x` | `(array (quote x))` | Quoted, wrapped in single-element array |
| Unquote `,x` | `(array x)` | Evaluated, wrapped in single-element array |
| Splice `,@x` | `x` | Evaluated, bare — must be an array |

The quasiquoted list becomes `(append <arg1> <arg2> ... <argN>)`.

**Atom expansion** — a quasiquoted atom (not an array) is simply quoted:

| Input | Expansion |
|---|---|
| `` `foo `` | `(quote foo)` |
| `` `42 `` | `(quote 42)` — self-evaluating |
| `` `"hello" `` | `(quote "hello")` — self-evaluating |

**Full examples**:

```lisp
;; Case 1: No unquotes
`(if true (console:log "yes"))
;; →
(array (quote if) (quote true) (array (quote console:log) (quote "yes")))

;; Case 2: Unquote
`(if ,test (console:log "yes"))
;; →
(append
  (array (quote if))
  (array test)
  (array (array (quote console:log) (quote "yes"))))

;; Case 3: Splice
`(if ,test ,@body)
;; →
(append
  (array (quote if))
  (array test)
  body)

;; Case 4: Mixed, nested lists
`(let ((,name ,value)) ,@body)
;; →
(append
  (array (quote let))
  (array (append
    (array (append
      (array name)
      (array value)))))
  body)
```

**ESTree nodes**: None — expansion produces internal AST nodes, not ESTree.

**Rationale**: Bawden's algorithm is the standard, handles nested
quasiquote correctly, and uses only `append` and `array` (never `cons`).
The uniform `append`/`array` structure is what makes splicing work —
splice is just a different wrapping of the same pattern.

**Note on terminology**: `array` here refers to the macro environment
API function that constructs internal AST nodes (flat JS arrays). This
is distinct from the user-facing `(array ...)` core form that constructs
JS arrays in compiled output, and from the user-facing `(list ...)` form
(DD-12) that constructs cons-cell data structures. All three use JS
arrays under the hood, but at different levels: the macro environment
API's `array` builds AST nodes for the expander; the `array` core form
builds runtime JS arrays; `list` builds nested two-element arrays
representing cons cells. See DD-12 for the full distinction.

### Nested quasiquote (depth tracking)

**Decision**: A depth counter tracks quasiquote nesting. `` ` ``
increments depth; `,` decrements depth. Unquoting only fires at
depth 0. At depth > 0, `quasiquote` and `unquote` forms are preserved
as literal data in the output.

**Derivation** — `` ``(a ,,b) ``:

The reader produces (processing left to right):

```lisp
(quasiquote (quasiquote (a (unquote (unquote b)))))
```

The expander processes the outer `quasiquote` at depth 0:
1. Sees inner `(quasiquote ...)` — depth increments to 1
2. Walks `(a (unquote (unquote b)))` at depth 1
3. `a` at depth 1 → literal, produces `(quote a)`
4. `(unquote (unquote b))` — outer unquote at depth 1 decrements to
   depth 0, so it fires: evaluate `(unquote b)`, which at depth 0
   evaluates `b`

Result:

```lisp
(array (quote quasiquote)
  (append
    (array (quote a))
    (array (array (quote unquote) b))))
```

This constructs the s-expression `(quasiquote (a (unquote <value-of-b>)))`.
When this result is later expanded (e.g., in a macro-writing macro), the
inner quasiquote resolves and `<value-of-b>` gets spliced in.

**Rationale**: Correct nested quasiquote is required for macro-writing
macros. The depth counter approach is Bawden's standard algorithm and
handles arbitrary nesting levels.

### Optimization: trivial cases

**Decision**: The expander may optimize common cases. These are
semantically transparent optimizations, not behavioral changes.

```lisp
;; No unquotes at all — can return quoted structure directly
`(if true (console:log "yes"))
;; Optimized: return the literal AST array node

;; No splices — can use array instead of append
`(if ,test ,body)
;; Instead of: (append (array (quote if)) (array test) (array body))
;; Optimized:  (array (quote if) test body)
```

**Rationale**: Most quasiquote usage is simple templates without splicing.
The optimization avoids unnecessary `append` calls in the common case.

## Rejected Alternatives

### Runtime representation of quoted data

**What**: Have `quote` and quasiquote produce JS values at runtime —
symbols as strings, lists as arrays, or symbols as `Symbol.for()`.

**Why rejected**: No use case requires quoted data to exist at runtime.
Reader macros and literal forms expand at read/expansion time. Introducing
runtime representation would either require a runtime library (violating
no-runtime principle) or lose type distinction (symbols indistinguishable
from strings, quoted lists from arrays).

### Reader-level validation of unquote

**What**: Have the reader track quasiquote depth and reject `,` or `,@`
outside `` ` `` at read time.

**Why rejected**: Adds complexity to the reader for no benefit. The
expansion pass must understand quasiquote semantics anyway (for Bawden's
algorithm), so it is the natural validation point. Source location metadata
from the reader enables good error messages at the expansion level. A
dumb reader is simpler and doesn't preclude future source-map tracing
through all transformation stages.

### Quote as a compiler core form

**What**: Have the compiler understand `quote` and emit some JS
representation.

**Why rejected**: Would require deciding what quoted data looks like in JS
output. No runtime representation is needed since all quote/quasiquote
usage resolves at expansion time. Keeping `quote` out of core forms
maintains the clean separation: expansion pass handles Lisp semantics,
compiler handles JS semantics.

### `cons`-based expansion

**What**: Use `cons` and `list*` in the expansion output, as in some
traditional Lisp implementations.

**Why rejected**: The `append`/`array` formulation from Bawden handles
all cases and maps naturally to JS array operations. Although DD-12
introduces `cons`, `list`, and dotted-pair syntax as user-facing data
structure forms, these are for constructing cons-cell data (nested
two-element arrays) — a different purpose than the macro environment
API's flat-array AST construction. The expansion algorithm does not
need cons cells; `append`/`array` is sufficient and simpler.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| Unquote outside quasiquote | Expansion error | `,foo` → error: "unquote outside of quasiquote" |
| Splice outside quasiquote | Expansion error | `,@foo` → error: "unquote-splicing outside of quasiquote" |
| Splice as direct child of quasiquote (not in list) | Expansion error | `` `,@foo `` → error: "unquote-splicing not inside a list" |
| Quote/quasiquote reaching compiler | Compile error | Safety net — expansion pass failed to resolve |
| Nested quasiquote | Depth tracking per Bawden | `` ``(a ,,b) `` preserves outer quasiquote, evaluates `b` |
| Colon syntax inside quasiquote | Preserved as symbol | `` `(console:log ,x) `` — `console:log` is a symbol; compiler handles splitting |
| Numbers and strings in quasiquote | Self-evaluating | `` `(foo 42 "bar") `` — returned as-is |
| Empty list in quasiquote | Valid | `` `() `` → empty array AST node |

## Dependencies

- **Depends on**: DD-01 through DD-09 (core forms that expansion targets),
  reader character reservations (`` ` ``, `,`, `#` reserved in v0.1.0 reader)
- **Affects**: DD-11 (`macro` — macros use quasiquote to build templates),
  DD-12 (`#` reader dispatch — reader macros may use quasiquote internally),
  DD-13 (macro expansion pipeline — quasiquote resolution is part of the
  expansion pass), DD-14 (macro modules — module macros use quasiquote)

## Open Questions

None.

## Version History

### v1.1 — 2026-03-26 (DD-12 amendment)

**Reason**: DD-12 introduces `cons`, `list`, `car`/`cdr`/`cadr`/`cddr`,
and dotted-pair syntax as user-facing data structure forms. The macro
environment API function previously called `list` is renamed to `array`
to avoid ambiguity with the new user-facing `(list ...)` form. The
internal AST representation (flat JS arrays) is unchanged.

**Changes**:

| Section | Before | After |
|---------|--------|-------|
| Summary | "using `append` and `list`" | "using `append` and `array`" |
| Bawden's algorithm — decision text | "`append`/`list` algorithm"; API functions "(`list`, `sym`, `append`, `quote`, etc.)" | "`append`/`array` algorithm"; API functions "(`array`, `sym`, `append`, `quote`, etc.)" |
| Element expansion rules table | `(list (quote x))`, `(list x)` | `(array (quote x))`, `(array x)` |
| Full examples (all four cases) | All `(list ...)` calls | All `(array ...)` calls |
| Bawden rationale | "uses only `append` and `list` (never `cons`, which lykn has no use for)" | "uses only `append` and `array` (never `cons`)" |
| Added | — | "Note on terminology" paragraph clarifying `array` (API) vs `array` (core form) vs `list` (DD-12) |
| Nested quasiquote result | `(list (quote quasiquote) ...)` | `(array (quote quasiquote) ...)` |
| Optimization examples | `(list (quote if) test body)` | `(array (quote if) test body)` |
| Rejected: cons-based expansion | "lykn has no `cons` or dotted pairs ... No reason to introduce cons cells." | Updated to acknowledge DD-12 introduces user-facing cons/list/dotted-pairs while affirming expansion algorithm still uses `append`/`array` |
| Edge case: empty list | "empty list AST node" | "empty array AST node" |
| ESTree nodes (quote section) | "lists, symbols, numbers, strings" | "arrays, symbols, numbers, strings" |