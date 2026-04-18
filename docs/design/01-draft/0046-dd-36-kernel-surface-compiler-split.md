---
number: 46
title: "DD-36: Kernel / Surface Compiler Split"
author: "file
extension"
component: All
tags: [change-me]
created: 2026-04-18
updated: 2026-04-18
state: Draft
supersedes: null
superseded-by: null
version: 1.0
---

# DD-36: Kernel / Surface Compiler Split

**Status**: Draft
**Date**: 2026-04-18
**Session**: Post-0.5.0 QA analysis — kernel/surface boundary
**Depends on**: DD-15 (language architecture), DD-20 (Rust surface compiler), DD-30 (pure-Rust codegen)
**Targets**: 0.6.0 (first release after 0.5.0)

## Summary

This DD evaluates the proposal to **split kernel and surface compilation
into two distinct, non-overlapping languages**, enforced by file
extension (`.lyk` for kernel, `.lykn` for surface) with a
`(kernel:<form> ...)` escape hatch and a blessed set of kernel forms
auto-promoted at surface-compile time.

**Recommendation**: *Endorse with refinements.* The intuition is
correct and the bulk of the work is already done on the Rust side.
The proposal is feasible in one release (0.6.0) if scoped carefully.
The largest risks are (a) the `kernel:` prefix collides semantically
with DD-01 colon syntax, (b) the test DSL complicates file-extension
gating, and (c) the JS compiler carries most of the remaining debt
and needs its own reorganisation, not a transliteration of the Rust
architecture. Three alternatives are presented; one (rename colliding
forms + keep soft gating) is a lower-cost fallback worth keeping in
reserve.

This is a workbench draft. It is opinionated on purpose — in the
spirit of the collaboration framework, I report my honest assessment,
flag pattern-matching where I notice it, and identify the places where
I am inferring rather than verifying.

## Context: where the language actually is right now

The lykn compiler today is two languages that share a compiler — not
two compilers that share a grammar. Concretely:

- **Kernel** ("`lykn/kernel`" in DD-15) is the compilation target:
  thin s-expression wrappers around JS statements and operators
  (`const`, `let`, `if`, `function`, `=>`, `array`, `object`, `=`,
  arithmetic and comparison operators, etc.).
- **Surface** ("`lykn/surface`" in DD-15) is the authoring language:
  `bind`, `func`, `fn`/`lambda`, `match`, `type`, `obj`, `cell`,
  threading macros, `if-let`/`when-let`, `and`/`or`/`not`, surface
  equality (`=`, `!=`), and so on.

Both dialects are accepted by the same reader. The Rust pipeline
classifies each form against two lists — `is_kernel_form()` and
`is_surface_form()` in `crates/lykn-lang/src/classifier/dispatch.rs`
— then builds a typed `SurfaceForm` AST, runs analysis, emits kernel
`SExpr`, and codegens JS. The JS pipeline is similar in spirit but
far less separated: `reader → expander → compiler`, where the
`compiler.js` dispatch table conflates kernel built-ins and surface
macros, and surface expansion sits inside `expander.js` alongside
macro-module loading and fixed-point expansion.

DD-20 calls the JS compiler "reference implementation and future
browser-path compiler"; DD-30 is actively replacing the Deno
subprocess that the Rust expander shells out to. Against that
backdrop, this DD is really asking: *can we finish the job DD-15
started?*

## Problem statement: concrete evidence

The pain Duncan described is not a vibe — it is visible in the
source. Four exhibits.

### 1. The `_kernel` marker in `expander.js`

`packages/lang/expander.js:731` contains this comment and gate:

```js
// Fixed-point macro expansion
// Skip re-expansion of forms marked as kernel output by surface macros
if (head.type === 'atom' && macroEnv.has(head.value) && !form._kernel) {
```

The marker exists because surface macros emit kernel output, and the
fixed-point expander would happily re-expand a kernel `=` back
through the surface `=` macro if the output were not tagged. The
`kernelArray()` helper in `packages/lang/surface.js` (line 24) is the
companion — it sets `node._kernel = true` on synthesised `array`
nodes so the expander leaves them alone. It is used in at least five
places (`reset!`, `swap!`, `set-symbol!`, `set!`, one in `if-let`).

