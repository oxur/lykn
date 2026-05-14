---
number: 49
title: "Identifier Mapping (lykn → JS)"
author: "far
   the"
component: All
tags: [change-me]
created: 2026-05-03
updated: 2026-05-13
state: Final
supersedes: null
superseded-by: null
version: 1.0
---

# Identifier Mapping (lykn → JS)

## Status

Proposed (pending Duncan/CDC review).

Scope expanded during 2026-05-02 CDC review from "`?`-suffix only" to
"full identifier-mapping rule." The composite rule generalises to all
non-JS-identifier characters lykn allows in symbol names; the original
`?`-suffix case is the most common instance.

## Context

- **V-02** (`(func valid? ...)` → `function valid?(x)`) and **V-03**
  (`(import "./x.js" (valid?))`) pass `?` through verbatim, producing
  invalid JS identifiers. Both compilers exhibit the same bug per M6.
- The `?` suffix is the documented predicate-naming convention in lykn
  surface syntax (SKILL.md naming conventions:
  `Predicate functions | ? suffix | even?, valid?, has-items?`).
- The `!` suffix is the mutation-ops convention (`swap!`, `reset!`,
  `set!`).
- Lykn's reader follows the Lisp tradition of permissive identifier
  characters: `?`, `!`, `*`, `+`, `=`, `<`, `>`, `&`, `%`, `$`, `/`,
  and multi-character combinations like `->`, `<-`, `->>` are all
  valid in symbol names. Most are illegal in JS identifiers.
- Per philosophy Principle 3 (compiler-owned output quality), the
  compiler must produce valid JS. Today it fails on any identifier
  containing these characters.
- The existing lisp-case → camelCase converter
  (`packages/lang/compiler.js:78` `toCamelCase`;
  `crates/lykn-lang/src/codegen/names.rs:15` `to_camel_case`) handles
  the `-` → camel boundary cleanly. Other punctuation is the gap.

## Options analyzed

### Option A: Predicate naming convention (`valid?` → `isValid`)

Transform `?`-suffix predicates to `is`-prefix camelCase: `valid?` →
`isValid`, `empty?` → `isEmpty`, `even?` → `isEven`.

**Pros:**

- Produces idiomatic JS identifiers that match the JavaScript
  `is`-prefix convention.
- The `is` prefix is already used in lykn examples (`is-void`,
  `is-empty`, `is-even`) — this extends the pattern to `?`-suffixed
  forms.
- Readable in stack traces and debugging: `isValid` is immediately
  understandable.

**Cons:**

- Non-obvious round-trip: developers debugging compiled JS see
  `isValid` but their source says `valid?`. The mapping isn't
  positional (suffix to prefix). Mitigated by error-message bridging
  (Decision Rule 7) until source maps land.
- Collision risk: a user could define both `valid?` and `is-valid` —
  both would compile to `isValid`. Detected at compile time
  (Decision Rule 6).
- Doesn't cover non-`?` punctuation. The composite Decision combines
  this option with Option B's mechanical escape for everything else.

**Examples:**

```
valid?      → isValid
empty?      → isEmpty
even?       → isEven
has-items?  → hasItems     (predicate-prefix detection — see Decision Rule 1)
```

### Option B: Mechanical escaping (`valid?` → `valid$q` or `valid_p` or `validQMARK`)

Replace each non-JS-identifier character with a fixed escape
abbreviation: `$q`, `_p`, or all-caps `QMARK`.

**Pros:**

- Deterministic, reversible 1:1 mapping. No ambiguity. No collision
  risk.
- Works for any punctuation (extends to `!`, `*`, `+`, `<`, `>`, etc.
  — every Lisp-tradition character has a mechanical mapping).
- Source maps can trivially reverse the mapping for debugging.

**Cons:**

- Uglier identifiers when applied to the common case (`valid$q`,
  `even_p` — JS programmers wouldn't write these).
- Not idiomatic JS for predicates. Predicates are the most common
  punctuation case; treating them mechanically misses the chance to
  produce idiomatic output.

**Examples:**

```
valid?       → validQMARK    (uglier than isValid)
swap!        → swapBANG      (uglier than swap)
*globals*    → STARGlobalsSTAR  (only sensible mapping for earmuffs)
string->json → stringTOjson  (uglier than stringToJson)
```

The composite Decision adopts Option B's mechanism for cases where
Option A doesn't apply (i.e., non-`?`-suffix and non-`!`-suffix
punctuation, including embedded and leading positions).

### Option C: Author-controlled mapping via metadata

Allow authors to specify the JS name via a metadata annotation:

