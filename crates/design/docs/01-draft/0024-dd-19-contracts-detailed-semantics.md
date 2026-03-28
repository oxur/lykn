---
number: 24
title: "DD-19: Contracts — Detailed Semantics"
author: "Duncan McGreggor"
component: All
tags: [change-me]
created: 2026-03-28
updated: 2026-03-28
state: Draft
supersedes: null
superseded-by: null
version: 1.0
---

# DD-19: Contracts — Detailed Semantics

**Status**: Decided
**Date**: 2026-03-28
**Session**: v0.3.0 surface language design

## Summary

`:pre` and `:post` each take a single boolean expression — no
vectors, no implicit AND. Multiple conditions are composed explicitly
with `and`/`or`, which provides natural short-circuit semantics.
Contract expressions are arbitrary boolean expressions with no
restrictions. Higher-order contracts are not special — contracts live
at the function definition site and fire when the function is called,
regardless of call context. Error messages preserve the original
s-expression source. This DD amends DD-16's examples to replace
vector syntax (`[...]`) with single expressions.

## Decisions

### `:pre` and `:post` each take a single expression

**Decision**: `:pre` takes one expression. `:post` takes one
expression. There is no vector form, no implicit AND across multiple
conditions. If a contract requires multiple conditions, the developer
composes them explicitly with `and`, `or`, or any other boolean
combinator.

One expression, one check, one error. The contract system has no
special-case semantics for combining conditions — it delegates to
the same `and`/`or` used everywhere else in the language.

**Syntax**:

```lisp
;; Single condition
(func abs
  :args (:number x)
  :returns :number
  :post (>= ~ 0)
  :body (if (< x 0) (- x) x))

;; Multiple conditions composed with and
(func withdraw
  :args (:number amount :account acct)
  :returns :account
  :pre (and (> amount 0)
            (<= amount (express (get acct :balance))))
  :post (>= (express (get ~ :balance)) 0)
  :body
  (assoc acct :balance (- (express (get acct :balance)) amount)))

;; Complex condition with or
(func set-priority
  :args (:string level)
  :pre (or (= level "low") (= level "medium") (= level "high"))
  :body (console:log level))

;; Both :pre and :post
(func clamp
  :args (:number x :number lo :number hi)
  :returns :number
  :pre (< lo hi)
  :post (and (>= ~ lo) (<= ~ hi))
  :body (Math:max lo (Math:min hi x)))
```

```javascript
// abs — single post-condition
function abs(x) {
  if (typeof x !== "number" || Number.isNaN(x))
    throw new TypeError("abs: arg 'x' expected number, got " + typeof x);
  const _result = x < 0 ? -x : x;
  if (!(_result >= 0))
    throw new Error("abs: post-condition failed: (>= ~ 0) — callee blame");
  return _result;
}

// withdraw — composed pre-condition
function withdraw(amount, acct) {
  if (typeof amount !== "number" || Number.isNaN(amount))
    throw new TypeError("withdraw: arg 'amount' expected number, got " + typeof amount);
  if (!(amount > 0 && amount <= acct.balance.value))
    throw new Error("withdraw: pre-condition failed: (and (> amount 0) (<= amount (express (get acct :balance)))) — caller blame");
  const _result = { ...acct, balance: acct.balance.value - amount };
  if (!(_result.balance.value >= 0))
    throw new Error("withdraw: post-condition failed: (>= (express (get ~ :balance)) 0) — callee blame");
  return _result;
}

// set-priority — or condition
function setPriority(level) {
  if (typeof level !== "string")
    throw new TypeError("setPriority: arg 'level' expected string, got " + typeof level);
  if (!(level === "low" || level === "medium" || level === "high"))
    throw new Error("setPriority: pre-condition failed: (or (= level \"low\") (= level \"medium\") (= level \"high\")) — caller blame");
  console.log(level);
}

// clamp — both pre and post
function clamp(x, lo, hi) {
  if (typeof x !== "number" || Number.isNaN(x))
    throw new TypeError("clamp: arg 'x' expected number, got " + typeof x);
  if (typeof lo !== "number" || Number.isNaN(lo))
    throw new TypeError("clamp: arg 'lo' expected number, got " + typeof lo);
  if (typeof hi !== "number" || Number.isNaN(hi))
    throw new TypeError("clamp: arg 'hi' expected number, got " + typeof hi);
  if (!(lo < hi))
    throw new Error("clamp: pre-condition failed: (< lo hi) — caller blame");
  const _result = Math.max(lo, Math.min(hi, x));
  if (!(_result >= lo && _result <= hi))
    throw new Error("clamp: post-condition failed: (and (>= ~ lo) (<= ~ hi)) — callee blame");
  return _result;
}

// All above — production mode (--strip-assertions)
function abs(x) { return x < 0 ? -x : x; }
function withdraw(amount, acct) {
  return { ...acct, balance: acct.balance.value - amount };
}
function setPriority(level) { console.log(level); }
function clamp(x, lo, hi) { return Math.max(lo, Math.min(hi, x)); }
```

