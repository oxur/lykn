---
number: 21
title: "DD-16: `func` — Function Definition with Contracts and Polymorphic Dispatch"
author: "checking whether"
component: All
tags: [change-me]
created: 2026-03-27
updated: 2026-03-27
state: Draft
supersedes: null
superseded-by: null
version: 1.0
---

# DD-16: `func` — Function Definition with Contracts and Polymorphic Dispatch

**Status**: Decided
**Date**: 2026-03-27
**Session**: v0.3.0 surface language design

## Summary

`func` is the canonical function definition form in lykn/surface. It
uses keyword-labeled clauses (`:args`, `:returns`, `:pre`, `:post`,
`:body`) for typed, contracted functions. Zero-argument functions use
a positional shorthand. Multi-clause functions support polymorphic
dispatch on both argument count (Erlang-style) and argument types,
with each clause carrying its own contract. The `~` character is
reserved as lykn's general placeholder sigil, used in `:post` clauses
to reference the return value.

## Decisions

### Keyword-labeled function structure

**Decision**: `func` uses keyword-labeled clauses. When a function
has arguments, all clauses use keywords. The keywords are:

| Keyword | Required? | Purpose |
|---------|-----------|---------|
| `:args` | Yes (when function has parameters) | Parameter list with optional types |
| `:returns` | Only when function returns a value | Return type |
| `:pre` | No | Precondition assertions (caller blame) |
| `:post` | No | Postcondition assertions (callee blame) |
| `:body` | Yes | Function body (all remaining expressions) |

**Syntax**:

```lisp
;; Minimal: args + body (void return)
(func log-message
  :args (:string msg)
  :body (console:log msg))
```

```javascript
function logMessage(msg) {
  console.log(msg);
}
```

```lisp
;; With return type
(func add
  :args (:number a :number b)
  :returns :number
  :body (+ a b))
```

```javascript
function add(a, b) {
  return a + b;
}
```

```lisp
;; Full contracts
(func withdraw
  :args (:number amount :account acct)
  :returns :account
  :pre [(> amount 0)
        (<= amount (express (get acct :balance)))]
  :post [(>= (express (get ~ :balance)) 0)]
  :body
  (assoc acct :balance (- (express (get acct :balance)) amount)))
```

```javascript
// Dev mode (assertions enabled)
function withdraw(amount, acct) {
  if (typeof amount !== "number" || Number.isNaN(amount))
    throw new TypeError("withdraw: arg 'amount' expected number");
  if (!(amount > 0))
    throw new ContractError("withdraw: pre-condition failed: (> amount 0) — caller blame");
  if (!(amount <= acct.balance.value))
    throw new ContractError("withdraw: pre-condition failed: (<= amount balance) — caller blame");
  const _result = { ...acct, balance: acct.balance.value - amount };
  if (!(_result.balance.value >= 0))
    throw new ContractError("withdraw: post-condition failed — callee blame");
  return _result;
}

// Production mode (--strip-assertions)
function withdraw(amount, acct) {
  return { ...acct, balance: acct.balance.value - amount };
}
```

**ESTree nodes**: `FunctionDeclaration`, `BlockStatement`. Type
assertions → `IfStatement` + `ThrowStatement` + `NewExpression`.
Contract assertions → same structure with `ContractError` message.

**Rationale**: Keyword-labeled clauses provide a consistent,
self-documenting structure. Every parameterized function has the
same visual shape. The clause ordering (`:args` → `:returns` →
`:pre` → `:post` → `:body`) reads top-to-bottom as a complete
specification: what goes in, what comes out, what must hold before,
what must hold after, what happens.

### `:args` keyword for parameters

**Decision**: The parameter clause uses `:args` (not `:params`).
In the `:args` list, type keywords tag the immediately following
symbol. Untyped parameters are bare symbols. A type keyword applies
only to the next symbol — no carry-forward.