This is a classic leak: the expander has to know about the surface
layer in order to *not* re-run surface rules. A clean split removes
the marker entirely.

### 2. The `=` / `!=` overlap in the Rust classifier

`crates/lykn-lang/src/classifier/dispatch.rs` lists `=` and `!=` in
**both** `is_surface_form()` (lines 30–31) and `is_kernel_form()`
(lines 78, 100). The classifier currently prefers the surface
interpretation; the kernel interpretation is reached through
`KernelPassthrough` from emitter output. This is the single clearest
case of "same atom, two meanings, disambiguated by position in the
pipeline." It is also the form that caused the 0.5.0 breaking
change to class-body `=` (see `workbench/release-notes-0.5.0.md`).

Other overlaps in the dispatch tables: `macro` and `import-macros`
appear in both lists; in practice the kernel occurrence is dead weight
(both are surface-only constructs), but their presence documents how
little the two lists have been audited against each other.

### 3. The `SetSymbol` TODO

`crates/lykn-lang/src/ast/surface.rs:294`:

```rust
// TODO: deprecate when surface/kernel syntaxes are separated;
// remove the release after that.
SetSymbol {
    obj: SExpr,
    key: SExpr,
    value: SExpr,
    span: Span,
},
```

The variant exists purely because the language cannot today
distinguish "kernel assignment of a property" from "surface
`set-symbol!` with its own semantics." A prior author already
anticipated this DD.

### 4. `.lyk` / `.lykn` are completely interchangeable today

`crates/lykn-cli/src/util.rs:7-9`:

```rust
pub fn is_lykn_ext(ext: &std::ffi::OsStr) -> bool {
    ext == "lykn" || ext == "lyk"
}
```

And `is_lykn_test_file()` in `main.rs` enumerates the four
combinations (`*_test.lykn`, `*.test.lykn`, `*_test.lyk`,
`*.test.lyk`) explicitly. The extension already carries
**connotation** (examples are sorted `examples/kernel/*.lyk` vs
`examples/surface/*.lykn`) but no **semantics**. The compiler will
happily accept surface forms in a `.lyk` file and vice versa.

### What these exhibits collectively say

Each leak is small. Together they describe a compiler that keeps one
rulebook and works by convention to decide which rule applies. Every
new feature that touches the kernel/surface boundary (class-body `=`,
threading macros with method calls, destructured parameters) adds a
case to the convention. This is the form of complexity that compounds
quietly and then presents itself as a cluster of QA bugs before a
release, which is exactly what Duncan described.

## The proposal

Duncan's proposal has four parts:

1. **Split compilation entirely.** Kernel and surface are compiled by
   separate front-ends (one per language, per host). No shared
   dispatch table.
2. **Hard-gate by extension.** `.lyk` files contain only kernel
   forms. `.lykn` files contain only surface forms.
3. **Escape hatch.** Surface code can drop into the kernel via
   `(kernel:<form-name> ...)` — e.g. `(kernel:if cond then else)`.
4. **Auto-promoted set.** A blessed subset of kernel forms is
   recognised in surface code without the `kernel:` prefix and
   expanded to it at surface-compile time. (E.g. `+`, `-`, `*`, `/`,
   `array`, `object`, `get`, literal forms — things that have no
   surface counterpart and no ambiguity.)

## Complete impact analysis

### Rust compiler: ~15% additional work over what exists

The Rust pipeline is already structured to support this. The split
is essentially a naming and gating exercise on top of the existing
architecture:

- `classifier::classify()` already routes known kernel forms to
  `KernelPassthrough` and known surface forms to typed variants.
- The `emitter` already lowers `SurfaceForm` → kernel `SExpr`.
- `codegen` already consumes kernel `SExpr` exclusively.

What needs to change:

- **Two entry points.** `compile_kernel(source) -> JS` and
  `compile_surface(source) -> JS`. The kernel path skips classifier
  and analysis entirely; it is just `reader → codegen` with a
  kernel-form whitelist check and hard errors on unknown forms.
- **Classifier becomes strict.** A surface compile that encounters
  a known-kernel-only form (e.g. bare `const`, `while`, `function`)
  errors out with a "use `kernel:` or an equivalent surface form"
  diagnostic. Today it silently admits these through
  `KernelPassthrough`.