**ESTree nodes**: `:pre` → `IfStatement` with negated condition
(`UnaryExpression` `!`) + `ThrowStatement` + `NewExpression`
(`Error`). `:post` → same structure, after `_result` binding
(`VariableDeclaration`). `and` → `LogicalExpression` (`&&`). `or` →
`LogicalExpression` (`||`). Error message → `Literal` string
containing serialized s-expression.

**Rationale**: Implicit AND (whether via vectors or multiple `:pre`
keywords) is implicit — it introduces contract-specific semantics
for combining conditions. Explicit `and`/`or` uses the same
composition tools available everywhere in the language. The developer
sees exactly what's being checked. The contract system is dead
simple: one expression, one check, one error. If the developer wants
to know which sub-condition of an `and` failed, they can break it
into separate functions with their own contracts, or use more
specific error handling. The contract system itself carries no
combinatorial machinery.

### Short-circuit via `and`/`or`

**Decision**: `and` and `or` in contract expressions follow standard
JavaScript short-circuit semantics (`&&` and `||`). This means later
conditions in an `and` only evaluate if earlier conditions pass.

```lisp
;; Safe: (length items) only evaluates if items is not null
:pre (and (not= items null)
          (> (length items) 0))
```

```javascript
if (!(items !== null && items.length > 0))
  throw new Error("...");
```

**Rationale**: Short-circuit is not a contract-specific feature — it
falls out naturally from `and` compiling to `&&`. The developer
orders conditions from cheapest/safest to most expensive/dependent,
exactly as they would in any boolean expression. No special
contract-aware short-circuit machinery needed.

### Contract expressions are arbitrary

**Decision**: `:pre` and `:post` expressions are arbitrary boolean
expressions. They can call functions, access properties, use any
surface form, and reference outer scope bindings via closures. The
compiler does not analyze them for purity or side effects.

```lisp
;; Calling a function in :pre
:pre (valid-email? email)

;; Property access in :pre
:pre (> (length items) 0)

;; Referencing outer scope
(bind max-retries 3)
(func retry
  :args (:number n)
  :pre (<= n max-retries)
  :body ...)

;; Complex expression in :post
:post (and (string? ~) (> (length ~) 0))
```

**Rationale**: Restricting contract expressions (e.g., to pure
predicates only) would require the compiler to determine purity —
analysis that belongs in v0.4.0+ effect tracking, not in the
contract system. Arbitrary expressions are more useful, and the
developer accepts responsibility for any side effects in their
contracts. Eiffel and Common Lisp both allow arbitrary expressions
in contracts.

### Higher-order contracts are not special

**Decision**: When a contracted function is passed as an argument to
another function, the contracts travel with it automatically. This
requires no special mechanism — contracts are emitted inside the
function body (DD-16), so they are part of the function object
itself. They fire whenever the function is called, regardless of
who calls it or how it was obtained.

```lisp
(func double
  :args (:number x)
  :returns :number
  :post (= ~ (* x 2))
  :body (* x 2))

;; Passing contracted function as argument
(map double items)
;; double's contracts fire for each call inside map
```

