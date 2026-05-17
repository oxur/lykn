---
number: 59
title: "DD-58: Kernel/Surface Separation — Closed Surface Namespace with `(kernel:<form> ...)` Escape"
author: "** CDC (cdc/compiler-coherence thread)"
component: All
tags: [change-me]
created: 2026-05-16
updated: 2026-05-16
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# DD-58: Kernel/Surface Separation — Closed Surface Namespace with `(kernel:<form> ...)` Escape

**Status:** Draft (CDC, 2026-05-17). Second revision incorporating
Duncan's 2026-05-17 calls on the open design questions.
**Author:** CDC (cdc/compiler-coherence thread)
**Date:** 2026-05-17 (second revision; first revision 2026-05-15)
**Depends on:** DD-13 (macro expansion pipeline), DD-15 (language
architecture), DD-20 (Rust surface compiler architecture), DD-22
(surface equality), DD-50 (position-aware compilation), DD-22
(surface equality and logical operators), DD-23 (`set!` for
property mutation).
**Complements:** DD-37 (JS surface compiler architecture — provides
the classifier infrastructure DD-58 depends on), DD-56 (canonical
form catalog — schema flows directly from DD-58's namespace
decisions).
**Supersedes:** DD-36's direction. DD-36's analysis is preserved as
historical record at `docs/design/01-draft/0046-dd-36-*.md`.
**Targets:** 0.6.0.

---

## Summary

lykn currently classifies forms at compile time against two
overlapping dispatch tables. Some atoms (`=`, `!=`) appear in both
tables; the JS expander uses an `_kernel` marker + `kernelArray()`
helper to prevent surface-macro output from being re-walked through
surface macros; multiple kernel atoms (`const`, `let`, `var`,
arithmetic, comparison, etc.) are recognised in surface code today
without an explicit surface form. The system works, but every
overlapping case requires per-feature disambiguation logic, and
the bug surface is visible: the M16-2 conversion to `compileBoth`
testing surfaced six classes of divergence, two of which were
correctness-grade Rust bugs (class-constructor `=` and
destructuring-assignment `=` both emit `===` instead of `=`); the
import-macros work surfaced a *latent Rust correctness bug* where
the Rust compiler omitted runtime-import declarations and produced
JS that would fail at runtime with `ReferenceError`.

DD-58 fixes this at the architecture level by making **surface lykn
a closed namespace, with `(kernel:<form> ...)` as the single escape
hatch into the kernel layer.** Every atom a user writes in a
`.lykn` file resolves to either a surface form (rich, passthrough,
or namesake-sharing), a user-defined macro, or — explicitly prefixed
— a kernel form. The classifier dispatches uniformly; there is no
fall-through, no auto-promotion, no per-feature disambiguation.
File-extension gating (`.lyk` for kernel, `.lykn` for surface)
lands as part of this work for architectural cleanliness.

The decision rests on three substantive observations:

1. **The closed-namespace model is more honest about what surface
   lykn already is.** Every form a user reaches for in surface code
   today is already a surface form *in intent* — they just happen
   to share names with kernel forms in some cases. DD-58 makes the
   intent structural.

2. **The `kernel:<form>` escape integrates cleanly with DD-01's
   colon-syntax.** The classifier dispatches before member-access
   compilation, recognising `kernel:` as a reserved head-position
   prefix. This is a small, bounded, documentable exception — not
   a new layer of syntax. The escape stays English-canonical
   across i18n locales (DD-56's form catalog gives translation
   tables for surface forms; kernel forms have no translation
   tables).

3. **The 0.7.0 i18n work requires the namespace decision in
   advance.** DD-56's form catalog (per-locale translation tables)
   has fundamentally different schema shapes under "closed
   namespace" vs. "overlapping namespaces." The 0.6.x parity-
   discipline problem and the 0.7.0 i18n unlock converge on the
   same artifact.

DD-58 is the architectural commitment; the implementation is
milestone-scoped via DD-37's classifier infrastructure and a
future M-something that converts the dispatch tables. The escape
syntax (`kernel:`) is a hard commitment.

---

## Context: where the language is now

Three current-state observations from a 2026-05-17 source survey
(post-rebase to `release/0.6.x` @ `4ee1eaa` plus the in-progress
cdc/compiler-coherence work).

### 1. The classifier dispatch tables overlap

`crates/lykn-lang/src/classifier/dispatch.rs` contains
`is_surface_form()` (32 names) and `is_kernel_form()` (~90 names).
The intersection is non-empty:

| Atom | Surface use | Kernel use |
|------|-------------|------------|
| `=`  | DD-22 equality (compiles to `===`) | Kernel assignment (class bodies pre-0.5.0; now legacy via `set!`) |
| `!=` | DD-22 inequality (compiles to `!==`) | Kernel `!=` operator |
| `macro` | Surface macro definition | Dead overlap — kernel never uses this name |
| `import-macros` | Surface DD-14 directive | Dead overlap — kernel never uses this name |

