---
number: 50
title: "Position-Aware Compilation of Conditional and Block Forms"
author: "emitting an"
component: All
tags: [change-me]
created: 2026-05-03
updated: 2026-05-13
state: Final
supersedes: null
superseded-by: null
version: 1.0
---

# Position-Aware Compilation of Conditional and Block Forms

## Status

Proposed (pending Duncan/CDC review).

Scope expanded during 2026-05-02 CDC review from "`if` in expression
position" to "position-aware compilation of conditional and block
forms." The composite rule covers `if`, `match`, `if-let`, `when-let`,
and `do`. The original `if` case (V-04 / V-05) is the most visible
instance.

## Context

- **V-04** (nested `if` as `func` body return expression) and **V-05**
  (`if` as `bind` initializer) emit JS `if`-statements in expression
  position, which is invalid JS. Both compilers exhibit the bug per
  M6.
- The Rust compiler has a single-case partial implementation: ternary
  is emitted for the `js:eq` form specifically, but the rule is not
  generalised. The JS compiler has no position-tracking at all
  (`compiler.js:245` `if` handler unconditionally produces
  `IfStatement`).
- Lykn surface forms already distinguish:
  - `if` — conditional form (today: statement only)
  - `?` — ternary expression form (`compiler.js:762`,
    `docs/guides/00-lykn-surface-forms.md:753`)
- `match`, `if-let`, and `when-let` are **already IIFE-wrapped**
  per the surface-forms guide (lines 450, 505) — so they're already
  expression-valid in any position.
- `cond` is **not** a current lykn surface form. Backlog item only,
  to be considered if `match` proves insufficient for any use case.
- The bug shape: `if` (and `do` in some constructions) is a statement
  form being used in expression position. The compiler must resolve
  this — by emitting an expression form, by IIFE-wrapping when
  necessary, or by erroring on incomplete user intent.

## Options analyzed

### Option A: Always-ternary

Compile every `(if cond then else)` to a ternary `cond ? then : else`,
regardless of position.

**Pros:**

- Simple uniform rule. No position-tracking needed in the compiler.
- `if` becomes sugar for `?` in all positions.

**Cons:**

- Ternary doesn't support side-effects-with-no-value cleanly.
  `(if cond (do-thing))` with no else branch would need a synthetic
  `undefined` or `null` else.
- Guard-clause `if` (for early validation, logging, etc.) becomes
  awkward: `cond ? doThing() : undefined` is valid JS but ugly and
  not what the author intended.
- Ternary cannot host statement-form branches (`throw`, `return`,
  `break`, `continue`, nested `if`-statements). Those are invalid
  JS as ternary operands.
- Conflates statement and expression semantics — the distinction
  between `if` and `?` at the surface level becomes meaningless.

### Option B: Position-aware compilation

Compile `if` to ternary `? :` when in expression position; compile
to `if`-statement when in statement position. Wrap in IIFE when an
expression-position branch is a statement form (`throw`, `return`,
`do`, etc.).

**Pros:**

- Most pragmatic — existing user code works without rewriting.
  `(bind x (if cond a b))` compiles to `const x = cond ? a : b`.
- Preserves `if` as statement for guard clauses and side-effect
  branches.
- IIFE fallback for statement-form branches mirrors the existing
  pattern used by `match`, `if-let`, `when-let`.
- No migration cost: broken code becomes correct code silently.

**Cons:**

- Requires the compiler to track expression vs statement position
  for every form. This is non-trivial but well-understood
  (most JS transpilers do it).
- The rule "same source form, different output depending on
  position" could surprise developers debugging compiled output.
- Need to precisely define "expression position" (see Decision
  Rule 1 — sub-section "Expression position").

### Option C: Surface-form discipline (compile-error on misuse)

Treat `if` as strictly statement-form. When the compiler detects
`if` in expression position, emit a compile error:
`"if" is a statement form; use "?" for conditional expressions`.

**Pros:**

- Cleanest surface semantics: `if` is statement, `?` is expression,
  no overlap.
- Educates users to write the correct form for their intent.
- Simplest compiler implementation (detect and error; no
  position-aware code generation).
- Makes the distinction between `if` and `?` load-bearing in the
  language.

**Cons:**

- Migration cost: users must find every broken site and rewrite.
  Option B requires no user action.
