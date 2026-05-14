---
number: 55
title: "`template` Macro Redesign — ICU MessageFormat & i18n Foundation"
author: "inspecting the"
component: All
tags: [change-me]
created: 2026-05-13
updated: 2026-05-13
state: Final
supersedes: null
superseded-by: null
version: 1.0
---

# DD-55: `template` Macro Redesign — ICU MessageFormat & i18n Foundation

**Status**: Draft
**Date**: 2026-05-13
**Session**: 0.6.0 planning — `template` as i18n cornerstone
**Depends on**: DD-15 (surface/kernel split), DD-25 (destructured func params),
DD-36 (kernel/surface compiler split — landed 0.5.0)
**Targets**: 0.6.0
**Author**: Claude (drafted with Duncan, 2026-05-13)

## Summary

Today's `(template ...)` is a concatenation form that compiles directly to
a JavaScript template literal: each string arg becomes a quasi segment,
each non-string arg becomes a `${expr}` interpolation. It is mislabeled
("template" implies a frame with holes; today's form treats literals and
holes as syntactic equals), and it is grammar-free in a way that pushes
all morphology (pluralization, capitalization, formatting) into ad-hoc
user code.

This DD proposes evolving `template` into a runtime ICU MessageFormat
form with named parameters, while preserving full backward compatibility
with the current concat-style invocation. The dispatch is decided by
inspecting the second positional argument: a keyword switches the form
into ICU mode; anything else falls through to the existing concat
behaviour.

The strategic angle is that this positions Lykn as the **first Lisp
with i18n baked into a core surface form**. There is no comparable
out-of-the-box capability in Clojure, Racket, Janet, Fennel, or any
modern Common Lisp implementation. We ship Phase A of the ICU subset
(simple slots, multi-reference, plural, select, escape) in 0.6.0 and
defer locale-aware date/time/number formatting to a later milestone.

**Recommendation**: *Endorse and ship in 0.6.0.* The work is contained
to `packages/lang/compiler.js` (JS pipeline) and the corresponding Rust
codegen path; classifier changes are minimal because `template`'s
surface form already dispatches through the macro table. The main risk
is dispatch ambiguity at the keyword/concat boundary; §Dispatch below
analyses it and proposes a precise rule.

This is a workbench draft. It is opinionated on purpose — the
conversation that generated it (Duncan ↔ Claude, 2026-05-13) explicitly
chose aesthetics-and-marketing-fit as the strongest fitness function.

## Context: where `template` is today

Current implementation (`packages/lang/compiler.js:1001-1032`):

```js
'template'(args) {
  if (args.length === 0) {
    return { type: 'TemplateLiteral', quasis: [makeTemplateElement('', true)], expressions: [] };
  }
  const quasis = [];
  const expressions = [];
  let currentSegment = '';
  for (let i = 0; i < args.length; i++) {
    if (args[i].type === 'string') {
      currentSegment += args[i].value;
    } else {
      quasis.push(makeTemplateElement(currentSegment, false));
      currentSegment = '';
      expressions.push(compileExpr(args[i], 'expression'));
    }
  }
  quasis.push(makeTemplateElement(currentSegment, true));
  return { type: 'TemplateLiteral', quasis, expressions };
}
```

Surface examples currently in the test corpus
(`test/forms/template_test.lykn`):

```lisp
(template "hello")                  ;; → `hello`;
(template name)                     ;; → `${name}`;
(template "Hello, " name "!")       ;; → `Hello, ${name}!`;
(template a b)                      ;; → `${a}${b}`;
(template "value: " x)              ;; → `value: ${x}`;
```

These compile to JS template literals — efficient, zero-runtime — but
the surface form has no notion of a "frame" the way ICU, Fluent, gettext,
Python f-strings, or Rust's `format!` do. There is also a tagged form,
`(tag fn (template ...))`, which compiles to `TaggedTemplateExpression`;
this DD preserves it unchanged.

## Problem statement

Three concrete pains.

### 1. The name "template" lies

`(template prefix ", " name "!")` reads as a sequence of fragments, not
a template. A reader cannot scan the literal frame ("Hello, NAME!") and
get the shape; they have to mentally re-stitch the arguments. Every
other language with the word "template" or "format" in this position
(Rust, Python, JS, CL, Racket, Clojure, Fluent, ICU) uses a single
control string with embedded holes — because that is how humans actually
read templated text. We are alone in this design and not for a good
reason.

