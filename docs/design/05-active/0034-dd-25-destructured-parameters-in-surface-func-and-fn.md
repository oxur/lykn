---
number: 34
title: "DD-25: Destructured Parameters in Surface `func` and `fn`"
author: "the accessor"
component: All
tags: [change-me]
created: 2026-04-14
updated: 2026-04-14
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# DD-25: Destructured Parameters in Surface `func` and `fn`

**Status**: Decided
**Date**: 2026-04-14
**Session**: Ch 15 book chapter exposed the gap

## Summary

Surface `func` and `fn` accept destructured parameters (`object`,
`array`) in `:args` position. Every destructured field requires a
type keyword (`:any` is opt-out), matching the surface principle:
"if you name it in `:args`, you type it." Nested destructuring and
`default` in destructured fields are explicitly deferred. The
implementation uses an adapter pattern (`ParamShape` with accessor
methods) that lets downstream code migrate incrementally.

## The decision

**Option B — typed fields everywhere.** A destructuring pattern
appears where a `:type name` pair would go, but without a preceding
type keyword (the `object`/`array` head implies the outer type).
Inside the pattern, fields follow the same `:type name` alternation.

```lisp
;; Object destructuring in func :args
(func process
  :args ((object :string name :number age) :string action)
  :returns :string
  :body (template name " (" age ") — " action))

;; Array destructuring
(func head-tail
  :args ((array :number first (rest :number remaining)))
  :body (console:log first remaining))

;; fn with destructured params
(bind f (fn ((object :string name :number age))
  (console:log name age)))

;; Mixed: destructured + simple
(func handler
  :args ((object :string method :string url) :any body)
  :body ...)
```

## Compiled output

```javascript
// Dev mode
function process({name, age}, action) {
  if (typeof name !== "string")
    throw new TypeError("process: arg 'name' expected string, got " + typeof name);
  if (typeof age !== "number" || Number.isNaN(age))
    throw new TypeError("process: arg 'age' expected number, got " + typeof age);
  if (typeof action !== "string")
    throw new TypeError("process: arg 'action' expected string, got " + typeof action);
  return `${name} (${age}) — ${action}`;
}

// --strip-assertions
function process({name, age}, action) {
  return `${name} (${age}) — ${action}`;
}
```

The surface emits per-field type checks as body statements. The
kernel handles the structural destructuring (`{name, age}`) via
DD-06.

## What is deferred

These are *designed-but-not-implemented* in this DD. The syntax is
reserved; the compiler emits a clear error pointing to the future.

**Nested destructuring**: Deferred because exhaustive testing of
nested patterns across both compilers is a separate complexity tier.
The parser *recognizes* nested patterns (a field whose name position
contains a list starting with `object`/`array`) and emits:
`"nested destructuring in func/fn params is not yet supported —
use a typed param with body destructuring"`.

```lisp
;; Deferred — compile error with helpful message
(func f
  :args ((object :string name
                 (alias :any addr (object :string city)))
   :body ...))
```

**`default` in destructured fields**: Deferred because it breaks
the clean `:type name` alternation and needs its own sub-form
design. The parser recognizes `(default ...)` inside destructured
params and emits: `"default values in destructured params are not
yet supported — use a typed param with body destructuring and
default"`.

```lisp
;; Deferred — compile error with helpful message
(func f
  :args ((object (default :string name "anon") :number age))
  :body ...)
```

**Why defer explicitly**: The parser *must* recognize these forms
even before implementing them, because the alternative is a
confusing generic parse error. A specific "not yet supported"
message with a workaround is dramatically better UX than "expected
type keyword at position 3."

## Addressing the ripple: `Vec<TypedParam>` → `Vec<ParamShape>`

This is CC's primary concern. The solution: **accessor methods
that preserve the old interface.**