```lisp
;; All typed
:args (:number a :number b)

;; All untyped
:args (a b)

;; Mixed — name is string, age is untyped
:args (:string name age)

;; Mixed — a is number, b is untyped, c is string
:args (:number a b :string c)
```

**Rationale**: "Arguments" is shorter and more familiar than
"parameters" in the Erlang/Clojure tradition. The keyword-tags-next-
symbol pattern is consistent with how keywords work in `obj`:
`:name "Duncan"` — keyword tags the next value. Same visual rhythm.

### `:returns` optional — absent means void

**Decision**: If `:returns` is absent, the function returns nothing.
The compiled JS has no `return` statement — body expressions execute
for side effects only. If `:returns` is present, the last expression
in `:body` is wrapped in a `ReturnStatement`.

```lisp
;; No :returns — void, no return emitted
(func log-user
  :args (:string name)
  :body (console:log (str "User: " name)))
```

```javascript
function logUser(name) {
  console.log("User: " + name);
}
```

```lisp
;; :returns present — last expression returned
(func add
  :args (:number a :number b)
  :returns :number
  :body (+ a b))
```

```javascript
function add(a, b) {
  return a + b;
}
```

```lisp
;; :returns :void — legal, same as omitting :returns
(func init
  :args (:object config)
  :returns :void
  :body (setup config))
```

```javascript
function init(config) {
  setup(config);
}
```

**Rationale**: Absent `:returns` makes side-effectful functions
self-documenting — the absence tells you "called for effects, not
for a value." The linter can flag cases where a void function's
result is used in a value position. `:returns :void` is valid but
redundant, available for developers who prefer explicitness.

### Multi-expression bodies

**Decision**: `:body` takes all remaining expressions in the form.
Multiple expressions after `:body` are compiled as sequential
statements. No `do`/`progn` wrapper needed.

```lisp
(func process
  :args (:string input)
  :returns :object
  :body
  (console:log "processing")
  (bind cleaned (str:trim input))
  (obj :value cleaned :length (length cleaned)))
```

```javascript
function process(input) {
  console.log("processing");
  const cleaned = input.trim();
  return { value: cleaned, length: cleaned.length };
}
```

**ESTree nodes**: Multiple statements in `BlockStatement.body`.
Last statement wrapped in `ReturnStatement` when `:returns` is
present.

**Rationale**: Requiring `(do ...)` for multi-expression bodies is
unnecessary ceremony. `:body` is always the last keyword clause,
so "everything remaining" is unambiguous.

### Zero-argument positional shorthand

**Decision**: Functions with no arguments use positional syntax —
no keywords required. The body is everything after the function name.

```lisp
;; Zero-arg: positional, no keywords
(func make-timestamp
  (Date:now))

;; Zero-arg: multi-expression body
(func init
  (console:log "starting")
  (setup-db))
```

```javascript
function makeTimestamp() {
  return Date.now();
}

function init() {
  console.log("starting");
  setupDb();
}
```

A list `(...)` immediately after the function name in keyword mode
(when the function has parameters) is a compile error:

```lisp
;; COMPILE ERROR
(func greet (name)
  (str "Hello, " name))
;; Error: unexpected list after function name.
;; Use :args (name) for parameters.
```

**Rationale**: Zero-arg functions are common (thunks, initializers,
factories, timestamp generators). They have no parameters to type-
annotate and no meaningful contract clauses. The shorthand is
unambiguous — the surface compiler detects positional mode by
checking whether the form after the name is a keyword or not.

**Zero-arg return values**: For zero-arg functions, there is no
`:returns` keyword available (positional mode). The compiler applies
the same rule as kernel `function`: the last expression is the
implicit return value. This is the one case where return behavior
is implicit rather than declared. The rationale is that zero-arg
functions are typically simple value producers (thunks), and
requiring `:returns` would force keyword mode for trivial functions.

### Multi-clause polymorphic dispatch