### 2. Morphology is unsupported

`(template "You have " count " message" (if (= count 1) "" "s") ".")`
compiles fine and produces the right output, but every place in user
code that pluralizes or capitalizes or formats a number is open-coded.
This is fine for English; it gets steadily worse for languages where
plural categories multiply (Russian: 3; Arabic: 6; Welsh: 4 with special
cases for 0/1/2/3/6), the sentence structure changes with the count, or
gender/formality changes whole clauses.

### 3. No i18n story at all

There is currently no way for a Lykn programmer to author a translatable
string. The closest thing — emitting a JS template literal — bakes the
English structure into the source and makes the string un-extractable
for translation. There is no `gettext`-equivalent, no Fluent integration,
no ICU support. For a language that bills itself as Lisp Flavoured
JavaScript, this is a noticeable gap; for a language with momentum
toward broader adoption, it is a marketing opportunity.

The i18n marketing angle: **every modern application needs translation;
no Lisp ships it as a core form**. Building it into `template` (rather
than as a library) makes it visible in tutorials, examples, and the
first hour of every new user's experience. That is exactly where
marketing differentiators live.

## Design

### Surface syntax

```lisp
;; ICU mode (new):
(template <icu-string>
  :keyword1 value1
  :keyword2 value2
  ...)

;; Concat mode (legacy, unchanged):
(template <expr1> <expr2> ... <exprN>)
```

The ICU string supports:

- **Simple slots**: `{name}` — replaced with the value bound to `:name`.
- **Multi-use slots**: the same `{name}` may appear any number of times
  in the template; it resolves to the same value at each site.
- **Plural**:
  `{count, plural, one {...} other {...}}` — selects a branch based on
  the value's CLDR plural category. Inside a branch, `#` is a shorthand
  for the selector's value.
- **Select**:
  `{role, select, owner {...} member {...} other {...}}` — selects a
  branch by string equality against the value.
- **Escape**:
  `'{'` and `'}'` for literal braces; `''` for a literal apostrophe.
  (Standard ICU rules.)
- **Nesting**: branches may contain further slots and selectors.

### Dispatch rule

The form `(template X ...)` dispatches as follows:

```
1. If the form has exactly one argument and it is a literal string:
   - parse the string as ICU. If it contains slots, treat as ICU mode
     (and fail at compile time on missing kwargs — there are none).
   - if it contains no slots, both interpretations produce the same
     output; pick ICU mode for forward-compat.

2. If the form has ≥2 arguments and arg[0] is a literal string and
   arg[1] is a keyword:
   - ICU mode. Remaining args are taken as keyword/value pairs.

3. Otherwise:
   - Concat mode (current behaviour).
```

This rule has the property that **every program that compiles today
compiles unchanged tomorrow**. The only way to land in ICU mode is to
write a literal string followed by a keyword, which today is a type
error (a keyword as a `${kw}` interpolation is exotic at best). Existing
concat-mode call sites do not stop at the first kw because they don't
have kwargs.

#### Edge cases

| Form                                              | Mode    | Reason |
|---------------------------------------------------|---------|--------|
| `(template "hello")`                              | ICU     | No slots → identical to concat result |
| `(template name)`                                 | Concat  | arg[0] is not a literal string |
| `(template "Hi, " name "!")`                      | Concat  | arg[1] is not a keyword |
| `(template "Hi, {name}!" :name n)`                | ICU     | matches rule 2 |
| `(template "Hi, {name}!")`                        | ICU     | matches rule 1; missing-kwarg compile error |
| `(template "Hi, " :name)`                         | Concat  | arg[1] is a keyword *value* (not a kwarg key), still concat — UNSAFE EDGE; see §Open questions |
| `(template "{a}" :a 1 :a 2)`                      | ICU     | duplicate kwarg → compile error |
| `(template "Hi, {missing}!" :name "X")`           | ICU     | unbound slot → compile error |
| `(template "Hi, {name}!" :name "X" :extra 1)`     | ICU     | extra kwarg → compile error |