- **`kernel:` prefix handling** at the classifier boundary: strip
  the prefix, validate the form name against the kernel whitelist,
  emit a `KernelPassthrough`. (Caveat: see DD-01 conflict below.)
- **Auto-promoted set** lives as a list in `classifier/dispatch.rs`;
  forms in it are rewritten to `kernel:…` during classification.
- **Retire `SetSymbol`.** The 0.5.0 release notes already promise
  this direction (surface `=` is now equality; mutation moves to
  `set!`).
- **Retire the overlap.** `=` and `!=` leave `is_kernel_form()`
  entirely (they're auto-promoted through a different route — see
  "Alternatives" below for a cleaner path).

Estimate: three to five focused PRs, modest risk, well-covered by
existing tests.

### JS compiler: the real work is here

`packages/lang/compiler.js` and `packages/lang/expander.js` are the
opposite of DD-20's modular pipeline — they are a single pass that
does reading, expansion, and emission with a shared macro table. A
clean split requires extracting at least three layers:

1. **A kernel compiler** — a thin function that takes an s-expression
   tree known to contain only kernel forms and produces ESTree (then
   JS via astring). This is most of the current `compileExpr` with
   surface cases deleted.
2. **A surface compiler** — `reader → expander → emitter (to kernel
   SExpr) → kernel compiler`. Today's expander already does most of
   this, but the output is passed directly into the same
   `compileExpr` without an explicit kernel-SExpr handoff.
3. **Removal of the `_kernel` marker** and `kernelArray()` helper.
   These disappear once the expander no longer re-walks surface
   output.

The JS side is also where `@lykn/testing` lives, and the test runner
compiles `.lykn` test files via the JS compiler (`main.rs:480` in the
CLI delegates to `import { lykn } from 'lang/mod.js'`). A botched JS
refactor takes the test suite with it.

Estimate: two to four focused PRs, higher risk than the Rust side,
sequencing matters (see phasing).

### Reader, macros, quasiquote

The reader is unaffected. DD-12 already decouples reader structure
from semantic interpretation — the reader produces `SExpr` and does
not care whether the head atom is a kernel or surface form.

Macro-module loading (`DD-14`, `DD-34`) is mostly unaffected, but
the **macros themselves** straddle the boundary: a user-defined
surface macro emits kernel code. The cleanest model is that macros
always emit kernel `SExpr` (with `kernel:` prefixes explicit in the
output), and the surface expander does not re-classify macro output.
This is approximately what the `_kernel` marker already simulates —
the DD would make it structural rather than a tag.

### Tests: partition, do not rename

The test tree currently has:

- `test/forms/*_test.lykn` — kernel-form tests authored with the
  surface test DSL (`bind`, `test`, `assert-equal`). Naming these
  `.lyk` would be wrong; they *are* surface files that happen to
  exercise kernel features.
- `test/surface/*_test.lykn` — surface-form tests.
- `test/surface/kernel-in-surface_test.lykn` — already exists,
  already tests the boundary Duncan wants to formalise.
- `examples/kernel/*.lyk` and `examples/surface/*.lykn` — already
  partitioned.

**Insight (honest caveat): the file-extension rule is subtler than
Duncan's proposal implies.** "Kernel forms in `.lyk`, surface forms
in `.lykn`" would force the kernel test files to be renamed, but the
test DSL itself is surface (it defines `test` and `bind` as surface
macros). The honest rule is closer to: "file extension determines
*what the reader/compiler will accept as valid top-level*, and the
test DSL is a surface tool that happens to produce kernel-heavy
output." This is fine; it just means the rule is "which compiler
runs" rather than "what forms appear."

New tests needed:

- Cross-boundary: `.lykn` file importing a `.lyk` module.
- Error cases: kernel-only form in `.lykn` without `kernel:`.
- Error cases: surface form in `.lyk`.
- `kernel:` prefix round-trips through all stages.
- Auto-promoted forms behave identically with and without prefix.

Snapshot tests (DD-35) catch regressions in the emitter output.

### Docs: reorganise, don't rewrite

- **DD-15** already names the split; cite it, do not recreate it.
- **Guides** in `docs/guides/` likely have examples that mix kernel
  and surface liberally (I did not do an exhaustive audit — flagging
  this as unverified). These need a review pass but not a rewrite.
