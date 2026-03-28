---
number: 23
title: "DD-18: Threading Macros and Conditional Binding"
author: "reading
top"
component: All
tags: [change-me]
created: 2026-03-28
updated: 2026-03-28
state: Draft
supersedes: null
superseded-by: null
version: 1.0
---

# DD-18: Threading Macros and Conditional Binding

**Status**: Decided
**Date**: 2026-03-28
**Session**: v0.3.0 surface language design

## Summary

`->` and `->>` provide standard thread-first and thread-last macro
expansion. `some->` and `some->>` provide nil-safe threading with
short-circuit on `null`/`undefined`, compiled to IIFEs with explicit
`== null` checks. `some->` is nil-checking only — not Option-aware;
Option threading uses `match`. `if-let` and `when-let` are
pattern-based conditional binding forms using the same pattern system
as `match` (DD-17). Compiler-generated nil checks use `== null`
(loose equality) — an accepted exception to the surface language's
strict equality rule, since the compiler generates this code
internally and `== null` is the precisely correct idiom for catching
both `null` and `undefined`.

## Decisions

### `->` thread-first

**Decision**: `->` takes an initial value and a series of steps.
Each step receives the threaded value as its first argument. Steps
can be bare symbols (wrapped in a single-argument call) or lists
(the threaded value is inserted as the first argument after the
function name). `->` is a pure syntactic transform — it expands to
nested calls at compile time with no runtime behavior.

**Syntax**:

```lisp
;; Thread-first with list steps
(-> user
  (get :name)
  (str:toUpperCase)
  (str:slice 0 10))
```

```javascript
user.name.toUpperCase().slice(0, 10)
```

```lisp
;; Kernel expansion (before compilation):
(str:slice (str:toUpperCase (get user :name)) 0 10)
```

```lisp
;; Thread-first with bare symbol steps
(-> 5 inc double)
```

```javascript
double(inc(5))
```

```lisp
;; Kernel expansion:
(double (inc 5))
```

```lisp
;; Mixed bare symbols and lists
(-> data
  parse
  (transform :format :json)
  validate)
```

```javascript
validate(transform(parse(data), "json"))
```

```lisp
;; Kernel expansion:
(validate (transform (parse data) :format :json))
```

**ESTree nodes**: None specific to `->` — it expands to kernel forms
before reaching the JS compiler. The expanded forms compile to
`CallExpression` and `MemberExpression` per existing kernel rules.

**Rationale**: Thread-first is standard Clojure behavior, adopted
unchanged. It eliminates deeply nested function calls by reading
top-to-bottom instead of inside-out. Bare symbols wrapping in calls
is convenient for unary functions. The transform is trivial — each
step wraps the accumulated expression as the first argument.

### `->>` thread-last

**Decision**: `->>` takes an initial value and a series of steps.
Each step receives the threaded value as its last argument. Same
rules as `->` for bare symbols (wrapped in a single-argument call —
identical to `->` for unary functions) and lists (threaded value
inserted as the last argument).

**Syntax**:

```lisp
;; Thread-last
(->> items
  (filter even?)
  (map double)
  (take 5))
```

```javascript
take(map(filter(items, isEven), double), 5)
```

```lisp
;; Kernel expansion:
(take (map (filter items even?) double) 5)
```

```lisp
;; Thread-last with bare symbols (same as thread-first for unary)
(->> 5 inc double)
```

```javascript
double(inc(5))
```

**ESTree nodes**: Same as `->` — expands to kernel forms.

**Rationale**: Thread-last complements thread-first for APIs where
the primary data argument is last (common in functional-style APIs
like `map`, `filter`, `reduce`). Together, `->` and `->>` cover the
two dominant calling conventions.

### `some->` nil-safe thread-first

**Decision**: `some->` is thread-first with nil short-circuiting.
After each step, the intermediate result is checked against
`null`/`undefined`. If nil, the chain short-circuits and returns the
nil value. If non-nil, threading continues. `some->` compiles to an
IIFE with explicit `== null` checks and sequential `const` bindings.

