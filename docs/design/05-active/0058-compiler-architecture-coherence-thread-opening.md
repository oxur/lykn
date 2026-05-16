---
number: 58
title: "Compiler Architecture Coherence — Thread Opening"
author: "** CDC (this session)"
component: All
tags: [change-me]
created: 2026-05-15
updated: 2026-05-15
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# Compiler Architecture Coherence — Thread Opening

**Date:** 2026-05-15 (initial draft 2026-05-15; substantial 2026-05-15 update
incorporating cdc/dep-ergonomics handoff)
**Author:** CDC (this session)
**Branch:** cdc/compiler-coherence (worktree at .worktrees/compiler-coherence;
originally forked off release/0.6.x @ f9b647a 2026-05-11; **rebased
2026-05-15** onto release/0.6.x @ 4ee1eaa, picking up 31 commits including
DD-50.6, DD-50.7, M10, M11, M13, DD-52, DD-53, DD-54, DD-55 ICU/i18n work,
and the DD-37 → 05-active/ promotion)
**Status:** Duncan's calls received 2026-05-15 — see "Resolutions" section
below. Track A next deliverables: separation DD draft (DD-58, `kernel:*`
syntax, 0.6.0 target) and DD-37 readiness review (joint with Duncan).
Track B next deliverable: one milestone spec covering all five rows.

---

## Purpose of this document

The kickoff prompt named five specific open items in the compiler-architecture
space (DD-36, DD-37, V-06, broader compileBoth conversion, JS/Rust error-format
divergence). Before scoping any of them into a milestone, the responsible move
is a single coordinating pass that (a) re-grounds DD-36/37 against what
actually landed in M7, (b) names the design questions only Duncan can answer,
and (c) proposes a shape for the work that respects the rest of Phase 2's
in-flight items (DD-50.6 [landed 2026-05-15], M9-release, M10-15 [M10,
M11, M13 landed; M12 in flight as cdc/dep-ergonomics linter work; M14,
M15 still open], M4 hold).

This is that pass. It is read-only investigation plus my honest assessment.
No file edits beyond this document and the .gitignore tweak that came with
worktree setup.

---

## Correction to the kickoff prompt

The kickoff said DD-36 and DD-37 were "drafted but never made it into
`docs/design/01-draft/` or beyond." That is incorrect. They are in
`01-draft/` as `0046-dd-36-kernel-surface-compiler-split.md` and
`0047-dd-37-js-surface-compiler-architecture.md`. The diff against the
workbench versions is essentially: frontmatter added; "Recommendation"
preamble removed; "This is a workbench draft" preamble removed. So they
were lightly polished and promoted from workbench → draft. They never
reached `05-active/`.

The substantive question is not "promote them out of workbench" — that's
already done — but "promote them from `01-draft/` to `05-active/`,
decommission, or rewrite." That changes the framing of the decision but not
the analysis below.

---

## Resolutions (2026-05-15)

Duncan's calls on the six top-level questions, plus the nine DD-37 open
questions and one substantive correction (the "auto-promote" reframe).
The numbering for the separation DD is now confirmed: **DD-58** (the
next available number; DD-55/56/57 were unavailable, and DD-56 in
workbench is the form catalog).

### Six top-level questions

1. **Kernel-call syntax: `(kernel:<form> ...)` namespace prefix.** Strong
   preference; aligns with my weak preference. The DD-01 collision is
   absorbed as a documented exception ("`kernel:` is a reserved
   head-position prefix").
2. **Timing: 0.6.0.** Aggressive. Competes with M14-15 for slots in the
   current release cycle (M10, M11, M13 already landed; M12 linter in
   flight on cdc/dep-ergonomics). Means the cdc/dep-ergonomics CDC's resumption
   is pulled forward; the 8 remaining doctest failures and DD-56 form
   catalog all land *before* 0.7.0.
3. **DD-37 promotion: joint review.** Not a unilateral CDC call —
   Duncan wants to walk through DD-37's current state, the three
   amendments I named, and bundle-size posture together before
   committing to `05-active/`.
4. **V-06: needs reminding.** V-06 was M6's finding that the JS
   compiler has zero analysis passes while Rust has ~4,000 lines
   (`analysis/` module covering exhaustiveness, scope, type registry,
   match check, etc.). The two reads I named: (A) build JS-side
   parity, (B) document the divergence as intentional and route users
   to `lykn check` for validation. (B) is the right answer, and DD-37
   already pre-commits to it ("No static analysis module" subsection).
   So V-06 is a documentation deliverable, not a new design decision.
5. **Track B: one milestone, not split.** Five rows in one
   M-something (likely M16). I draft the spec.
6. **Cross-thread coordination: Duncan handles directly.** I don't
   need to write a reply handoff for the cdc/dep-ergonomics CDC.

### Nine DD-37 open questions (walked in the readiness review doc)

Duncan reviewed all nine. Dispositions:

1. **`@lykn/browser` placement.** *Agreed* — import the full
   six-module toolchain; document filesystem-dependent features as
   browser-unsupported.
2. **Bundle-size measurements.** *Agreed* — Phase 0 acceptance
   criterion (per Amendment 1 in the readiness review).
3. **Source maps.** *Agreed* — deferred to future DD.
4. **REPL support.** *Agreed* — deferred to future DD.
5. **Linter / formatter.** *Agreed*, with the qualifier that the
   linter work isn't deferred for long — happening shortly after
   this thread's work lands. (cdc/dep-ergonomics M11/M12 already
   in flight in a separate worktree.)
6. **Incremental compilation.** *Agreed* — deferred.
7. **Parity testing with the Rust compiler.** *Agreed* —
   `compileBoth` covers end-to-end output; kernel-JSON parity is
   future work.