The `=`/`!=` overlap is the load-bearing one. The M16-2 work
surfaced the user-visible manifestation: in `(class Foo ()
(constructor (x) (= this:x x)))`, the user clearly intends
assignment (per ID-38's kernel-`=`-as-block-level-assignment
semantics), but the Rust compiler's surface classifier intercepts
`=` as the equality form (per DD-22) and emits `===`, producing
JS that fails to set the property. JS has a class-form-specific
path that preserves the assignment intent; Rust doesn't. Both
paths share a root cause that DD-58 resolves structurally:
under DD-58, surface `=` is unambiguously equality (compiles to
`===`); property assignment is `set!`.

### 2. The JS compiler has no classifier

`packages/lang/surface.js` (2,315 lines) registers every surface
form as a macro in the shared `macroEnv`. The expander's
fixed-point loop needs to know not to re-expand surface macros'
kernel-shaped output — which is what the `_kernel` marker
(`expander.js:731`, `:736`, `surface.js:27`) and `kernelArray()`
helper exist for. DD-37 (now in `05-active/`) proposes the JS-side
classifier infrastructure that makes DD-58 implementable on the JS
side. DD-58's strict enforcement turns on once DD-37's classifier
is in place.

### 3. The recent compiler-coherence work surfaced four
correctness-grade Rust bugs

The cdc/compiler-coherence thread's compileBoth-broadening work
surfaced:

- **Latent Rust correctness bug** in `(import-macros ...)`
  handling: Rust omitted runtime-import declarations; output
  failed at runtime with `ReferenceError`. Fixed 2026-05-16
  (commit `56894c7`).
- **Latent Rust correctness bug** in class-constructor `(= this:x x)`:
  Rust emits `===` instead of `=`. Deferred to DD-58 for resolution.
- **Latent Rust correctness bug** in destructuring-assignment
  `(= (object a b) obj)`: same root cause as the class-constructor
  bug. Deferred to DD-58.
- **Latent Rust correctness bug** in tagged template content:
  double-escape of backslashes. Fixed 2026-05-16.

The class-constructor and destructuring-assignment bugs are
*resolved by the closed-namespace model* — once surface `=` is
unambiguously equality and assignment moves to `set!`, the source
of these bugs disappears at the language level.

---

## The closed-namespace model

### Architectural rule

**Every atom a user writes in surface position is unambiguously
classified as one of:**

1. A **surface form** (a known head atom in the surface namespace).
2. A **`kernel:`-prefixed kernel form** (head atom starts with
   `kernel:`; the classifier strips the prefix and routes to
   kernel handling).
3. A **user-defined macro invocation** (head atom not in the
   surface namespace; resolved at expansion time against the
   macro environment).
4. A **diagnostic** (head atom is not in surface namespace AND no
   macro is registered).

There is no "fall through to kernel" for unrecognised atoms. There
is no auto-promotion of kernel forms into surface code without the
`kernel:` prefix.

### Three implementation flavors of surface forms

Surface forms come in three implementation flavors. The flavor is
an implementation detail; from the user's perspective they're all
just surface forms.

**(a) Rich, surface-unique.** The form exists only in surface
syntax; there is no same-named kernel form (or the kernel form is
never reached from surface). The classifier produces a fully-typed
AST node; the emitter does elaborated work (multi-pass expansion,
IIFE-wrapping, type-check insertion, etc.). Examples: `bind`,
`func`, `match`, `obj`, `cell`, `if-let`, threading macros, `lambda`.

**(b) Passthrough surface form.** The surface form shares a name
with a kernel form and is a literal passthrough; the emitter wraps
the form unchanged. The classifier still produces a typed AST node
(`PassthroughSurfaceForm(name, args)` or equivalent); having one
ensures the user-facing namespace is closed. Examples: `+`, `*`,
`array`, `template`, `new`, `typeof`.

**(c) Rich, namesake-sharing.** The surface form shares a name
with a kernel form *and* the surface emitter does elaborated work
distinct from a passthrough. The classifier produces a typed AST
node; the emitter handles the elaborated semantics. Examples:
surface `if` (position-aware ternary vs. IIFE per DD-50), surface
`try` (value-producing per D-2), surface `class` and `class-expr`
(multi-clause methods, contracts, surface-rich class semantics).

The user does not need to know which flavor applies. Every form
they write in surface is classified the same way; the classifier
dispatches to the appropriate emitter; the emitter does the right
thing.

### The `kernel:` escape syntax

A head atom prefixed `kernel:` is the **explicit drop-into-kernel
escape**. The classifier strips the prefix and routes the form
through the kernel-passthrough emitter, which emits the underlying
kernel syntax unchanged.

```lykn
(kernel:if cond then else)   ; ternary in kernel position
(kernel:const x 42)          ; raw kernel const declaration
(kernel:function f (x) ...)  ; raw JS function declaration
(kernel:=> () 1)             ; raw JS arrow expression
(kernel:quote literal)       ; only way to reach kernel quote
```