The case `(template "Hi, " :name)` is the only genuine ambiguity. A
programmer who writes a keyword as a positional concat arg is doing
something unusual but legal today (keywords print as `:name` in Lykn).
Per rule 2, ICU mode requires arg[0] to be a literal string AND arg[1]
to be a keyword — both conditions hold here. To disambiguate, we treat
this as a **compile-time error** in 0.6.0: "ambiguous template form;
add a value for keyword `:name` for ICU mode, or use `(concat ...)`
for printing the keyword literal." This forces the programmer to be
explicit, which is preferable to picking a default that surprises one
audience or the other.

(Alternative considered: silently fall through to concat when there is
no value following the keyword. Rejected because the failure mode is
silent — a missing value for `:name` would produce wrong output without
warning. Hard error is the kinder default.)

### ICU semantics — Phase A (ships in 0.6.0)

#### Simple slots

```lisp
(template "Hello, {name}!" :name "Duncan")
;; → "Hello, Duncan!"
```

The slot's contents are stringified per `to-string` (the existing
surface form for value-to-string conversion).

#### Multi-use slots

```lisp
(template "{name}, please review {name}'s changes. Thanks, {name}!"
  :name "Bob")
;; → "Bob, please review Bob's changes. Thanks, Bob!"
```

Each `{name}` site resolves to the same value. The kwarg `:name` is
bound *once* per `template` invocation; compile-time emission references
it as many times as it appears.

#### Plural

```lisp
(template "You have {count, plural, one {1 message} other {# messages}}."
  :count 3)
;; → "You have 3 messages."

(template "You have {count, plural, one {1 message} other {# messages}}."
  :count 1)
;; → "You have 1 message."
```

Inside a plural branch, `#` is shorthand for the selector's value
(equivalent to writing `{count}` inside the branch). The branch
selection rule for 0.6.0 is the **English CLDR rule**:

- `one`  → `count == 1`
- `other` → otherwise

Plus optional explicit-value branches with `=N` syntax:

```lisp
(template "{count, plural, =0 {No messages} one {1 message} other {# messages}}"
  :count 0)
;; → "No messages"
```

Explicit-value branches take priority over category branches when both
match.

(Locale-aware plural category resolution — needed for non-English —
is deferred to a later milestone with explicit dependency on a CLDR
data shim. The Phase A implementation is structured so adding the
shim is a single-function swap, not a rewrite.)

#### Select

```lisp
(template "{role, select, owner {You own this repository.} member {You are a member of this repository.} other {You have read access.}}"
  :role "member")
;; → "You are a member of this repository."
```

Select branches are matched by string equality against the selector's
value. The `other` branch is required and serves as the fallback.

#### Escape

- `'{'` → literal `{`
- `'}'` → literal `}`
- `''`  → literal `'`
- A `'` not followed by `{`, `}`, or `'` is itself a literal `'`.

(This matches ICU's quoting rules, not Python's `{{ }}`. Worth noting
because seasoned ICU users will expect this; users coming from Python
will need a doc reminder.)

#### Composed example (the marketing screenshot)

```lisp
(template
  "{role, select,
     owner {Welcome back, {name}! You have {count, plural,
                                              =0 {no pending tasks}
                                              one {1 pending task}
                                              other {# pending tasks}}.}
     member {Hi {name}. You have {count, plural,
                                    =0 {no items to review}
                                    one {1 item to review}
                                    other {# items to review}}.}
     other {Hello, guest.}}"
  :role  "member"
  :name  "Bob"
  :count 3)
;; → "Hi Bob. You have 3 items to review."
```

This single form demonstrates: multi-use of `{name}`, multi-use of
`{count}` (twice through the plural categories), nested selectors,
and explicit-value plural branches. It is the screenshot we want in
release notes.

### Compile-time vs. runtime

The ICU string is parsed at **compile time** into an internal
representation (an MFT — Message Format Tree — of literal segments,
slot references, plural blocks, and select blocks). The compiler then
emits a JavaScript expression that evaluates the tree against the
keyword bindings.

Emission strategy: **emit a JS template literal where each branching
construct compiles to a ternary or IIFE**, so the runtime cost is one
template-literal evaluation plus zero-or-more conditionals. No runtime
ICU parser is shipped. Example:

```lisp
(template "You have {count, plural, one {1 message} other {# messages}}."
  :count n)
```

emits something like (post-formatting):

```js
`You have ${(() => {
  const _v = n;
  if (_v === 0 && false) return ``;  // no =0 branch
  if (_v === 1) return `1 message`;
  return `${_v} messages`;
})()}.`
```