There is no mechanism for specifying contracts *about* function
arguments — e.g., "this callback must accept numbers and return
strings." The `:function` type keyword checks `typeof f ===
"function"` and nothing more. If a function argument needs contracts,
those contracts belong at the definition site of that function. When
a contract violation occurs, the stack trace leads back to the
contracted function, providing clear blame attribution regardless of
the call chain.

Full higher-order contract wrapping (Racket-style `->` contracts
that create wrapper functions to check argument/return types at each
call site) is deferred to v0.4.0+.

**Rationale**: Contracts at the definition site is the simplest
model that works. The function carries its contracts wherever it
goes. Stack traces provide blame attribution. Racket-style
higher-order contracts are powerful but introduce runtime wrapping
(a function passed through a contract boundary becomes a different
object), which conflicts with lykn's zero-dependency philosophy.
The simple model covers the vast majority of use cases.

### Error messages preserve source s-expressions

**Decision**: Contract error messages include the original lykn
expression serialized as a string. The Rust surface compiler
converts the `:pre`/`:post` AST back to s-expression text for
inclusion in the error message. This means error messages show the
source language, not the compiled JavaScript.

```lisp
:pre (and (> amount 0) (<= amount (express (get acct :balance))))
```

```javascript
throw new Error("withdraw: pre-condition failed: (and (> amount 0) (<= amount (express (get acct :balance)))) — caller blame");
```

**Error message format**:

```
<function-name>: pre-condition failed: <source-expression> — caller blame
<function-name>: post-condition failed: <source-expression> — callee blame
<function-name>: arg '<param-name>' expected <type>, got <actual-type>
```

The source expression is the exact text from the `.lykn` file (after
reader processing but before macro expansion). This preserves the
developer's naming and structure.

**Rationale**: Developers debug in the source language, not in
compiled output. Showing `(and (> amount 0) (<= amount balance))`
is immediately meaningful; showing `!(amount > 0 && amount <=
balance)` requires mental translation from JS back to lykn. The
source expression is available at compile time (the Rust surface
compiler has the AST), so serializing it into the error string costs
nothing at runtime — it's just a string literal.

### DD-16 amendment: vector syntax replaced

**Decision**: DD-16's `:pre` and `:post` examples used vector syntax
(`[...]`) for multiple conditions. This is amended: `:pre` and
`:post` each take a single expression. The vector syntax was
incorrect — lykn has no vector literal syntax. DD-16's examples
should be read with `and` composition instead of vectors:

**DD-16 original**:

```lisp
;; DD-16 original (SUPERSEDED)
:pre [(> amount 0)
      (<= amount (express (get acct :balance)))]
:post [(>= (express (get ~ :balance)) 0)]
```

**DD-19 replacement**:

```lisp
;; DD-19 corrected
:pre (and (> amount 0)
          (<= amount (express (get acct :balance))))
