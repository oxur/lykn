---
number: 22
title: "DD-17: `type` + `match` — Algebraic Data Types and Pattern Matching"
author: "constructor clauses"
component: All
tags: [change-me]
created: 2026-03-27
updated: 2026-03-27
state: Draft
supersedes: null
superseded-by: null
version: 1.0
---

# DD-17: `type` + `match` — Algebraic Data Types and Pattern Matching

**Status**: Decided
**Date**: 2026-03-27
**Session**: v0.3.0 surface language design

## Summary

`type` defines algebraic data types with named, type-annotated fields.
Constructors compile to tagged plain objects with dev-mode type
validation (strippable). `match` provides exhaustive pattern matching
on ADT variants, literals, and structural object patterns. `match` is
an expression (IIFE in value position). Non-exhaustive matches are a
compile error. `Option` and `Result` are defined in `lykn/core` stdlib
and auto-imported via a prelude; the compiler recognizes them for
enhanced error messages and integration with other surface forms.

## Decisions

### `type` syntax — positional constructors with required type annotations

**Decision**: `type` defines an algebraic data type. The form is the
type name followed by constructor clauses. Each constructor is a
parenthesized group containing the constructor name and its fields.
Fields use the keyword-tags-next-symbol convention from `func`'s
`:args` — a type keyword followed by a field name. Every field must
have a type keyword; bare symbols are a compile error. Use `:any` to
explicitly opt out of type checking. Zero-field constructors are bare
names (no parens).

**Syntax**:

```lisp
;; Two constructors, one with a field, one without
(type Option
  (Some :any value)
  None)

;; Both constructors have fields
(type Result
  (Ok :any value)
  (Err :any error))

;; Multiple fields, mixed types
(type Shape
  (Circle :number radius)
  (Rect :number width :number height)
  (Point))

;; User-defined types as field types
(type Color (Red) (Green) (Blue))
(type StyledCircle
  (StyledCircle :number radius :Color color))
```

```javascript
// Option constructors
function Some(value) {
  return { tag: "Some", value };
}
const None = { tag: "None" };

// Result constructors
function Ok(value) {
  return { tag: "Ok", value };
}
function Err(error) {
  return { tag: "Err", error };
}

// Shape constructors
function Circle(radius) {
  return { tag: "Circle", radius };
}
function Rect(width, height) {
  return { tag: "Rect", width, height };
}
const Point = { tag: "Point" };

// Color constructors
const Red = { tag: "Red" };
const Green = { tag: "Green" };
const Blue = { tag: "Blue" };

// StyledCircle constructor
function StyledCircle(radius, color) {
  return { tag: "StyledCircle", radius, color };
}
```

**ESTree nodes**: Constructor functions → `FunctionDeclaration` with
`ReturnStatement` containing `ObjectExpression`. Zero-field
constructors → `VariableDeclaration` (`const`) with `ObjectExpression`.
The tag property → `Property` with `Literal` key `"tag"` and `Literal`
value (the constructor name string). Field properties → `Property`
with `Identifier` key and `Identifier` value (shorthand).

**Rationale**: The keyword-tags-next-symbol convention is established
in `func`'s `:args` (DD-16). Reusing it in `type` fields creates one
type vocabulary across the entire surface language — function
parameters, constructor fields, and (future) `bind` annotations all
use the same syntax. Requiring type annotations on all fields makes
`type` definitions fully self-documenting: you can never look at a
constructor and wonder whether the absence of a type was intentional
or accidental. This is a stronger requirement than `func` (where
params are optionally typed), which makes sense — data definitions
flow through an entire program while function params are local.

### Tagged object representation

**Decision**: All constructors emit plain JavaScript objects with a
`tag` string property. Fields use named properties matching the field
names from the `type` definition. Zero-field constructors are also
objects (`{ tag: "Name" }`), not bare strings. One representation,
one dispatch mechanism, no special cases.

**Syntax**:

```lisp
(Some 42)          ;; → { tag: "Some", value: 42 }
(Rect 10 20)       ;; → { tag: "Rect", width: 10, height: 20 }
(Point)            ;; → { tag: "Point" } (referencing the const)
None               ;; → { tag: "None" } (referencing the const)
```