**Decision**: `func` supports multiple clauses for dispatch on both
argument count (arity) and argument types. Each clause is a
parenthesized group containing its own `:args`, `:returns`, `:pre`,
`:post`, and `:body` keywords. The surface compiler generates
dispatch code that checks arity first (cheapest), then types.

**Syntax**:

```lisp
;; Multi-arity dispatch
(func greet
  (:args (:string name)
   :returns :string
   :body (str "Hello, " name))

  (:args (:string greeting :string name)
   :returns :string
   :body (str greeting ", " name)))
```

```javascript
function greet(...args) {
  if (args.length === 1 && typeof args[0] === "string") {
    const name = args[0];
    return "Hello, " + name;
  }
  if (args.length === 2 && typeof args[0] === "string" && typeof args[1] === "string") {
    const greeting = args[0];
    const name = args[1];
    return greeting + ", " + name;
  }
  throw new TypeError("greet: no matching clause for arguments");
}
```

```lisp
;; Multi-type dispatch
(func add
  (:args (:number a :number b)
   :returns :number
   :body (+ a b))

  (:args (:string a :string b)
   :returns :string
   :body (str a b)))
```

```javascript
function add(...args) {
  if (args.length === 2 && typeof args[0] === "number" && typeof args[1] === "number") {
    const a = args[0];
    const b = args[1];
    return a + b;
  }
  if (args.length === 2 && typeof args[0] === "string" && typeof args[1] === "string") {
    const a = args[0];
    const b = args[1];
    return `${a}${b}`;
  }
  throw new TypeError("add: no matching clause for arguments");
}
```

```lisp
;; Multi-clause with contracts
(func divide
  (:args (:number a :number b)
   :returns :number
   :pre [(not= b 0)]
   :body (/ a b))

  (:args (:number a)
   :returns :number
   :body (/ 1 a)))
```

```javascript
function divide(...args) {
  if (args.length === 2 && typeof args[0] === "number" && typeof args[1] === "number") {
    const a = args[0];
    const b = args[1];
    if (!(b !== 0))
      throw new ContractError("divide: pre-condition failed: (not= b 0) — caller blame");
    return a / b;
  }
  if (args.length === 1 && typeof args[0] === "number") {
    const a = args[0];
    return 1 / a;
  }
  throw new TypeError("divide: no matching clause for arguments");
}
```

**Detection**: The surface compiler detects multi-clause by checking
whether the form after the name starts with `(` containing `:args`
(multi-clause) or `:args` directly (single-clause).

**ESTree nodes**: `FunctionDeclaration` with rest params
(`RestElement`). `IfStatement` chain for dispatch. `MemberExpression`
for `args.length` and `args[N]`. `ThrowStatement` + `NewExpression`
for no-match error.

**Rationale**: Erlang-style function head matching is one of the most
powerful features in the BEAM ecosystem. Combined with type-based
dispatch, it provides polymorphism without classes or inheritance.
Each clause carries its own contract, making preconditions specific
to each dispatch path. The `:args` types serve double duty — they're
contracts AND dispatch discriminators.

### Multi-clause consistency rules

**Decision**: The surface compiler enforces the following rules for
multi-clause functions:

1. **All clauses must agree on return behavior.** Either all clauses
   have `:returns` or all omit it. A function cannot sometimes return
   a value and sometimes not.

2. **Overlapping clauses are a compile error.** If two clauses could
   match the same arguments (same arity, same types or both untyped),
   the surface compiler rejects the function. First-match-wins is not
   supported — ambiguity must be resolved by the developer.

3. **Clause ordering in compiled output is deterministic.** Clauses
   are ordered by: arity (longer first), then type specificity (typed
   before untyped), then declaration order. This ordering is an
   implementation detail — since overlapping clauses are rejected,
   order never affects behavior.

4. **The no-match throw is always emitted.** Even if the developer
   believes the clauses are exhaustive, the compiled JS includes
   the final `throw`. The surface compiler may elide it in a future
   version with exhaustiveness analysis.