**Reader treatment:** the reader keeps `kernel:if` as a single atom
(matching DD-01's "colons are ordinary characters; compiler splits
them"). The classifier dispatches on the colon.

**Classifier validation:** the form after `kernel:` MUST be a
recognised kernel form. `(kernel:nonsense ...)` is a compile-time
diagnostic; the classifier won't pass through an unknown atom.

**Per-language-neutrality:** the `kernel:` prefix is **untranslated
in i18n contexts**. Kernel is "the thin skin over JavaScript" (per
book Ch 2.2); JS itself isn't translatable; the kernel namespace
stays English-canonical regardless of the user's surface-language
locale. Surface forms get translation tables (per DD-56); kernel
forms do not.

### File-extension gating

`.lyk` files contain only kernel forms (no surface namespace
applies). `.lykn` files contain surface forms (with kernel forms
reachable via `kernel:` escape only).

The CLI dispatches based on file extension:
- `lykn compile foo.lyk` → kernel path (reader → kernel codegen).
  Skips classification and surface expansion entirely.
- `lykn compile foo.lykn` → surface path (reader → classifier →
  expander → emitter → kernel codegen).
- Mixed-extension violations are diagnostics: surface forms in a
  `.lyk` file → "surface form 'X' not permitted in kernel file."
  Kernel forms (bare, not `kernel:`-prefixed) in a `.lykn` file →
  "kernel form 'X' requires the `kernel:` prefix in surface code."

File-extension gating provides a second layer of enforcement on
top of the closed-namespace rule. Together they make the
boundary visible at both source-level (file extension declares
intent) and form-level (every atom unambiguously classified).

---

## Per-layer form enumeration

This section enumerates the surface namespace under DD-58. **All
calls below incorporate Duncan's 2026-05-17 design decisions.**

### Surface namespace (closed)

**Flavor (a) — Rich, surface-unique** (no kernel namesake reached
from surface):

| Form | Purpose |
|------|---------|
| `bind` | Type-annotated variable binding (DD-24) — the canonical surface binding form, replaces direct use of `const`/`let`/`var` |
| `func` | Function with contracts (DD-16) |
| `genfunc` | Generator function with contracts |
| `genfn` | Generator function expression |
| `fn` | Function expression (alias for `lambda`) |
| `lambda` | Function expression — emits kernel `function` (anonymous), NOT `=>` |
| `match` | Pattern matching (DD-17) |
| `type` | ADT definition (DD-17) |
| `obj` | Object construction with keyword args |
| `cell` | Mutable cell (`{ value: x }`) — the canonical surface mutability primitive |
| `express` | Member-access syntactic sugar |
| `swap!`, `reset!`, `set!`, `set-symbol!` | Mutation forms (DD-23) — `set!` is the property-assignment form |
| `->`, `->>`, `some->`, `some->>` | Threading macros (DD-18) |
| `if-let`, `when-let` | Conditional binding (DD-18) |
| `conj`, `assoc`, `dissoc` | Collection operators |
| `macro`, `import-macros` | Macro definition / import (DD-11, DD-14) |
| `do` | Block in expression position (DD-50) |
| `and`, `or`, `not` | Surface n-ary logical operators that fold into binary kernel chains. `(and a b c)` → `(&& (&& a b) c)`; `(or a b c)` → `(\|\| (\|\| a b) c)`; `(not x)` → `(! x)`. Same JS short-circuit semantics as kernel `&&`/`\|\|`/`!`; the distinction is purely syntactic ergonomics — Lisp-style n-ary chains vs. JS-style binary operators. |

**Flavor (b) — Passthrough surface forms** (literal passthrough to
same-named kernel):

| Category | Forms |
|----------|-------|
| Arithmetic | `+`, `-`, `*`, `/`, `%`, `**` |
| Strict comparison | `===`, `!==` |
| Order comparison | `<`, `>`, `<=`, `>=` |
| Bitwise | `&`, `\|`, `^`, `<<`, `>>`, `>>>`, `~` |
| Update | `++`, `--`, `+=`, `-=`, `*=`, `/=`, `%=`, `**=`, `<<=`, `>>=`, `>>>=`, `&=`, `\|=`, `^=`, `&&=`, `\|\|=`, `??=` |
| Logical (kernel forms) | `&&`, `\|\|`, `??` (note: surface `and`/`or` are flavor (a) with IIFE semantics; these passthrough forms are for users wanting raw JS short-circuit) |
| Literal constructors | `array`, `object`, `get`, `template`, `tag`, `regex` |
| Destructuring helpers | `spread`, `rest`, `default`, `alias` |
| Type/identity ops | `new`, `delete`, `typeof`, `instanceof`, `in`, `void` |
| Async ops | `await`, `yield`, `yield*` |
| Module forms | `import`, `export` |
| Control flow | `block`, `while`, `do-while`, `for`, `for-of`, `for-in`, `for-await-of`, `switch`, `break`, `continue`, `return`, `throw`, `label`, `seq`, `debugger` |
| Arrow function | `=>` — surface arrow function with lexical `this`. Passthrough to kernel `=>`. Provides modern-JS arrow ergonomics for users who specifically want lexical-`this` semantics (vs. `fn`/`lambda` which emit kernel `function` with dynamic `this`). |

**Flavor (c) — Rich, namesake-sharing** (elaborated semantics on a
same-named kernel atom):

| Form | Elaborated semantics |
|------|----------------------|
| `if` | Position-aware: ternary in expression position, IIFE if branches are statements, kernel `if` in statement position. (DD-50, DD-50.5, DD-50.6, DD-50.7.) |
| `try` | Position-aware value-producing: IIFE-wrap in expression position; kernel `try` in statement position. |
| `=`, `!=` | Surface equality / inequality (DD-22) — compile to `===` / `!==`. Distinct from kernel `=` (assignment) and kernel `!=` (loose inequality). Property assignment is `set!`, not `=`. |
| `class`, `class-expr` | Rich surface class with multi-clause methods, contracts, and class-body conventions. Distinct from kernel `class`/`class-expr` which is the raw JS form. |

### Kernel-only namespace (reached via `kernel:` escape only)

**Forms with no surface counterpart** — users reach them only via
`(kernel:<form> ...)`. Duncan's 2026-05-17 call: **all JS
declaration / binding constructs are kernel-only via escape;
surface provides richer alternatives.**

| Form | Surface alternative | Notes |
|------|---------------------|-------|
| `function` | `func`, `fn`, `lambda` | Not exposed in surface — Duncan 2026-05-17 |
| `const` | `bind` | No specific surface use case; kernel-only |
| `let` | `bind` + `cell` for mutability | Kernel-only |
| `var` | `bind` + `cell` for mutability | Legacy JS; kernel-only |
| `quote` | n/a | Reader / macro internals; not idiomatic surface |
| `quasiquote` | n/a | Same as `quote` |

**Why these JS declaration / binding constructs are kernel-only:**

- `bind` is the canonical surface binding (DD-24): types,
  destructuring, contracts. Users who want raw `const` semantics
  in surface have no specific use case beyond JS-interop edge
  cases.
- `cell` is the canonical surface mutability primitive. Users
  who want `let`'s mutable binding can use `(bind name (cell
  initial))` and idiomatic surface mutation operators.
- `function` is not supported in surface at all per Duncan's
  2026-05-17 call. The surface alternatives (`func`, `fn`,
  `lambda`, `=>`) cover the function-declaration use cases with
  richer semantics or distinct `this`-binding semantics.

**Note:** `=>` is **surface flavor (b) passthrough**, NOT
kernel-only. Modern JS devs writing `(=> (x) x)` get a real
arrow function with lexical `this`. The function-form choice
in surface is:

- `func` — full surface function with contracts, types,
  multi-clause dispatch.
- `fn` / `lambda` — anonymous function (emits kernel
  `function`, dynamic `this`).
- `=>` — arrow function (emits kernel `=>`, lexical `this`).

### A note on `lambda`, `fn`, and `=>`

Per Duncan's 2026-05-17 recall of the original language design
plus the 2026-05-17 follow-up confirming surface `=>`:

- **Surface `lambda` emits kernel `function` (anonymous).** Not
  `=>`. This means lambda functions have **dynamic** `this`
  binding (per traditional Lisp `lambda` semantics).
- **Surface `fn` is an alias for `lambda`**, so also emits
  kernel `function`.
- **Surface `=>` is a passthrough to kernel `=>`** — flavor (b).
  Users who want arrow-function **lexical** `this` semantics
  write `(=> ...)` directly. Modern-JS-idiomatic.

So users have a clean three-way choice in surface for function
forms, distinguished by `this`-binding semantics:

| Surface form | Emits | `this` binding | When to use |
|--------------|-------|----------------|-------------|
| `func name :args ... :body ...` | kernel `function` (named, with contracts) | dynamic | Surface-rich function with types and contracts |
| `(fn ...)` or `(lambda ...)` | kernel `function` (anonymous) | dynamic | Anonymous function, traditional Lisp semantics |
| `(=> ...)` | kernel `=>` | lexical | Arrow function, modern JS semantics |

The current Rust compiler emits `=>` for both `lambda` and `fn`;
this is a divergence from JS (which emits `function`) and from
the original language-design intent. DD-58 aligns Rust to JS:
both compilers emit `function` for surface `lambda` and `fn`,
and emit `=>` for surface `=>`. This is in the **Breaking Changes
Inventory** below.

---

## Migration sequencing

DD-58's implementation lands in a sequence designed to fail fast
at each step.

### Phase 0 — Preconditions

- DD-37 in `05-active/` with three amendments applied. *(Done
  2026-05-15.)*
- M16 (Cross-Compiler Hygiene) and downstream cleanups landed.
  *(In flight; will complete before Phase 1.)*
- DD-30 (pure-Rust kernel→JS codegen) substantively complete.
  *(Implementation done.)*
- DD-37's Phase 0 acceptance criterion satisfied (baseline bundle
  size + CI guard + one-form pilot). Per DD-37's acceptance
  criterion, this gates per-form migration work.

### Phase 1 — Classifier-strict mode (Rust)

- Add a `strict` mode flag to `classifier::classify()`. In strict
  mode, the dispatch table is the closed surface namespace; a
  surface-position atom not in the namespace AND not `kernel:`-
  prefixed AND not a registered user macro produces a diagnostic.
- Implement the `kernel:` prefix recognition: strip prefix,
  validate the form against a kernel-form whitelist, emit
  `KernelPassthrough`.
- Land the closed-namespace dispatch tables (per the enumeration
  above) replacing the current overlapping `is_surface_form` /
  `is_kernel_form` pair.

### Phase 2 — JS classifier (DD-37 Phase 3+)

- DD-37's six-module decomposition lands the JS classifier.
- The JS dispatch tables mirror the Rust dispatch tables.
- The `_kernel` marker and `kernelArray()` helper are retired.

### Phase 3 — Form-catalog source-of-truth (DD-56)

- DD-56's form catalog at `spec/forms.toml` (or wherever DD-56
  lands) becomes the canonical source for the dispatch tables.
- Both compilers' classifiers consume the catalog via codegen.

### Phase 4 — Extension-based dispatch (file-level)

Per Duncan's 2026-05-17 call: file-extension gating lands as part
of this work for architectural cleanliness.

- `.lyk` files compile through the kernel path; `.lykn` files
  through the surface path.
- `lykn fmt`, `lykn check`, `lykn compile`, `lykn run`,
  `lykn test` all dispatch on extension.
- Hard errors at the dispatch boundary: surface forms in `.lyk`
  files produce diagnostics; bare (non-`kernel:`-prefixed) kernel
  forms in `.lykn` files produce diagnostics.
- Test discovery accepts both extensions; tests are themselves
  written as `.lykn` (surface) because the test DSL is surface.

### Phase 5 — Retirement work

- `SetSymbol` variant in `SurfaceForm` removed; users migrate to
  `set!` (DD-23 already provides this).
- `=`/`!=` removed from `is_kernel_form()` in dispatch.rs (the
  overlap is structurally impossible under closed-namespace).
- `macro`/`import-macros` removed from `is_kernel_form()` (dead
  overlap, already not used).
- `_kernel` marker and `kernelArray()` helper retired from JS
  (DD-37 Phase 2).
- The class-constructor `=` and destructuring-assignment `=` bugs
  are resolved at the language level (no codegen work; the surface
  classifier no longer routes `=` to ambiguous contexts).
- Rust lambda/fn emission realigned to JS: emit `function`, not
  `=>` (closes the lambda divergence fast-follow from M16-2).

### Phase 6 — Documentation

- New guide: "The kernel escape hatch and when to use it." Most
  users never need `kernel:`; the guide names the cases that do
  (advanced JS interop, generated-code escape, etc.).
- Updated guides: review examples for any reference to "bare
  kernel forms in surface code." None should remain.
- New guide: "Closed surface namespace — the architectural rule
  and how forms are classified."
- Release notes for 0.6.0: migration section covering breaking
  changes (per inventory below).

---

## Rejected alternatives

Four kernel-call syntax candidates were considered. The chosen one
is `(kernel:<form> ...)`. The others are documented here with the
specific reasons they were rejected.

### Rejected: `(kernel (<form> ...))` enclosing-form syntax

**What:** Explicit "drop to kernel" form wrapping a kernel call.

**Why rejected:** Every kernel call inside surface code requires
double nesting (`(kernel (if c t e))` instead of `(kernel:if c t e)`).
For the common case of a single kernel form embedded in surface,
the double-nesting is unergonomic. The empirical evidence from
the M16-2 work suggests kernel-form patterns will be needed in
surface code regularly enough that the ergonomic cost is
significant.

### Rejected: `#kernel(...)` or `#k(...)` reader macro

