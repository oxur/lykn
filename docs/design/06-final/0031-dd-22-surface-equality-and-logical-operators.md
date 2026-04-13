---
number: 31
title: "DD-22: Surface Equality and Logical Operators"
author: "the surface"
component: All
tags: [change-me]
created: 2026-04-12
updated: 2026-04-12
state: Final
supersedes: null
superseded-by: null
version: 1.0
---

# DD-22: Surface Equality and Logical Operators

**Status**: Decided
**Date**: 2026-04-12
**Amends**: DD-15 (Language Architecture)
**Release**: v0.4.0

## Summary

The surface compiler intercepts `=`, `!=`, `and`, `or`, and `not` and
emits safe JS equivalents. `(= a b)` compiles to `a === b` (strict
equality), not `a = b` (assignment). Loose equality is only available
via the `js:eq` escape hatch. This restores the hazard mitigation
designed in DD-15 and documented in the JavaScript Hazard Landscape
research, which the v0.3.0 implementation failed to enforce.

## Problem

In v0.3.0, all operators pass through the surface compiler unmodified
to the kernel. This creates three hazards:

1. **`(= a b)` is assignment, not equality.** Every Lisp programmer
   and most developers from other languages expect `=` to mean equality.
   Writing `(= x 1)` silently assigns `1` to `x` instead of comparing.
   This is the single most dangerous operator confusion in the language.

2. **`==` is available as a bare operator.** The Hazard Landscape
   research (Pradel & Sen, ECOOP 2015) identified non-strict equality
   between different types as the single most prevalent harmful
   coercion. `==` should not be casually available — DD-15 explicitly
   restricted it to the `js:eq` escape hatch.

3. **`and`/`or`/`not` compile as function calls.** `(and x y)` emits
   `and(x, y)` — a runtime function call to an undefined function,
   not `x && y`. Developers from every Lisp dialect expect these to
   be logical operators.

Surface lykn has no legitimate use for raw assignment: `bind` handles
initial binding, `reset!` replaces cell values, `swap!` updates cells
via function, and `assoc`/`dissoc`/`conj` handle immutable data
updates. Assignment is a kernel concern (compiled output for `reset!`,
class constructors, `for` loop counters, etc.), not a surface concern.

## Research basis

From the JavaScript Hazard Landscape research:

- **Pradel & Sen (ECOOP 2015)**: Non-strict equality between different
  types is the #1 harmful coercion pattern across 138.9 million runtime
  events. 269 implicit coercions per 1 explicit conversion — compile-time
  enforcement is essential.

- **BugAID (Hanam et al., FSE 2016)**: Pattern #4 (wrong equality
  operator) is one of 13 pervasive bug patterns mined from 105K commits.
  Directly preventable by syntax/compiler design.

- **Hazard Landscape, Part 4, Tier 1**: "Strict equality only (C). The
  compiler never emits `==`. The form `(= a b)` compiles to `a === b`.
  [...] This eliminates the entire coercion equality table, BugAID
  pattern #4, and the single most prevalent harmful coercion. CoffeeScript
  proved this works and gets adopted."

DD-15 adopted this recommendation. The v0.3.0 implementation did not
enforce it. This DD restores the original design.

## Decisions

### 1. `=` is strict equality in surface syntax

**Decision**: The surface compiler intercepts `(= a b)` and emits
`a === b`. Assignment is not available as a surface operator.

**Syntax**:

```lisp
;; Surface lykn
(= a b)
```

```javascript
// Compiled JS
a === b
```

```lisp
;; Variadic — all elements compared pairwise
(= a b c)
```

```javascript
// Compiled JS
a === b && b === c
```

**ESTree nodes**: `BinaryExpression` (`===`), `LogicalExpression`
(`&&`) for variadic

**Rationale**: Every Lisp dialect uses `=` for equality. Every lykn
surface form for mutation already has a named form (`bind`, `reset!`,
`swap!`). There is zero surface-language need for `=` as assignment.
The kernel `=` (assignment) remains available in kernel syntax for
class bodies, `for` loops, and JS interop.

### 2. `!=` is strict inequality in surface syntax