**Rationale**: Overlapping clauses are the source of subtle bugs in
languages with first-match-wins semantics. Compile-time rejection
forces the developer to be explicit about which clause handles which
arguments. This is the same philosophy as DD-13's rejection of
duplicate macro names — ambiguity is always an error.

### Dispatch ordering rules

**Decision**: When the surface compiler generates the dispatch chain,
it orders clauses from most specific to least specific:

1. Longer arity before shorter arity
2. All-typed before partially-typed before untyped (within same arity)
3. Declaration order as tiebreaker (within same arity and specificity)

```lisp
;; These clauses are ordered by the compiler as:
;; 1. (number, number) — arity 2, all typed
;; 2. (number) — arity 1, all typed
;; 3. (a) — arity 1, untyped (if it existed, overlap with #2)
(func double
  (:args (:number a)
   :returns :number
   :body (* a 2))

  (:args (:number a :number b)
   :returns :number
   :body (* (+ a b) 2)))
```

**Rationale**: Most-specific-first ensures that typed clauses are
checked before untyped fallbacks. Since overlapping clauses are
rejected, the ordering is purely for efficiency — typed checks
(which are more restrictive) should run before less restrictive
ones to short-circuit the dispatch chain.

### `~` as return value placeholder

**Decision**: In `:post` clauses, `~` refers to the return value.
`~` is reserved as lykn's general placeholder sigil for future use
in other contexts (anonymous function shorthand, format strings).

```lisp
:post [(number? ~)
       (>= ~ 0)
       (< ~ 100)]
```

The surface compiler replaces `~` in `:post` expressions with a
gensym'd binding that captures the return value:

```lisp
;; Surface
(func clamp
  :args (:number x)
  :returns :number
  :post [(>= ~ 0) (<= ~ 100)]
  :body (Math:max 0 (Math:min 100 x)))
```

```javascript
function clamp(x) {
  const _result = Math.max(0, Math.min(100, x));
  if (!(_result >= 0))
    throw new ContractError("clamp: post-condition failed: (>= ~ 0) — callee blame");
  if (!(_result <= 100))
    throw new ContractError("clamp: post-condition failed: (<= ~ 100) — callee blame");
  return _result;
}
```

**Reader reservation**: `~` is reserved at the reader level as a
special character. It cannot appear in symbol names. The reader
dispatch entry `#~(...)` is reserved for a future placeholder
anonymous function form (not defined in DD-16).

**Rationale**: `~` is the format-string placeholder character in
Common Lisp and Erlang. Using it as lykn's placeholder sigil
connects to this tradition while extending it beyond format strings.
`~` is visually distinctive — it cannot be confused with a variable
name, operator, or keyword.

### Built-in type keywords

**Decision**: The following type keywords are available for use in
`:args` and `:returns`:

| Type keyword | Compiled check | Notes |
|---|---|---|
| `:number` | `typeof x === "number" && !Number.isNaN(x)` | Excludes NaN |
| `:string` | `typeof x === "string"` | |
| `:boolean` | `typeof x === "boolean"` | |
| `:function` | `typeof x === "function"` | |
| `:object` | `typeof x === "object" && x !== null` | Plain object, not array |
| `:array` | `Array.isArray(x)` | |
| `:symbol` | `typeof x === "symbol"` | |
| `:bigint` | `typeof x === "bigint"` | |
| `:any` | (no check) | Explicit opt-out of type checking |
| `:void` | (return type only) | Function returns nothing |
| `:promise` | `x instanceof Promise` | For async return types |

User-defined types (from `type` / ADTs, DD-17) use tag checks:
`:option` → `x != null && typeof x === "object" && "tag" in x`.
The specific tag checks depend on DD-17's ADT design.

**`:number` excludes NaN**: The check `typeof x === "number" &&
!Number.isNaN(x)` is stricter than raw `typeof`. This is intentional
— NaN propagation is a documented hazard (DLint found `$NaN` on IKEA
and eBay production sites). Raw JS number semantics including NaN
are available via `:any` with a `:pre` guard, or through `js:typeof`.