### The `ParamShape` type

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ParamShape {
    Simple(TypedParam),
    DestructuredObject {
        fields: Vec<TypedParam>,
        span: Span,
    },
    DestructuredArray {
        elements: Vec<ArrayParamElement>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayParamElement {
    Typed(TypedParam),
    Rest(TypedParam),
    Skip(Span),
}
```

### Accessor methods that insulate downstream code

```rust
impl ParamShape {
    /// All typed params — flattened. This is what most downstream
    /// code needs: "give me the names and types, I don't care
    /// about the structural shape."
    pub fn typed_params(&self) -> Vec<&TypedParam> {
        match self {
            Self::Simple(tp) => vec![tp],
            Self::DestructuredObject { fields, .. } => {
                fields.iter().collect()
            }
            Self::DestructuredArray { elements, .. } => {
                elements.iter().filter_map(|e| match e {
                    ArrayParamElement::Typed(tp) => Some(tp),
                    ArrayParamElement::Rest(tp) => Some(tp),
                    ArrayParamElement::Skip(_) => None,
                }).collect()
            }
        }
    }

    /// All bound names — for scope tracking.
    pub fn bound_names(&self) -> Vec<&str> {
        self.typed_params().iter().map(|tp| tp.name.as_str()).collect()
    }

    /// The type keyword for dispatch purposes.
    /// Simple: the actual type keyword.
    /// Destructured object: synthetic `:object`.
    /// Destructured array: synthetic `:array`.
    pub fn dispatch_type(&self) -> &str {
        match self {
            Self::Simple(tp) => &tp.type_keyword,
            Self::DestructuredObject { .. } => "object",
            Self::DestructuredArray { .. } => "array",
        }
    }

    /// Convert to kernel param form (for emission).
    pub fn to_kernel(&self) -> SExpr { ... }

    /// Emit type check AST nodes (for emission).
    pub fn type_checks(&self, func_name: &str) -> Vec<SExpr> { ... }
}
```

### Why this solves the ripple

Most downstream code that currently iterates `Vec<TypedParam>` does
one of three things:

1. **Gets names** (scope tracking, unused binding detection):
   → Call `param.bound_names()` — works for both simple and
   destructured.

2. **Gets types** (overlap detection, type check emission):
   → Call `param.typed_params()` — returns `&TypedParam` for
   every field, flat. Overlap detection uses `param.dispatch_type()`
   for the outer dispatch level.

3. **Emits kernel form** (single-clause, multi-clause, fn):
   → Call `param.to_kernel()` and `param.type_checks(name)` —
   the emission helpers are *on the type*, not in the emitter.

This means the actual change at each call site is mechanical:
replace `tp.name` with `param.bound_names()` or
`param.typed_params()`. The compiler stays compiling at every step
because the accessor methods provide the same *information* the
old `TypedParam` did, just through a method call instead of direct
field access.

**Migration pattern for the 194KB emitter**: Search for every
reference to `clause.args` or `params` that accesses `.name`,
`.type_keyword`, or iterates. Replace with the accessor. Each
replacement is local — it doesn't depend on other replacements
being done first.

## Addressing parallel JS/Rust changes

Both compilers must produce identical kernel JSON. The strategy:

1. **JS first.** The JS surface compiler is smaller, faster to
   iterate, and produces the canonical test fixtures.
2. **Capture fixtures.** Save kernel JSON output for every test
   case to `test/fixtures/surface/func-destructuring.json`.
3. **Rust follows.** Build the Rust implementation to match the
   fixtures. The cross-compiler test (`tests/cross_compiler.rs`)
   catches any divergence.

This is the same JS-first, Rust-follows strategy used for all
v0.3.0 surface forms. It works. The JS implementation is the
reference; the Rust implementation is verified against it.

**Both changes can be in the same PR** — the JS changes are
~50 lines across 3 functions, and the Rust changes are guarded
by the accessor pattern. Neither compiler will reject valid
existing code; they just accept a new syntax that was previously
an error.

## Addressing multi-clause dispatch

CC's concern is correct: two clauses with `(object :string name)`
and `(object :number id)` *do* overlap at the dispatch level,
because both accept objects.

**The rule**: For dispatch purposes, a destructured `object` param
has dispatch type `:object`. A destructured `array` param has
dispatch type `:array`. Two clauses that both destructure objects
at the same position *overlap* — because at runtime, the dispatch
can only check `typeof args[i] === "object"`, not the shape of
the object's properties.

```lisp
;; COMPILE ERROR: overlapping clauses — both accept objects at position 0
(func bad
  (:args ((object :string name))
   :body (use-name name))
  (:args ((object :number id))
   :body (use-id id)))
```

This is consistent with the existing overlap rule: dispatch is on
*type*, not on *shape*. Shape-level dispatch would require runtime
property checking in the dispatch chain, which is a different
feature (structural dispatch — deferred).

**Implementation**: `ParamShape::dispatch_type()` returns `"object"`
or `"array"` for destructured params. Maranget's algorithm already
operates on type keywords as constructors. No algorithm change
needed — just a new accessor.

**What IS valid**: mixing destructured and simple params at
different positions, or mixing `object` and `array` destructuring:

```lisp
;; OK: different types at position 0
(func process
  (:args ((object :string name) :string action)
   :body ...)
  (:args (:string raw-input :string action)
   :body ...))
;; Clause 1 dispatches on :object, clause 2 on :string — no overlap

;; OK: different destructuring kinds
(func transform
  (:args ((object :string name))
   :body ...)
  (:args ((array :number first))
   :body ...))
;; Clause 1 dispatches on :object, clause 2 on :array — no overlap
```

## Addressing nested destructuring ambiguity

CC asks: is the inner `(object ...)` a nested pattern or a value?
The answer is unambiguous given the existing syntax rules:

Inside a destructured param, the parser is consuming `:type name`
pairs. A nested pattern would appear in the *name* position — the
position after a type keyword. If the name position contains a list
starting with `object` or `array`, it's a nested pattern. If it
contains an atom, it's a simple name. There is no ambiguity.

```lisp
;; This is unambiguous:
(object :string name :object addr)
;;       ^^type ^^name ^^type ^^name
;; addr is a simple binding of type :object

;; This would be nested (deferred):
(object :string name (alias :any addr (object :string city)))
;;       ^^type ^^name  ^^alias sub-form, inner (object...) is nested
```

The parser doesn't need to "know where to stop" — it's already
alternating `:type name` pairs. A list in name position is the
signal for nesting (or for `alias`/`default` sub-forms). Since
nesting is deferred, the parser recognizes the shape and emits a
helpful error. No syntax ambiguity exists.

## Addressing `default` in destructured fields

CC asks: design now, implement later?

**Design now.** The syntax fits naturally:

```lisp
;; default in destructured fields (deferred implementation)
(func f
  :args ((object (default :string name "anon") :number age))
  :body ...)
```

`(default :type name value)` is a 3-element sub-form that appears
where a `:type name` pair would go. It's unambiguous: if the parser
sees a list starting with `default` in the `:type` position, it's a
default sub-form. The parser currently expects a keyword in that
position, so a list is a clear signal.

**Compiled output** (when implemented):

```javascript
function f({name = "anon", age}) {
  // name check: typeof name !== "string" → error
  // age check: typeof age !== "number" || NaN → error
}
```

**For now**: the parser recognizes `(default ...)` and emits a
clear error. The syntax is reserved; the implementation follows.

## Implementation phases

### Phase 1: JS surface compiler (same PR, ~2 hours)

This phase requires loading the JS skill and associated guides:

- ~/lab/cnbb/ai-design/skills/nodeless-js/SKILL.md
- ~/lab/cnbb/ai-design/guides/js/*

All changes in `src/surface.js`. Three functions, ~50 lines total.

**1a.** Add `parseDestructuredParam(listNode)`:

- Validates head is `object` or `array`
- Parses remaining values as `:type name` pairs
- For `array`: handles `(rest :type name)` sub-form
- Recognizes and rejects nested patterns and `default` with
  helpful errors
- Returns `{ destructured: true, kind, fields, rest }`

**1b.** Update `parseTypedParams` to variable-step:

- List at position `i` → `parseDestructuredParam`, step 1
- Keyword at position `i` → existing `:type name` pair, step 2
- Otherwise → error

**1c.** Add helpers `paramToKernel(p)` and `paramTypeChecks(p, name)`:

- Simple param: existing behavior
- Destructured: kernel form + per-field type checks

**1d.** Update three call sites:

- `buildSingleClauseFunc` → use helpers for paramNames and typeChecks
- `buildMultiClauseFunc` → use `dispatch_type` logic for arity check,
  use helpers for param binding and type checks
- `fnMacro` → use helpers

**1e.** Capture test fixtures: `test/fixtures/surface/func-destructuring.json`

**1f.** Write tests: `test/surface/func-destructuring.test.js`

### Phase 2: Rust AST + classifier (~2 hours)

This phase requires loading the Rust skill and associated guides:

- ~/lab/oxur/ai-rust-skill/skills/claude/SKILL.md
- ~/lab/oxur/ai-rust-skill/guides/*

**2a.** Add `ParamShape` enum and `ArrayParamElement` enum to
`ast/surface.rs`. Add all accessor methods (`typed_params`,
`bound_names`, `dispatch_type`, `to_kernel`, `type_checks`).

**2b.** Change `FuncClause::args` from `Vec<TypedParam>` to
`Vec<ParamShape>`.

**2c.** Change `SurfaceForm::Fn::params` from `Vec<TypedParam>` to
`Vec<ParamShape>`.

**2d.** Add `From<TypedParam> for ParamShape` so that
`ParamShape::Simple(tp)` wrapping is automatic. This lets existing
code that constructs `TypedParam` values wrap them trivially.

**2e.** **Leave `Constructor::fields` as `Vec<TypedParam>`.**
Destructuring doesn't apply to type constructors. Add
`parse_simple_typed_params` (the original behavior) for type
constructor parsing only.

**2f.** Update `parse_typed_params` in `classifier/forms.rs` to
return `Vec<ParamShape>`. Variable-step loop matching JS. Add
`parse_destructured_param` with deferred-feature error messages.

**Compilation check**: At this point, the Rust compiler will have
type errors at every site that accesses `clause.args` as
`Vec<TypedParam>`. These are the migration sites for Phase 3.

### Phase 3: Rust call-site migration (~2 hours)

This phase requires loading the Rust skill and associated guides:

- ~/lab/oxur/ai-rust-skill/skills/claude/SKILL.md
- ~/lab/oxur/ai-rust-skill/guides/*

**This phase is mechanical.** Every compiler error from Phase 2
is a site that needs updating. For each one:

**Emitter** (`emitter/forms.rs`, `emitter/type_checks.rs`,
`emitter/contracts.rs`):

- Where code iterates `clause.args` and accesses `.name`:
  use `param.typed_params()` or `param.bound_names()`
- Where code builds kernel param lists:
  use `param.to_kernel()`
- Where code emits type checks:
  use `param.type_checks(func_name)`

**Analysis** (`analysis/scope.rs`):

- Where code tracks introduced bindings:
  use `param.bound_names()`

**Analysis** (`analysis/func_check.rs`):

- Where overlap detection uses type keywords:
  use `param.dispatch_type()`

**Pattern**: Every migration is the same shape — replace a direct
field access with an accessor method call. The accessor returns the
same information. The only new behavior is that destructured params
return *multiple* typed params from a single `ParamShape`.

### Phase 4: Rust tests + cross-compiler verification (~1 hour)

- Add Rust classifier tests for destructured param parsing
- Add Rust emitter tests for destructured param emission
- Run `tests/cross_compiler.rs` — verify Rust output matches JS
  fixtures from Phase 1e

### Phase 5: Book update (~30 min)

Update `src/part3/chapter15/3-parameter-destructuring.md` and the
corresponding test file to show the clean surface syntax.

## Edge cases

| Case | Behavior |
|------|----------|
| Bare name in destructured pattern | Compile error: "field 'x' missing type annotation (use :any to opt out)" |
| `:any` field | No type check emitted for that field |
| Nested destructuring | Compile error: "nested destructuring in func/fn params is not yet supported — use a typed param with body destructuring" |
| `default` in destructured fields | Compile error: "default values in destructured params are not yet supported — use a typed param with body destructuring and default" |
| Destructured param in `type` constructor | Not supported — constructors use `parse_simple_typed_params` |
| Multi-clause: two `object` params at same position | Overlap — compile error |
| Multi-clause: `object` vs `:string` at same position | Not overlapping — different dispatch types |
| Multi-clause: `object` vs `array` at same position | Not overlapping — different dispatch types |
| `--strip-assertions` | Type checks removed, destructuring pattern preserved |
| Empty destructured pattern `(object)` | Compile error: "empty destructuring pattern — at least one field required" |
| `_` skip in array param | Valid: `(array :number first _ :number third)` |

## Rejected alternatives

### Option A — no type annotations in destructured params

**What**: Allow bare names inside destructured params:
`(func f :args ((object name age)) ...)`

**Why rejected**: Violates the surface principle. Every other
binding site in the surface language requires type annotations.
An exception here creates inconsistency and loses type safety at
exactly the boundary where it matters most.

### Option C — whole-pattern type annotation

**What**: Type the entire pattern as `:object`:
`(func f :args (:object (object name age)) ...)`

**Why rejected**: Redundant — the `object` head already implies
the type. Writing `:object (object ...)` is saying the same thing
twice. And it doesn't provide per-field type checking.

### Incremental migration via `destructured_args` field

**What**: Add a second field `destructured_args: Vec<ParamShape>`
alongside the existing `args: Vec<TypedParam>`, migrate consumers
one at a time.

**Why rejected**: Two sources of truth for the same information.
Every consumer must check both fields. The accessor method approach
achieves the same incremental benefit without the duplication.

## Dependencies

- **Depends on**: DD-06 (kernel destructuring — the substrate),
  DD-16 (`func` typed params, type check emission),
  DD-15 (surface type annotation principle)
- **Affects**: DD-17 (`match` patterns — unaffected, different
  pattern system), DD-21 (Maranget overlap detection — uses
  `dispatch_type()` accessor), Book Ch 15

## Open questions

None. Nested destructuring and `default` in destructured fields
are explicitly deferred with designed syntax and helpful error
messages. They are future features, not open questions.