- **A new guide** — "the kernel escape hatch and when to use it" —
  is probably a ~200-line document. Most users should never need
  `kernel:`.
- **Release notes for 0.6.0** will need a migration section
  covering any surface file that currently uses bare kernel forms.

### Publishing pipeline: largely unaffected

DD-33's three package kinds (`runtime`, `macro-module`, `tooling`)
don't depend on file extension. `lykn build --dist` stages source
files verbatim. The only change is that `.lyk` and `.lykn` are now
carrying different semantics, so `dist/` layouts for packages that
contain both extensions need to preserve them rather than
normalising. I believe the pipeline already does this — I did not
verify.

### CLI: small surface-area changes

- `lykn fmt` needs to dispatch on extension (kernel formatter vs
  surface formatter).
- `lykn check` similarly.
- `lykn compile` already takes a filename, so extension dispatch is
  natural.
- Test discovery is unaffected (both extensions continue to match).

### Breaking changes inventory

In rough decreasing order of user-visible impact:

1. Surface files (`.lykn`) containing bare kernel forms must either
   (a) be renamed to `.lyk`, (b) switch to surface equivalents, or
   (c) prefix kernel calls with `kernel:`.
2. The `SetSymbol` path is removed; users must migrate to `set!`.
3. Any surface macro that emits bare kernel forms without the
   `kernel:` prefix in its output must be updated. (This may be
   zero in the standard library — worth auditing.)

## Feasibility: yes, with coordination

The change is sequenceable and reversible at each step. I assess it
as **feasible for a 0.6.0 target** on three conditions:

1. **It does not land while DD-30 is in-flight.** DD-30 (pure-Rust
   codegen) is replacing the Deno subprocess in the Rust expander.
   Landing the kernel/surface split while the expander architecture
   is also in flux doubles the coordination cost. Finish DD-30
   first, or finish this first — not both simultaneously.
2. **The JS refactor is treated as its own piece of work.** It is
   not a transliteration of the Rust architecture. DD-20 describes
   the Rust architecture; there is no corresponding DD for the JS
   side, and the JS compiler was written before DD-20 existed.
   Writing that DD is a prerequisite, not an afterthought.
3. **The migration is announced in 0.5.x patch notes and enforced
   as a hard break in 0.6.0.** A deprecation cycle that accepts
   both for a release is tempting but doubles the work (you are
   maintaining both rulebooks *and* the new split for a release).
   Pre-1.0 is the right time for a hard cut.

## Is it a good idea? My honest assessment

Yes, with one reservation.

**What I am confident about:** The split addresses a real category of
bug (boundary ambiguity), not a cosmetic concern. The Rust
architecture already anticipates it. DD-15 already names it. The
`SetSymbol` TODO explicitly calls for it. The class-body `=` story
in 0.5.0 — where the fix was a breaking change to disambiguate —
is the canonical example of the current design forcing breaking
changes that a cleaner split would make routine.

**The reservation is the `kernel:` prefix specifically.** DD-01
makes colon syntax member access: `foo:bar` compiles to `foo.bar`.
`(kernel:if cond t e)` as a surface form is therefore parsing as "a
call whose head is a member access `kernel.if`." DD-01 says the
reader treats colons as ordinary characters and the *compiler*
splits on them; so a classifier that runs before member-access
compilation can claim `kernel:` as a namespace prefix. But this
means `kernel` becomes a **reserved head-position atom with special
colon semantics**, which is a subtle exception to DD-01. It is
workable, and the gain (explicit, greppable, visually obvious
escape hatch) is real. I flag it because it is the one place where
the proposal adds a new exception rather than removing one.

**Pattern-matching I noticed in myself, flagged in the spirit of
honest engagement:** this proposal has the shape of "the codebase
has accrued special cases, let's introduce a structural rule that
subsumes them," which is a shape I am predisposed to endorse. I
checked by asking what would be *worse* under the new regime and
found two candidates: (a) the `kernel:` exception above; (b) a user
who is learning and wants to write a quick kernel-style script in
`.lykn` now has to either rename or prefix. Both are real but both
are minor.

## Alternatives

Four options, roughly in order of increasing conservatism.