**Rationale**: The type keywords map to the most common JavaScript
type checks. `:number` is strict because the surface language
prioritizes safety. The set covers all JS primitive types plus
`:array` (which requires `Array.isArray` because `typeof []` is
`"object"`), `:any` (explicit opt-out), `:void` (no return), and
`:promise` (async).

### `fn` / `lambda` — positional anonymous functions

**Decision**: `fn` and `lambda` are always positional. They support
type annotations in the parameter list using the same keyword-tags-
next-symbol convention, but do not support `:pre`/`:post` contracts
(contracts require a function name for error messages).

```lisp
;; Untyped
(fn (x y) (+ x y))

;; Typed parameters
(fn (:number x :number y) (+ x y))

;; lambda is an alias
(lambda (x y) (+ x y))

;; Zero-arg
(fn () (Date:now))
```

```javascript
// Untyped
(x, y) => x + y

// Typed (dev mode — assertions emitted)
(x, y) => {
  if (typeof x !== "number" || Number.isNaN(x))
    throw new TypeError("anonymous fn: arg 'x' expected number");
  if (typeof y !== "number" || Number.isNaN(y))
    throw new TypeError("anonymous fn: arg 'y' expected number");
  return x + y;
}

// Production mode
(x, y) => x + y
```

The surface compiler chooses between `ArrowFunctionExpression` and
`FunctionExpression` based on context (single expression vs multi-
statement body). `fn` never binds `this` — it always compiles to an
arrow function or a function expression in strict mode.

**ESTree nodes**: `ArrowFunctionExpression` (preferred) or
`FunctionExpression`.

**Rationale**: Anonymous functions are typically short and inline.
Keyword labels would be excessive noise in callback position:
`(map (fn (:number x) (* x 2)) items)` is already readable without
`:args`/`:body` keywords. Contracts are omitted because error
messages need a function name for blame attribution — anonymous
functions have no name to report.

### `async` interaction

**Decision**: `async` wraps `func`, `fn`, and `lambda` following
DD-03's pattern. No changes to the wrapping mechanism.

```lisp
;; async func
(async (func fetch-user
  :args (:string id)
  :returns :promise
  :body (await (fetch (str "/api/users/" id)))))

;; async fn
(async (fn (:string url) (await (fetch url))))
```

```javascript
async function fetchUser(id) {
  return await fetch("/api/users/" + id);
}

async (url) => await fetch(url)
```

**ESTree nodes**: Sets `async: true` on enclosed function node.

**Rationale**: DD-03's `async` wrapper pattern is already established
and composes cleanly with both `func` and `fn`. No new design needed.

### Assertion stripping

**Decision**: Type assertions and contract checks (`:pre`/`:post`)
are emitted in dev mode and stripped in production mode. The mode is
controlled by a CLI flag: `--strip-assertions`. Default is dev mode
(assertions enabled).

```bash
# Dev mode (default) — assertions emitted
lykn compile src/app.lykn

# Production mode — assertions stripped
lykn compile --strip-assertions src/app.lykn
```

When `--strip-assertions` is active:
- Type checks in `:args` are not emitted
- `:returns` type check is not emitted
- `:pre` clauses are not emitted
- `:post` clauses are not emitted
- The `:body` compiles as if no contracts existed
- Multi-clause dispatch checks remain (they are runtime semantics,
  not assertions)

**Rationale**: Contract checks have runtime cost. Production code
should be fast. Development code should be safe. The toggle is a
deployment decision, not a code decision — the same source compiles
differently based on the flag. This follows Clojure.spec's
`*compile-asserts*` pattern and Eiffel's assertion monitoring levels.

### `ContractError` is a standard `Error`

**Decision**: Contract violations throw `new Error(message)` with a
structured message format, not a custom error class. The message
format includes the function name, the violated clause, the original
expression, and blame attribution.