`some->` is nil-checking only — it does not check for `Option`
(`Some`/`None`). For Option-aware threading, use `match`. This keeps
`some->` simple and aligned with JS interop where nil values are the
norm.

**Syntax**:

```lisp
;; Nil-safe property chain
(some-> user
  (get :address)
  (get :city)
  (str:toUpperCase))
```

```javascript
(() => {
  const _t0 = user;
  if (_t0 == null) return _t0;
  const _t1 = _t0.address;
  if (_t1 == null) return _t1;
  const _t2 = _t1.city;
  if (_t2 == null) return _t2;
  return _t2.toUpperCase();
})()
```

```lisp
;; Nil-safe with function calls
(some-> (find-user id)
  (get :name)
  (str:trim)
  (validate-name))
```

```javascript
(() => {
  const _t0 = findUser(id);
  if (_t0 == null) return _t0;
  const _t1 = _t0.name;
  if (_t1 == null) return _t1;
  const _t2 = _t1.trim();
  if (_t2 == null) return _t2;
  return validateName(_t2);
})()
```

```lisp
;; Nil-safe with mixed steps
(some-> config
  (get :database)
  (get :host)
  (connect)
  (query "SELECT 1"))
```

```javascript
(() => {
  const _t0 = config;
  if (_t0 == null) return _t0;
  const _t1 = _t0.database;
  if (_t1 == null) return _t1;
  const _t2 = _t1.host;
  if (_t2 == null) return _t2;
  const _t3 = connect(_t2);
  if (_t3 == null) return _t3;
  return query(_t3, "SELECT 1");
})()
```

**ESTree nodes**: `CallExpression` (IIFE) wrapping
`ArrowFunctionExpression`. Body contains alternating
`VariableDeclaration` (`const`) and `IfStatement` (with `== null`
check and `ReturnStatement`). Final step wrapped in
`ReturnStatement`. The `== null` check → `BinaryExpression` with
`==` operator and `Literal` (`null`).