### Alt A: Duncan's proposal as stated

Covered above. Hard gating, `kernel:` prefix, auto-promoted set,
full split. Best long-term structure, highest short-term cost.

### Alt B: Split, but use a reader tag instead of `kernel:`

Use `#k(if cond t e)` (a reader-level dispatch, DD-12 already
defines `#` as the reader dispatch char) instead of `(kernel:if …)`.

**Pros:**
- No DD-01 collision.
- Reader handles it, classifier stays clean.
- Visually distinct — kernel code looks different from surface code.

**Cons:**
- `#k` is an unusual spelling that reads as "hex" or "macro" to
  newcomers; `kernel:` is self-documenting.
- A reader tag is heavier machinery than a naming convention for the
  common case of "use `if` as kernel-if."

### Alt C: Rename the handful of colliding forms; skip the gating

Audit the current overlap and rename kernel forms that collide with
surface forms so the language has no ambiguous atoms. Concretely:
rename kernel assignment to `:=`, kernel equality to `===`/`!==`
(which already exist in the kernel list), move `lambda`, `macro`,
`import-macros` to surface-only. Then keep a single compiler but
with *no* overlap in the dispatch table.

**Pros:**
- No file-extension gating needed.
- No `kernel:` prefix needed.
- Far less JS compiler work.
- Reversible; each rename is an independent decision.

**Cons:**
- Does not solve the `_kernel` marker / `kernelArray()` leak
  (surface macros still emit into the kernel layer and the expander
  still needs to know not to re-walk).
- Does not structurally separate the two languages; future features
  that straddle the boundary still need per-feature disambiguation
  rules.
- "Kernel vs surface" as a category becomes fuzzier, not clearer, in
  user docs.

### Alt D: `#lang` pragma (Racket-style)

Files declare their dialect with a first-line pragma: `#lang
lykn/kernel` or `#lang lykn/surface`. Extension remains
informational.

**Pros:**
- Precedent (Racket, Clojure `.cljc` with reader conditionals).
- Orthogonal to file extension (a single `.lyk` file could declare
  itself surface, for example, though we would not want that).
- Keeps the door open for future dialects without burning more
  extensions.

**Cons:**
- Every file needs a header (or the absence of a header is a
  default, which defeats the explicitness).
- File-extension gating is simpler, and we already have two
  extensions that users have started treating as meaningful.

### My ranking

1. **Alt A (proposal as stated)** — best long-term shape, modulo the
   `kernel:` / DD-01 collision. Land with care.
2. **Alt A + Alt B combined** — gating by extension, reader tag
   `#k(...)` for the escape hatch. This is my actual recommendation.
   It preserves the structural split while sidestepping the only
   part of the proposal that introduces a new exception.
3. **Alt C (rename only)** — good fallback if 0.6.0 runs short on
   time. Fixes the worst concrete symptom (`=` / `!=` overlap)
   without taking on the JS refactor.
4. **Alt D (`#lang` pragma)** — elegant, but the two-extension
   situation is already in the wild; adding a pragma on top feels
   like belt-and-suspenders.

## Recommended organisation

Assuming Alt A+B, sequenced for 0.6.0 after DD-30 lands:

**Phase 0 — Preconditions (0.5.x)**
- DD-30 is done or explicitly deferred.
- A "DD-20 for JS" draft exists describing the target JS pipeline
  post-split.
- Audit pass: list every bare kernel form currently used in
  `.lykn` files in the stdlib, examples, and tests.

**Phase 1 — Rust classifier becomes strict (Rust PR 1)**
- Add a "strict" mode flag to `classifier::classify()`.
- Add the auto-promoted list; unknown kernel forms in surface code
  error out in strict mode.
- Snapshot tests (DD-35) protect emitter output.

**Phase 2 — `kernel:` (or `#k(...)`) escape hatch (Rust PR 2)**
- Implement the prefix/dispatch in the classifier.
- Surface regression tests.
- Update the 0.5.x release notes with a migration preview.

**Phase 3 — JS compiler refactor (JS PRs 1–3)**
- Extract a pure kernel compiler (no surface awareness).
- Rewrite surface expander to emit kernel SExpr explicitly, not via
  `_kernel` markers.
- Retire `kernelArray()`. Retire the marker.
- Run the full test suite at each step.