```javascript
Some(42)           // { tag: "Some", value: 42 }
Rect(10, 20)       // { tag: "Rect", width: 10, height: 20 }
Point              // { tag: "Point" }
None               // { tag: "None" }
```

**ESTree nodes**: `CallExpression` for constructors with fields.
`Identifier` for zero-field constructors (referencing the `const`
binding).

**Rationale**: Named fields over positional (`_0`, `_1`) because
named fields are debuggable — `console.log(shape)` shows
`{ tag: "Rect", width: 10, height: 20 }`, not `{ tag: "Rect",
_0: 10, _1: 20 }`. JS interop is natural — any JS code can read
`.width` without knowing lykn conventions. Zero-field constructors
as objects (not strings) ensures uniform dispatch — `match` always
checks `x.tag`, never branches on `typeof x === "string"` vs
`typeof x === "object"`.

### Construction-time type validation

**Decision**: Constructor functions emit dev-mode type checks on
field values, using the same `TypeError` format as `func`. Type
checks are stripped by `--strip-assertions`. The checks use the
same compiled forms as DD-16's built-in type keywords.

**Syntax**:

```lisp
(Circle "not a number")  ;; TypeError in dev mode
```

```javascript
// Dev mode
function Circle(radius) {
  if (typeof radius !== "number" || Number.isNaN(radius))
    throw new TypeError("Circle: field 'radius' expected number, got " + typeof radius);
  return { tag: "Circle", radius };
}

// Production mode (--strip-assertions)
function Circle(radius) {
  return { tag: "Circle", radius };
}
```

User-defined types as field types generate variant-aware checks:

```javascript
// Dev mode — :Color field check
function StyledCircle(radius, color) {
  if (typeof radius !== "number" || Number.isNaN(radius))
    throw new TypeError("StyledCircle: field 'radius' expected number, got " + typeof radius);
  if (typeof color !== "object" || color === null ||
      !["Red", "Green", "Blue"].includes(color.tag))
    throw new TypeError("StyledCircle: field 'color' expected Color");
  return { tag: "StyledCircle", radius, color };
}
```

**ESTree nodes**: `IfStatement` + `ThrowStatement` + `NewExpression`
(same pattern as DD-16 type assertions).

**Rationale**: Harmonizes with `func`. Constructor fields and function
parameters use the same type keywords, the same check format, the
same error messages, and the same stripping mechanism. One mental
model for type checking across the entire language. User-defined type
checks validate against the full variant set because the Rust surface
compiler knows all variants at the definition site.

### `match` clause structure

**Decision**: `match` takes an expression to match on, followed by
clauses. Each clause is a parenthesized group containing a pattern,
an optional `:when` guard, and one or more body expressions. The
guard expression is evaluated after the pattern matches; if it
returns falsy, matching continues to the next clause.

**Syntax**:

```lisp
;; Basic ADT matching
(match opt
  ((Some v) (use v))
  (None (default)))

;; With guards
(match opt
  ((Some v) :when (> v 0) (use-positive v))
  ((Some v) (use-nonpositive v))
  (None (default)))

;; Multi-expression body
(match status
  ((Ok data)
    (console:log "success")
    (process data))
  ((Err e)
    (console:log "failure")
    (handle e)))
```

```javascript
// Basic ADT matching
if (opt.tag === "Some") {
  const v = opt.value;
  use(v);
} else if (opt.tag === "None") {
  default();
}

// With guards
if (opt.tag === "Some" && opt.value > 0) {
  const v = opt.value;
  usePositive(v);
} else if (opt.tag === "Some") {
  const v = opt.value;
  useNonpositive(v);
} else if (opt.tag === "None") {
  default();
}
```

**ESTree nodes**: `IfStatement` chain. Pattern → tag check via
`BinaryExpression` (`===`) on `MemberExpression` (`.tag`). Field
destructuring → `VariableDeclaration` (`const`) with
`MemberExpression`. Guard → additional `LogicalExpression` (`&&`)
in the `IfStatement` test.

**Rationale**: `(pattern [:when guard] body...)` is the standard
shape across ML, Erlang, LFE, Racket, and Clojure. `:when` as a
keyword is consistent with lykn's keyword-labeled design. The guard
appearing between pattern and body reads naturally: "if this shape,
when this condition, do this."