```javascript
// Pre-condition violation
throw new Error("withdraw: pre-condition failed: (> amount 0) — caller blame");

// Post-condition violation
throw new Error("withdraw: post-condition failed: (>= (express (get ~ :balance)) 0) — callee blame");

// Type violation
throw new TypeError("withdraw: arg 'amount' expected number, got string");
```

Type violations use `TypeError` (JS built-in). Contract violations
use `Error` (JS built-in). No custom error classes, no runtime
dependencies.

**Rationale**: Custom error classes would introduce a runtime
dependency — the one thing lykn prohibits. `Error` with a structured
message provides all the information needed for debugging without
any dependency. The message format is designed for grep-ability:
searching for "pre-condition failed" or "caller blame" finds all
contract violations in logs.

## Rejected Alternatives

### `:params` instead of `:args`

**What**: Use `:params` for the parameter clause.

**Why rejected**: "Arguments" is shorter and more familiar in the
Erlang/Clojure tradition. While type theory distinguishes parameters
(definition site) from arguments (call site), the practical usage
favors `:args`.

### Positional syntax for parameterized functions

**What**: Allow `(func add (a b) (+ a b))` without keywords.

**Why rejected**: Positional syntax for parameterized functions
creates ambiguity when types or contracts are added. Is `(func add
(:number a b) ...)` positional with types, or is `:number` a keyword
clause? The keyword-labeled structure eliminates all ambiguity and
provides a consistent visual shape for every parameterized function.

### Separate `declare` form for type signatures

**What**: Coalton/Haskell-style `(declare add (-> :number :number
:number))` separate from the function body.

**Why rejected**: The contract-based design makes `declare`
redundant — types are part of the `:args`/`:returns` contract
clauses. A separate declaration can drift from the implementation.
A future DD may introduce `declare` for library API documentation,
but it is not needed for v0.3.0.

### Inline type annotations without keywords

**What**: `(func add (:number a :number b) :number (+ a b))` —
types in the parameter list but no `:args`/`:body` keywords.

**Why rejected**: Once contracts are part of the function form,
the keyword structure is needed to delimit `:pre`/`:post` clauses.
Mixing positional (params, return type) with keyword (`:pre`,
`:post`) creates parsing ambiguity. All-keyword is consistent.

### First-match-wins for overlapping clauses

**What**: When multiple clauses could match, use the first one.

**Why rejected**: First-match-wins is a source of subtle bugs.
Clause order becomes semantically significant, and reordering
clauses changes behavior silently. Compile-time rejection of
overlapping clauses forces the developer to be explicit, consistent
with DD-13's rejection of duplicate macro names.

### `%` as return value placeholder

**What**: Use Clojure's `%` convention for the return value in
`:post`.

**Why rejected**: `%` is a single character easily mistaken for a
typo or the modulo operator. `~` has stronger precedent as a
placeholder character (Common Lisp `format`, Erlang `io:format`)
and is more visually distinctive.

### `result` as return value name

**What**: Use the word `result` in `:post` clauses.

**Why rejected**: `result` could shadow a user binding. A sigil
(`~`) is unambiguous — it can never be a variable name.

### Contracts on `fn` / `lambda`

**What**: Support `:pre`/`:post` on anonymous functions.

**Why rejected**: Contract error messages need a function name for
blame attribution. Anonymous functions have no name. The developer
should extract the function into a named `func` if it needs contracts.

### `ContractError` custom class

**What**: Define a `ContractError` class for contract violations.

**Why rejected**: Custom error classes introduce a runtime dependency.
Standard `Error` with a structured message format provides all
necessary information without any dependency.

### `:number` including NaN

**What**: `:number` checks only `typeof x === "number"`, including
NaN.