**Phase 4 — Extension enforcement (Rust + JS)**
- `.lyk` files go through the kernel path; `.lykn` files go through
  the surface path.
- CLI (`fmt`, `check`, `compile`) dispatches on extension.
- Hard errors, not warnings.

**Phase 5 — Retire `SetSymbol` and other scars**
- Remove the `SetSymbol` variant; confirm `set!` covers every case.
- Remove `=`, `!=`, `macro`, `import-macros` from `is_kernel_form()`.
- Remove any remaining surface-aware code from the kernel path.

**Phase 6 — Documentation & release**
- New guide: kernel escape hatch.
- Updated guides: review examples for mixed-dialect code.
- Migration notes in 0.6.0 release notes.
- Update DD-15 to reference this DD as the implementation.

This is roughly 10–14 PRs across two months of focused work,
assuming the JS refactor is the long pole.

## Open questions

1. **Macros that emit kernel code.** Should user-defined macros be
   required to emit `kernel:` prefixes explicitly, or is macro
   output implicitly kernel? The latter is ergonomic; the former is
   explicit. I lean explicit for published macros and implicit for
   file-local ones, but this needs a deliberate decision.
2. **The test DSL problem.** Test files like
   `test/forms/function_test.lykn` exercise kernel features using
   surface scaffolding. Under strict gating these remain `.lykn` —
   but someone reading the tree would reasonably expect kernel
   tests to live in `.lyk` files. Is the DSL allowed to "contain"
   kernel content in a way the rule doesn't normally permit?
3. **REPL behaviour.** Does the REPL default to surface, kernel, or
   require an explicit choice? If we adopt an `#lang`-like pragma
   (Alt D) ever, the REPL is the cleanest place to introduce it.
4. **The auto-promoted set.** What exactly is in it? My provisional
   answer: arithmetic (`+ - * / % **`), comparison (`< > <= >=
   === !== == !=`), bitwise (`& | ^ << >> >>> ~`), literal
   constructors (`array`, `object`, `get`, `quote`, `quasiquote`),
   and possibly the update operators. Anything that has a surface
   counterpart stays off the list.
5. **Cross-compiler parity.** The JS compiler needs its own DD
   (sibling to DD-20). Who writes it, and when?

## Citations

Source files referenced in this DD, with approximate locations:

- `packages/lang/expander.js` — `_kernel` marker at line 731 and the
  fixed-point expansion loop that uses it.
- `packages/lang/surface.js` — `kernelArray()` helper at line 24;
  usages at lines 1026, 1133, 1148, 1167, 1181.
- `crates/lykn-lang/src/classifier/dispatch.rs` — `is_surface_form()`
  (lines 1–36) and `is_kernel_form()` (lines 38–134); `=` and `!=`
  appear in both.
- `crates/lykn-lang/src/ast/surface.rs` — `SetSymbol` TODO at line
  294; `KernelPassthrough` at line 430.
- `crates/lykn-cli/src/util.rs` — `is_lykn_ext()` at lines 7–9.
- `crates/lykn-cli/src/main.rs` — `is_lykn_test_file()` around line
  433.
- `docs/design/06-final/0020-dd-15-language-architecture…md` —
  canonical statement of the kernel/surface split.
- `docs/design/06-final/0001-dd-01-colon-syntax-and-camelcase-conversion.md`
  — the DD-01 decision that `kernel:` must be reconciled with.
- `docs/design/05-active/0030-pure-rust-kerneljs-codegen.md` —
  DD-30; must land before or explicitly not during this work.
- `workbench/release-notes-0.5.0.md` — breaking change for class-body
  `=` and confirmation that `.lyk`/`.lykn` are both supported today.

## Verification notes

Claims in this DD are drawn from direct reads of the source files
above. Two categories I flag as unverified:

- **Comprehensive audit of `packages/lang/surface.js`.** I read ~200
  lines and grepped for the `_kernel` / `kernelArray` usages. A full
  audit of every surface macro is a prerequisite for Phase 3, not
  this DD.
- **Guide review.** I did not read the content of `docs/guides/*`
  to count mixed-dialect examples. The "docs need a review pass"
  claim is an expectation, not a measurement.

Everything else is cited to line ranges I read.