**Decision**: The surface compiler intercepts `(!= a b)` and emits
`a !== b`.

**Syntax**:

```lisp
(!= a b)
```

```javascript
a !== b
```

**ESTree nodes**: `BinaryExpression` (`!==`)

**Rationale**: Symmetric with `=`. No loose inequality in surface
syntax.

### 3. No loose equality in surface syntax

**Decision**: `==` and `!==` as kernel operators are not intercepted
by the surface compiler — they remain available in kernel syntax. But
surface code should use `=` and `!=` (which emit strict comparisons)
or the `js:eq` escape hatch for the rare `== null` idiom.

The compiler-generated `== null` checks (inside `some->`, `some->>`,
`if-let`, `when-let`) remain unchanged — these are correct and
intentional.

**Syntax**:

```lisp
;; Surface: strict only
(= x y)       ;; → x === y

;; Escape hatch: explicit loose equality (greppable)
(js:eq x null) ;; → x == null

;; Kernel passthrough (not intercepted, not recommended in surface code)
(== x null)    ;; → x == null
```

**Rationale**: `js:eq` is the designed escape hatch from DD-15. Loose
equality via `==` remains technically available through kernel
passthrough (the surface compiler does not block it), but the guides
and SKILL.md should recommend `js:eq` for visibility and auditability.

### 4. `and`, `or`, `not` are logical operators in surface syntax

**Decision**: The surface compiler intercepts `and`, `or`, and `not`
and emits `&&`, `||`, and `!` respectively.

**Syntax**:

```lisp
(and x y)
```

```javascript
x && y
```

```lisp
(or x y)
```

```javascript
x || y
```

```lisp
(not x)
```

```javascript
!x
```

```lisp
;; Variadic
(and a b c d)
```

```javascript
a && b && c && d
```

```lisp
;; Variadic
(or a b c d)
```

```javascript
a || b || c || d
```

**ESTree nodes**: `LogicalExpression` (`&&`, `||`),
`UnaryExpression` (`!`)

**Rationale**: Every Lisp uses `and`/`or`/`not` as logical operators.
The current behavior (compiling to function calls) is a bug, not a
design choice. These are short-circuit operators in JS — function-call
semantics would evaluate both arguments eagerly, changing behavior.
The kernel operators `&&`, `||`, `!` remain available for kernel syntax.

### 5. Surface operator summary

| Surface form | Compiles to | Category |
|---|---|---|
| `(= a b)` | `a === b` | Strict equality |
| `(!= a b)` | `a !== b` | Strict inequality |
| `(and a b)` | `a && b` | Logical AND (short-circuit) |
| `(or a b)` | `a \|\| b` | Logical OR (short-circuit) |
| `(not x)` | `!x` | Logical NOT |

Kernel operators unchanged: `===`, `==`, `!==`, `!=`, `&&`, `||`,
`!`, `=` (assignment), `+=`, `-=`, `++`, `--` all remain available in
kernel syntax.

## Rejected Alternatives

### Keep `=` as assignment, use `===`/`==` for equality

**What**: The v0.3.0 status quo. JS-aligned naming — operators mean
exactly what they mean in JS.

**Why rejected**: Violates every Lisp programmer's expectations.
Creates a silent mutation hazard (the user writes what they think is
a comparison and gets an assignment). Contradicts the Hazard Landscape
research recommendations adopted in DD-15. Surface lykn has no need
for raw assignment — all mutation paths have named forms.

### Add `eq` as a new form, leave `=` as assignment

**What**: Introduce `(eq a b)` for equality, keep `(= a b)` as
assignment.

**Why rejected**: Invents a new name when every Lisp already uses `=`
for equality. Forces developers to learn a non-standard form. Does not
prevent the accidental-assignment hazard — `=` would still silently
assign.

### Block `==` entirely (compile error in surface)

**What**: Make `(== a b)` a compile error in surface code, forcing
use of `=` (strict) or `js:eq` (loose).

**Why rejected**: Unnecessarily restrictive. Kernel passthrough is a
design principle — surface code can always drop to kernel forms. The
guides and linter (v0.3.1+) can warn on `==` usage without making it
a hard error. The surface compiler intercepts `=` and `!=` to provide
safe defaults; it does not need to block kernel operators.

