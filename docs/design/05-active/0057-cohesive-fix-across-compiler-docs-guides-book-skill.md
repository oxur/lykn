---
number: 57
title: "Cohesive Fix Across Compiler, Docs, Guides, Book, Skill"
author: "the audit"
component: All
tags: [change-me]
created: 2026-05-14
updated: 2026-05-14
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# Synthesis Plan: Cohesive Fix Across Compiler, Docs, Guides, Book, Skill

**Date:** 2026-05-14
**Status:** Draft for Duncan's review; first complete pass.
**Inputs:**
- Phase 1a (`phase-1a-test-failure-triage-2026-05-14.md`)
- Phase 1c (`phase-1c-classification-audit-2026-05-14.md`)
- Phase 2 main catalog (`phase-2-divergence-catalog-2026-05-14.md`)
- Phase 2 book addendum (`~/lab/cnbb/lykn/workbench/phase-2-book-addendum-2026-05-14.md`)
- Full pattern audit (this document, §1)

**Duncan's Phase 3 calls (per 2026-05-14 conversation):**
- **D-1**: ✓ JS-side fix. Rust is correct.
- **D-2**: ✓ D-2.γ — surface `try` becomes position-aware (Lisp/Haskell/Rust family alignment).
- **D-3**: ✓ Doc-only fix via `,compile-fail` annotations.
- **D-4 / D-5 / D-6**: ✓ Deeper architectural fix, not minimal patch.

This document operationalizes those calls into a concrete, sequenced plan across all surfaces.

---

## §1 — Full pattern audit findings

The audit swept every surface for D-1, D-2, and D-3 patterns. Below is the complete inventory.

### 1.1 — D-1 instances (factory pattern `:returns :function :body … (fn …)`)

**`docs/guides/` (4 files, 8 instances):**

| File | Line | Function |
|---|---|---|
| 06-functions-closures.md | 538 | `create-logger` |
| 06-functions-closures.md | 550 | `create-multiplier` |
| 06-functions-closures.md | 905 | `create-filter` |
| 07-async-concurrency.md | 582 | `debounce` |
| 07-async-concurrency.md | 589 | `throttle` |
| 08-performance.md | 445 | `memoize` |
| 08-performance.md | 470 | `memoize-lru` |
| 11-documentation.md | 267 | `debounce` (alt) |

**Lykn book (2 chapters, 3 instances):**

| File | Line | Function |
|---|---|---|
| ch4/3-scope.md | 54 | `make-greeter` |
| ch7/4-closures.md | 8 | `make-greeter` |
| ch7/4-closures.md | 27 | `make-counter` |

**SKILL.md:** No D-1 instances. The skill teaches `fn` as anonymous arrow but doesn't show the factory pattern.

**Mycelium downstream:** No D-1 patterns in `mycelium/packages/**/*.lykn`. The factory pattern is not in use in real downstream code today.

**Tests:** No D-1 tests in `test/forms/` exercise the surface-vs-kernel boundary that the pattern crosses.

### 1.2 — D-2 instances (try-as-expression: `(func … :returns :T :body (try …))` or any try-position requiring a value)

**`docs/guides/03-error-handling.md` (extensive — try-as-expression is the dominant pattern in this guide):**

| Line | Function | Pattern shape |
|---|---|---|
| 112 | `load-config` | `:returns :object :body (try … (catch err (throw …)))` |
| 158 | (unnamed example) | selective catch with `instanceof` |
| 339 | `try-parse-json` | `:returns :any :body (try … (catch undefined))` |
| 343 | `valid-json?` | `:returns :boolean :body (try (block …) (catch false))` |
| 390 | `load-user` | `:returns :any :body (try … (await response:json) (catch …))` |
| 466 | (Promise:any handler) | try wrapping `await Promise:any` |
| 519 | `load-data` | `:returns :any :body (try … (catch …))` |

The guide treats try-as-expression as **the** standard async error handling shape. Almost every `func :returns :T :body` example with error handling uses it.

**Lykn book (4 chapters, 5+ instances):**

| File | Line | Function | Pattern |
|---|---|---|---|
| ch17/4-error-handling.md | 7 | `safe-fetch` (1st version) | try → unwrapped value, catch → null |
| ch17/4-error-handling.md | 44 | `safe-fetch` (2nd, Result version) | try → `(Ok …)`, catch → `(Err e:message)` |
| ch25/2-json.md | 22 | `parse-json-safe` | try → `(Ok …)`, catch → `(Err e:message)` |
| ch27/5-fetch.md | 35 | `fetch-json` | try → `(Ok …)`, catch → `(Err …)` |
| ch37/5-routes.md | 29 | (handler) | try with body computation |
| ch38/3-api.md | 11 | `shorten` | try with body computation |
| ch38/3-api.md | 24 | `list-entries` | try with body computation |