Implementation notes are in §Implementation outline.

### Backward compatibility

All 7 existing call patterns in `test/forms/template_test.lykn`
continue to compile to identical JS, because none of them match
rule 2 (none has a literal string at arg[0] followed by a keyword
at arg[1]). The 4 in-tree examples
(`test/surface/kernel-in-surface_test.lykn`,
`examples/surface/browser-app.lykn`,
`examples/surface/browser-quotes.lykn`,
`test/surface/func-destructuring_test.lykn`) are all concat-mode and
unaffected.

There is no deprecation. Concat mode and ICU mode are both supported
for the indefinite future. The two modes do not overlap on any input.

## Examples

### Replacing the original `make-greeter`

The example from the conversation that started this:

```lisp
;; Today (concat mode, still works):
(func make-greeter
  :args (:string prefix)
  :returns :function
  :body
  (fn (:string name)
    (template prefix ", " name "!")))

;; Tomorrow, ICU mode (equivalent):
(func make-greeter
  :args (:string prefix)
  :returns :function
  :body
  (fn (:string name)
    (template "{prefix}, {name}!" :prefix prefix :name name)))
```

For this trivial case, concat mode wins on terseness. Where ICU mode
earns its keep is when morphology shows up — pluralization,
selection, multi-reference, anything beyond pure concatenation.

### Notifications widget

```lisp
(func notification-text
  :args (:string actor :number count :string action)
  :returns :string
  :body
  (template
    "{actor} performed {count, plural,
                          =0 {no actions}
                          one {1 {action}}
                          other {# {action}s}}."
    :actor  actor
    :count  count
    :action action))

(notification-text "Alice" 0 "merge")  ;; → "Alice performed no actions."
(notification-text "Alice" 1 "merge")  ;; → "Alice performed 1 merge."
(notification-text "Alice" 7 "merge")  ;; → "Alice performed 7 merges."
```

Note `{action}` is referenced twice (once in `one`, once in `other`).
This is the "multi-use of a named parameter" demonstration Duncan
asked for.

### Status line

```lisp
(template "{user} • {n, plural, =0 {idle} one {{n} task} other {{n} tasks}} • {state}"
  :user  current-user
  :n     queue-length
  :state cluster-state)
```

`{n}` is referenced three times: once as a direct slot, once as the
plural selector, and twice as `{n}` inside branches. Same value at
each site.

## Error handling

Compile-time errors (with example messages):