8. **Surface form error-message style.** *Partial answer for now;
   revisit when closer to needing definitive resolution.* M7's
   DD-49 settled identifier-error format; Finding #4 is a Track B
   row; broader style guide stays open with the explicit plan to
   gather more data before deciding.
9. **Keyword handling.** *Agreed* — surface position = identifier;
   kernel position = string. Worth pinning down in the emitter spec.

### DD-37 disposition

**Option A confirmed.** Promote DD-37 to `05-active/` as soon as the
next update lands (the three amendments + this resolution record).
Phase 0 acceptance criterion governs the promotion-to-implementation
gate, not the promotion-to-active gate.

### The "auto-promote" correction — substantive

In my read of DD-36's original direction, I'd carried forward the
"auto-promoted set of kernel forms recognized in surface code without
prefix" concept. Duncan rejected that framing and corrected it to a
cleaner model. The corrected model:

**Three categories of forms:**

1. **Surface-only forms.** No kernel namesake used in surface code.
   Examples: `bind`, `func`, `match`, `obj`, `cell`, `if-let`,
   `when-let`, threading macros, `set!`, `swap!`, `reset!`.
2. **Kernel forms that surface doesn't use.** Exist in the kernel
   vocabulary but have no surface name. Reachable only via
   `(kernel:foo ...)`. Examples: probably `quote`, `quasiquote`,
   maybe `class`, raw low-level JS-statement constructs not exposed
   in surface idiomatic style.
3. **Kernel forms that surface DOES use.** These get a *thin wrapper*
   in surface — a surface form with the same name that passes through
   to the kernel form. Examples: arithmetic (`+`, `-`, `*`, `/`),
   comparison (`<`, `>`, `<=`, `>=`, `===`, `!==`), literal
   constructors (`array`, `object`, `get`, `template`, `regex`).

**The architectural rule:** every form a user writes in surface code
is unambiguously a surface form. Some surface forms are rich
(elaborate semantics like `bind` or position-aware `if`); some are
thin wrappers (literal passthrough to kernel). All are classified the
same way by the classifier. There is no "fall through to kernel" for
unrecognized atoms in surface code — those are either user macros or
errors.

**Three implementation flavors** of surface forms (CDC's framing,
Duncan confirmed):

- **Rich, surface-unique:** `bind`, `func`, `match`, etc. No kernel
  namesake.
- **Thin wrapper:** `+`, `array`, `template`, etc. Surface name
  matches kernel name; emitter passes through.
- **Rich, namesake-sharing:** surface `if` (position-aware
  ternary/IIFE per DD-50), surface `try` (value-producing per D-2
  from the handoff). Surface name matches kernel name but the
  emitter does elaborated work.