### Use `equal?` or `eq?` (Scheme-style predicate naming)

**What**: `(equal? a b)` for equality, following Scheme's `?` suffix
convention for predicates.

**Why rejected**: lykn already uses `?` suffix for user-defined
predicates (`even?`, `valid?`). Making the core equality operator a
`?`-suffixed name is unnecessarily verbose. `=` is the universal Lisp
equality operator.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| Variadic `=` | Pairwise `&&`-chained strict equality | `(= a b c)` → `a === b && b === c` |
| Variadic `and`/`or` | Left-to-right short-circuit chain | `(and a b c)` → `a && b && c` |
| Single-arg `not` only | `not` is unary; 2+ args is compile error | `(not x)` → `!x` |
| `=` with one arg | Compile error (needs at least 2 operands) | `(= x)` → error |
| `=` in kernel syntax | Unchanged — still assignment | `(= x 1)` in kernel → `x = 1` |
| `and`/`or` in kernel | Unchanged — still function calls if user-defined | Kernel does not intercept |
| Nested `not` | Standard JS double-negation | `(not (not x))` → `!!x` |
| `=` with `null` | Strict comparison | `(= x null)` → `x === null` (NOT `== null`) |
| Null/undefined check | Use `js:eq` or `some->` | `(js:eq x null)` → `x == null` |

## Implementation

These are five surface-to-kernel macro expansions. In the JS surface
compiler (`src/surface.js`), add cases for `=`, `!=`, `and`, `or`,
`not` that emit the corresponding kernel operators. In the Rust surface
compiler, add surface form handlers that emit the correct kernel
s-expressions.

**JS surface compiler** — each is approximately one `case` clause:

- `=` → emit `===` kernel operator (binary or variadic-to-chain)
- `!=` → emit `!==` kernel operator (binary)
- `and` → emit `&&` kernel operator (binary or variadic-to-chain)
- `or` → emit `||` kernel operator (binary or variadic-to-chain)
- `not` → emit `!` kernel operator (unary, enforce arity 1)

**Rust surface compiler** — add five form handlers in the classifier
and emitter. Each transforms the surface AST node to the corresponding
kernel operator node.

**No kernel changes.** The kernel compiler is frozen (DD-15 principle
#1). All changes are in the surface layer.

## Testing

### New test fixtures

Add `test/fixtures/surface/equality.json`:

```json
[
  { "input": "(= a b)", "output": "a === b;\n" },
  { "input": "(!= a b)", "output": "a !== b;\n" },
  { "input": "(= a b c)", "output": "a === b && b === c;\n" },
  { "input": "(and a b)", "output": "a && b;\n" },
  { "input": "(or a b)", "output": "a || b;\n" },
  { "input": "(not x)", "output": "!x;\n" },
  { "input": "(and a b c d)", "output": "a && b && c && d;\n" },
  { "input": "(or a b c d)", "output": "a || b || c || d;\n" },
  { "input": "(not (not x))", "output": "!!x;\n" }
]
```

### Regression tests

- Verify that `reset!` still compiles correctly (it emits kernel `=`
  for assignment internally — must not be intercepted)
- Verify that `for` loop counters still work (kernel `=`, `++`, `+=`)
- Verify that class constructors with `this:-field` assignment still
  work (kernel `=`)
- Verify that `swap!` output is unchanged
- Verify that `some->` / `if-let` / `when-let` still emit `== null`
  (compiler-generated, not user-written `=`)

### Cross-compiler tests

Add to `crates/lykn-lang/tests/cross_compiler.rs`: verify JS and Rust
compilers produce identical output for all new surface forms.

## Dependencies

- **Depends on**: DD-15 (language architecture — this restores DD-15's
  original design), DD-20 (Rust surface compiler architecture — the
  implementation mechanism)
- **Affects**: DD-19 (contracts — contract expressions may use `=` for
  equality checks in `:pre`/`:post`; this DD makes that work correctly),
  all lykn guides and SKILL.md (operator documentation), Guide 00
  (surface forms reference — must be regenerated after implementation)

## Open Questions

None.