:post (>= (express (get ~ :balance)) 0)
```

Single-condition `:post` needs no `and` — it's already one
expression.

## Rejected Alternatives

### Vector syntax for multiple conditions

**What**: `:pre [(cond1) (cond2) (cond3)]` with implicit AND across
vector elements.

**Why rejected**: lykn has no vector literal syntax. Additionally,
implicit AND is implicit — it introduces contract-specific semantics
for combining conditions. Explicit `and`/`or` uses the same tools
available everywhere in the language.

### Multiple `:pre` keywords

**What**: Allow repeated `:pre` keywords, each with one expression,
implicitly AND'd:

```lisp
:pre (> amount 0)
:pre (<= amount balance)
```

**Why rejected**: Multiple `:pre` keywords are implicit AND with
different spelling. The implicit semantics are the same whether
packed into a vector or spread across keywords. One `:pre`, one
expression, explicit composition.

### Implicit AND with compiler-expanded error messages

**What**: Accept a vector/list of conditions, AND them implicitly,
but have the compiler expand `and` into sequential checks with
individual error messages per sub-condition.

**Why rejected**: Too clever. The contract system should be dead
simple — one expression, one check, one error. If granular error
reporting is needed, the developer restructures their code (separate
functions with their own contracts). The contract system carries no
combinatorial machinery.

### Restricted contract expressions (pure predicates only)

**What**: Only allow pure predicate functions in `:pre`/`:post` —
no side effects, no function calls, no property access.

**Why rejected**: Determining purity requires effect analysis, which
belongs in v0.4.0+ (effect tracking), not in the contract system.
Restricting expressions would prevent common patterns like
`(valid-email? email)` or `(> (length items) 0)`. Eiffel and Common
Lisp both allow arbitrary expressions.

### Racket-style higher-order contract wrapping

**What**: When a contracted function is passed as an argument,
wrap it in a proxy that checks argument/return types at each call
site, providing blame attribution to the caller who passed the
wrong function.

**Why rejected (deferred)**: Contract wrapping creates new function
objects (the wrapper is a different object than the original
function), which affects identity equality and creates runtime
overhead. This conflicts with lykn's zero-dependency philosophy.
The simple model — contracts live at the definition site, fire when
called, stack trace provides blame — covers the vast majority of
use cases. Deferred to v0.4.0+.

### `ContractError` custom class

**What**: Define a `ContractError` class extending `Error` for
contract violations.

**Why rejected**: Already decided in DD-16. Custom error classes
introduce a runtime dependency. Standard `Error` with a structured
message format provides all necessary information. `TypeError` for
type violations (JS built-in), `Error` for contract violations (JS
built-in). Zero dependencies.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| `:pre` expression with side effects | Valid — developer's responsibility | `:pre (and (log "checking") (> x 0))` — `log` runs in dev mode, stripped in production |
| `:pre` expression that throws | Exception propagates — not wrapped in contract error | `:pre (> (parse x) 0)` — if `parse` throws, that exception surfaces |
| `:pre` on void function | Valid — preconditions apply to all functions | `(func init :args (:object config) :pre (not= config null) :body ...)` |
| `:post` without `:returns` | Compile error (DD-16) — no return value to check | `:post` requires `~` to reference something |
| `:pre` and `:post` both present | Both checked — `:pre` first, then body, then `:post` | Standard contract checking order |
| `:pre` with `or` — all branches false | Error thrown with full `or` expression | `"pre-condition failed: (or (= x 1) (= x 2))"` |
| `:pre` with deeply nested `and`/`or` | Valid — compiles to nested `&&`/`\|\|` | Complex but supported |
| Contract on multi-clause `func` | Each clause has its own `:pre`/`:post` (DD-16) | Contracts are per-clause |
| Contract on exported function | Contracts remain in dev mode, stripped in production | Same behavior as non-exported |
| `:pre` referencing other function's contract | No mechanism — each function has its own | Composition via function calls in `:pre` |
| Long contract expression in error message | Full expression preserved regardless of length | May produce long error strings — acceptable |
| `~` in `:pre` | Compile error (DD-16) — `~` only valid in `:post` | Return value not yet available |
| Contract stripping in multi-clause dispatch | Dispatch checks remain, only `:pre`/`:post`/type checks stripped (DD-16) | Dispatch is runtime semantics, not assertions |

## Dependencies

- **Depends on**: DD-15 (surface language architecture — strict
  equality rule that `== null` is an exception to in DD-18, `js:eq`
  escape hatch), DD-16 (`func` — contract syntax `:pre`/`:post`,
  `~` placeholder, `TypeError`/`Error` format, `--strip-assertions`,
  multi-clause contracts, `:post` requires `:returns`). DD-19 amends
  DD-16 to replace vector syntax with single expressions.
- **Affects**: DD-20 (Rust surface compiler — s-expression
  serialization for error messages, `and`/`or` compilation in
  contract context)

## Open Questions

- [ ] Higher-order contract wrapping (Racket-style `->` contracts) —
  deferred to v0.4.0+. Requires design for wrapper identity,
  performance, and blame attribution across module boundaries.
- [ ] Contract interaction with `async` — can `:post` check the
  resolved value of a promise? Currently `:post` checks the return
  value, which for `async` functions is a `Promise`. Checking the
  resolved value requires `await`ing in the post-condition, which
  has implications for error handling and stripping. Deferred.
- [ ] Contract documentation generation — should the Rust surface
  compiler emit contract expressions into JSDoc comments or `.d.ts`
  files for library documentation? Deferred to DD-20.
- [ ] Contract inheritance in multi-clause dispatch — DD-16 says
  each clause has its own contracts. Should there be a way to specify
  "base" contracts that apply to all clauses? Deferred — explicit
  per-clause contracts are sufficient for v0.3.0.
- [ ] Interaction with future effect tracking — when effect types
  are introduced (v0.4.0+), should the compiler verify that `:pre`
  and `:post` expressions are pure? This would prevent side-effectful
  contracts from causing different behavior between dev and production
  mode.

## Version History

### v1.0 — 2026-03-28

Initial version. Amends DD-16 v1.0: replaces vector syntax in
`:pre`/`:post` examples with single expressions using `and`/`or`
composition.