**Three benefits (Duncan's enumeration, my framing):**

- **No ambiguity.** The classifier never has to fall through or
  guess. Every form in surface code is unambiguously classified.
- **No surprise from future changes.** The surface namespace is
  closed; adding a new surface form can't steal kernel semantics
  from previously-auto-promoted code because there's no
  auto-promotion.
- **Clean separation of concerns.** The kernel is a JS abstraction,
  not a sometimes-user-facing namespace. Surface is what users
  write; kernel is what surface compiles to.

This is *better* than DD-36's original "auto-promoted set" framing.
DD-58 will use this three-flavor model.

### Effect on the form catalog (DD-56)

Under the corrected model, DD-56's schema gets cleaner:

- **Surface entries** enumerate the full user-facing namespace
  (every form a user can write). Per-locale translation hooks live
  here.
- **Kernel entries** enumerate the JS-language vocabulary the kernel
  compiles to. No translation hooks (kernel stays
  language-neutral / English-canonical).
- **Thin-wrapper surface entries** carry an `expands_to` field
  pointing at the same-named kernel form.
- **Surface forms with kernel namesakes but richer semantics** carry
  an `expansion` field describing the elaborated behavior (e.g.,
  `"position-aware"` for surface `if`/`try`).

The DD-56 author can pick up the corrected model as input.

### Effect on Track B's five rows

No change. Track B is mechanical; the correction affects DD-58 (Track
A) but not the Track B work items.

### Net: every blocker for the immediate next deliverables is resolved

See "Immediate next deliverables" subsection below (within Proposal).

---

## Update: 2026-05-14 handoff from cdc/dep-ergonomics

The other CDC, working on `cdc/dep-ergonomics`, produced a handoff document
(`workbench/handoff-surface-kernel-separation-2026-05-14.md`) that
materially reframes this thread. Summary of what changed:

### The empirical forcing function

That CDC was triaging 14 post-rebase doctest failures in `docs/guides/*.md`.
Classification (Phase 1a):

- **Class A1 — factory pattern (6 failures).** Functions returning surface
  `(fn ...)` as the last body expression. The JS compiler rejects them
  because DD-50.6's Q2=A check sees `fn` as "statement-only," even though
  `fn` always expands to a kernel `(=> ...)` arrow expression that IS
  value-producing.
- **Class A2 — try-as-expression (2 failures).** Functions whose body is
  `(try ... (catch ...))` and declared `:returns :T`. Both compilers
  reject; the docs/book teach the pattern as valid.
- **Class B — intentional-error blocks in i18n doc (6 failures).** Doc
  framework treating "should fail" examples as "should compile." Closed
  via doctest annotation fix (W-4d); 6 of 14 cleared.

Classes A1 and A2 are *architectural*, not doc fixes.

The Phase 1c audit (`workbench/phase-1c-classification-audit-2026-05-14.md`)
established three further findings that converge with my survey here:

1. **Both compilers conflate two different questions onto one list.**
   `STATEMENT_FORM_HEADS` (Rust, `emitter/forms.rs:2567`) /
   `STATEMENT_ONLY_HEADS` (JS, `surface.js:902`) is used to answer
   (Q-emit) "does this form emit as a JS statement?" AND (Q-value) "can
   this form be the last expression of a `:returns :T` body?" These
   overlap but aren't the same set. The shipped code has two
   override-patches bolted onto the shared Rust list to fix the
   mismatch — symptoms of "one list, two consumers."
2. **The two compilers' lists have drifted out of sync.** Rust includes
   `if`/`throw`/`return`/`break`/`continue`; JS doesn't. JS includes
   `fn`; Rust doesn't. DD-50.6 Q4=A promised "two lists kept in sync via
   compile-both tests" — the tests existed but didn't exercise the
   surface-vs-kernel boundary, so the drift was invisible.
3. **The JS compiler implements DD-50.6 Q3=C incorrectly.** Q3=C was
   decided as "compile-then-check the post-expansion form's head." Rust
   runs the check on `emit_body(...)` output (post-expansion); JS runs
   it on the surface form (pre-expansion). Rust correctly accepts
   `(fn ...)` as last-expression (because after expansion it's `=>`,
   which isn't statement-only); JS incorrectly rejects.

This is exactly the "structural divergence I was pointing at abstractly"
manifesting as concrete user-visible bugs. The other CDC's audit
sharpens my survey's "M7 didn't touch any of the divergences DD-36/37
named" claim into "the divergences are not latent — they're shipping
broken docs."

### The strategic forcing function

Duncan revealed during Phase 3 synthesis that 0.7.0 will introduce
native-language readers — users writing lykn in Russian, Japanese, etc.,
with surface forms translated. Example from the handoff:

```lykn
;; lang: ru
(привязка имя "Дункан")
(функция приветствие
  :аргументы (:строка имя)
  :возвращает :строка
  :тело (шаблон "Привет, {имя}!" :имя имя))
```

(`привязка = bind`, `функция = func`, `:аргументы = :args`, etc.)

This requires a canonical, machine-readable enumeration of every form,
every keyword clause, every type keyword, with per-locale translation
columns. That's DD-56, drafted at
`workbench/dd-56-canonical-form-spec-2026-05-14.md`.

DD-56's schema sketch has `kind = "surface"` and `kind = "kernel"`
entries with the *same form name* (`try` appears twice). That
representation only makes sense under the overlapping model. Under
separation, the kernel entries have different syntactic identifiers and
the catalog's structure is different.

This means the 0.7.0 i18n catalog cannot be cleanly designed until the
separation question is decided. The 0.6.x parity-discipline problem and
the 0.7.0 i18n unlock converge on the *same artifact* — and that
artifact's shape depends on the separation outcome.

### What this changes about Track A

In my original survey, Track A (DD-36 + DD-37 + V-06) was framed as
"decide whether to commit to the architectural restructuring." The
handoff reframes that question as already answered. Two independent CDC
threads have now surfaced converging cases for the separation:

- **My thread:** structural debt (overlapping dispatch tables,
  `_kernel` marker, `kernelArray`, no JS classifier) that DD-36/37
  named in April. Hadn't been weakened by M7.
- **cdc/dep-ergonomics thread:** 14 failing doctests, cross-compiler
  list drift, JS Q3=C bug, 0.7.0 i18n catalog dependency.

Same architectural fact, two empirical perspectives. The substance is
not in dispute.

The remaining call is *which kernel-call syntax wins*. The handoff
enumerates four plausible shapes:

1. **Namespace prefix on kernel calls:** `(kernel:if cond then else)`,
   consistent with the existing `js:` namespace pattern. (DD-36's
   original Alt A.)
2. **Special enclosing form:** `(kernel (if cond then else))` — explicit
   "drop to kernel." Bigger syntactic weight; harder to nest.
3. **Reader macro:** `#kernel(if cond then else)` or `#k(if ...)`.
   Reader-level distinction. Compact. (DD-36's Alt B.)
4. **Inverse marking:** kernel forms stay bare; surface forms get a
   marker (`(surface:if ...)`). Probably wrong — surface is the
   user-facing language and should stay unmarked.

The handoff also raises the i18n implication: kernel forms presumably
stay language-neutral (English-canonical) because the kernel is "the
thin skin over JS" and JS itself isn't translatable. Surface forms get
translation tables; kernel forms get one canonical spelling. Worth
confirming.

### What this changes about Track B

Three of the cdc/dep-ergonomics CDC's planned workstreams (W-1 JS Q3=C
fix, W-2 surface `try` as value-producing, W-3 form catalog) are now
*paused* because they bake in current-overlap assumptions:

- W-1's Q3=C "compile-then-check post-expansion form" is *literally a
  workaround for surface/kernel sharing syntactic heads*. Under
  separation, "just check the syntactic head" is sufficient; the Q3=C
  machinery becomes unnecessary in its current shape.
- W-2's "surface `try` becomes value-producing while kernel `try` stays
  statement-only" is *literally the separation applied to one form*.
  Under separation, surface `try` and kernel `try` are syntactically
  distinct, so there's no context-disambiguation logic needed.
- W-3 (DD-56) schema has `kind = "surface"` and `kind = "kernel"`
  entries that share names. Schema is different under separation.

So Track B's compileBoth-conversion + error-format-alignment items
still stand as-is. But the three items I imported from the handoff
("Fast-follow items handed off from other threads" section below) need
re-evaluation — DD-52 #2 (`__surface_macro__` sentinel) is now clearly
*deleted* rather than cleaned up (because surface forms leave the macro
env entirely under DD-37); the other items are still valid Track B
candidates.

### Cross-thread coordination — important

The cdc/dep-ergonomics thread is *waiting* on this thread to land the
separation DD before resuming. The handoff's "Recommended sequence
after separation lands" section enumerates seven steps; steps 3–7 all
depend on the separation outcome. That means:

- This thread is now load-bearing for cdc/dep-ergonomics.
- The separation DD's design-call resolution and CC implementation
  affect when DD-56 can be written, when W-1/W-2 can ship, and when
  the 14 failing doctests can be properly closed (W-4d closed only 6
  of 14; the remaining 8 are A1+A2 which need the separation).
- 0.7.0 i18n is downstream of all of this.

Whatever timing Duncan picks for this thread's implementation should be
communicated back to cdc/dep-ergonomics so that CDC knows when to
resume.

---

## What DD-36 and DD-37 propose (one-paragraph each)

**DD-36 (Kernel/Surface Compiler Split).** Hard-gate compilation by file
extension: `.lyk` files contain only kernel forms (the JS-dialect IR);
`.lykn` files contain only surface forms (`bind`, `func`, `match`, etc.).
Surface code escapes into the kernel via `(kernel:<form-name> ...)` (or
the alternative `#k(...)` reader-tag). A blessed "auto-promoted" set of
operators and literal constructors works in both without prefix. Retires
the `=` / `!=` overlap, the `SetSymbol` variant, and the `_kernel` marker.
Targets 0.6.0. Requires DD-37 as Phase 0 prerequisite. Has one explicit
reservation: `kernel:` collides with DD-01 colon syntax (member access),
which is workable but introduces a new exception.

**DD-37 (JS Surface Compiler Architecture).** Six-module decomposition of
the JS compiler: `reader → classifier → (macro expander | static surface
transforms) → kernel emitter → kernel compiler → astring`, with a typed
surface AST as the lingua franca. Built-in surface forms become static
transforms, not macros. User macros keep DD-13's three-pass
`new Function()` pipeline. Deletes the `_kernel` marker and `kernelArray()`
helper. Explicitly preserves the no-static-analysis stance for the JS
compiler: validation lives in Rust. Has a serious bundle-size caveat:
estimated +8–20KB gzipped (squarely in the "investigate" band per
project guidance), with a pre-agreed reduced-scope fallback ("Alt C —
classifier only, keep `_kernel` marker") if measurements push past
budget.

---

## What M7 actually did (and did not) change about this picture

M7 closed three DDs (DD-49 identifier mapping, DD-50 position-aware
compilation including DD-50.5 and addendum, DD-51 deno-tool boundaries),
plus surfaced DD-50.6 (implicit-return-of-statement) as a fast-follow
bug fix. **As of 2026-05-15: DD-50.6 has landed; DD-50.7 also landed
(`DD-50.7: fix DD-50 emission for real downstream patterns +
emit_if_iife`, commit `2769eb4`); and a tactical patch removing `fn`
from the JS-side statement-only list shipped in commit `e50edc9` —
addressing part of the Class A1 doctest failure class the handoff
identified.** The architectural fix (DD-58 separation + DD-37
classifier infrastructure) remains the durable resolution; the
tactical patches close the worst user-visible bugs while DD-58 is
drafted.

**What this changes about the structural picture: very little.** The
relevant M7 outcomes for DD-36/37 are:

1. **DD-49 made identifier mapping byte-identical across compilers.**
   Both compilers now apply the same composite algorithm
   (predicate-prefix, mechanical escape, collision detection,
   bridged error messages). This is parity at the *output* level for
   one specific concern; it doesn't restructure either pipeline.
2. **DD-50.5 added per-form context profiles in the Rust emitter
   (`KernelChildProfile` enum).** This is a refinement *inside* the
   Rust emitter — it doesn't restructure the boundary with the JS
   side; it formalises which child positions of which kernel forms
   want Statement-context vs. Value-context vs. mixed compilation.
3. **DD-50.5 addendum added `compileBoth()`** — a test helper that
   runs both compilers on the same source and asserts byte-equivalent
   normalised output. Currently used in 4 tests in `dd-50_test.lykn`
   plus 7 in `dd-50.6_test.lykn` (the WIP). Has already surfaced
   two real bugs (`KernelPassthrough` skip-emitter-descent and the
   PATH-vs-local-build issue) and one phantom (the `=` vs `===`
   misdiagnosis).

**What M7 did not touch:** every structural divergence DD-36/37 named is
still 100% present in the source as of `f9b647a`:

| Concern | DD-36/37 status | Confirmed in source today |
|---------|-----------------|---------------------------|
| `_kernel` marker in `expander.js` | proposed for deletion (DD-37) | Present at lines 731, 736, and `surface.js:27` |
| `kernelArray()` helper in `surface.js` | proposed for deletion (DD-37) | Present at line 25, 6 callers (lines 1054, 1120, 1161, 1176, 1195, 1209) |
| `=` and `!=` in both `is_kernel_form()` and `is_surface_form()` | proposed for cleanup (DD-36) | Both still in both lists in `dispatch.rs` |
| JS compiler has no classifier | proposed module (DD-37) | Surface forms still register into `macroEnv` via `registerSurfaceMacros()` at `surface.js:899` |
| JS compiler has no separate emitter / kernel-compiler split | proposed (DD-37) | `compiler.js` (1,790 lines) handles everything; `surface.js` (2,315 lines) is one flat macro file |
| JS compiler has no static-analysis module | DD-37 explicitly: keep absent; defer to Rust | Confirmed: zero analysis passes on JS side |
| Rust compiler has no JS-equivalent unused-binding analyzer absence (V-06) | M6 surfaced | Rust `analysis/` is ~4,000 lines; JS is zero |
| `SetSymbol` variant in Rust AST | proposed for retirement (DD-36) | Still present in `crates/lykn-lang/src/ast/surface.rs` |
| `bridge.rs` (Deno-shellout for kernel→JS) | DD-30 was supposed to retire | Present but appears unused (`grep` found no `mod bridge` declaration) |

The DD-30 status is interesting. DD-30 ("pure-Rust kernel→JS codegen") is
in `05-active/` and the Rust `codegen/` module exists with substantial
implementation (3,436 lines across emit.rs, names.rs, format.rs,
precedence.rs). But `bridge.rs` (the Deno-shellout path) is still in the
source tree and not declared as a module. So DD-30's *implementation* is
likely complete (or nearly so) but its *closure* may not be — worth a
separate verification before treating it as done.

**Bottom line:** DD-36/37 are not superseded by M7. M7 was tactical
(bug fixes + one new helper); DD-36/37 are architectural. The decision
about them is just as live today as it was on 2026-04-18 when they
were drafted.

---

## Open item triage

### Item 1 — DD-36 (kernel/surface compiler split)

**Original framing (pre-handoff):** promote, decommission, or rewrite?

**Revised framing (post-handoff):** the substance is decided by the
empirical + strategic case the handoff makes; the remaining call is
*which kernel-call syntax wins* among the four candidates the handoff
enumerated, and *how to sequence implementation* relative to
cdc/dep-ergonomics and 0.7.0.

**My honest assessment of the four syntax candidates:**

1. **`(kernel:<form> ...)` namespace prefix.** Reuses the existing
   `js:` interop pattern, so users already understand "colon-namespace
   means escape from surface semantics." DD-01 collision worry from
   the original DD-36 is real but workable — the classifier runs
   before member-access compilation and can claim `kernel:` as a
   reserved head-position prefix. Cost: a new exception to DD-01.
2. **`(kernel (form ...))` enclosing form.** Cleanest semantically:
   the kernel is literally a sub-language and "drop into the kernel"
   is a single explicit operation. But: every kernel call inside
   surface code now nests twice (`(kernel (if c t e))`), which hurts
   for the case of "a single kernel form embedded in surface" —
   exactly the case the escape hatch is for. Probably wrong if kernel
   escape is a common need; right if it's genuinely rare.
3. **`#kernel(...)` or `#k(...)` reader macro.** Reader-level. Compact.
   No DD-01 conflict (reader dispatches on `#` before colons are
   considered). Familiar to Lispers. Cost: `#k` reads as "hex" or
   "macro" to newcomers; `#kernel(...)` is self-documenting but more
   ceremonious.
4. **Inverse marking — kernel bare, surface gets the marker.** Almost
   certainly wrong. Surface is the user-facing language; surface
   should be unmarked.

**My lean:** #1 or #3. Between them, the call is "is the DD-01
exception (small) worse than the `#k` learning curve (also small)?" My
weak preference is #1 (`kernel:` prefix) because:

- The exception is bounded and documentable ("when a head atom starts
  with `kernel:`, it's a kernel escape, not member access").
- The `js:` precedent already exists; users know one colon-namespace
  pattern, so a second one extends an existing mental model rather
  than introducing a new piece of reader syntax.
- The i18n consideration is favorable: `kernel:` is unambiguous in
  any translation regime because the prefix is a piece of compiler
  vocabulary that stays English-canonical (per the handoff's note
  that kernel forms presumably aren't translated). A reader macro
  `#k(...)` would need an i18n story for what the `k` letter means
  in non-Latin scripts.

But I'd want to hear Duncan's preference before committing. The
handoff was neutral on this; the other CDC explicitly left the syntax
choice to this thread.

**The "after DD-30 is done" precondition.** DD-30 is in `05-active/`,
the `codegen/` module exists and is substantial (3,436 lines), but
`bridge.rs` (the Deno-shellout path) is still in the source tree
without a `mod bridge` declaration anywhere. So DD-30 is *implementation
complete* and *dormant code remains*. Treating DD-30 as effectively
closed for the purpose of DD-36 sequencing is reasonable; a separate
cleanup PR can delete `bridge.rs` once we're sure nothing depends on
it. Worth verifying with a focused audit, but not a blocker.

### Item 2 — DD-37 (JS surface compiler architecture)

**Original framing (pre-handoff):** promote, decommission, or rewrite,
with bundle-size measurement caveat?

**Revised framing (post-handoff):** the 14 failing doctests *are*
DD-37's architectural debt manifesting as user-visible bugs. The JS
compiler's lack of a classifier and its treatment of surface forms as
macros is *precisely why* the JS implementation of DD-50.6 Q3=C took the
shortcut of checking the pre-expansion form's head — there's no
typed-AST distinction between "this is a surface form that will expand"
and "this is a kernel form as-is." DD-37's "built-in surface forms
become static transforms, not macros" decision *is the fix* for that
class of bug.

**My honest assessment:** the bundle-size caveat in DD-37 is still
real, and the "Alt C — classifier only" fallback the DD pre-agreed is
still the right escape valve if measurements push past budget. But the
urgency calculus has changed:

- Pre-handoff: DD-37 was architectural hygiene with no near-term
  forcing function. Deferral was a real option.
- Post-handoff: DD-37 is the JS-side prerequisite for the kernel/surface
  separation, which is itself the prerequisite for DD-56 (the form
  catalog), which is itself the prerequisite for 0.7.0 i18n. Deferral
  cascades into the 0.7.0 release.

The Phase 0 measurement task (baseline `lykn-browser.js` size + per-PR
size budget in CI) is still the right first step regardless of timing.
That work can start *immediately*, in parallel with the DD-36 syntax
choice, because the bundle-size question doesn't depend on which
syntax wins.

**My lean:** promote to `05-active/` *now*, with three concrete
amendments:

1. **Add a "Phase 0 acceptance criterion" subsection** as I originally
   suggested: the DD is in active state once the baseline measurement
   and CI guard land. The per-form migration is milestone-scoped.
2. **Add a "Relationship to DD-36" subsection** that pins the
   dependency: DD-37 ships first (typed surface AST in the JS
   compiler), then DD-36's syntactic separation can be enforced on
   top of it. The handoff confirms this ordering — the JS classifier
   has to exist before the JS compiler can reject "kernel form
   appearing where surface is expected" (and vice versa).
3. **Refer forward to the separation DD** (whichever number wins; I'd
   suggest DD-58 since DD-56 is the form catalog) so the relationship
   between DD-36's original direction and the as-implemented
   separation is documented.

### Item 3 — V-06 (JS-side unused-binding analyzer)

**The two reads from the kickoff:**
- (A) Add a JS-side analyzer for parity (build new infrastructure).
- (B) Document the divergence explicitly: "Rust is the validation-pass
  compiler; JS is the fast-emit compiler. Users running validation
  should use `lykn check` (Rust)."

**My honest assessment:** (B) is the right answer for now, and DD-37
has *already pre-committed to it*. From DD-37's "No static analysis
module" section: "The JS compiler does **not** ship the analysis module
that DD-20 specifies for Rust. No exhaustiveness checking, no overlap
detection, no unused-binding warnings, no type registry."

So V-06 isn't really a new design call — it's a documentation gap. The
divergence is already a deliberate architectural decision; we just
haven't surfaced it in user-facing terms. The work item is:

1. Document the divergence in the JS-vs-Rust comparison page (or
   wherever users discover that `lykn check` only catches certain
   classes of issue).
2. Cite DD-37 as the rationale.
3. Possibly add a soft hint in the JS compiler when it's invoked
   directly (e.g., a one-line note: "for static analysis, use `lykn
   check` against the Rust compiler"). This last is optional and arguable.

This is small and shouldn't need its own DD; it's a documentation
deliverable for a milestone. The (A) path becomes a real design call
only if/when someone has a concrete user case for "JS-side warnings in
the browser REPL" — at which point DD-37's typed-AST decision means it's
buildable without re-architecting.

### Item 4 — Broader compileBoth conversion

**Question for Duncan:** which tests should convert, how to script the
conversion, what level of normalisation is acceptable in compileBoth's
divergence detection?

**My honest assessment:** this is a methodology / infrastructure task,
not a design decision. The current `compileBoth()` implementation
(`packages/testing/helpers.js:60`) is solid: writes to temp file,
shells out via `LYKN_BIN` (defaulting to `./bin/lykn`), strips Rust
warnings, normalises whitespace + trailing-semicolon variations, and
asserts equality. It's been used in 11 tests so far (4 in `dd-50_test`,
7 in `dd-50.6_test`) and has caught real bugs.

The question of "which tests should convert" has a defensible default:
**any test whose substantive claim is about emitted JS structure or
content rather than runtime behaviour.** Tests that exercise runtime
semantics (e.g., "does this `match` actually pattern-match correctly?")
don't benefit from compileBoth — both compilers emit JS, both run on
Deno, both produce the same runtime answer because the JS engine is the
same. But tests that assert "this compiles to `if (x === 1) ...`" benefit
enormously from cross-compiler verification.

A pragmatic conversion strategy:

1. Audit `test/forms/*_test.lykn` and `test/surface/*_test.lykn` for
   tests whose body uses `compile(...)` (i.e., asserts on emitted JS,
   not runtime behaviour). Those are the candidates.
2. Convert in batches by file. Each batch is one PR.
3. Track divergences as findings: triage as bugs (fix in the
   appropriate compiler) vs. cosmetic normalisation issues (extend
   `compileBoth`'s normaliser, with explicit note in the helper's
   docstring about what's being normalised and why).
4. Cap the normaliser additions: anything beyond whitespace + trailing
   semicolons + warning-stripping needs an explicit "is this
   normalisation hiding a real divergence we should fix instead?" check
   before merging.

Scope: probably 3–6 PRs over 2–3 weeks of focused work, with most of
the cost being divergence triage rather than mechanical conversion.

### Item 5 — JS/Rust error-message format divergence (Finding #4)

**The divergence, precisely:**

- JS path (`packages/lang/surface.js:96-118`): the same `buildTypeCheck`
  function handles both parameter checks and return-type checks. When
  called with `paramNode = resultVar` (the gensym holding the return
  value), the gensym name leaks into the message:
  `"funcName: return 'result__gensym0' expected boolean, got <type>"`.

- Rust path (`crates/lykn-lang/src/emitter/type_checks.rs:124-149`): a
  dedicated `emit_return_type_check` function is used for return checks.
  It uses the literal label `"return value"`:
  `"funcName: return value expected boolean, got <type>"`. The function's
  doc comment explicitly calls out the design intent — "the error
  message says `\"return value\"` instead of `\"result__gensym0\"`."

**My honest assessment:** the Rust version is right. The gensym name is
an internal implementation detail; surfacing it in error messages is
incidental, not informative. The fix on the JS side is small: add a
sibling function (or branch in `buildTypeCheck` keyed on the `label`
arg) that uses the literal `"return value"` instead of the param name
when generating return-type checks. The two callers of `buildTypeCheck`
with `label = "return"` (`surface.js:1802` and `1852`) are the only
sites that need updating; the param-check path stays as-is.

This doesn't need its own DD. It's a one-PR fix, regression-covered by
adding a Rule-7-style assertion to the existing test infrastructure.
It's a candidate to fold into Item 4's "broader compileBoth conversion"
work if the converted tests would catch it (they would — return-type
checks are part of emitted JS).

---

## Proposal: how to scope this thread

The five original items + the handoff's findings reshape into three
tracks rather than two:

**Track A: Architecture (DDs 36 + 37 + V-06 + a new separation DD).**
The substance is decided. The work breaks into:

1. **Syntax choice DD** — the actual separation DD that supersedes
   (or radically refines) DD-36. Probably numbered DD-58 to leave
   DD-56 for the form catalog. Picks one of the four syntax candidates
   (`kernel:` prefix / enclosing form / `#k(...)` reader macro /
   inverse marking), with the loser-alternatives documented as
   rejected.
2. **JS classifier infrastructure (DD-37 Phase 0).** Bundle-size
   baseline + per-PR CI guard. Doesn't depend on syntax choice;
   can start immediately.
3. **JS classifier implementation (DD-37 Phase 1+).** Once Phase 0
   sets the budget, per-form migration of surface macros to typed AST
   nodes + static transforms. Milestone-scoped.
4. **Rust classifier strictness (DD-36's Phase 1 in the original
   sequencing).** Once the syntax DD lands, make the Rust classifier
   strict: kernel-only form in `.lykn` without escape → diagnostic;
   surface form in `.lyk` → diagnostic.
5. **V-06 documentation.** Once DD-37 is in active state, document
   the "Rust is validation; JS is fast-emit" divergence.

**Track B: Cross-compiler hygiene that doesn't depend on the
separation outcome.** These move now:

- Broader compileBoth conversion (originally Item 4). Sharpened by
  the handoff: prioritize tests that exercise the surface-vs-kernel
  boundary, which is where DD-50.6 Q4=A's drift was invisible.
- JS/Rust error-format alignment (originally Item 5).
- DD-50.7 #1 (if-profile-audit pattern) and DD-50.7 #2 (line-356
  Value override refactor) — both Rust-emitter cleanup, independent
  of the separation.
- DD-52 #3 (directory-path vs explicit `.lykn` import convergence) —
  cross-compiler expander convergence; independent.

**Track C: Held pending Track A's resolution.** These get reframed
once the separation lands:

- DD-52 #2 (`__surface_macro__` sentinel cleanup) — *deleted* once
  surface forms leave the macro env (per DD-37 decision); not
  "cleaned up."
- The cdc/dep-ergonomics CDC's paused W-1 (JS Q3=C fix), W-2 (surface
  `try` as value-producing), W-3 (DD-56 form catalog) — all need
  reframing against the separated model.
- The 8 remaining Class A1/A2 doctest failures (Phase 1a) — properly
  fixed by the separation; not by patching the current overlap.

**My recommendation for shape:**

1. **Today's deliverable:** this document, plus your calls on:
   - **Syntax choice** for the separation DD (the four candidates).
   - **Timing**: 0.6.0 (compressed; competes with M10-15 for slots),
     0.6.x patch release, or 0.7.0 (clean alignment with i18n).
   - **Track B as a near-term milestone** vs. folded into the
     separation work.
2. **Once calls are made:** I draft the separation DD (probably
   DD-58) for `01-draft/` with full design rationale and the
   rejected alternatives section. Bounded scope; should be one
   focused review iteration.
3. **In parallel:** Track B can move as M16 (or whatever the next
   open slot is) without waiting for Track A. The cross-compiler
   hygiene work has standalone value and won't be invalidated by
   the separation outcome.
4. **Cross-thread coordination:** once you've made the Track A calls,
   ping the cdc/dep-ergonomics CDC so they know when to resume W-1,
   W-2, W-3, and the remaining 8 doctest failures.

Track A is now genuinely substantial (it's the foundation for 0.7.0,
not architectural polish). Track B is the parallel-safe near-term
work. Track C waits.

---

## What I'd ask Duncan to decide before this thread proceeds

**Resolved 2026-05-15.** See the "Resolutions (2026-05-15)" section near
the top of this document. The questions below are preserved as
historical record of the open-question state immediately before
Duncan's calls.

---

The handoff settled most of the "whether" questions; what remains is
mostly "which" and "when."

1. **Kernel-call syntax (the main call):** which of the four candidates
   from the handoff wins?
   - `(kernel:<form> ...)` namespace prefix *(my weak preference)*
   - `(kernel (<form> ...))` enclosing form
   - `#kernel(...)` or `#k(...)` reader macro
   - Inverse marking (almost certainly wrong)
2. **Timing:** target 0.6.0 (compressed; competes with M14-M15 for
   slots since M10/M11/M13 are landed), a 0.6.x patch release, or
   0.7.0 (aligns cleanly with i18n)? The handoff's "0.7.0 i18n
   requires the form catalog requires the separation" chain means the
   latest acceptable target is "before 0.7.0 ships." Earlier is fine;
   later isn't.
3. **DD-37 promotion shape:** promote to `05-active/` with the three
   amendments I named (Phase 0 acceptance criterion, Relationship-to-
   separation-DD subsection, forward reference to the separation DD),
   or hold until the syntax-choice DD is drafted?
4. **V-06 disposition:** confirm (B) is the answer; queue documentation
   work as a Track B row?
5. **Track B as a separate milestone:** is this the right shape, and
   do you want me to draft it as an M-something for review? With the
   added DD-50.7 #1, #2, and DD-52 #3 rows, it's 5–6 items — large
   enough to be a real milestone, possibly large enough to split.
6. **Cross-thread coordination protocol:** once you've made the
   syntax + timing calls, do you want me to write a brief reply
   handoff for the cdc/dep-ergonomics CDC, or do you handle that
   communication directly?

(The DD-30 closure question is mostly resolved: implementation is
done; `bridge.rs` is dormant code that a separate cleanup PR can
delete. Worth verifying before treating DD-30 as fully closed, but
not a blocker.)

These are mostly one-question-each calls. I don't think any of them
need a long discussion — they're choices, not analyses.

---

## What this document is not

- Not a milestone spec. The shape of any subsequent milestone depends on
  Duncan's calls above.
- Not the separation DD itself. That's a follow-up deliverable (probably
  DD-58) drafted once Duncan picks the syntax. DD-36 stays in
  `01-draft/` as the historical record / supersedes-link target; the
  new DD captures the as-implemented design.
- Not a DD rewrite for DD-37. My position is that DD-37 stands as
  drafted, gets the three amendments named in Item 2, and promotes to
  `05-active/`.
- Not a recommendation to do everything at once. The Track A / B / C
  split exists precisely so Track B can move without prejudice.
- Not a closing report. There's no work to close yet — this is opening
  the thread (and integrating a substantial cross-thread handoff), not
  closing a unit of it.

---

## Fast-follow items handed off from other threads

Duncan flagged the following on 2026-05-10 as possibly belonging in this
thread (sourced from a conversation in the `cdc/dep-ergonomics` worktree).
My triage on each:

### In scope for this thread

**DD-50.7 #1 — if-profile-audit pattern for forms with special-case
intercepts.** Directly in scope. The `KernelChildProfile` machinery
from DD-50.5 + addendum is exactly the per-form intercept system this
item names; an audit pattern for it is structural-coherence work on
the Rust emitter side. Likely belongs in Track A or as a precursor
deliverable that doesn't block on DD-36/37.

**DD-50.7 #2 — Surface-form-handler Value override refactor
("line-356 thing").** Same category as #1 — Rust emitter
profile/handler machinery. Need to locate the specific site Duncan's
referring to (a grep against `emitter/forms.rs:356` would resolve it)
before scoping. In scope.

**DD-52 #3 — Directory-path vs explicit `.lykn` import convergence
between Rust/JS expanders.** Cross-compiler convergence in the
expander layer. Different mechanism than DD-49 (identifiers) or DD-50
(position-aware emission), but the same *category* of work: aligning
behaviour between the two compilers on a specific feature. Squarely
in scope; a natural Track B candidate (mechanical alignment, doesn't
need an architecture decision first).

### Absorbed by the separation work (formerly "Conditional")

**DD-52 #2 — `__surface_macro__` sentinel cleanup.** Post-handoff,
this item is no longer "conditional on Track A" — it's *deleted* by
the Track A work. DD-37's "built-in surface forms become static
transforms, not macros" decision means the sentinel has no callers
once the migration completes; the cleanup is "delete the sentinel
and its mechanism," not "add a kind field to `CompiledMacro`."
Track C (held).

### Out of scope — flagging for redirection

**DD-54 — `lykn cache clean` subcommand (0.7.0 candidate).** CLI
feature, not compiler architecture. Already tagged as 0.7.0 in
Duncan's note; belongs in a CLI features milestone or the 0.7.0
implementation plan, not this thread. Logging here only so it doesn't
get lost.

**Cross-cutting — `lykn test` auto-invoking `lykn build` post-M11
(UX paper cut).** Same — CLI / UX concern. Probably belongs alongside
DD-54 in a 0.7.0 CLI ergonomics milestone, or wherever M11's
follow-ups are tracked. Not architectural; not in scope here.

### Net effect on Track A / B / C framing

Track A (the separation DD + DD-37 + supporting work) is now the
load-bearing piece for 0.7.0 i18n; it's no longer optional
architectural polish.

Track B (cross-compiler hygiene that doesn't depend on the separation
outcome):
- The compileBoth-conversion row (already named).
- The JS/Rust error-format alignment row (already named).
- DD-50.7 #1 (if-profile-audit pattern). *new from handoff*
- DD-50.7 #2 (surface-form-handler Value override refactor). *new from handoff*
- DD-52 #3 (directory-path vs explicit `.lykn` import convergence). *new from handoff*

That's 5 firm Track B rows, which is starting to look like a
real milestone. If you split it (compileBoth + error-format as one
milestone, per-form/expander cleanup as another), the split also
gives a clean dependency boundary between "work that informs Track A"
and "work that benefits from Track A landing."

Track C (held pending Track A's resolution):
- DD-52 #2 — `__surface_macro__` sentinel deletion. Was "conditional";
  now firmly absorbed by Track A's DD-37 implementation.
- cdc/dep-ergonomics W-1 (JS Q3=C fix), W-2 (surface `try`
  value-producing), W-3 (DD-56 form catalog) — paused there awaiting
  the separation.
- The 8 remaining Class A1/A2 doctest failures — fixed by the
  separation, not by patching the current overlap.

---

## File-handling notes for this thread

- Worktree at `/Users/oubiwann/lab/lykn/lang/.worktrees/compiler-coherence/`
  on branch `cdc/compiler-coherence` off `release/0.6.x` @ `f9b647a`.
- One uncommitted change in this worktree: `.gitignore` adds
  `.worktrees/` line. Sandbox can't commit due to permissions on
  `.git/worktrees/` lock files; please commit from a regular shell
  when convenient.
- Two cleanup items in the parent `lang/` working tree (sandbox
  could not unlink): `.gitignore.tmp` (delete it) and `.worktrees/`
  shows as untracked (will go away when the `.gitignore` line is
  cherry-picked or re-added on `release/0.6.x`).
- `workbench/` in the worktree is a *symlink* to `../../workbench`,
  i.e. shared with `lang/workbench/`. The file tools, when targeting
  the worktree path, can't always follow this symlink (an earlier
  Write attempt to the worktree-relative path was a phantom success).
  Reliable approach: write workbench files via the
  `/Users/oubiwann/lab/lykn/lang/workbench/` path directly. Both
  views see the same file because of the symlink. This document is
  written there.