```lykn
(func valid? :js-name "isValid" :args (:any x) :body ...)
```

**Pros:**

- Full control. No surprises.
- Authors choose what's idiomatic for their API.

**Cons:**

- Adds syntax complexity to every predicate definition.
- Boilerplate-heavy for common cases.
- Doesn't help import bindings (V-03) — you can't annotate
  `(import "..." (valid?))`.
- Defeats the purpose of a naming convention — if you have to spell
  it out, why have the convention?

### Option D: Configurable scheme via `project.json`

A project-level setting in `project.json` or `deno.json`:

```json
{ "lykn": { "identifierMapping": "is-prefix" | "mechanical" } }
```

**Pros:**

- Per-project control without per-function annotation.
- Different teams can choose their preference.

**Cons:**

- Two projects using different schemes produce different compiled
  output for the same source — interop confusion.
- Adds configuration surface for what should be a simple, predictable
  compiler behaviour.
- Complexity budget spent on a marginal choice.

## Decision

A composite rule that combines **Option A** (predicate-naming for
trailing `?`/`!`) with **Option B** (mechanical uppercase escapes for
embedded / leading / non-`?`-non-`!` punctuation), plus a small
override registry for language-primitive forms.

The composite preserves idiomatic JS for the common cases and
mechanically supports everything lykn's reader allows.

### Rule 1: Trailing `?` — predicate naming convention

If an identifier ends in `?`:

1. Strip the trailing `?`.
2. If the remainder (after stripping) is **empty**, undo: do not
   strip and fall through to Rule 3 (treat the lone `?` as an
   embedded escape). This handles pure-punctuation identifiers like
   `?`.
3. If the remainder starts with one of the predicate prefixes —
   `is-`, `has-`, `can-`, `should-`, `will-`, `does-`, `was-`,
   `had-` — apply lisp-case → camelCase as normal.
4. Otherwise, prepend `is-` and apply lisp-case → camelCase.

Examples:

```
valid?         → isValid
empty?         → isEmpty
even?          → isEven
has-items?     → hasItems       (prefix already present)
is-void?       → isVoid         (prefix already present)
does-match?    → doesMatch      (prefix already present)
was-modified?  → wasModified    (prefix already present)
?              → QMARK          (degenerate; falls through to Rule 3)
```

### Rule 2: Trailing `!` — strip

If an identifier ends in `!`:

1. Strip the trailing `!`.
2. If the remainder is empty, undo and fall through to Rule 3.
3. Apply lisp-case → camelCase to the remainder.

The `!` is a source-side mutation marker with no JS-visible effect.

Examples:

```
swap!    → swap
reset!   → reset
set!     → set
!        → BANG    (degenerate; falls through to Rule 3)
```

### Rule 3: Embedded / leading punctuation — uppercase abbreviation

Any other punctuation character appearing in an identifier — including
punctuation in mid- and leading positions, and trailing punctuation
other than `?`/`!` — maps to an uppercase abbreviation, inserted at a
camelCase boundary.

The committed abbreviation table:

| Character | Abbreviation | Notes                                       |
|-----------|--------------|---------------------------------------------|
| `?`       | `QMARK`      | (only when not in trailing position)        |
| `!`       | `BANG`       | (only when not in trailing position)        |
| `*`       | `STAR`       | earmuffs: `*globals*` → `STARGlobalsSTAR`   |
| `+`       | `PLUS`       |                                             |
| `=`       | `EQ`         |                                             |
| `<`       | `LT`         |                                             |
| `>`       | `GT`         |                                             |
| `&`       | `AMP`        |                                             |
| `%`       | `PCT`        |                                             |
| `/`       | `SLASH`      |                                             |
| `->`      | `To`         | longest-match before per-char escape        |
| `<-`      | `From`       | longest-match before per-char escape        |

**Note on `$`:** `$` is a valid JavaScript identifier character (per
ECMAScript identifier-name production: identifiers may include letters,
digits, `$`, and `_`). Lykn passes `$` through unchanged in
identifiers; no escape is applied. This is a refinement applied during
the initial implementation of DD-49 — the original abbreviation table
listed `$ → DOLLAR`, but escaping `$` would have broken lykn's internal
macro API (`$array`, `$sym`, `$gensym`).

Multi-character combinations (`->`, `<-`) match longest-first. So
`string->json` matches `->` as `To` rather than per-char `GT`. The
casing of the multi-char arrows is **mixed-case** (`To`, `From`)
rather than the all-caps shape of single-char escapes — this is
intentional: arrow forms are common idioms producing readable names.
All single-char escapes use **all-caps** (`QMARK`, `BANG`, `STAR`),
which is the lykn standard for escape abbreviations.