**Rationale**: IIFE codegen is general — it works for arbitrary
function calls, not just property access chains (which would be the
only case where optional chaining `?.` applies). The IIFE approach
is consistent with `match` expression codegen (DD-17). `== null`
(loose equality) catches both `null` and `undefined` in a single
check — this is an accepted exception to DD-15's strict equality
rule because the compiler generates this code internally and `== null`
is the precisely correct JS idiom (it is the only loose equality
check that ESLint's `eqeqeq` rule explicitly exempts). Each
intermediate result is bound to a `const` — no mutation, consistent
with the functional commitment.

### `some->>` nil-safe thread-last

**Decision**: `some->>` is thread-last with nil short-circuiting.
Same mechanics as `some->` but the threaded value is inserted as
the last argument of each step. Same IIFE codegen with `== null`
checks.

**Syntax**:

```lisp
;; Nil-safe thread-last
(some->> items
  (find-first even?)
  (multiply 2)
  (clamp 0 100))
```

```javascript
(() => {
  const _t0 = items;
  if (_t0 == null) return _t0;
  const _t1 = findFirst(isEven, _t0);
  if (_t1 == null) return _t1;
  const _t2 = multiply(2, _t1);
  if (_t2 == null) return _t2;
  return clamp(0, 100, _t2);
})()
```

**ESTree nodes**: Same as `some->`.

**Rationale**: Complements `some->` for last-argument APIs, same as
`->>` complements `->`.

### `if-let` — pattern-based conditional binding

**Decision**: `if-let` binds a value and branches based on whether
a pattern matches. The binding clause is `(pattern expr)`. If the
pattern matches, the then-branch executes with the bindings in scope.
If it doesn't match, the else-branch executes. `if-let` uses the
same pattern system as `match` (DD-17).

Pattern types in `if-let`:

- **ADT constructor pattern**: `((Some v) expr)` — matches if the
  value's tag matches the constructor, binds fields
- **Structural pattern**: `((obj :key binding ...) expr)` — matches
  if the object has the specified properties
- **Simple binding** (bare lowercase symbol): `(name expr)` — matches
  if the value is not nil (`!= null`), binds the value

Simple bindings are distinguished from zero-field constructor
patterns by case: constructor names start with uppercase (`Some`,
`None`, `Ok`), binding names start with lowercase (`user`, `name`).

**Syntax**:

```lisp
;; ADT pattern
(if-let ((Some user) (find-user id))
  (greet user)
  (console:log "not found"))
```

```javascript
(() => {
  const _t = findUser(id);
  if (_t != null && typeof _t === "object" && _t.tag === "Some") {
    const user = _t.value;
    return greet(user);
  } else {
    return console.log("not found");
  }
})()
```

```lisp
;; Structural pattern
(if-let ((obj :name name :age age) (get-user-data id))
  (console:log name age)
  (console:log "bad data"))
```

```javascript
(() => {
  const _t = getUserData(id);
  if (typeof _t === "object" && _t !== null &&
      "name" in _t && "age" in _t) {
    const name = _t.name;
    const age = _t.age;
    return console.log(name, age);
  } else {
    return console.log("bad data");
  }
})()
```

```lisp
;; Simple binding — nil check
(if-let (user (find-user id))
  (greet user)
  (console:log "not found"))
```

```javascript
(() => {
  const _t = findUser(id);
  if (_t != null) {
    const user = _t;
    return greet(user);
  } else {
    return console.log("not found");
  }
})()
```

```lisp
;; if-let in statement position — no IIFE
(if-let ((Some user) (find-user id))
  (greet user)
  (console:log "not found"))
```

```javascript
const _t = findUser(id);
if (_t != null && typeof _t === "object" && _t.tag === "Some") {
  const user = _t.value;
  greet(user);
} else {
  console.log("not found");
}
```

**ESTree nodes**: Value position → `CallExpression` (IIFE) wrapping
`ArrowFunctionExpression` with `IfStatement`. Statement position →
`VariableDeclaration` (`const`) + `IfStatement`. Pattern checks
reuse the same `BinaryExpression`/`LogicalExpression` patterns as
`match` (DD-17).

**Rationale**: Pattern-based `if-let` subsumes both truthiness-based
(Clojure) and nil-based binding. Since DD-17 built a full pattern
system, it would be wasteful not to use it here. The
uppercase/lowercase distinction for constructors vs bindings is
natural — ADT constructors are PascalCase by convention (Haskell,
Rust, Elm, ML), and lykn follows this. Rust's `if let` is
pattern-based and is one of its most ergonomic features.

### `when-let` — pattern-based conditional binding without else

**Decision**: `when-let` is `if-let` without an else branch. If the
pattern matches, the body executes. If it doesn't match, nothing
happens (returns `undefined`). Body can contain multiple expressions.

**Syntax**:

```lisp
;; ADT pattern
(when-let ((Some user) (find-user id))
  (console:log (get user :name))
  (greet user))
```

```javascript
(() => {
  const _t = findUser(id);
  if (_t != null && typeof _t === "object" && _t.tag === "Some") {
    const user = _t.value;
    console.log(user.name);
    return greet(user);
  }
})()
```

```lisp
;; Simple binding
(when-let (data (fetch-data url))
  (process data)
  (save data))
```

```javascript
(() => {
  const _t = fetchData(url);
  if (_t != null) {
    const data = _t;
    process(data);
    return save(data);
  }
})()
```

```lisp
;; Statement position — no IIFE
(when-let ((Ok result) (try-parse input))
  (console:log "parsed successfully")
  (use result))
```

```javascript
const _t = tryParse(input);
if (_t != null && typeof _t === "object" && _t.tag === "Ok") {
  const result = _t.value;
  console.log("parsed successfully");
  use(result);
}
```

**ESTree nodes**: Same as `if-let` but `IfStatement` has no
`alternate` (no else branch).

**Rationale**: `when-let` is the side-effectful counterpart to
`if-let`. Common pattern: "if this data exists, do something with
it." No else branch means no need to provide a fallback value — the
body runs for effects.

### `== null` as accepted exception to strict equality

**Decision**: Compiler-generated nil checks in `some->`, `some->>`,
`if-let`, and `when-let` use `== null` (loose equality). This is an
accepted exception to DD-15's strict equality rule. The rationale:

- DD-15's `=` → `===` rule governs user-written surface code.
  Compiler-generated code is not user-written.
- `== null` is the precisely correct JS idiom for "is null or
  undefined" — it matches exactly those two values and nothing else.
- It is the only loose equality pattern that ESLint's `eqeqeq` rule
  explicitly exempts.
- The alternative (`=== null || === undefined`) is two checks where
  one suffices, producing noisier compiled output with no safety
  benefit.

User-written code still uses `=` → `===`. Loose equality is only
available to the user via `(js:eq a b)`.

## Rejected Alternatives

### Optional chaining (`?.`) for `some->`

**What**: Compile `some->` to JavaScript optional chaining:
`user?.address?.city`.

**Why rejected**: Optional chaining only works for property access
and method calls. `some->` supports arbitrary function calls as
steps — `(some-> x (validate) (transform :format))` has no `?.`
equivalent. The IIFE approach is general and works for all step
types. A hybrid approach (optional chaining when possible, IIFE
otherwise) adds complexity and makes compiled output unpredictable.

### Nested ternaries for `some->`

**What**: Compile `some->` to `x == null ? x : f(x) == null ? ...`.

**Why rejected**: Repeated sub-expressions. Each intermediate result
gets evaluated twice (once for the check, once for the next step).
This is both a performance issue and a correctness issue if any step
has side effects.

### `some->` as Option-aware

**What**: `some->` checks for `None` (ADT tag) in addition to or
instead of `null`/`undefined`.

**Why rejected**: Conflates two distinct concepts — JS nil values
(`null`/`undefined`) and lykn ADT variants (`None`). `some->` is a
JS interop tool for nil-safe value threading. For Option-aware
control flow, `match` is the right tool — it provides exhaustiveness
checking, pattern destructuring, and explicit handling of both
variants. Mixing nil checks and tag checks in one form creates
ambiguity about what `some->` actually tests. A future form
(potentially `option->`) could provide Option-specific threading if
demand emerges.

### Truthiness-based `if-let`

**What**: `if-let` checks JS truthiness (not just nil) — binding
fails for `false`, `0`, `""`, `NaN` in addition to `null`/`undefined`.

**Why rejected**: Truthiness is a JS footgun — `0` and `""` are
legitimate values that happen to be falsy. Pattern-based `if-let`
with nil checking for simple bindings is more precise: `(if-let
(count expr) ...)` succeeds for `0` because `0 != null`. If the
developer wants truthiness-based branching, they can use `if`
directly.

### `if-let` without pattern matching

**What**: `if-let` only supports simple `(name expr)` bindings with
a nil check — no pattern matching.

**Why rejected**: DD-17 built a full pattern system. Not using it in
`if-let` would mean developers write `(if-let (x expr) (match x
((Some v) ...) ...) ...)` — nesting `match` inside `if-let` to get
pattern matching. Pattern-based `if-let` eliminates this unnecessary
nesting.

### `=== null || === undefined` in compiler-generated code

**What**: Use strict equality for nil checks instead of `== null`.

**Why rejected**: Two checks where one suffices. `== null` is
precisely correct — it matches exactly `null` and `undefined`, no
other values. The strict equality rule in DD-15 governs user-written
surface code, not compiler-generated output. Noisier compiled JS
with no safety benefit.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| `->` with no steps | Returns the initial value | `(-> x)` → `x` |
| `->` with one step | Single function call | `(-> x f)` → `(f x)` |
| `->>` with bare symbol | Same as `->` for unary | `(->> x f)` → `(f x)` |
| `some->` with no steps | Returns the initial value (with nil check) | `(some-> x)` → IIFE returning `x` |
| `some->` initial value is nil | Short-circuits immediately | `(some-> null (get :name))` → `null` |
| `some->` step returns `undefined` | Short-circuits | `(some-> user (get :missing-key))` → `undefined` |
| `some->` step returns `false` or `0` | Continues — not nil | `0` and `false` are not nil |
| `if-let` with `None` pattern | Matches zero-field constructor | `(if-let (None expr) ...)` — checks `_t.tag === "None"` |
| `if-let` with literal pattern | Matches literal value | `(if-let (42 expr) ...)` — checks `_t === 42` |
| `if-let` with `_` pattern | Always matches — else branch unreachable | Linter warning: else branch is dead code |
| `when-let` with `_` pattern | Always executes body | Linter warning: `when-let` with `_` is just `bind` + body |
| `if-let` in value position | IIFE wrapping | Returns then-value or else-value |
| `if-let` in statement position | No IIFE — plain `const` + `if` | Cleaner compiled output |
| `when-let` in value position | IIFE wrapping, returns `undefined` when unmatched | No else branch in IIFE |
| Nested `if-let` | Valid — outer binding in scope for inner | `(if-let ((Some x) a) (if-let ((Some y) b) ...))` |
| `some->` with `await` in step | Problem — IIFE is not async | See open questions |
| `->` threading into `match` | Valid — `match` is an expression | `(-> data parse (match ((Ok v) v) ((Err e) (throw e))))` |
| `if-let` multi-expression then/else | Supported — multiple exprs in each branch | Same as `match` clause bodies |

## Dependencies

- **Depends on**: DD-01 (colon syntax — keyword compilation,
  member access), DD-15 (surface language architecture — `=` → `===`
  rule that `== null` is an exception to, `js:eq` as user-facing
  loose equality escape hatch), DD-17 (`type` + `match` — pattern
  system reused by `if-let`/`when-let`, `Option`/`Result` ADTs,
  IIFE codegen pattern for expression-position forms)
- **Affects**: DD-20 (Rust surface compiler — context detection for
  IIFE vs statement codegen across `match`, `some->`, `if-let`,
  `when-let`; all four forms share the same IIFE codegen
  infrastructure)

## Open Questions

- [ ] `some->` and `await` — the IIFE wrapping breaks if a step
  needs to `await`. Same issue as `match` (DD-17 open question).
  The compiler could emit an `async` IIFE, or require restructuring.
  Shared solution needed across `match`, `some->`, `if-let`, and
  `when-let`.
- [ ] `cond->` / `cond->>` — Clojure's conditional threading where
  each step has a test: `(cond-> x (pred1?) (step1) (pred2?)
  (step2))`. Useful but not essential for v0.3.0. Deferred.
- [ ] `as->` — Clojure's threading with explicit binding name:
  `(as-> x $ (f $ 1) (g 2 $))` where `$` is the threaded value,
  placeable anywhere. Useful for APIs where the argument position
  varies per step. Deferred.
- [ ] `option->` — dedicated Option-aware threading that checks
  `Some`/`None` tags instead of `null`/`undefined`. Deferred until
  usage patterns emerge. `match` handles Option unwrapping for v0.3.0.
- [ ] `if-let` with multiple bindings — `(if-let ((Some a) expr1
  (Some b) expr2) ...)` where all bindings must match. Clojure
  supports this. Deferred.
- [ ] `guard` clauses in `if-let` — `(if-let ((Some v) expr :when
  (> v 0)) ...)`. Pattern-based `if-let` could support `:when`
  guards, same as `match`. Deferred — use nested `if` for now.
- [ ] Statement-position optimization for `some->` — when the
  result isn't used, the IIFE is unnecessary overhead. The compiler
  could emit a series of `const` + `if (x == null) return` inside
  the enclosing function, or use early-exit blocks. Needs DD-20
  design for context detection.

## Version History

### v1.0 — 2026-03-28

Initial version.