**What:** Reader-level dispatch tag. The reader sees `#k(if c t e)`
and produces a `KernelTag` node; the classifier accepts it as
already-classified.

**Pros:** No DD-01 collision. Compact. Lisp-familiar.

**Why rejected:**
1. The `js:` precedent — lykn already uses colon-namespace syntax
   for JS interop (`(js:call ...)`, `(js:typeof x)`). Users know
   one colon-namespace mental model; `kernel:` extends it.
2. i18n consideration — `#k(...)` requires deciding what `k`
   means in non-Latin scripts. `kernel:` as a head-position prefix
   stays English-canonical regardless of locale.
3. Reader-level dispatch is heavier machinery than a naming
   convention. Adding a dispatch tag for kernel escape adds
   reader complexity for a benefit (no DD-01 collision) that's
   bounded.

### Rejected: Inverse marking — kernel forms bare, surface forms marked

**What:** Kernel forms are the default; surface forms get a marker
(`(surface:if ...)`, `(s:func ...)`, etc.).

**Why rejected:** This is structurally wrong. Surface is the
user-facing language; surface forms should be unmarked. Marking
surface and leaving kernel unmarked inverts the relationship
between the two layers.

### Chosen: `(kernel:<form> ...)` namespace prefix

**Pros:**
- Reuses the `js:` colon-namespace mental model.
- Compact for the common single-form escape case.
- Stays English-canonical across i18n locales.
- The DD-01 collision is bounded — the classifier dispatches
  before member-access compilation; `kernel:` is a documented
  exception.