The escape acts as an **implicit camelCase boundary**: the next
alphanumeric character (if any) is uppercased. This is what produces
`STARGlobalsSTAR` from `*globals*` and `funcQMARKThing` from
`func?-thing`.

Examples:

```
?valid           → QMARKValid
func?-thing      → funcQMARKThing
set!-state       → setBANGState
*globals*        → STARGlobalsSTAR
+constant+       → PLUSConstantPLUS
string->json     → stringToJson
json<-string     → jsonFromString
=val             → EQVal
&rest            → AMPRest
%scratch         → PCTScratch
$ref             → $ref              ($-passthrough; see note above)
path/to          → pathSLASHTo
```

Pure-punctuation identifiers map to the abbreviation alone:

```
?    → QMARK
!    → BANG
*    → STAR
->   → threadFirst    (Rule 4 — macro-name override)
->>  → threadLast     (Rule 4 — macro-name override)
```

### Rule 4: Macro-name override registry

A small set of forms get hand-picked JS names rather than mechanical
outputs, because they are language-standard names whose JS-side name
is part of the language design rather than a mechanical derivation.

Initial registry:

| Lykn form | JS name       |
|-----------|---------------|
| `->`      | `threadFirst` |
| `->>`     | `threadLast`  |

The override applies when the form appears as a **complete**
identifier (i.e., the macro head). When `->` or `<-` appear
**embedded in** a longer identifier (like `string->json`), Rule 3
applies (`To`/`From`).

This list is extensible via small follow-up DDs as new threading or
arrow-named macros are introduced.

### Rule 5: Doubled trailing punctuation

If an identifier has multiple trailing punctuation characters (e.g.,
`valid??`, `swap!!`), only the **final** character gets its
trailing-rule treatment; earlier characters are treated as embedded
(Rule 3).

Examples:

```
valid??    → isValidQMARK
swap!!     → swapBANG
swap!?     → isSwapBANG
```

These are unusual but accepted; lykn's reader allows them and the
rule produces deterministic output.

### Rule 6: Collision detection

Two collision classes are introduced by Rules 1–3:

- **Class A** — `valid?` and `is-valid` both compile to `isValid`.
- **Class B** — `has-items?` and `has-items` both compile to
  `hasItems` (and similarly for any `<prefix>-foo?` / `<prefix>-foo`
  pair where `<prefix>` is in the prefix list).

The compiler must error at compile time when both forms exist in the
same module scope, pointing at both source forms. Detection logic:
after applying Rules 1–4, the compiler checks for duplicate JS-side
identifiers within the module's exported and module-scope binding
namespace.

### Rule 7: Error-message format

When the compiler emits runtime type-check error strings (currently
of the form `"name: return 'arg' expected type, got typeof"`), the
format becomes:

```
"<jsName> (<lyknSourceName>): return 'arg' expected type, got typeof"
```

Example, V-02 after fix:

```js
function isValid(x) {
  if (typeof x !== "string")
    throw new TypeError("isValid (valid?): arg 'x' expected string, got " + typeof x);
  const result__gensym0 = ...;
  if (typeof result__gensym0 !== "boolean")
    throw new TypeError("isValid (valid?): return 'result__gensym0' expected boolean, got " + typeof result__gensym0);
  return result__gensym0;
}
export {isValid};
```

The format applies to **all** compiler-emitted runtime error strings
that name a binding or function: type-check assertions, `assert`
macro outputs, contract messages, and similar.

**Bridging guard:** the parenthesized `js_name (lykn_name)` form
is rendered only when the source identifier contains non-hyphen
punctuation (`?`, `!`, `*`, `+`, `=`, `<`, `>`, `&`, `%`, `/`, or
arrow combinations like `->`, `<-`). Pure lisp-case → camelCase
transformations (`my-func` → `myFunc`, `get-element-by-id` →
`getElementById`) do not trigger bridging — the rule prevents
error-message noise for the common case where the transformation
is mechanically obvious (every lisp-case name → camelCase). The
bridging is reserved for transformations where the source name
and JS name diverge non-trivially: `valid?` → `isValid` (suffix
flips to prefix), `swap!` → `swap` (character stripped),
`*globals*` → `STARGlobalsSTAR` (escape-substituted), etc.

Contrasting example: `(func my-func :args (:number n) :body n)`
produces `"myFunc: arg 'n' expected number, got "` — no
parenthesized form, since `my-func` → `myFunc` is pure camelCase.