- The "migration" is from broken code (never produced valid JS) to
  correct code — but Option B achieves the same end with no user
  burden.
- More verbose at every site: `(? cond a b)` instead of
  `(if cond a b)`.

### Option D: Partial-ternary with diagnostic

Compile `if` to ternary in expression position (like B) but emit a
soft warning suggesting users explicitly use `?`. Transitional
approach.

**Pros:**

- Code works (like B) but educates (like C).
- Allows a future release to tighten to C (error) after users have
  migrated.

**Cons:**

- Warnings are noise if you can't act on them immediately.
- The "transitional" framing implies a future breaking change —
  better to just decide now.
- Complexity of both B and C without the clarity of either.

## Decision

**Option B: Position-aware compilation,** with explicit rules for
each branch shape and a forbidden case (no-else `if` in expression
position).

### Rule 1: `if` — position-aware emission

The compiler classifies each occurrence of `(if cond consequent alternate)`
by position:

- **Statement position:** emit `IfStatement` —
  `if (cond) { consequent } else { alternate }`. Side-effect branches
  work as expected.
- **Expression position, both branches are pure expressions:** emit
  `ConditionalExpression` — `cond ? consequent : alternate`.
- **Expression position, one or both branches are statement forms**
  (`throw`, `return`, `break`, `continue`, nested `if`-as-statement,
  `do` blocks, etc.): IIFE-wrap the whole `if` —
  `(() => { if (cond) { return consequent } else { return alternate } })()`.

The classification of "pure expression vs statement form" is made on
the **compiled** branch: if the compiled node's ESTree (or kerneljs)
type is an expression type (`Literal`, `Identifier`, `BinaryExpression`,
`CallExpression`, `ConditionalExpression`, `ArrowFunctionExpression`,
etc.), the branch is pure-expression. If it's a statement type
(`IfStatement`, `ThrowStatement`, `ReturnStatement`, `BlockStatement`,
etc.), the branch requires IIFE wrapping.

This compile-then-check approach handles nested cases naturally: a
`do` block in a branch (Rule 4) compiles first to a CallExpression
(IIFE), which is then a pure expression for Rule 1's purposes.
Result: `cond ? IIFE-of-do : other` — ternary still applies.

**Expression position is defined as:**

- Initializer of a `bind` / `const` / `let` / `var` declaration
- The body-expression of a `func` / `fn` / `=>` (last expression,
  used as return value)
- Argument position in a function call
- Right-hand side of an assignment
- Value position in an object literal
- Element of an array literal
- Operand of another expression (arithmetic, comparison, etc.)
- The condition of an `if` / `while` / `for` is **not** expression
  position for the purposes of this DD — it's a boolean-context
  position, but the value-producing classification is the same
  (it must be an expression, which `if` already would be after
  this rule applies).

**IIFE semantics — acknowledged implication:** IIFE-wrapped branches
have lexical scope semantics for `return`, `break`, `continue`. These
forms inside an IIFE-wrapped branch return / break / continue from the
IIFE itself, not from the enclosing function or loop. This is the
same constraint that `match` / `if-let` / `when-let` already have
today (per the surface-forms guide). Users wanting
return-from-enclosing-function semantics should use `if` in statement
position (i.e., not as a `bind` initializer or value-producing
expression). The compiler does not warn about this; it's accepted as
a known consequence of IIFE wrapping, mirrored across all
IIFE-using forms.

### Rule 2: `if` in expression position with no else — compile error

If `(if cond consequent)` (no else branch) appears in expression
position, the compiler emits a compile error:

```
if in expression position requires an else branch.
Add an else branch, or restructure as a statement.
```

**Rationale:** `(if cond value)` in a value position is more often a
forgotten else than a deliberate "I want undefined when false." A
synthetic `cond ? value : undefined` would silently produce
`undefined` for the false case — a footgun. Erroring catches the
forgotten-else at compile time. Users who genuinely want undefined-
on-false can write `(? cond value undefined)` or
`(if cond value undefined)` — the meaning is then explicit.

`(if cond consequent)` in **statement** position remains valid (Case I
in the analysis above) — it's a side-effect-only guard with no value
needed.

### Rule 3: Other conditional surface forms

The position-aware rule applies uniformly to all current conditional
surface forms:

| Form         | Today                                | After this DD                     |
|--------------|--------------------------------------|-----------------------------------|
| `if`         | `IfStatement` only (broken in expr)  | Position-aware (Rule 1 + Rule 2)  |
| `match`      | Always IIFE                          | Always IIFE (unchanged)           |
| `if-let`     | Always IIFE                          | Always IIFE (unchanged)           |
| `when-let`   | Always IIFE                          | Always IIFE (unchanged)           |
| `?`          | Always `ConditionalExpression`       | Always `ConditionalExpression` (unchanged) |

`match` / `if-let` / `when-let` are already expression-valid via IIFE
in any position; no change needed for them. `?` remains the explicit
ternary form.

`cond` is not currently a lykn surface form. Backlog item only — if
future use cases show `match` cannot serve, a follow-up DD adds `cond`
under the same position-aware rule.

### Rule 4: `do` blocks in expression position

`(do stmt1 stmt2 ... final)` — a sequence-of-expressions form whose
value is the value of the final expression — follows the same
position-aware emission:

- **Statement position:** emit a `BlockStatement` containing each
  expression as a statement.
- **Expression position:** IIFE-wrap, with the final expression
  returned —
  `(() => { stmt1; stmt2; ...; return final; })()`.

This makes `do` expression-valid in any position. The same
IIFE-semantics caveat from Rule 1 applies (`return` / `break` /
`continue` inside an IIFE-wrapped `do` returns from the IIFE).

### Rule 5: Style guidance

For SKILL.md and the surface-forms guide (updated as part of M8):

> **Prefer `?` for expression position, `if` for statement
> position.**
>
> The compiler treats `(if cond a b)` and `(? cond a b)` as
> functionally equivalent in expression position (both compile to
> ternary). The preferred style is to use `?` when the conditional
> is the value of an expression, and `if` when it's a statement
> (guard clause, side-effect branch, etc.). This makes intent
> explicit at the source level.
>
> **Note for LLM-generated code:** treat this preference as a hard
> rule (always use `?` in expression position, always use `if` in
> statement position). LLMs flatten soft style preferences toward
> uniform compliance, so explicit phrasing makes the convention
> reliable in generated output.

This is style guidance, not a compiler-enforced rule — both forms
remain functionally equivalent in expression position.

### Rationale (summary)

1. **Pragmatic user experience.** `(bind x (if cond a b))` is a
   natural thing to write in a Lisp; making it Just Work is the
   least-surprising behaviour. The user wrote a conditional
   expression; the compiler produces a conditional expression in JS.
2. **No migration cost.** Code triggering V-04 / V-05 today is
   already broken (invalid JS). Making it correct is a pure fix, not
   a breaking change.
3. **IIFE precedent already exists.** `match` / `if-let` /
   `when-let` already always-IIFE; extending the same wrapping to
   statement-form branches inside `if` (and to `do` in expression
   position) is consistent with the existing language design.
4. **Compile-error for forgotten-else.** Silent `undefined` for the
   false case would be a footgun; erroring catches the bug at
   compile time.
5. **Style guidance preserves intent.** `?` and `if` remain
   distinguishable at the source level — users who want explicit
   expression-form intent can keep using `?`; the compiler treats
   them as equivalent in expression position.

### Why other options rejected

- **Option A (always-ternary):** Breaks side-effect `if`
  (guard clauses), can't host statement branches at all (would
  produce invalid JS for `(if cond (throw err) ok)`), conflates
  statement and expression semantics.
- **Option C (compile-error):** Migration cost without benefit. The
  broken code isn't "working and about to break" — it's already
  broken. Option B fixes it silently with no user action; Option C
  would just produce a different error.
- **Option D (transitional warning):** Complexity without
  commitment. If B is the right answer (it is), commit.

## Implementation outline

**JS compiler** (`packages/lang/compiler.js`):

- The current `if` handler (line 245) unconditionally produces
  `IfStatement`. Replace with a position-aware emitter.
- Add a position-context parameter (or threading via a small
  context object) through `compileExpr`. Position values: `statement`,
  `expression`. Most forms inherit position from their parent; some
  (e.g., `bind` initializer, `=>` body, function-call arguments)
  override to `expression`.