**Why rejected**: NaN propagation is a documented hazard (DLint
found `$NaN` prices on IKEA and eBay). The surface language
prioritizes safety. `:number` excluding NaN catches this class of
bugs at function boundaries. Raw JS number semantics are available
via `:any` or `js:typeof`.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| `func` with no name | Compile error — use `fn` for anonymous | `(func :args ...)` → error |
| Zero-arg with `:args ()` | Valid keyword mode, empty arg list | `(func f :args () :returns :number :body 42)` |
| `:body` with single expression | No `do` needed | `:body (+ a b)` |
| `:body` with zero expressions | Compile error | `:body` → error: empty body |
| `:pre` with empty vector | Valid, no checks emitted | `:pre []` |
| `:post` without `:returns` | Compile error — `:post` requires a return value to check | `:post [(> ~ 0)]` without `:returns` → error |
| `~` outside `:post` | Compile error — `~` only valid in `:post` context | `:pre [(> ~ 0)]` → error: `~` not available in `:pre` |
| `~` in zero-arg positional form | Not applicable — zero-arg has no contract clauses | N/A |
| Multi-clause with mixed `:returns` | Compile error — all clauses must agree | One clause with `:returns`, one without → error |
| Multi-clause all void | Valid — all clauses omit `:returns` | Side-effectful dispatch |
| Multi-clause with overlapping args | Compile error | Two clauses both `(:number a :number b)` → error |
| Multi-clause with untyped catch-all | Valid if no overlap with typed clauses | `(:args (a b))` after `(:args (:number a :number b))` — different specificity |
| `:args` with destructuring | Supported — kernel destructuring patterns in arg position | `:args ((object name age) :array items)` |
| Unknown type keyword | Compile error unless registered by `type` (DD-17) | `:args (:foo x)` → error unless `foo` is a defined type |
| `async` wrapping multi-clause | Valid — `async` applies to entire function | `(async (func f (:args ...) (:args ...)))` |
| Exported func | `(export (func ...))` — same as kernel export wrapping | `export function ...` |
| `fn` with types in production mode | Types stripped, compiles to bare arrow | `(fn (:number x) (* x 2))` → `(x) => x * 2` |
| `:returns :void` explicitly | Valid, same behavior as omitting `:returns` | No `return` statement emitted |

## Dependencies

- **Depends on**: DD-01 (colon syntax — keywords for type tags, `js:`
  namespace), DD-02 (`function` kernel form as compilation target),
  DD-03 (`async` wrapper), DD-06 (destructuring in parameter
  patterns), DD-15 (surface language architecture, `bind`, keywords,
  functional commitment, `js:` interop namespace)
- **Affects**: DD-17 (`type` + `match` — user-defined types used as
  type keywords in `:args`), DD-19 (contracts detailed design —
  DD-16 establishes the syntax, DD-19 may extend with higher-order
  contracts), DD-20 (Rust surface compiler — multi-clause dispatch
  ordering, overlap detection, exhaustiveness analysis)

## Open Questions

- [ ] Higher-order contracts — when a contracted function is passed
  as an argument to another function, should the contract travel
  with it? Deferred to DD-19.
- [ ] `declare` form for library documentation — separate type
  signature (Coalton-style) that the surface compiler checks against
  the `func` definition. Deferred to type system DD (v0.4.0+).
- [ ] Computed/dynamic dispatch — should lykn support Clojure-style
  `defmulti` where dispatch is on an arbitrary function of the args?
  Deferred.
- [ ] Interaction between multi-clause dispatch and `async` — can
  different clauses have different async behavior? Probably not —
  the `async` flag applies to the whole function.
- [ ] Performance of multi-clause dispatch — rest params + dispatch
  chain vs overloaded functions. The Rust surface compiler could
  optimize common cases (e.g., arity-only dispatch with no type
  checks can use `arguments.length` without rest params).
- [ ] `:returns` for multi-expression body — should the compiler
  verify that only the last expression could produce the declared
  type? Deferred to type system (v0.4.0+).
- [ ] `#~(...)` reader dispatch for placeholder anonymous functions —
  reserved but not defined. Future DD.

## Version History

### v1.0 — 2026-03-27

Initial version.