**Rationale:** until source maps are wired up, lykn users debugging
compiled JS need both the JS-side name (which they see in stack
traces and the actual identifier) and the lykn-source name (which
they wrote and recognize). Carrying both bridges the gap. When source
maps land in a future milestone, this format can be revisited — the
parenthesized lykn name may become redundant if stack traces resolve
to lykn locations.

### Rule 8: Import-binding emission

`(import "./x.js" (valid?))` compiles to:

```js
import { isValid } from "./x.js";
```

The import binding name is the JS-side name (after applying Rules
1–4). If the upstream module does not export `isValid`, the standard
JS "not exported" error fires at runtime — that's a normal
consumer-side error, not an identifier-mapping concern.

`(import "./x.js" (valid? :as ok?))` compiles to:

```js
import { isValid as isOk } from "./x.js";
```

Both names get the rule treatment.

### Composition algorithm (reference)

For implementers, the rules compose as a single left-to-right pass:

1. **Macro-override phase.** If the entire identifier matches an
   entry in the macro-override registry, return the registered name.
2. **Trailing-rule phase.** Examine the final character of the
   original identifier. `?` → drop, mark "predicate." `!` → drop. If
   the remainder is empty, undo. Otherwise no trailing action.
3. **Prefix-detection phase.** If "predicate" mode and the remainder
   doesn't start with one of the predicate prefixes, prepend `is-`.