**Book Ch 9.5 (Exceptions) — the canonical try chapter:** treats try as kernel-only / statement-only. Quote: *"`throw`, `try`, `catch`, `finally` are kernel forms with no surface transformation."* No value-producing examples in Ch 9.5.

**SKILL.md:** Line 446: "`try`/`catch`/`finally`: kernel forms used directly in surface code. **MUST**" — agrees with Ch 9.5. Example at line 724 shows statement-form usage only.

**`docs/guides/09-anti-patterns.md`:** Two `(try … (catch err …))` instances (lines 592, 597). Both are statement-form (used for the rethrow pattern, no value channel). Not D-2.

**Mycelium downstream:** No D-2 patterns in `mycelium/packages/**/*.lykn`. The try-as-expression pattern is not in use in real downstream code today either.

**Tests:** `test/forms/try-catch_test.lykn` and `test/forms/try-catch.test.js` exist. Did not audit their contents in this pass — flag as Phase-3-implementation item.

**README.md:** Table at line 365 shows `(try body (catch e ...) (finally ...))` → `try { body } catch(e) { ... } finally { ... }`. This is the kernel mapping; doesn't position try as expression-producing. Fine as-is; gets a parenthetical update under D-2.γ.

### 1.3 — D-3 instances (intentional-error code blocks in 17-template-and-i18n.md)

**6 blocks**, all in `docs/guides/17-template-and-i18n.md`, lines 144, 152, 160, 167, 174, 182 (the `;; ERROR:` patterns). Phase 1a Class B.

No equivalents in any other doc surface — these are ICU-template-specific error illustrations.

### 1.4 — New divergences surfaced by the audit

**1.4.1 — Book Ch 9.5 internal inconsistency with Ch 17.4 / 25.2 / 27.5 / 37.5 / 38.3.**

Ch 9.5 declares `try` as "kernel form with no surface transformation." Five later chapters use try-as-expression patterns that *require* surface transformation. The book is internally inconsistent on this point right now, independent of compiler behavior.

**1.4.2 — SKILL.md aligns with Ch 9.5, diverges from those later chapters.**

The lykn skill teaches try as kernel-statement-only (line 446). Under D-2.γ resolution, the skill's framing is wrong — surface `try` becomes position-aware and the "kernel forms used directly" framing becomes technically incorrect (it's a *surface* form that happens to compile through the kernel `try`, just like surface `if` compiles through kernel `if`/ternary).

**1.4.3 — Ch 9.1 (Conditionals) describes `if` as a pure kernel form.**

Ch 9.1 line 4: *"Lykn's `if` is a kernel form that maps directly to JavaScript's `if` statement."* This is pre-DD-50 framing. After DD-50/.5/.6/.7, surface `if` is position-aware (ternary in value position, IIFE in mixed position, statement in statement position). The book chapter hasn't been updated to reflect this.

This is in scope for the existing book-drift-inventory thread. Mentioned here because the prose-update pattern needed for Ch 9.5 (under D-2.γ) will mirror what Ch 9.1 needs.

**1.4.4 — No writers'-guide constraint on doc resolution.**