### Pattern types

**Decision**: `match` supports four kinds of patterns:

**ADT constructor patterns**: `(ConstructorName binding...)` for
constructors with fields. Bare `ConstructorName` for zero-field
constructors. The constructor name is the discriminator; bare symbols
inside are fresh bindings.

**Literal patterns**: numbers, strings, keywords, booleans, `null`,
`undefined`. Compile to `===` checks.

**Wildcard**: `_` matches anything and binds nothing. Consistent with
DD-06 destructuring.

**Structural object patterns**: `(obj :key pattern :key pattern ...)`
using surface keyword-value alternation. Keywords are property names;
values are patterns (literals match, symbols bind). See "Structural
matching" decision below.

**Syntax**:

```lisp
;; ADT constructor pattern
(match shape
  ((Circle r) (area-circle r))
  ((Rect w h) (area-rect w h))
  (Point 0))

;; Literal patterns
(match status
  (200 "ok")
  (404 "not found")
  (_ "unknown"))

;; Keyword literal patterns
(match direction
  (:north (go-up))
  (:south (go-down))
  (_ (stay)))

;; Boolean patterns
(match flag
  (true "yes")
  (false "no"))

;; Nested ADT patterns
(match response
  ((Ok (Some v)) (use v))
  ((Ok None) (use-default))
  ((Err e) (handle e)))
```

```javascript
// ADT constructor pattern
if (shape.tag === "Circle") {
  const r = shape.radius;
  areaCircle(r);
} else if (shape.tag === "Rect") {
  const w = shape.width;
  const h = shape.height;
  areaRect(w, h);
} else if (shape.tag === "Point") {
  0;
}

// Literal patterns
if (status === 200) {
  "ok";
} else if (status === 404) {
  "not found";
} else {
  "unknown";
}

// Keyword literal patterns (keywords compile to strings)
if (direction === "north") {
  goUp();
} else if (direction === "south") {
  goDown();
} else {
  stay();
}

// Boolean patterns
if (flag === true) {
  "yes";
} else if (flag === false) {
  "no";
}

// Nested ADT patterns
if (response.tag === "Ok" && response.value !== null &&
    typeof response.value === "object" && response.value.tag === "Some") {
  const v = response.value.value;
  use(v);
} else if (response.tag === "Ok" && response.value !== null &&
           typeof response.value === "object" && response.value.tag === "None") {
  useDefault();
} else if (response.tag === "Err") {
  const e = response.error;
  handle(e);
}
```

**ESTree nodes**: Tag checks → `BinaryExpression` (`===`) on
`MemberExpression`. Literal checks → `BinaryExpression` (`===`).
Nested patterns → chained `LogicalExpression` (`&&`). Bindings →
`VariableDeclaration` (`const`) with `MemberExpression`.

**Rationale**: ADT patterns, literals, and wildcards are the
universal core of pattern matching. Every ML-family and Lisp pattern
matcher provides these. Bare symbols as bindings follows the
convention from destructuring (DD-06). Zero-field constructors as
bare names (not wrapped in parens) avoids ambiguity with function
calls.

### Binding rules in patterns

**Decision**: Every bare symbol in a pattern position creates a fresh
binding. There is no pin syntax for matching against a variable's
existing value. To match a variable's value, use a guard:

```lisp
;; Match against the value of expected-tag
(match item
  ((obj :tag t) :when (= t expected-tag) (process item))
  (_ (skip)))
```