4. **Walk phase.** Left-to-right with a `cap_next` flag.
   - At each position, try longest-match against the multi-char arrow
     table. If matched, emit the abbreviation and set `cap_next`.
   - Else if the character is in the single-char abbreviation table,
     emit the abbreviation and set `cap_next`.
   - Else if the character is `-`, set `cap_next` (don't emit).
   - Else (alphanumeric): emit uppercased if `cap_next`, else as-is;
     clear `cap_next`.

Pure-walk verification (a few cases):

```
*globals*        →  *  → STAR (cap)  →  g → G  →  lobals → lobals  →  * → STAR (cap)
                 →  STARGlobalsSTAR
string->json     →  string → string  →  -> → To (cap)  →  j → J  →  son → son
                 →  stringToJson
func?-thing      →  func → func  →  ? → QMARK (cap)  →  - → (cap)  →  t → T  →  hing
                 →  funcQMARKThing
```

### Rationale (summary)

1. **Idiomatic output for the common case.** Trailing `?` is by far
   the most common lykn-specific punctuation. Mapping it to
   `is`-prefixed camelCase produces JS that looks hand-written.
2. **Mechanical fallback for everything else.** Lisp tradition allows
   liberal punctuation in identifiers; the language shouldn't ban
   them. Uppercase abbreviations mark the escape clearly and are
   deterministic.
3. **Macro-name overrides for language primitives.** Threading macros
   are named entities in the language design; their JS-side names are
   chosen, not derived.
4. **Collision detection makes the rule safe.** The two collision
   classes are detectable at compile time; users get a clear error
   rather than silent shadowing.
5. **Error-message bridging until source maps.** Until lykn has
   source-map support, the bridge between JS names and lykn names
   lives in error strings.

### Why other options rejected (in their pure form)

- **Pure Option B (mechanical for everything, including `?`):** would
  produce non-idiomatic identifiers for the most common case.
  `validQMARK` is uglier than `isValid`; the predicate convention
  earns its keep. The composite Decision uses Option B's mechanism
  only where Option A doesn't apply.
- **Option C (author-controlled mapping):** too much boilerplate;
  doesn't solve V-03 (import bindings); defeats the convention.
- **Option D (configurable scheme):** adds configuration surface for a
  choice that should be predictable and uniform. Two projects
  compiling the same source differently is a footgun.

## Implementation outline

**JS compiler** (`packages/lang/compiler.js`):

- Existing `toCamelCase` (line 78) handles the `-` → camel boundary.
  Replace with a unified walker that implements all four phases of
  the composition algorithm above.
- Add macro-name override registry as a small lookup object consulted
  in the macro-override phase.
- Update error-message-emitting sites to use the
  `<jsName> (<lyknSourceName>)` format. Search the compiler for the
  string `expected` to locate the assertion-emit code paths; both
  argument-side and return-side assertions need updating.
- Update the import-binding emission (lines around 19–22, 44–45,
  456, 469–476 per current `compiler.js` — `import` form handling)
  to apply the identifier-mapping rule to imported and exported
  names.

**Rust compiler** (`crates/lykn-lang/src/codegen/names.rs`):

- Replace `to_camel_case` (line 15) with the unified walker.
  Existing single-test fixtures (`test_to_camel_case_*`) extend with
  positive cases for each abbreviation-table row, the macro-override
  cases, and the four worked examples in the rationale.
- Add the macro-override registry as a `const` slice in the same
  module.
- Apply the error-message-format change in
  `crates/lykn-lang/src/codegen/emit.rs` wherever runtime type-check
  assertions are emitted.
- Apply the import-binding change in the import-form codegen path
  (locate via `grep -nr 'import' crates/lykn-lang/src/codegen/`).

**Test strategy:**

- **V-02 / V-03 regression:** repros from M6 must now produce
  `isValid`, `import { isValid }`, with the new error-message
  format. Cross-compiler equivalence asserted (same V-shaped script
  M6 used).
- **Abbreviation table coverage:** positive tests for each row, in
  both compilers.
- **Predicate prefix list:** positive tests for each prefix
  (`is-`, `has-`, `can-`, `should-`, `will-`, `does-`, `was-`,
  `had-`).
- **Collision detection:** Class A (`valid?` + `is-valid`) and
  Class B (`has-items?` + `has-items`) tests in both compilers;
  expect compile error, not silent shadowing.
- **Macro overrides:** `->` and `->>` as form names compile to
  `threadFirst` / `threadLast`.
- **Edge cases:** doubled trailing punctuation, pure-punctuation
  identifiers (`?`, `!`, `*`), leading-punctuation identifiers
  (`?valid`, `*globals*`).
- **Error-message format:** runtime tests confirming the
  parenthesized source-name appears in thrown messages for both
  argument-type and return-type checks.

**M5 context-aware split:** implementations diverge in mechanism (JS
uses `compiler.js`'s walker; Rust uses `names.rs`'s walker), converge
in user-visible behaviour. The macro-override registry, the
abbreviation table, the predicate-prefix list, and the
error-message format must be byte-identical across compilers.

## Backward compatibility considerations

This is a **breaking change** for any code that currently compiles to
invalid JS. Since invalid JS doesn't run, no functioning consumer
code exists that depends on the broken output. The change ships with
0.6.0; no migration path needed because there's nothing to migrate
from.

## Relationship to other DDs / surface forms

- **SKILL.md naming conventions table:** the abbreviation rules and
  the predicate-prefix list should be added to the table after M8
  ships.
- **DD-50 (`if`-in-expression):** independent — DD-49 governs
  identifiers, DD-50 governs control flow.
- **Surface forms guide (`docs/guides/00-lykn-surface-forms.md`):**
  the `?` suffix, `!` suffix, threading macros (`->`, `->>`), and
  identifier syntax all need a reference back to DD-49's mapping
  rules after M8.
- **Source maps (future work):** when source-map support lands, the
  error-message format (Rule 7) can be revisited.

## Open questions

- **Past-tense prefix extensions.** The committed list includes
  `was-` and `had-`. If usage shows other auxiliary forms are common
  (`were-`, `did-`?), the list extends via small follow-up DD.
- **Reader-character scope.** This DD assumes the reader accepts the
  characters in the abbreviation table as identifier-character class.
  If any character listed is currently rejected by the reader, that's
  a parse-level fix tracked in M8 alongside this work.
- **Multi-char arrow extensions.** `<->`, `<=>`, etc. — currently
  fall through to per-char escape via greedy left-to-right matching.
  If these become idiomatic in the lykn ecosystem, the longest-match
  arrow table extends.
- **Formatter behaviour for doubled trailing punctuation.** Whether
  `lykn fmt` should warn on `valid??` or `swap!!` is out of scope
  for this DD; tracked as a fast-follow.

---

## Refinement log

**2026-05-05** — `$` passthrough refinement (initial implementation,
commit `49defbf`). The original abbreviation table listed `$ → DOLLAR`.
During implementation, escaping `$` was found to break lykn's internal
macro API (`$array`, `$sym`, `$gensym`), and `$` is a valid JS
identifier character per the ECMAScript spec. `$` was removed from the
table; identifiers containing `$` pass through unchanged. See note
under Rule 3.

**2026-05-05** — Rule 7 bridging-guard refinement (iteration 2,
commit `404ce0e`). The original Rule 7 specified that the
parenthesized `js_name (lykn_name)` form should render whenever
`js_name != lykn_name`. During implementation testing, this was
found to produce noisy error messages for the common case of
pure lisp-case → camelCase transformations (e.g., every
`my-func` becomes `"myFunc (my-func): …"`). The rule was
tightened to render parens only when the source identifier
contains non-hyphen punctuation. Pure hyphen-only transformations
render the JS name alone. See Rule 7 "Bridging guard."