```
;; (template "Hello, {name}!" :name n :extra v)
ERROR: template: unused keyword argument :extra
  in (template "Hello, {name}!" ...)
  expected slots: name
  provided kwargs: name, extra
  hint: remove :extra, or add a {extra} slot to the template

;; (template "Hello, {missing}!")
ERROR: template: no binding for slot {missing}
  in (template "Hello, {missing}!")
  expected slots: missing
  provided kwargs: (none)
  hint: add :missing <value> to the template call

;; (template "{a, plural, one {x} two {y}}" :a 1)
ERROR: template: plural block for {a} missing required `other` branch
  ICU plural blocks must include `other` as a fallback
  hint: add `other {...}` to cover values that don't match other categories

;; (template "{a, plural, weird {x} other {y}}" :a 1)
ERROR: template: unknown plural category `weird` for {a}
  valid categories: zero one two few many other
  hint: use `other` for unmatched values, or `=N` for specific values

;; (template "{a" :a 1)
ERROR: template: unclosed slot in ICU template
  in "{a"
  position 2: expected `}` to close slot opened at position 0
  hint: escape with `'{'` to write a literal `{`

;; (template "Hi, " :name)
ERROR: template: ambiguous form
  arg 0 is a literal string and arg 1 is a keyword (:name) with no
  following value, which matches both ICU mode (missing kwarg value)
  and concat mode (keyword as positional arg).
  hint:
    - for ICU mode, add a value: (template "Hi, " :name "World")
    - for concat mode, use (concat ...) instead
```

Runtime errors should be impossible by design — the compile-time
checker guarantees every emitted code path has its bindings. (The
exception is `:count`-bound values that aren't numbers reaching a
plural block; we either coerce-to-number or emit a runtime type
error — see §Open questions.)

## Test coverage

Required test categories for 0.6.0 (each gets its own
`test/forms/template_*_test.lykn` file, snapshotted via `insta`):

**1. Backward compatibility** — every form in the current
`template_test.lykn` continues to produce identical output. (Direct
re-run, no changes.)

**2. ICU simple slots** — single slot, multiple slots, multi-use of
same slot, empty template, slot at start / middle / end / only-slot.

**3. ICU plural** — `one` / `other` branches; explicit `=N` branches;
explicit branch priority over category branch; `#` shorthand inside
branches; missing `other` branch (error); unknown category (error).

**4. ICU select** — basic select; required `other` branch; missing
`other` (error); nested template inside branch.

**5. ICU escape** — `'{'`, `'}'`, `''`, lone `'` followed by
non-special; literal `{` and `}` in branches.

**6. ICU nesting** — plural inside select; select inside plural;
slots inside branches; multi-level escape interaction.

**7. Dispatch ambiguity** — every row of the edge-case table in
§Dispatch rule, including the ambiguous-form compile error.

**8. Compile-time error messages** — each error class from §Error
handling, asserting the message text, error code, and source-location
attribution.

**9. Tagged template still works** — `(tag fn (template ...))`
unchanged for both concat-mode and ICU-mode inner templates.

**10. Runtime equivalence** — for a corpus of (template-call,
expected-string) pairs, the emitted JS produces the expected string
when evaluated against the binding values.

Coverage target: every ICU production in the grammar has at least one
positive and one negative test. Goal is 100% line coverage on the
new parser and emitter modules, measured via `cargo tarpaulin` (Rust
side) and `deno test --coverage` (JS side).

## Implementation outline

Five phases, all targeting 0.6.0:

**Phase 1 — ICU parser (JS pipeline)**

- `packages/lang/icu-parser.js`: pure ICU-string → MFT parser.
  Hand-written recursive descent; ~300 LoC. No runtime dependencies.
- Tests: 30+ unit tests covering grammar productions and error cases.

**Phase 2 — Compiler dispatch and emission (JS)**

- `packages/lang/compiler.js`: split current `template` handler into
  `templateConcat` (existing logic) and `templateIcu` (new logic);
  add dispatch wrapper per the rule in §Dispatch.
- MFT → ESTree emission: literal segments become `TemplateElement`,
  slots become `${Identifier}`, plural/select compile to IIFE
  expressions returning a `TemplateLiteral`.
- Snapshot tests for emitted JS for each grammar production.

**Phase 3 — Rust mirror**

- `crates/lykn-lang/src/codegen/`: parallel ICU parser and emitter
  in Rust. Same grammar, same emission strategy. Snapshot tests via
  `insta` covering identical cases to JS.
- `crates/lykn-lang/src/classifier/dispatch.rs`: register `template`
  with both surface and (where applicable) macro form classifiers; no
  kernel change needed.

**Phase 4 — Error message quality**

- Each compile-time error gets a span pointing at the offending input
  (slot name, branch keyword, etc.). Reuse the existing `Span` type.
- Surface error messages match the §Error handling examples verbatim
  in the test suite.

**Phase 5 — Docs and examples**

- `docs/guides/<N>-template-and-i18n.md`: full surface-syntax guide
  with the ICU subset, examples, error catalogue.
- `examples/surface/i18n-notifications.lykn`: the "marketing
  screenshot" composed example, runnable.
- Lykn Book chapter update covering the new mode.
- Release notes for 0.6.0 lead with this as a feature.

Estimated effort: ~2 weeks of focused work for one engineer, plus
review cycles. Most of the risk is in error-message taste; the
parsing and emission are well-understood ground.

## Open questions

**Q1. `to-string` coercion for slot values.** When `:count` is bound
to a non-number value (e.g., a string `"3"` or `nil`), what happens
inside a plural block? Options: (a) coerce via `Number(...)`, (b)
emit a runtime type error, (c) emit a compile-time type error if
the binding's static type is wrong. Inclination: (c) when type info
is available, fall back to (b) when it isn't. Defer to type-system
discussion in DD-21.

**Q2. Nested same-name slots in branches.** Should
`{a, plural, one {{a, plural, one {x} other {y}}} other {z}}` be
allowed? Semantically it's well-defined (the inner block sees the same
`a` value), but it's deeply confusing. Inclination: allow at parser
level, lint-warn at semantic level.