**Cons (honest):**
- A new exception to DD-01 (bounded, documentable).
- `kernel:` is not the prettiest spelling.

The trade-off was decided 2026-05-15: the `js:`-precedent
ergonomic + i18n alignment outweighs the new DD-01 exception.

---

## Relationships to other DDs

### DD-37 — JS Surface Compiler Architecture

DD-37 provides the JS-side classifier and typed surface AST that
DD-58 requires for enforcement on the JS side. DD-37 ships first
(its Phase 0 acceptance criterion governs); DD-58's strict
enforcement turns on once DD-37's classifier is in place. The two
are complementary, not sequential dependencies. DD-37's three
2026-05-15 amendments include a "Relationship to DD-58" subsection
that names this dependency from the JS side.

### DD-56 — Canonical Form Catalog

DD-56 (drafted by cdc/dep-ergonomics at
`workbench/dd-56-canonical-form-spec-2026-05-14.md`) becomes the
source-of-truth for both compilers' dispatch tables once DD-58
lands. The catalog's schema design depends on DD-58's namespace
decisions:

- **Surface entries** enumerate the full user-facing namespace
  with per-locale translation hooks.
- **Kernel entries** enumerate the JS-language vocabulary; no
  translation hooks.
- **Passthrough surface entries** carry an `expands_to` field
  pointing at the same-named kernel form.