`~/lab/cnbb/lykn-writers-guide/authoring-guide.md` covers voice, structure, mdbook conventions, and code-block tagging (use ` ```lisp ` for lykn source). No rule about compile-testing examples, no rule that constrains how D-1/D-2/D-3 examples need to be written. Free hand.

**1.4.5 — `compileBoth` test corpus doesn't exercise the surface-vs-kernel boundary.**

`test/forms/dd-50.6_test.lykn` and `test/forms/dd-50.7.test.js` exercise statement-only patterns (`while`, `for`, no-else `if`, `block`). None of them exercise surface forms that *expand* to a value-producing kernel form (`fn → =>`, `match → if-chain`, `if-let → if`, etc.). This is the gap that let D-1 slip past Q4=A.

**1.4.6 — Mycelium has no D-1 or D-2 patterns.**

The canonical downstream compiles cleanly. So D-1 and D-2 aren't blocking real downstream — they're blocking the documented and taught patterns. This affects sequencing (no production fire) but not necessity.

---

## §2 — The cohesive plan

Five workstreams, each with its own deliverable shape and gating tests. They overlap in space but not in dependency order.

### Workstream W-1 — JS compiler: implement Q3=C properly

**Goal:** JS surface compiler implements DD-50.6 Q3=C (compile-then-check on emitted form, not surface form). After this, both compilers agree on what counts as "valueless last expression," and the lists in each impl are derivable from a single semantic question.

**Scope (per Duncan's call: deeper fix):**

1. Restructure JS surface macros (`packages/lang/surface.js`) so the `func` macro's Q2=A check operates on the *post-expansion kernel form*, not the surface form. Concrete shape:
   - The `func` macro currently calls `isStatementOnlyForm(lastBodyExpr)` *before* expansion.
   - Change to: expand `lastBodyExpr` to its kernel form first (via a recursive expansion through the macro environment), then check.
   - For the common cases (`fn → =>`, threading macros → call, `match → if-chain`, `if-let → if`), this single-step expansion is enough. For deeper compositions, recursive expansion handles arbitrary depth.

2. Apply the same shape to JS `if` IIFE-vs-ternary decision in `compiler.js` (the `is_statement_form` analog). Make it operate on post-expansion forms.

3. Add a `compile-both`-style test corpus that *specifically* exercises the surface-vs-kernel boundary. At minimum:
   - `(func f :returns :function :body (fn (:any x) x))` — D-1 canonical
   - `(func f :returns :any :body (match x ((Some v) v) (None nil)))` — match-as-value
   - `(func f :returns :any :body (if-let ((Some v) (find x)) v default))` — if-let
   - `(func f :returns :any :body (-> x (transform-a) (transform-b)))` — threading

These tests would have caught D-1 at DD-50.6 closing time.

**Gating:** all 14 doctest failures from Phase 1a Class A1 resolve. Mycelium acceptance stays green. New boundary tests pass on both compilers.

**Estimated complexity:** moderate. The JS macro layer doesn't have a "kernel form after expansion" handle today; adding one is real architectural work but follows the natural shape of the macro environment.

**Workstream W-1 closes D-1 and D-5 simultaneously.**

### Workstream W-2 — Both compilers: position-aware surface `try` (D-2.γ)

**Goal:** Surface `try` becomes value-producing in expression/Tail context, mirroring DD-50.7's treatment of `if`. The Lisp/Haskell/Rust family choice.

**Architecture (mirrors DD-50.7):**

1. **Rust side**, `crates/lykn-lang/src/emitter/forms.rs`:
   - In `func`-body emit paths, when the last body expression is `(try …)` and the context is Value/Tail, IIFE-wrap. This is exactly the pattern DD-50.7 added for `if`.
   - The IIFE wrap: `(() => { try { ... return value-producing-last-of-try-body } catch(e) { return value-producing-of-catch-body } })()`.
   - When the context is Statement (e.g., `try` as a middle statement in a body), keep current statement emit. No change.
   - Remove `"try"` from `STATEMENT_FORM_HEADS` *after* the IIFE-wrap logic is in place — it's no longer "valueless as last body" once the emitter wraps it.

2. **JS side**, `packages/lang/compiler.js`:
   - Add a `tryAsExpression(args)` path that emits an IIFE-wrapped TryStatement.
   - Route the `try` kernel form to it when in Value/Tail context.
   - The `Try` kernel form stays statement-only at the kernel level (Ch 2.2's "the kernel is the thin skin"); the IIFE wrap is the surface adaptation.

3. **Both compilers**: handle the edge cases:
   - `(try …)` with no catch/finally — compile error (existing behavior).
   - `(try … (catch e expr))` where the catch body's last expression is statement-only — propagate appropriately.
   - `(try … (finally cleanup))` without catch — value of try-body; finally runs as side effect. Mirror existing Rust IIFE patterns.
   - Nested try in expression position — recursive IIFE wraps.

4. **Surface tests** (`test/forms/try-catch_test.lykn` extended):
   - `(func f :returns :string :body (try (parse s) (catch e "default")))` → returns parsed value or "default"
   - `(bind result (try (compute) (catch e default-value)))` → bind initializer
   - `(func f :returns :any :body (try (Ok (parse s)) (catch e (Err e:message))))` → Result pattern
   - Statement-position try unchanged: `(do (try (cleanup1) (catch e ...)) (try (cleanup2) (catch e ...)))` stays statement-form.

**Gating:** All 8 doctest failures from Phase 1a Class A2 resolve (when the docs are updated alongside in W-4). All Ch 17.4, 25.2, 27.5, 37.5, 38.3 patterns now compile correctly. New try-as-expression tests pass on both compilers.

**Estimated complexity:** moderate. The pattern from DD-50.7 is established; this is "do the same thing for `try`."

**Workstream W-2 closes D-2.**

### Workstream W-3 — Compiler architecture: single source of truth for form classifications

**Goal:** Implement Duncan's standing direction — separate lists per semantic question, single source of truth across the two compilers. Q4=A (lists kept in sync) becomes structurally enforced rather than hopefully-maintained.

**Approach options (pick one in Phase 3 finalization):**

**Option A — Lykn-side source-of-truth.** A single `assets/form-classifications.toml` or similar declares each form with its semantic flags (`emits-as-statement`, `produces-value-as-last-body`, `is-control-transfer`, etc.). Both compilers' lists are generated from this file (Rust via `build.rs`; JS via a build step that runs the lykn CLI or a small JS tool). Cleanest end state; build-system weight.

**Option B — Test-fixture source-of-truth.** A single `test/fixtures/form-classifications.yaml` or similar declares each form's semantic answers. Both compilers' unit tests assert against this fixture. Lists in each compiler stay hand-maintained, but drift fails CI. Lighter weight; correctness via test instead of structure.

**Option C — Just structural test coverage.** Keep the lists hand-maintained, but add the surface-vs-kernel boundary tests (from W-1) and additional cross-impl tests that exercise every semantic question for every form. Drift surfaces in test failures, not in a separate source-of-truth file. Lowest tooling weight.

**Per Duncan's "separate lists per semantic question" framing**, options A and B are best aligned. Option C is the minimum-viable variant.

Concretely, the semantic questions to enumerate (from Phase 1c §"The conflation, named"):
- **Q1**: After full kernel expansion, does this form emit as a JS statement? (used for IIFE-vs-ternary)
- **Q2**: As the last body expression with `:returns :T`, can this form produce a value? (used for Q2=A diagnostic — but only *after* W-1's compile-then-check makes the question well-formed)
- **Q3**: Is this form a control-transfer that makes value-producing-ness moot? (`return`/`throw`/`break`/`continue`)
- **Q4**: When this form is a parent of children, what context does each child inherit? (the `KernelChildProfile` question, Rust-only today)

**W-3 also delivers D-6's piece** — if option A or B is chosen, the JS side gains the `ExprContext` / `KernelChildProfile` machinery as part of the unified source. If option C, those stay Rust-only and the architectural drift continues.

**Gating:** Cross-compiler tests cover every (form, semantic question) pair. Adding a new form to the language updates one place. Removing a form from one list while leaving it in another fails CI.

**Estimated complexity:** depends on option. A is multi-day. B is half-day. C is hours.

**Workstream W-3 closes D-4 and D-6.**

### Workstream W-4 — Documentation: align the four written surfaces

After W-1 and W-2 land, the compiler is correct against the *teaching* of all five docs surfaces. But several prose passages in the book and SKILL.md *describe* try as statement-only — that prose needs to be updated to reflect D-2.γ's reality.

**Subscope W-4a — Book Ch 9.5 (Exceptions) prose update.**

Update the opening from:

> `throw`, `try`, `catch`, `finally` are kernel forms with no surface transformation.

To something like:

> `throw`, `catch`, and `finally` are kernel forms. `try` exists at both layers: the kernel `try` is a JavaScript try-statement; surface `try` is position-aware — used as a statement, it emits a statement; used in expression position (the last expression of a function body, as the argument to a `bind`, etc.), it emits an IIFE-wrapped try that produces the value of the matched branch.

Plus add a section that shows the value-producing examples (referencing the patterns from Ch 17.4, Ch 25.2). Cross-link to Ch 17.4 for async use.

This prose update mirrors what Ch 9.1 will eventually need under DD-50's position-aware framing (already logged for book-drift thread).

**Subscope W-4b — SKILL.md line 446 update.**

From:

> **`try`/`catch`/`finally`**: kernel forms used directly in surface code. **MUST**

To:

> **`try`/`catch`/`finally`**: position-aware surface forms. In statement position they emit JavaScript try-statements; in expression position (last body expression of a `func`/`fn`, RHS of a `bind`, function argument) they IIFE-wrap and produce the value of the matched branch. **SHOULD** use as an expression for parse-and-validate patterns; **SHOULD** prefer `Result` types over exceptions for expected failures.

Plus add one example showing the expression form (parallel to the existing match-is-an-expression treatment at line 308).

**Subscope W-4c — `docs/guides/03-error-handling.md` ID-12 prose update.**

The patterns work after W-2; the prose is mostly correct already. Tighten the wording at ID-12 to explicitly call out that surface `try` is position-aware, so readers understand *why* `try-parse-json` works rather than thinking it's a magic exception.

**Subscope W-4d — `docs/guides/17-template-and-i18n.md` fence annotations.**

The D-3 fix. Change six fences from ` ```lykn ` to ` ```lykn,compile-fail ` at lines 144, 152, 160, 167, 174, 182. Done.

**Subscope W-4e — README.md table touch-up (optional).**

Line 365 table entry for `try` is fine as-is but a parenthetical note about position-awareness would be helpful for a casual reader scanning the surface-form summary. Low priority; doesn't block anything.

**Gating:** Every doc surface that *teaches* a try-as-expression pattern compiles cleanly. Every doc surface that *describes* try as statement-only is updated to the position-aware framing. Internal consistency across Ch 9.5 / Ch 17.4 / Ch 25.2 / etc.

**Estimated complexity:** primarily prose. Most of it is short focused edits. Subscope W-4a (book Ch 9.5) is the longest individual write.

### Workstream W-5 — Methodology / test discipline

**Goal:** Close the methodology gap that let D-1 ship past DD-50.6's stated Q4=A invariant.

**Deliverables:**

1. New rule for future DD-style design decisions involving form classification: every list must have a `compile-both` test that exercises the surface-vs-kernel boundary, not just the simple cases.
2. Add a `test/forms/cross-compiler-boundary_test.lykn` (or similar) that becomes the standing acceptance test for any "Q4=A" claims.
3. Document the new rule in `LEDGER_DISCIPLINE.md` or a new section in `docs/philosophy.md` covering compiler-parity discipline.

**Estimated complexity:** small. Mostly documenting and adding the standing test file. The actual tests are produced by W-1 and W-3.

---

## §3 — Sequencing and dependencies

```
W-3 (architecture: source of truth)
   ↓ enables
W-1 (JS Q3=C deeper fix) ── closes D-1, D-5
   ↓
W-2 (both compilers: position-aware try) ── closes D-2
   ↓
W-4 (docs/book/skill prose alignment) ── closes D-2 prose, D-3
W-5 (methodology) ── closes test-discipline gap

Independent of the chain:
W-4d (D-3 fence annotations) — can land any time, no dependencies
```

**Optimal landing order:**

1. **W-4d first.** Six fence-annotation edits, zero risk, immediate green for Class B's 6 tests.
2. **W-3 in parallel with W-4d** (Option B or C is fast enough). Establishes the source-of-truth pattern so W-1 and W-2 plug into it.
3. **W-1 next.** Closes D-1 (8 doctest failures) and D-5. Mycelium stays green throughout.
4. **W-2 after W-1.** Closes D-2 (the remaining doctest failures including the A2 pair). Note: W-2 requires W-1 first because W-1 establishes the JS-side Q3=C machinery that W-2's "is `try` in value position?" check rides on.
5. **W-4a, W-4b, W-4c last.** Prose updates after the compilers are correct. Update the book, the skill, and the error-handling guide to describe the new reality.
6. **W-5 in parallel or trailing.** Methodology discipline can land alongside or after.

**Total doctest failure resolution:**
- W-4d alone: 6/14 (Class B)
- W-1 alone: +6 (Class A1, but only 6 of A1's 6 — the bigger A1 plus A2 numbers in Phase 1a were against an earlier classification)
- W-2 alone: +2 (Class A2)
- All: 14/14 ✓

**Mycelium stays green throughout** because mycelium uses none of the affected patterns today.

---

## §4 — Open Phase 3 sub-calls

A few sub-decisions remain to fully scope the plan:

**§4.1 — W-3 option A vs B vs C.** Lykn-side toml source-of-truth, test-fixture source-of-truth, or just structural test coverage. Recommend B for 0.6.x (lower tooling weight, fast win), A as a 0.7+ aspiration.

**§4.2 — Is W-2's position-aware `try` shipped in 0.6.0, or 0.6.x patch, or 0.7?** Per the "ship at least minimum-fix" framing, W-1 + W-4d alone (closing D-1 + D-3) is enough to make all 14 tests pass *if* W-4c restructures the A2 patterns to bind-first work-arounds. But W-2 is the right long-term answer; the question is timing. Recommend: ship W-2 in 0.6.x (not 0.6.0) so the architectural work has room to breathe.

**§4.3 — Ch 9.1 update.** Triggers a separate book-drift item to update for DD-50 position-aware `if`. Same shape as W-4a. Confirm with Duncan whether to fold into the same prose pass or hand to the book-drift thread.

**§4.4 — Should the JS compiler eventually be deprecated (D-6 long-term)?** Out of scope for this synthesis, but the W-1 + W-3 work makes the JS compiler more maintainable in the meantime. Worth flagging that "actually keep parity forever" vs "phase out JS compiler post-bootstrap" is the strategic background question.

---

## §5 — Summary table

| Workstream | Closes | Surfaces touched | Estimated effort | Gating |
|---|---|---|---|---|
| W-1 | D-1, D-5 | JS compiler (`surface.js`, `compiler.js`), tests | Days | 6 A1 failures green; surface-vs-kernel test corpus |
| W-2 | D-2 | Rust compiler (`forms.rs`, `emit.rs`), JS compiler (`compiler.js`), tests | Days | 2 A2 failures green; try-as-expr tests on both compilers |
| W-3 | D-4, D-6 | Both compilers + tests + (optionally) build system | Hours (option B) to days (option A) | Form list drift fails CI |
| W-4a | D-2 prose | Book Ch 9.5 | Hours | Internal consistency with Ch 17.4 / 25.2 |
| W-4b | D-2 prose | SKILL.md | Hours | Internal consistency with W-4a |
| W-4c | D-2 prose | docs/guides/03-error-handling.md ID-12 | Hours | Tightens existing prose |
| W-4d | D-3 | docs/guides/17-template-and-i18n.md | Minutes | 6 B failures green |
| W-4e (optional) | D-2 prose | README.md | Minutes | Surface-form table accuracy |
| W-5 | Methodology | LEDGER_DISCIPLINE.md, test/forms/, philosophy.md | Hours | Future DD-style fixes can't reproduce the D-1 / D-4 drift |

---

## §6 — What this plan deliberately doesn't cover

- **Ch 9.1 (Conditionals) update for DD-50 framing** — already in book-drift thread; W-4a establishes the prose pattern.
- **JS-side `ExprContext`/`KernelChildProfile` full port** — W-3 option A delivers this; W-3 option B or C doesn't. Per Duncan's "deeper fix" call, lean A. But it's a 0.7+ candidate, not a 0.6.x blocker.
- **Mycelium fixes** — mycelium isn't broken; W-1 and W-2 will be regression-tested against it but no mycelium source needs to change.
- **Other guides (00, 01, 02, 04, 05, 10, 12-16, 18+)** — audited for D-1/D-2/D-3 patterns; none found. They don't need changes for this plan.
- **Tests in the dev/macro-expansion suite** — not yet audited for their classification assumptions. Phase-3-implementation item: when W-1 lands, sanity-check that the macro-expansion tests don't break (they might assume the old surface-form classification).

---

## §7 — Phase 3 is decided. Phase 4 = implementation prompts.

After this synthesis (and any final Phase 3 sub-calls Duncan makes on §4), each workstream gets a CC implementation prompt in the established two-turn protocol. Suggested ordering:

- **W-4d prompt (or just-do-it)** — five-minute edit, doesn't need protocol weight.
- **W-3 option call + prompt** — needs Duncan's option pick first, then a clean prompt with Deliverables 1–4 per the dep-ergo thread's established discipline.
- **W-1 prompt** — full two-turn protocol; biggest architectural piece.
- **W-2 prompt** — full two-turn protocol; closely tracks DD-50.7's template.
- **W-4a/b/c prompts** — prose edits, light protocol.
- **W-5 prompt** — methodology doc, light protocol.

End state of the synthesis: a sequence of implementation prompts that, when run in order, take lykn from "14 failing tests + multiple silent doc divergences" to "everything green, everything internally consistent, methodology discipline strengthened, language is now properly Lisp/Haskell/Rust-family for `try`."

This is a 0.6.x-scope plan. The piece reaching into 0.7+ (W-3 option A, W-1's full architectural port) can wait — minimum-viable 0.6.x close requires only W-1 enough to fix D-1, W-2 enough to fix D-2, W-4d for D-3, plus prose alignment.