- For `if` in expression position: compile both branches in expression
  context, classify the resulting node types, emit ternary or IIFE
  per Rule 1.
- For `if` in expression position with no alternate: emit compile
  error per Rule 2.
- For `do` in expression position: IIFE-wrap per Rule 4.
- The `?` handler (line 762) is unchanged; it's the explicit
  expression form.
- The IIFE shape: `CallExpression` wrapping `ArrowFunctionExpression`
  with `BlockStatement` body containing the statements followed by
  `ReturnStatement` of the final value.

**Rust compiler** (`crates/lykn-lang/src/`):

- Primary file: `crates/lykn-lang/src/emitter/forms.rs` (form-specific
  emission, including the `if` handling and the existing partial
  `js:eq` ternary path — useful breadcrumb for understanding what
  triggers the existing single-case ternary).
- Supporting files:
  - `crates/lykn-lang/src/codegen/emit.rs` — general code emission;
    position threading lives here.
  - `crates/lykn-lang/src/codegen/precedence.rs` — operator
    precedence for ternary; wraps inner ternaries with parens where
    needed (nested `(cond1 ? a : (cond2 ? b : c))` shape).
- Same position-context parameter through the emitter; same
  classification rule for IIFE wrapping. The existing partial
  `js:eq` ternary path is the starting point but not a generalisable
  framework — the position-tracking infrastructure is new in both
  compilers.

**Test strategy:**

- **V-04 / V-05 regression:** repros from M6 must now produce valid
  JS. V-04 → nested ternary; V-05 → ternary as `bind` initializer.
  Cross-compiler equivalence asserted.
- **Statement-position `if`:** `(if cond (console:log "x"))` (no
  else) compiles to `if (cond) { console.log("x"); }` — unchanged.
- **Expression-position pure-expression branches:**
  `(bind x (if cond a b))` → `const x = cond ? a : b`.
- **Expression-position statement branch:** `(bind x (if cond a (throw err)))`
  → IIFE wrapping. Verify the throw propagates (caught by outer
  try/catch).
- **Expression-position no-else:** `(bind x (if cond value))`
  triggers the compile error from Rule 2.
- **Nested ternary:** `(bind x (if c1 a (if c2 b c)))` → properly
  parenthesised nested ternary.
- **`do` in expression position:** `(bind x (do a b c))` → IIFE
  returning `c`.
- **`match` / `if-let` / `when-let` in any position:** unchanged
  (always IIFE); regression tests confirm no degradation.
- **IIFE return-semantics (acknowledged constraint):** test
  documenting that `(bind x (if cond (return early) other))` returns
  `early` from the IIFE and binds `x` to `early`, NOT returning
  early from the enclosing function. Same as `match`/`if-let`/`when-let`
  today.

## Backward compatibility considerations

This is a **breaking change** for any code that currently compiles to
invalid JS. Since invalid JS doesn't run, no functioning consumer
code exists that depends on the broken output. The change ships with
0.6.0; no migration path needed.

Code that currently uses `?` in expression position is unchanged.
Code that uses `match` / `if-let` / `when-let` is unchanged.

## Relationship to other DDs / surface forms

- **DD-49 (identifier mapping):** independent — DD-49 governs
  identifiers, DD-50 governs control flow.
- **`?` (ternary surface form):** unchanged. Remains the explicit
  expression form.
- **`match` / `if-let` / `when-let`:** unchanged. Already always-IIFE.
- **SKILL.md:** Rule 5's style guidance should be added after M8.
- **Surface forms guide (`docs/guides/00-lykn-surface-forms.md`):**
  the `if` form's documentation should be updated to note position-
  aware compilation; the `?` form's documentation should reference
  the style preference.

## Open questions

- **`cond` form addition (backlog).** Not currently a lykn surface
  form. If future use cases show `match` is insufficient for
  multi-branch conditional expressions, a follow-up DD adds `cond`
  under the same position-aware rule.
- **Source-line preservation.** When `if` becomes ternary or IIFE,
  source-line tracking for stack traces / debuggers may shift.
  Source maps are out of scope for this DD; tracked separately.
- **Formatter behaviour.** Whether `lykn fmt` should auto-rewrite
  `(if cond a b)` in expression position to `(? cond a b)` per the
  Rule 5 style preference is out of scope; tracked as a fast-follow.