- **Rich namesake-sharing surface entries** (`if`, `try`, `class`,
  `class-expr`) carry an `expansion` descriptor (e.g.,
  `"position-aware"`, `"rich-class"`).

The cdc/dep-ergonomics CDC's W-3 (DD-56 implementation) was paused
pending DD-58's resolution; this DD's promotion unblocks that work.

### DD-50.x — Position-aware compilation

DD-50, DD-50.5, DD-50.6, DD-50.7 are all **preserved intact**.
DD-58 makes their machinery structurally clean: surface `if`
(flavor c) has position-aware behaviour as its emission semantics;
the classifier no longer has to decide whether a given `(if ...)`
is surface or kernel via context inspection.

### DD-22 — Surface Equality

DD-22's surface `=` (compiles to `===`) and `!=` (compiles to
`!==`) are preserved unchanged. Under DD-58, they are surface
flavor (c) forms whose kernel namesakes (kernel `=` for assignment,
kernel `!=` for loose inequality) are reached only via
`(kernel:= ...)` and `(kernel:!= ...)`. Property assignment is
unambiguously `set!`, not `=` — closing the M16-2 class-constructor
and destructuring-assignment correctness bugs at the language
level.

### DD-23 — `set!` for property mutation

DD-23 is preserved unchanged. The closed-namespace model makes
`set!` the unambiguous property-assignment form, eliminating the
ambiguity that the M16-2 correctness-grade bugs exploited.

### DD-36 — Kernel/Surface Compiler Split (superseded)

DD-36 proposed the split with `(kernel:<form> ...)` as one of two
alternative escape syntaxes (Alt A — the chosen one) and `#k(...)`
as the other (Alt B — rejected). DD-58 takes Alt A, adds the
closed-namespace discipline, settles the syntax and ordering, and
supersedes DD-36's direction. DD-36's analysis is preserved at
`docs/design/01-draft/0046-dd-36-*.md`.

### DD-30 — Pure-Rust Kernel→JS Codegen

DD-30's pure-Rust codegen is substantively complete. DD-58 doesn't
depend on DD-30 beyond knowing it's done.

---

## Breaking changes inventory

In rough decreasing order of user-visible impact:

1. **Surface code that uses bare kernel-only forms must add the
   `kernel:` prefix.** Likely-affected:
   - `(const x 42)` → `(bind x 42)` (idiomatic) OR
     `(kernel:const x 42)` (explicit).
   - `(let x 0)` → `(bind x (cell 0))` (idiomatic) OR
     `(kernel:let x 0)` (explicit).
   - `(var x)` → `(bind x (cell undefined))` OR
     `(kernel:var x)` (explicit).
   - `(function f (x) ...)` → `(func f :args (:any x) :body ...)`
     OR `(kernel:function f (x) ...)`.
   - `(quote ...)` / `(quasiquote ...)` → require `kernel:` prefix
     (no surface idiom for these).

   **Not affected:** `(=> (x) (+ x 1))` stays as written — `=>` is
   a surface passthrough form (flavor b). `(+ a b)`, `(array 1 2)`,
   `(new Foo)`, `(typeof x)`, etc. — all surface passthrough forms,
   no migration needed.

2. **File extensions are gated.** Surface forms in `.lyk` files →
   diagnostic. Bare (non-`kernel:`-prefixed) kernel forms in
   `.lykn` files → diagnostic.

