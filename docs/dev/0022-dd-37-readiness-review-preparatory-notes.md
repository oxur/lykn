# DD-37 Readiness Review — Preparatory Notes

**Date:** 2026-05-15
**Author:** CDC (this session)
**Branch:** cdc/compiler-coherence
**Status:** Async prep for joint working session with Duncan. Read first,
then we walk through together and converge on a disposition.

---

## Purpose and how to use this doc

DD-37 ("JS Surface Compiler Architecture") was drafted 2026-04-18 and
promoted to `docs/design/01-draft/0047-*.md`. Per Duncan's 2026-05-15
call (Resolutions item #3 in the thread-opening doc), promotion to
`05-active/` is a joint review, not a unilateral CDC decision.

This doc is the **input** to that review. It walks DD-37 section by
section with my honest read on what's still correct, what needs
amendment, and what's actively contradicted by what's landed since.
At the end I list disposition options and my lean.

**How to use:** read it through before our session. Mark anywhere your
read differs from mine. We'll work through the marked items in the
session and converge.

**Target length:** ~400 lines. I've aimed at "comprehensive but not
exhaustive" — enough to ground the conversation; not enough to
substitute for it.

---

## State drift since 2026-04-18

DD-37 reasons from a four-week-old source snapshot. The snapshot is now
materially out of date in three ways.

### JS file line counts: total +19%, `compiler.js` +32%

| File          | DD-37 snapshot | Current (f9b647a) | Delta  |
|---------------|---------------:|------------------:|-------:|
| `mod.js`      |             19 |                19 |   ±0   |
| `reader.js`   |            320 |               320 |   ±0   |
| `expander.js` |          1,350 |             1,494 |  +144  |
| `surface.js`  |          2,262 |             2,315 |   +53  |
| `compiler.js` |          1,600 |             2,117 | **+517** |
| `icu-parser.js` |  *(didn't exist)* |          362 | **+362** |
| **Total**     |          5,551 |             6,627 | **+1,076** |

`compiler.js`'s +517 lines is mostly M7 work:
- DD-49 identifier mapping (predicate-prefix algorithm, mechanical
  escape, collision detection).
- DD-50 position-aware compilation (`convert_to_expression`, ternary
  vs IIFE decision, the 64-call expression-call audit).

`icu-parser.js` is new — landed for the 0.6.x i18n template work that
DD-37 doesn't account for. It's a self-contained module and probably
doesn't change DD-37's six-module decomposition, but it does mean the
JS toolchain is *seven* modules already in practice (six lang + ICU
parser).

**Implication for DD-37:** the "shrinks as surface forms move out"
claim is still directionally right, but the baseline is bigger than
DD-37 estimated. The migration would now strip more lines than the DD
predicted. This is *better* for DD-37's case, not worse — but the
bundle-size estimate (+8–20KB gzipped) was based on Apr-18 line counts
and should be re-measured before Phase 3.

### Post-rebase chronology note (2026-05-15)

This doc was originally drafted against the worktree's pre-rebase
state (off `f9b647a`, 2026-05-11). The worktree was rebased onto
`release/0.6.x @ 4ee1eaa` on 2026-05-15, picking up 31 commits. JS
line counts are unchanged from this doc's analysis (still 2,117
`compiler.js`, 1,494 `expander.js`, etc.), so the bundle-size and
file-growth arguments still hold. What *has* changed: DD-50.6 +
DD-50.7 have landed (no longer "in flight"); M10, M11, M13 are
landed; DD-52, DD-53, DD-54, DD-55 ICU/i18n work all landed; a
tactical patch removed `fn` from the JS statement-only list (commit
`e50edc9`) — closing part of the Class A1 doctest failure class
ahead of the architectural fix. DD-37 itself was promoted to
`05-active/` (commit `4ee1eaa`); the three amendments below have
been applied to the 05-active file 2026-05-15.

### M7 landed three load-bearing changes

- **DD-49 (identifier mapping).** Both compilers produce byte-identical
  output for `?`-suffix, `!`-suffix, and other Lisp punctuation. The
  `to_js_identifier` / `toJsIdentifier` pair is mirrored across crates
  and packages. Relevant to DD-37 because *this is exactly the pattern
  DD-37 wants the broader architecture to follow* — typed AST in Rust,
  static transforms in JS, byte-identical output. M7 produced the
  precedent. *(Plus: M7's identifier-mapping work is also what produced
  most of `compiler.js`'s +517-line growth since the original DD-37
  draft — context for the bundle-size baseline measurement.)*
- **DD-50 (position-aware compilation) + DD-50.5 (per-form context
  profile).** The Rust emitter now has `KernelChildProfile` —
  per-kernel-form classification of which child positions want Value
  vs. Statement context. This is **partial JS-classifier-equivalent
  infrastructure on the Rust side** that DD-37 doesn't account for.
  Worth thinking about whether the JS classifier DD-37 proposes
  mirrors `KernelChildProfile` directly, or whether the two compilers
  end up with parallel-but-not-identical schemas.
- **DD-50.6 (implicit-return-of-statement) — landed 2026-05-15.**
  Closed the `wrapReturnLast` bug. Doesn't structurally affect DD-37,
  but it's one more "patch in the current architecture that the
  separation eliminates" data point. **DD-50.7** also landed (`fix
  DD-50 emission for real downstream patterns + emit_if_iife`,
  commit `2769eb4`) — same category. Two more patches in the
  current architecture; DD-58 separation removes the need for both
  classes.

### The handoff (2026-05-14 from cdc/dep-ergonomics)

Covered in detail in the thread-opening doc; one-paragraph summary
here for the review session's purposes:

The other CDC's Phase 1c audit established that both compilers conflate
two questions (Q-emit and Q-value) onto one list, and that
`STATEMENT_FORM_HEADS` (Rust) and `STATEMENT_ONLY_HEADS` (JS) have
drifted out of sync. The JS-side DD-50.6 Q3=C bug — JS checks the
pre-expansion form's head instead of the post-expansion form's — is
*precisely the kind of bug that disappears under DD-37's typed AST*,
because the classifier produces a typed surface node that already
encodes "this is a `Fn` node that emits as a value-producing kernel
arrow function" without needing post-expansion inspection.

**This is the clearest argument for DD-37 to date.** It's no longer
"architectural hygiene"; it's "the fix for a shipping bug class."

---

## Section-by-section read of DD-37

I'll annotate each substantive section: **STILL CORRECT** / **NEEDS
AMENDMENT** / **ACTIVELY CONTRADICTED**.

### "Summary" — STILL CORRECT

The target shape (`reader → classifier → (macro expander | static
surface transforms) → kernel emitter → kernel compiler → astring`) and
the load-bearing decisions ("built-in surface forms become static
transforms, not macros") are unchanged by anything since.

### "Context: what the JS compiler is today" — NEEDS AMENDMENT

The line counts are stale (see above). The "What works" / "What
doesn't scale" sections are still accurate; the bug class identified
("the JS compiler cannot distinguish ... actual surface form input")
is now empirically validated by the 14 doctest failures.

**Proposed amendment:** update the line-count table with current
numbers; add a sentence acknowledging the M7 work and the handoff's
empirical findings. ~50 lines of edit.

### "Decisions: Six-module decomposition" — STILL CORRECT

The architecture diagram and the per-module responsibilities are
unchanged. One small thing: DD-37 lists the seven consumer rows
(`lykn()`, kernel-only compilation, browser runtime, test runner,
formatter, linter). Worth confirming in the review that no new
consumer has emerged (e.g., did anyone start a JSON-output mode? A
language-server prototype?).

### "Decisions: Surface AST as tagged object literals" — STILL CORRECT

JSDoc typedefs + plain JS objects with a `type` discriminator is the
right call. DD-37's worked example (`mkFunc`, `isFunc`) is fine.

### "Decisions: Two-level AST mirroring DD-20" — STILL CORRECT

### "Decisions: Built-in surface forms as static transforms, not macros" — STILL CORRECT, and now empirically forced

This is *the* load-bearing decision. The handoff's findings make it
no longer optional. The Q3=C bug in the JS compiler exists because
there's no typed-AST distinction between "this is a `fn` that will
expand to value-producing `=>`" and "this is statement-only `fn`"; the
current code makes the wrong call because it's looking at the
syntactic head before any classification has happened.

### "Decisions: User-defined macros via `new Function()` — no subprocess" — STILL CORRECT

### "Decisions: No static analysis module — explicit trade-off" — STILL CORRECT, and resolves V-06

This is the section that pre-commits to V-06's answer (B). The
section is still correct; the V-06 documentation deliverable is a
*consequence* of this DD landing, not a separate decision.

### "Decisions: Kernel compiler is its own module" — NEEDS AMENDMENT

DD-37 says `compiler.js` is 1,600 lines and shrinks. Current state:
2,117 lines and growing. The architectural claim is still right but
the numerical claim is stale.

**Proposed amendment:** update line counts; add a paragraph
acknowledging that M7's work has *added* to `compiler.js`'s surface,
strengthening the case for the migration (more debt to strip).

### "Decisions: Emitter module: surface AST → kernel SExpr" — STILL CORRECT

The emission table is fine. Worth confirming in the review that the
table is *complete* — i.e., we haven't added surface forms since
2026-04-18 that aren't in DD-37's list.

### "Decisions: Classifier module: dispatch and validation" — NEEDS AMENDMENT

The `SURFACE_FORMS` and `KERNEL_FORMS` set sketches in DD-37 still
have `=` and `!=` in *both* sets — DD-37 noted this would be cleaned
up by DD-36. With DD-36 superseded by the new separation DD (DD-58),
this section needs to point at the new DD instead.

**Proposed amendment:** replace "Per DD-36, these sets will be
disjoint after the kernel/surface split lands" with the actual
status — that the separation DD (DD-58) enforces disjointness, and
that the classifier in DD-37 produces typed nodes that make the
`kernel:<form>` escape syntactic rather than positional. ~20 lines.

### "Decisions: Diagnostics module" — STILL CORRECT

The structured-diagnostic shape is fine.

### "Decisions: JSON interchange with Rust (kernel SExpr format)" — STILL CORRECT

Worth confirming we haven't drifted from DD-20's JSON format since.

### "Decisions: Kernel-only compilation path (DD-36 integration)" — NEEDS AMENDMENT

Same as the classifier section: needs to reference DD-58 (the new
separation DD) instead of DD-36 as the "what triggers extension-based
dispatch" precondition.

### "Decisions: Gradual migration, not a big-bang rewrite" — STILL CORRECT

The eight-step migration sequence is sound and the per-step
testability claim still holds. Step 0 (the bundle-size baseline) is
the thing we'd unblock with Phase 0 acceptance criterion.

### "Rejected alternatives" — STILL CORRECT

### "Bundle size considerations" — NEEDS AMENDMENT

The estimate band (+8–20KB gzipped, +1,700–2,100 lines source) was
derived from the 2026-04-18 source. With the codebase now 19% larger,
the *absolute* delta may be similar but the *relative* picture has
shifted. More importantly: the Phase 0 measurement task is still
listed as an "Open measurement task," but Duncan's 2026-05-15 call to
target 0.6.0 means Phase 0 needs to happen *now*, not deferred.

**Proposed amendment:** the Phase 0 acceptance criterion (see next
section).

---

## The three amendments — sketched wording

### Amendment 1 — Phase 0 acceptance criterion

Insert a new subsection just before "Bundle size considerations" or
as a top-level "Acceptance state" section:

> ## Acceptance state and Phase 0 criterion
>
> DD-37's promotion from `01-draft/` to `05-active/` is contingent on
> Phase 0 being substantively underway:
>
> 1. **Baseline measurement landed.** A reproducible script that
>    measures `lykn-browser.js` (or the equivalent published artifact)
>    minified + gzipped size at `HEAD`, with the result recorded in
>    this DD's refinement log.
> 2. **CI guard wired up.** A `make` target (or CI step) that fails
>    on PR-introduced bundle growth exceeding a per-PR threshold. My
>    proposed thresholds, subject to Duncan's call: +2KB gzipped is a
>    warning, +5KB gzipped is a hard fail without explicit sign-off.
>    The numbers translate the "+1KB acceptable / +20KB investigate /
>    +100KB unacceptable" guidance into per-PR units.
> 3. **One full-pipeline migration prototyped.** Per DD-37's "open
>    measurement tasks" list: pick the smallest surface form (probably
>    `not` or `reset!`), migrate it end-to-end through the new
>    architecture, measure the delta, and extrapolate. Result recorded
>    in this DD as Phase 0 evidence.
>
> Until these three are in place, DD-37 stays in `01-draft/` even
> though the architectural direction is settled. This is "we know what
> we're building; we don't yet know what it costs."

### Amendment 2 — Relationship-to-separation-DD subsection

Insert a new subsection in Decisions (probably after "Six-module
decomposition") or in Dependencies:

> ## Relationship to DD-58 (the kernel/surface separation DD)
>
> DD-37 and DD-58 are *complementary, not sequential dependencies* —
> they describe different facets of the same restructuring:
>
> - **DD-58** defines the syntactic split: `.lyk` for kernel, `.lykn`
>   for surface, `(kernel:<form> ...)` as the escape hatch from
>   surface into kernel, and the classifier-strict rule that surface
>   files cannot contain bare kernel forms (and vice versa).
> - **DD-37** defines the JS-side architecture that *makes* the
>   classifier enforceable: a typed surface AST, static surface
>   transforms (not macros), a kernel emitter module, and a kernel
>   compiler that refuses surface-only forms.
>
> DD-37's classifier is the implementation surface for DD-58's split.
> DD-37 ships first (the architecture has to exist before the rules
> can be enforced); DD-58's strict enforcement turns on after DD-37's
> classifier is in place.
>
> This relationship supersedes DD-36's original sequencing claim that
> "DD-37 is Phase 0 of DD-36." DD-58 replaces DD-36 with a cleaner
> direction (the `kernel:<form>` syntax decision made 2026-05-15) and
> preserves DD-36's substance as a historical analysis.

### Amendment 3 — Forward reference to DD-58

The "Depends on" line in DD-37's header currently reads:

> **Depends on**: DD-13 (macro expansion pipeline), DD-15 (language
> architecture), DD-20 (Rust surface compiler architecture), DD-36
> (kernel/surface compiler split — this is its JS-side Phase 0
> prerequisite)

Proposed replacement:

> **Depends on**: DD-13 (macro expansion pipeline), DD-15 (language
> architecture), DD-20 (Rust surface compiler architecture).
> **Complements**: DD-58 (kernel/surface separation — `kernel:<form>`
> escape syntax). DD-36's analysis is preserved as historical record;
> DD-58 supersedes its direction.

The "Citations" section gets a similar update: DD-36 reference stays
(as historical), DD-58 reference added.

---

## Bundle-size posture: what Phase 0 requires operationally

Three concrete pieces of work:

1. **A `make bundle-size` (or similar) target** that:
   - Builds the JS toolchain into a single minified file (probably
     `dist/lykn-browser.min.js` per the DD-37 sketch).
   - Reports the gzipped size.
   - Optionally compares against a checked-in baseline file
     (`assets/bundle-size-baseline.txt` or similar) and reports the
     delta.
2. **A CI step** that runs the above and fails on threshold breach.
   Thresholds: my proposal is +2KB warning, +5KB hard fail; happy to
   adjust.
3. **A one-form pilot migration** to validate the per-form cost
   estimate. DD-37 suggests `not` or `reset!` as the smallest target.
   The pilot's purpose is purely measurement — it doesn't have to
   make the migration permanent. Once we have a real per-form number,
   we extrapolate to ~20 forms and check against the +8–20KB band.

The pilot is the load-bearing piece. The other two are mechanical and
can land in any order. Total estimated work: 2-3 CC iterations across
2-4 PRs.

**Question for the review:** is Phase 0 a single CC milestone, a
sub-milestone within a larger DD-58+DD-37 implementation milestone, or
a "tooling work" item that doesn't get its own milestone?

---

## DD-37's nine open questions — my read on each

1. **Where does `@lykn/browser` sit?** *(STILL OPEN.)* DD-37's lean
   is "import the full toolchain; document filesystem-dependent
   features as unsupported in browser." I agree. Current state:
   `packages/browser/` has its own `compiler.js` wrapper plus
   `scripts.js` (auto-processes DOM `<script type="text/lykn">`
   tags). Under DD-37 it would re-export the new six-module
   toolchain. No new work required by DD-37 itself; the bundle-size
   discipline handles "lean browser" if it's needed.

2. **Bundle-size measurements.** *(NOW URGENT — Phase 0.)* Becomes
   Amendment 1's acceptance criterion.

3. **Source maps.** *(STILL OPEN, deferred.)* No change. Future DD.

4. **REPL support.** *(STILL OPEN, deferred.)* `compileExpr` and
   `expandExpr` exports already support it. REPL design is its own DD.

5. **Linter / formatter.** *(STILL OPEN, deferred.)* The
   cdc/dep-ergonomics CDC's M12 (linter) work is dependent on
   DD-37's classifier landing, but the linter rules and policies are
   their own DDs. (M11 was build-dir reorg, not linter; it's already
   landed.)

6. **Incremental compilation.** *(STILL OPEN, deferred.)* No change.

7. **Parity testing with the Rust compiler.** *(PARTIALLY ANSWERED.)*
   `compileBoth` already exists (DD-50.5 addendum) and operates at
   the *output JS* level, not the kernel SExpr level. DD-37 envisions
   a separate kernel-JSON parity test suite once `emitter.js`
   produces deterministic SExpr. Both layers are valuable; `compileBoth`
   covers end-to-end, kernel-JSON parity would cover the JSON
   interchange contract.

8. **Surface form error-message style.** *(PARTIALLY ANSWERED by
   M7.)* DD-49 settled the identifier-error format (`isValid
   (valid?):` bridging for punctuation transformations). Finding #4
   (gensym-leak in return-type errors) is one of the Track B rows.
   The broader style guide is still open.

9. **Keyword handling.** *(STILL OPEN.)* DD-37 names this; the
   answer is probably "keywords are identifiers in surface position,
   strings in kernel position" and the emitter spec needs to make it
   explicit. Worth a paragraph in DD-37 or its own follow-up. Low
   priority but easy to settle.

**Net:** of the nine open questions, two are now urgent (Phase 0 +
amendments), two are partially answered by M7's work, and five remain
deferred-but-tracked.

---

## Disposition options + my lean

Three plausible dispositions:

### Option A — Promote to `05-active/` with the three amendments

What it means: DD-37 moves to active state today; the three amendments
land as the same PR; Phase 0 work starts immediately under the
acceptance criterion; per-form migration is milestone-scoped (a future
M-something).

**Risk:** Phase 0 surfaces a bundle-size number worse than the +8–20KB
estimate, and DD-37 has to fall back to Alt C (classifier-only). The
DD pre-commits to that fallback, so the architecture has somewhere to
land — but we'd be promoting to "active" before we know which variant
is final.

### Option B — Hold in `01-draft/` until Phase 0 completes

What it means: do Phase 0 work first (baseline measurement + CI guard
+ one-form pilot), then promote DD-37 with the empirical numbers
baked into the DD. Either the full architecture lands or Alt C is
selected based on evidence.

**Risk:** delays the cdc/dep-ergonomics CDC's resumption by however
long Phase 0 takes (probably 2–3 weeks of focused CC work). Given the
0.6.0 target, that's a meaningful slice.

### Option C — Rewrite DD-37 against the as-implemented separation

What it means: DD-37's content was structured around DD-36 as the
companion DD. With DD-58 superseding DD-36, the framing changes
non-trivially. Rather than amend, rewrite.

**Risk:** wasteful. The substantive content of DD-37 is right;
amendments (Option A) capture the delta without losing the analysis.
Rewriting would re-derive most of what's already there.

### My lean: Option A

The three amendments are bounded; Phase 0 acceptance criterion is the
load-bearing piece. Promoting *with* a Phase 0 criterion is the
honest middle ground: "we've decided the direction; the implementation
gates remain." That's what `05-active/` means in this project's
methodology (per other active DDs).

Option B is the conservative alternative if you'd rather see the
numbers first; I'd accept it but lean against because 0.6.0 timing is
already aggressive.

Option C I'd actively recommend against.

---

## Cross-cutting open items for the joint session

1. **DD numbering** — *resolved 2026-05-15.* The separation DD is
   **DD-58**. DD-55/56/57 were unavailable; DD-56 in workbench is
   cdc/dep-ergonomics' form catalog.
2. **The `kernel:` reader-precedence question** — *resolved
   2026-05-15.* Reader keeps `kernel:if` as a single atom; classifier
   dispatches on the colon. (Confirmed by Duncan; consistent with
   DD-01.)
3. **The form enumeration** — *substantively reframed 2026-05-15.*
   The original framing assumed a three-bucket split (surface-only /
   kernel-only / auto-promoted in surface). Duncan rejected the
   "auto-promoted" bucket and replaced it with a closed-surface-
   namespace model. The corrected model:

   **Every form a user writes in surface code is a surface form.**
   Surface forms come in three implementation flavors, all
   classified uniformly:

   - **Rich, surface-unique** (`bind`, `func`, `match`, `obj`,
     `cell`, threading macros, `if-let`, etc.). No kernel namesake.
   - **Thin wrapper** (`+`, `-`, `*`, `array`, `object`,
     `template`, `get`, etc.). Surface name matches kernel name;
     classifier produces a typed surface node; emitter is literal
     passthrough.
   - **Rich, namesake-sharing** (surface `if`, surface `try`).
     Surface name matches kernel name but emitter does elaborated
     work (position-aware ternary/IIFE for `if`, value-producing
     IIFE for `try`).

   Kernel forms that surface doesn't use are reachable *only* via
   `(kernel:foo ...)`. There is no auto-promotion, no fall-through.

   This is *better* than DD-36's original framing. DD-58 will use
   this three-flavor model. The form catalog (DD-56) gets a cleaner
   schema as a consequence: surface entries enumerate the full user-
   facing namespace (with i18n hooks); kernel entries are
   language-neutral; thin wrappers carry `expands_to`; rich
   namesake-sharing forms carry an `expansion` descriptor (e.g.,
   `"position-aware"`).

   **For the session:** worth sketching the canonical list of which
   forms fall in each flavor. Probably ~20 rich-surface-unique forms,
   ~30–40 thin-wrapper forms (arithmetic, comparison, bitwise,
   logical, literal constructors), ~5–10 rich-namesake-sharing forms
   (`if`, `try`, possibly others surfaced by D-2-style analysis).

4. **DD-37's "consumers" table** has six entries (compiler, kernel
   path, browser, test runner, formatter, linter). cdc/dep-ergonomics'
   M11 (build-dir reorg) + M13 (lykn publish dirty-check) landed
   already (commit `a6d1e2e`); M12 linter remains in flight in
   another worktree — coordination is yours to handle per your
   2026-05-15 call.

---

## What this doc isn't

- Not the separation DD (DD-58). That's the next deliverable after
  this review.
- Not the Track B milestone spec. That's the deliverable after DD-58.
- Not a substitute for reading DD-37 itself. The review session
  works best if you re-read the DD (especially the Decisions sections)
  before we meet.

---

## Session-prep summary (the TL;DR for our walk-through)

- DD-37 is **substantively still correct**. Four weeks of source
  drift haven't invalidated any decision.
- The handoff makes DD-37 **more urgent**, not different.
- **Three concrete amendments** are proposed, with sketched wording
  in this doc. Read the wording; if it lands, we promote.
- **Phase 0 (bundle-size baseline + CI guard + one-form pilot)** is
  the load-bearing precondition for promotion. Either it's the
  acceptance criterion (Option A, my lean) or it's the gate that
  precedes promotion (Option B).
- **My lean: Option A** — promote with the three amendments, do
  Phase 0 under the acceptance-criterion frame. *Duncan confirmed
  Option A 2026-05-15.*
- **Cross-cutting items: two pre-resolved, two for the session.**
  DD numbering (→ DD-58) and reader-precedence (→ reader keeps as
  one atom, classifier splits) are settled. Form-enumeration sketch
  and linter coordination remain for the session — the form
  enumeration is the more substantive of the two, and it carries
  Duncan's "auto-promote correction" (three implementation flavors,
  closed surface namespace) as input.

That's the review. Ready when you are.