**Q3. Whitespace inside ICU strings.** ICU preserves whitespace
exactly, including newlines and indentation between branches. Lykn
strings preserve newlines literally. The "marketing screenshot"
example would look terrible without some whitespace-trimming
convention. Options: (a) ICU-exact (faithful), (b) trim runs of
whitespace adjacent to `{...}` openers (Fluent-style), (c) introduce
a sigil for "this is a multi-line ICU template; trim indent."
Inclination: (a) for 0.6.0 (predictable), revisit if the docs
examples suffer.

**Q4. Should `select` be in Phase A?** Marginal additional cost
versus plural; clear marketing value. Inclination: yes, ship in
0.6.0.

**Q5. Should the keyword form support positional reference too?**
ICU MessageFormat traditionally uses `{0}`, `{1}` for positional
args. Lykn could allow `(template "Hi {0}!" "World")` as a
shorthand. Inclination: **no** — positional ICU is at war with
Lykn's preference for named binding. Keep ICU mode name-only.

**Q6. Interaction with `tag`.** `(tag fn (template ...))` with an
ICU-mode inner template is well-defined (the `tag` function receives
the JS template literal that ICU mode emits), but unusual. Document
explicitly that the tag function sees the *post-ICU-expansion* template
literal, not the ICU string. Open question whether to provide a
`(tag-icu fn ...)` variant that exposes the MFT for things like
i18n extraction tools.

**Q7. Where does locale-aware plural land?** This DD ships English
CLDR rules only. The future-work DD will add a locale parameter
(`(template :locale "ru" "{n, plural, ...}")`) and a runtime CLDR
plural-rules table. Coordinate with the upcoming "language i18n"
DD (see §Related work) for whether locale becomes a global state, a
per-call argument, or both.

## Risks and non-goals

**Risks:**

- *ICU is big.* Phase A covers ~80% of real-world use, but users
  familiar with full ICU will run into "wait, why doesn't `date,
  short` work?" early. Mitigation: doc page explicitly enumerates
  what's in and what's out, with a roadmap for the rest.
- *Error-message quality is a tax.* Bad ICU error messages would
  destroy the marketing story. Phase 4 is non-skippable.
- *Snapshot drift.* The Rust and JS emitters must produce identical
  output for the cross-compiler tests in
  `crates/lykn-lang/tests/e2e_tests.rs`. Snapshot review discipline
  matters more than usual here.

**Non-goals for 0.6.0:**

- Locale-aware date, time, or number formatting (date, time,
  number, currency).
- CLDR plural rules for non-English locales.
- Translation-extraction tooling (gettext-style `.po` extraction).
- Runtime hot-swap of templates by locale (planned for the
  language-i18n work; see §Related work).
- Tagged-template MFT exposure.

## Related work

This DD is a **prerequisite** for the upcoming "Lykn language i18n"
proposal (forthcoming bootstrap prompt — see
`workbench/cowork-bootstrap-language-i18n.md`), which proposes
translating Lykn's own surface forms and reserved names into other
scripts. Two reasons it's a prerequisite:

1. ICU MessageFormat is the proven canonical surface for translatable
   strings; building it into `template` first gives the language-i18n
   work a concrete user-facing demonstration of "i18n is core."
2. The compiler infrastructure for parsing a string into a
   structured tree at compile time, and emitting alternative code
   per branch, is the same infrastructure that the language-i18n
   work will need for parsing translated source.

## Decision log

- 2026-05-13: Initial draft after Duncan ↔ Claude conversation.
  Duncan chose ICU MessageFormat (Option 3 in the design-options
  email) over (a) reverting to a non-template name like `fmt`, (b)
  string quasiquote, (c) format-with-named-slots-plus-directive-DSL,
  on the basis of "users will be most comfortable with and least
  surprised by template-as-a-runtime-string." Marketing angle (i18n
  baked in) explicitly endorsed.

- 2026-05-13: Backward-compat dispatch decided by inspecting
  arg[1] for keyword-ness. Alternative (explicit `:icu` mode
  marker) rejected as visually noisier. Alternative (rename to
  `format`, leave `template` as-is) rejected because Duncan asked
  to keep the name `template`.

- 2026-05-13: Phase A scope set to slots + plural + select +
  escape. Date/time/number/currency formatting deferred.