3. **Surface `lambda` and `fn` emit kernel `function` (anonymous),
   not `=>`.** Rust currently emits `=>` for both; this is a
   divergence from JS and from the original language-design intent.
   After DD-58, both compilers emit `function` for `lambda`/`fn`,
   and emit `=>` for the new surface `=>` passthrough form.
   **`this` binding semantics change for surface `lambda`/`fn`:
   from lexical (arrow) to dynamic (function).** Users relying on
   lexical `this` in `lambda`/`fn` bodies must migrate to
   `(=> ...)` (the surface arrow form — no `kernel:` prefix
   needed; modern-JS-idiomatic).

4. **Class-constructor `=` and destructuring-assignment `=` move
   to `set!`.** Pattern: `(class Foo () (constructor (x) (= this:x x)))`
   → `(class Foo () (constructor (x) (set! this:x x)))`.
   Pattern: `(= (object a b) obj)` → use `(bind {a b} obj)` for
   destructuring bind OR `(set! ...)` for property mutation as
   appropriate. **This closes the M16-2 correctness-grade bugs at
   the language level — the bugs disappear because the ambiguous
   syntax is no longer permitted.**

5. **`SetSymbol` variant retired.** Users on `set-symbol!` must
   migrate to `set!` (DD-23). Documented in 0.6.0 release notes.

6. **`(kernel:=) ...)` for kernel assignment is now the only path
   to kernel `=`.** Users of class-body assignment pre-0.5.0
   already migrated to `set!` (DD-22 breaking change). Any user
   macro emitting kernel `=` directly may break; audit suggests
   this is rare to nonexistent in the in-repo stdlib.

7. **The `_kernel` marker is removed from JS expander.**
   User-defined macros that set `_kernel` (rare) may break.

---

## Resolved questions (from first DD-58 draft, 2026-05-15)

The six open questions from the first draft are now resolved per
Duncan's 2026-05-17 calls:

1. **`const`/`let`/`var`/`function`/`=>` — kernel-only via escape.**
   Surface uses `bind`+`cell`+`func`/`fn`/`lambda` idiomatically.
2. **`class`/`class-expr` — flavor (c) namesake-sharing**, rich
   surface semantics on top of kernel `class`/`class-expr`.
3. **User-defined macros emitting kernel forms — implicit kernel
   output**, matching current behaviour.
4. **File-extension gating — lands in 0.6.0** as part of this work
   (per Phase 4 of migration sequence).
5. **Lambda direction — `lambda` and `fn` emit kernel `function`
   (anonymous)**, not `=>`. Rust aligned to JS and to original
   language-design intent. **Surface `=>` exists as a flavor (b)
   passthrough** (2026-05-17 follow-up call) for users who want
   modern JS arrow semantics with lexical `this`.
6. **The M16-2 deferred items + lambda divergence + JS template
   double-escape — enumerated in Breaking Changes Inventory above**
   as items resolved by this work.

---

## Remaining open questions

These are smaller calls that can be resolved during implementation
or in a future amendment.

1. **Diagnostic posture for unrecognised surface atoms.**
   Proposed: "unknown form `foo`; did you mean `bar` (close
   match), or `(kernel:foo ...)` (if `foo` is a kernel form), or
   register a macro for `foo`?" Worth a follow-up DD or just
   bedding in during implementation.
2. **REPL behaviour under closed-namespace.** The REPL should
   probably default to surface mode; users who want kernel REPL
   semantics use a flag or different invocation. Separate DD if
   needed.

---

## Verification notes

Claims in this DD drawn from direct reads of the source:

- `crates/lykn-lang/src/classifier/dispatch.rs` — full read.
  Surface and kernel form lists verbatim.
- `crates/lykn-lang/src/ast/surface.rs` — read for enum variant
  names (including `Class`, `ClassExpr`, `Lambda`, `Fn`).
- `crates/lykn-lang/src/emitter/forms.rs` — read for
  `kernel_child_profile` registrations and surface-form intercept
  site (line ~356, post-M16-5 refactor).
- `crates/lykn-lang/src/codegen/emit.rs` line 193:
  `"=" => emit_assignment(w, args)?` — kernel codegen does have an
  assignment path; the M16-2 correctness bugs are at surface
  classifier level, not codegen.
- `packages/lang/surface.js` — grepped for `macroEnv.set(...)`
  registrations; full structure not read end-to-end.
- `packages/testing/helpers.js` — `compileBoth()` helper and
  current normalizer state.
- `docs/guides/01-core-idioms.md` ID-38 — confirms kernel `=` is
  context-dependent (top-level: equality; block-level: assignment).
- `workbench/2026-05-16-m16-formatting-divergences-diagnosis.md`
  — empirical findings for the correctness-grade bugs.

Unverified claims I should flag:

- **`lambda`/`fn` aliasing in the JS expander.** I verified
  `macroEnv.set("fn", fnMacro); macroEnv.set("lambda", fnMacro);`
  exists, but the precise emission to `function` (anonymous) vs.
  `=>` I'd want to spot-check during implementation. CC will
  verify when Rust is aligned to JS.