Pin syntax (Elixir's `^`) is deferred to a future DD if demand
emerges.

**Rationale**: Patterns-only-bind is the simplest model and the one
used by ML, Haskell, and Elm. Pin syntax adds parsing complexity and
a new sigil for a use case adequately handled by guards. Simpler is
better for the first version.

### `match` as expression — IIFE codegen

**Decision**: `match` is an expression. It can appear in any value
position, including as the initializer of `bind`, as a function
argument, or as the body of `fn`. In statement position (where the
value is unused), `match` compiles to a plain if-chain. In value
position, `match` compiles to an immediately-invoked function
expression (IIFE).

**Syntax**:

```lisp
;; Statement position — plain if-chain
(match status
  (200 (console:log "ok"))
  (_ (console:log "other")))

;; Value position — IIFE
(bind label (match status
  (200 "ok")
  (404 "not found")
  (_ "unknown")))
```

```javascript
// Statement position
if (status === 200) {
  console.log("ok");
} else {
  console.log("other");
}

// Value position
const label = (() => {
  if (status === 200) return "ok";
  if (status === 404) return "not found";
  return "unknown";
})();
```

**ESTree nodes**: Statement position → `IfStatement` chain. Value
position → `CallExpression` wrapping `ArrowFunctionExpression`
containing `IfStatement` chain with `ReturnStatement` in each branch.

**Rationale**: `match` as expression is fundamental — pattern matching
is most useful when it produces a value. IIFE codegen preserves the
`const`-only discipline (no `let` temporary variable needed), produces
valid JavaScript, and works for arbitrarily complex match bodies
including multi-expression arms. The Rust surface compiler detects
context (statement vs value) and chooses the cleaner codegen.

### Exhaustiveness is a compile error

**Decision**: Non-exhaustive `match` expressions are compile errors,
not warnings. The Rust surface compiler performs exhaustiveness
analysis using Maranget's algorithm for usefulness and exhaustiveness
checking.

**Rules**:

- **ADT matches**: every variant must be covered, either by an
  explicit constructor pattern or by `_`.
- **Literal matches on open types** (`:number`, `:string`, `:any`):
  `_` wildcard required. The compiler cannot enumerate all possible
  values.
- **Boolean matches**: `true` + `false` = exhaustive (when the
  compiler knows the matched value is boolean). Otherwise `_` required.
- **Guards make clauses partial**: a guarded clause `((Some v) :when
  (> v 0) ...)` does not satisfy exhaustiveness for the `Some`
  variant. An unguarded `(Some v)` or `_` clause must also be present.
- **Nested ADTs**: the compiler tracks coverage across all
  combinations of nested variants.
- **Structural object patterns**: inherently non-exhaustive, `_`
  always required (see "Structural matching" below).

**Error examples**:

```lisp
;; COMPILE ERROR: non-exhaustive match — missing None
(match opt
  ((Some v) v))

;; COMPILE ERROR: non-exhaustive — no wildcard for open type
(match status
  (200 "ok")
  (404 "not found"))

;; COMPILE ERROR: guard makes (Some v) partial — missing unguarded Some or _
(match opt
  ((Some v) :when (> v 0) (use v))
  (None (default)))

;; OK: explicit crash satisfies exhaustiveness
(match opt
  ((Some v) :when (> v 0) (use v))
  (_ (throw (Error "unexpected"))))
```

**Rationale**: Non-exhaustive matches are bugs until proven otherwise.
Warnings get ignored; errors force resolution. If a developer
genuinely wants partial handling, `(_ (throw (Error "...")))` makes
the crash explicit and intentional — visible in code review,
greppable, impossible to miss. This follows Elm and Rust, which
treat exhaustiveness as one of their most valued features. The
JavaScript hazard research (BugAID) identifies "dereferenced
non-values" as the #1 bug pattern — exhaustive matching on `Option`
directly eliminates this entire class.

### Structural matching on plain JS objects

**Decision**: `match` supports structural patterns using `obj` with
keyword-value alternation (surface syntax). Keywords are property
names; values are patterns — literals match, symbols bind. Structural
patterns are inherently non-exhaustive; the compiler requires a `_`
wildcard when any clause uses a structural pattern. No exhaustiveness
analysis is performed on structural clauses.

**Syntax**:

```lisp
;; Structural matching on JS interop values
(match response
  ((obj :ok true :data d) (process d))
  ((obj :ok false :error e) (handle e))
  (_ (throw (Error "unexpected response"))))

;; Nested structural patterns
(match result
  ((obj :status 200 :body (obj :users users)) (process users))
  ((obj :status 404) (not-found))
  (_ (throw (Error "unexpected"))))
```

```javascript
// Structural matching
if (typeof response === "object" && response !== null &&
    response.ok === true && "data" in response) {
  const d = response.data;
  process(d);
} else if (typeof response === "object" && response !== null &&
           response.ok === false && "error" in response) {
  const e = response.error;
  handleError(e);
} else {
  throw new Error("unexpected response");
}

// Nested structural patterns
if (typeof result === "object" && result !== null &&
    result.status === 200 && "body" in result &&
    typeof result.body === "object" && result.body !== null &&
    "users" in result.body) {
  const users = result.body.users;
  process(users);
} else if (typeof result === "object" && result !== null &&
           result.status === 404) {
  notFound();
} else {
  throw new Error("unexpected");
}
```

**ESTree nodes**: Object type check → `LogicalExpression` chain
(`typeof` + `!== null`). Property existence → `BinaryExpression`
(`in`). Literal property match → `BinaryExpression` (`===`) on
`MemberExpression`. Binding → `VariableDeclaration` (`const`) with
`MemberExpression`.

**Rationale**: lykn compiles to JavaScript, and JS code produces
plain objects everywhere — fetch responses, DOM APIs, parsed JSON,
third-party library returns. If `match` only works on ADTs, developers
will use manual if-chains for half their code, defeating the purpose
of pattern matching. Structural patterns with mandatory `_` provide
ergonomic destructuring without false safety promises. The compiler
knows it can't check coverage on shapes it doesn't control, so it
requires an explicit fallback.

### `Option` and `Result` — stdlib with compiler recognition

**Decision**: `Option` and `Result` are defined as normal `type`
forms in the `lykn/core` standard library. They are auto-imported
into every surface module via a prelude (no explicit `import`
required). The Rust surface compiler recognizes these types by module
path and provides enhanced behavior: improved error messages,
integration with threading macros (DD-18), and future operator
support.

**Stdlib definitions**:

```lisp
;; lykn/core/option.lykn
(export (type Option
  (Some :any value)
  None))

;; lykn/core/result.lykn
(export (type Result
  (Ok :any value)
  (Err :any error)))
```

**Prelude behavior**: Every surface module behaves as if it begins
with:

```lisp
(import (Option Some None) "lykn/core/option")
(import (Result Ok Err) "lykn/core/result")
```

These imports are injected by the compiler before processing. They
can be shadowed by local definitions — a module that defines its own
`Option` type uses its local version and loses compiler-enhanced
behavior.

**Enhanced compiler behavior for blessed types**:

- **Error messages**: "this function can return None but you haven't
  handled the empty case" instead of generic "non-exhaustive match —
  missing None"
- **Threading macro integration** (DD-18): `some->` and `some->>`
  desugar to `match` on `Some`/`None` internally
- **Future `?`-style operator**: `(try! expr)` could unwrap `Ok` or
  early-return `Err`, requiring compiler knowledge of `Result`
- **Linter guidance**: "this function can fail but you're not using
  Result"

**Rationale**: The define-in-stdlib, recognize-in-compiler pattern is
established by Rust (`Option`/`Result` in `core`, compiler knows
them for `?` and `#[must_use]`), Haskell (`Maybe`/`Either` in
`Prelude`, GHC optimizes them), and Elm (`Maybe`/`Result` in
`elm/core`, compiler uses them for exhaustiveness). It avoids the
philosophical problem of builtins-that-aren't-user-definable while
getting all the practical benefits. Prelude auto-import eliminates
ceremony for the two most fundamental types in the language —
requiring `(import (Option Some None) "lykn/core/option")` in every
file would be noise.

## Rejected Alternatives

### Bare strings for zero-field constructors

**What**: Zero-field constructors like `None` compile to the string
`"None"` instead of `{ tag: "None" }`.

**Why rejected**: Breaks uniform dispatch. `match` would need to
branch on `typeof x === "string"` vs `typeof x === "object"` before
checking tags. One representation (`{ tag: "Name" }` for all
constructors) means one dispatch mechanism. Consistency wins over
the marginal allocation savings.

### Positional field representation (`_0`, `_1`)

**What**: Multi-field constructors use positional properties:
`{ tag: "Rect", _0: 10, _1: 20 }`.

**Why rejected**: Named fields are debuggable — `console.log(shape)`
shows `{ tag: "Rect", width: 10, height: 20 }`. JS interop is
natural — any JS code can read `.width`. Positional properties
require knowledge of field ordering to interpret.

### Untyped constructor fields (bare symbols)

**What**: Allow `(type Option (Some value) None)` without type
annotations on fields.

**Why rejected**: Data definitions flow through an entire program.
Requiring type annotations makes `type` self-documenting — you can
never look at a constructor and wonder whether the absence of a type
was intentional or accidental. `:any` is the explicit opt-out. This
is a stronger requirement than `func` params (which are optionally
typed), which is appropriate — data definitions are more significant
than local function parameters.

### `(None)` with parens in patterns

**What**: Zero-field constructors in patterns require parens:
`(match opt ((Some v) ...) ((None) ...))`.

**Why rejected**: `(None)` looks like a function call. Bare `None`
is visually distinct from constructors with fields and is the
convention in Haskell, Rust, Elm, and LFE. No ambiguity — the
compiler knows `None` is a zero-field constructor from the `type`
definition.

### Pin syntax for value matching in patterns

**What**: A sigil (e.g., Elixir's `^`) to match against a variable's
existing value rather than binding a fresh name.

**Why rejected (deferred)**: Guards handle this use case adequately:
`(v :when (= v expected))`. Pin syntax adds parsing complexity and a
new sigil. Deferred to a future DD if community demand emerges.

### Or-patterns

**What**: `((Circle r) | (Oval r) body)` — multiple patterns sharing
one body.

**Why rejected (deferred)**: Adds parsing complexity. Two clauses
with the same body work today. The compiler could eventually lint
"these clauses have identical bodies, consider or-pattern." Deferred
to a future DD.

### Non-exhaustiveness as warning (not error)

**What**: Non-exhaustive `match` emits a compiler warning rather
than an error.

**Why rejected**: Warnings get ignored. The entire point of
lykn/surface is that the compiler enforces safety. A non-exhaustive
match is a bug until proven otherwise. If partial handling is
intentional, `(_ (throw (Error "...")))` makes it explicit. Elm and
Rust both treat exhaustiveness as errors and it's one of their most
valued features.

### `object` (kernel) in surface patterns

**What**: Use kernel `(object (key val) ...)` grouped-pair syntax
for structural patterns.

**Why rejected**: Surface language uses `obj` with keyword-value
alternation (DD-15). `object` with grouped pairs is kernel-only.
The surface/kernel distinction applies consistently — construction
uses `obj`, patterns use `obj`.

### `Option` and `Result` as hardcoded builtins

**What**: `Option` and `Result` are built into the compiler, not
defined in user-visible source.

**Why rejected**: Creates types that are magical — users can't read
their definition, can't understand how they work, can't learn from
them. The stdlib-with-recognition pattern (Rust, Haskell, Elm)
provides all practical benefits while keeping the types transparent
and user-definable.

### `Option` and `Result` as pure stdlib (no compiler recognition)

**What**: `Option` and `Result` are stdlib types with no special
compiler knowledge.

**Why rejected**: Loses enhanced error messages, threading macro
integration, and future operator support. The compiler can provide
significantly better developer experience when it knows about
these types — "you haven't handled the None case" is more helpful
than "non-exhaustive match — missing variant."

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| Constructor field missing type keyword | Compile error | `(type T (A value))` → error: field 'value' missing type annotation |
| `type` with no constructors | Compile error | `(type Empty)` → error: type requires at least one constructor |
| `type` with one constructor | Valid — often used for wrapper types | `(type UserId (UserId :string id))` |
| Duplicate constructor names within a type | Compile error | `(type T (A :any x) (A :any y))` → error |
| Constructor name collision across types | Compile error within same module scope | `(type T1 (X :any a))` `(type T2 (X :any b))` → error: constructor X already defined |
| `match` on non-ADT with ADT patterns | Runtime `TypeError` — `.tag` access on non-object | `(match 42 ((Some v) v) (_ 0))` → checks `(42).tag === "Some"` → falls through to `_` |
| `match` with zero clauses | Compile error | `(match x)` → error: match requires at least one clause |
| `match` clause with zero body expressions | Compile error | `(match x ((Some v)))` → error: clause requires a body |
| Guard referencing unbound name | Compile error | `((Some v) :when (> w 0) ...)` → error: `w` is not defined |
| Nested `match` | Valid — inner `match` is an expression | `(match a ((Some x) (match x ...)) ...)` |
| `match` in tail position of `func` | IIFE not needed — `func` already wraps in `return` | Compiler optimizes to plain if-chain with `return` |
| Structural pattern with no keywords | Compile error | `(match x ((obj) ...))` → error: empty structural pattern |
| Structural pattern without `_` | Compile error | `(match x ((obj :a v) (use v)))` → error: structural match requires wildcard clause |
| ADT match without `_` but all variants covered | Valid — exhaustive | See exhaustiveness rules |
| Mixed ADT and structural patterns | Valid — ADT clauses get exhaustiveness, structural require `_` | `_` satisfies both requirements |
| Shadowing prelude `Option` with local `type` | Valid — local definition wins, loses compiler-enhanced behavior | `(type Option (Just :any v) Nothing)` — works but no enhanced errors |
| Zero-field constructor in expression position | References the const | `None` → `None` (the const binding) |
| Constructor as first-class value | Valid — constructor is a function | `(map Some items)` → `items.map(Some)` |
| `:when` with side effects in guard | Valid but discouraged — guard may run and clause may not match | Linter warning |
| `match` in IIFE inside `async` function | Valid — IIFE is not async | `await` cannot appear inside match IIFE; use `bind` + `match` separately |

## Dependencies

- **Depends on**: DD-01 (colon syntax — keywords for type tags,
  `:` splitting for field access on tagged objects), DD-06
  (destructuring — `obj` keyword-value pattern syntax follows surface
  conventions), DD-15 (surface language architecture — `type` and
  `match` as surface forms, keyword-value alternation, functional
  commitment, `obj` as surface form), DD-16 (`func` — type keyword
  convention, `TypeError` format, `--strip-assertions` mechanism,
  built-in type keywords table)
- **Affects**: DD-18 (threading macros — `some->`/`some->>` desugar
  to `match` on `Option`; `if-let`/`when-let` use pattern matching
  internally), DD-19 (contracts — `:pre`/`:post` can reference ADT
  types, contract checks may use pattern matching), DD-20 (Rust
  surface compiler — exhaustiveness analysis via Maranget's algorithm,
  blessed type registry, prelude injection, context detection for
  IIFE vs if-chain codegen)

## Open Questions

- [ ] Parametric types — `(type Result :ok :err (Ok :ok value)
  (Err :err error))` with type parameters. Deferred to gradual type
  system (v0.4.0+). For v0.3.0, `:any` serves as the escape hatch.
- [ ] Array patterns in `match` — `(match xs ((array first rest...)
  ...))` using DD-06 array destructuring syntax. Likely supported
  (consistent with `obj` structural patterns) but needs detailed
  design for exhaustiveness interaction.
- [ ] Multi-value matching — `(match (tuple a b) ...)` for Erlang-
  style simultaneous matching on multiple values. Deferred.
- [ ] How `type` interacts with `export` — presumably
  `(export (type Option ...))` exports the type and all constructors.
  Detailed semantics in DD-20 (Rust compiler architecture).
- [ ] Interaction with `cell` — `(match (express counter) ...)` is
  the correct pattern (read first, then match). Direct `(match
  counter ...)` on a cell should be a compile error with a helpful
  message ("cannot match on cell — use express"). Functional
  language precedent is unanimous: read the container first.
- [ ] Optimization: the Rust surface compiler could compile ADT
  matches to `switch` statements on the tag string instead of
  if-chains, for better JS engine optimization on large variant sets.
- [ ] `match` inside IIFE and `await` — if a match arm needs to
  `await`, the IIFE approach breaks (arrow function isn't async).
  The compiler could detect this and either emit an async IIFE or
  require the developer to `bind` the match result separately.
- [ ] ADT constructor arity errors — should `(Some 1 2)` (too many
  args) be a compile error or a runtime error? Compile error is
  preferable since the compiler knows the constructor's field count.
- [ ] Pattern matching on `null` and `undefined` — should `(match x
  (null ...) (undefined ...))` check with `===` or with `==`?
  Recommendation: `===` (consistent with DD-15's strict equality
  commitment). Matching both null and undefined requires two clauses
  or `_`.

## Version History

### v1.0 — 2026-03-27

Initial version.