- **Whether `class`/`class-expr` are flavor (c) namesake-sharing
  or flavor (a) surface-unique with different kernel names.** I
  read the `SurfaceForm` enum and saw `Class`/`ClassExpr`
  variants. Duncan confirmed flavor (c). The implementation will
  need to surface any places where the surface and kernel
  semantics diverge (e.g., method definitions) and document them
  in DD-56.

---

## Citations

Source files referenced:

- `crates/lykn-lang/src/classifier/dispatch.rs` — current dispatch tables.
- `crates/lykn-lang/src/ast/surface.rs` — `SurfaceForm` enum.
- `crates/lykn-lang/src/emitter/forms.rs` — `kernel_child_profile` registrations + surface intercept site.
- `crates/lykn-lang/src/codegen/emit.rs` — kernel codegen including the assignment path.
- `crates/lykn-lang/src/expander/pass0.rs` — runtime-import handling (post-2026-05-16 fix).
- `packages/lang/surface.js` — JS surface macros.
- `packages/lang/expander.js` — `_kernel` marker.
- `packages/testing/helpers.js` — `compileBoth` helper with `--source-context-path`.
- `docs/design/01-draft/0046-dd-36-kernel-surface-compiler-split.md` — DD-36 (superseded).
- `docs/design/05-active/0047-dd-37-js-surface-compiler-architecture.md` — DD-37 (with 2026-05-15 amendments).
- `docs/design/05-active/0050-position-aware-compilation-of-conditional-and-block-forms.md` — DD-50.
- `docs/design/06-final/0031-dd-22-surface-equality-and-logical-operators.md` — DD-22 (surface `=`).
- `docs/design/06-final/0032-dd-23-set-external-property-mutation.md` — DD-23 (`set!`).
- `docs/guides/01-core-idioms.md` — ID-38 kernel-`=` context-dependence.
- `workbench/dd-56-canonical-form-spec-2026-05-14.md` — DD-56 (cdc/dep-ergonomics).
- `workbench/handoff-surface-kernel-separation-2026-05-14.md` — cdc/dep-ergonomics handoff.
- `workbench/2026-05-10-compiler-coherence-thread-opening.md` — this thread's opening + Resolutions.
- `workbench/2026-05-16-m16-formatting-divergences-diagnosis.md` — M16-2 divergence findings.
- `workbench/2026-05-16-wishlist-cleanup-closing-report.md` — wishlist closure (lambda + template + paren fixes).

---

## Refinement log

### 2026-05-17 (second revision)

Duncan's design calls on the six open questions from the first
draft (2026-05-15) baked in:

- **JS declaration / binding constructs (`function`, `const`,
  `let`, `var`) move to kernel-only via escape.** Surface
  namespace stays focused on `bind`+`cell`+`func`/`fn`/`lambda`.
- **`class`/`class-expr` confirmed as flavor (c) namesake-sharing.**
- **Macro expansion stays implicit-kernel-output** (matches current
  behaviour).
- **File-extension gating lands in 0.6.0** as Phase 4 of the
  migration sequence (was deferred in first draft).
- **Lambda direction settled:** `lambda`/`fn` emit kernel
  `function` (anonymous); Rust aligned to JS and to original
  language-design intent.
- **M16-2 deferred items + lambda divergence + JS template
  double-escape** enumerated in Breaking Changes Inventory as items
  this DD resolves.
- **"Passthrough surface form"** is the canonical term for flavor
  (b) (previously "thin wrapper" in first draft).

The first draft's mid-document course-correction ("Wait — that's
wrong. Let me re-think this") is removed in this revision; the
final enumeration stands without the reasoning trace.

### 2026-05-17 (second-revision follow-up — surface `=>` + `and`/`or` correction)

Two refinements after Duncan reviewed the second revision:

- **Surface `=>` added as flavor (b) passthrough.** Modern JS devs
  writing `(=> (x) x)` get a real arrow function with lexical
  `this`. The function-form choice in surface is now: `func`
  (rich), `fn`/`lambda` (anonymous `function`, dynamic `this`),
  `=>` (arrow, lexical `this`). Migration path for users wanting
  lexical `this` in lambda/fn bodies is now `(=> ...)`, not
  `(kernel:=> ...)`.

- **`and`/`or`/`not` characterisation fixed.** First and second
  revisions described these as having "short-circuit IIFE
  semantics, distinct from kernel `&&`/`||`/`!`." Source-read
  shows that's wrong: surface `and`/`or`/`not` are direct
  emissions to kernel `&&`/`||`/`!` with n-ary syntactic sugar
  (left-associative fold for n-ary). Same JS short-circuit
  semantics in both forms; only difference is ergonomic
  (Lisp-style n-ary chains vs. JS-style binary operators). The
  flavor (a) entry is rewritten to reflect actual semantics.

The `=>` change ripples through Breaking Changes Inventory #1
(removed from list of kernel-prefixed migrations), Breaking
Changes Inventory #3 (lambda divergence migration path now uses
surface `=>`), the "Note on lambda, fn, and =>" subsection, and
Resolved Questions #5. Remaining Open Questions #3 (about
surface `=>` for 0.7.0) is removed — resolved here.
