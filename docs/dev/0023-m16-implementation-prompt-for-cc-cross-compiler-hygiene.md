# M16 Implementation Prompt for CC — Cross-Compiler Hygiene

## Read this first

Your milestone is M16. The spec is at
`workbench/milestones/M16-cross-compiler-hygiene-ledger.md`. That
file is canonical — every acceptance criterion is enumerated there
with a grep-verifiable Verify command. **Work against the ledger.
Do not invent additional scope; do not silently drop ledger items.**

The thread that produced this milestone is documented at
`workbench/2026-05-10-compiler-coherence-thread-opening.md`. The
five substantive rows (M16-2 through M16-6) come from the Track B
list in that thread's Resolutions section.

---

## MUST framing — what you MUST and MUST NOT do

- **You MUST load `assets/ai/LEDGER_DISCIPLINE.md` before writing
  any code** and follow its CC protocol throughout. The protocol's
  named failure mode is *compliance theatre* — paper compliance
  exceeding observed compliance. The per-row walk in the closing
  report is the structural protection against that failure mode.
  Do not write a prose summary; walk each row with evidence.
- **You MUST follow the subagent delegation policy.** Per
  `assets/ai/SUBAGENT-DELEGATION-POLICY.md`: subagents are for
  **lookup-only work** (grepping, finding call sites, listing
  files). Design decisions, refactor judgment, prose-writing,
  evaluation-of-correctness, and any work requiring judgment about
  the result stay in main CC context. **You MUST NOT delegate
  thinking work to subagents** for any row of this milestone.
- **You MUST stop and surface on dissonance.** If a ledger
  criterion is wrong, impossible, or supersedable, raise it as an
  amendment request. Do not silently work around it.
  (LEDGER_DISCIPLINE §CC protocol point 2.)
- **You MUST NOT auto-pass safety-bypass flags** to underlying
  tools per `assets/ai/CLAUDE.md` "Lykn CLI safety gates." This
  rule continues to apply throughout M16.

---

## Required reading (before writing any code)

1. `assets/ai/LEDGER_DISCIPLINE.md` — the protocol.
2. `assets/ai/SUBAGENT-DELEGATION-POLICY.md` — subagent rules.
3. `assets/ai/CLAUDE.md` "Lykn CLI safety gates" section.
4. **The M16 ledger itself** — `workbench/milestones/M16-cross-compiler-hygiene-ledger.md`.
5. The source materials listed in the ledger's "Source materials"
   section (read in the order given there).

---

## Per-row preflights

The ledger's "CC instructions" section names the order of work and
the anti-shortcut rules. Two rows benefit from explicit preflight
discipline beyond what the ledger states:

### M16-5 preflight — Value-override refactor diagnosis BEFORE refactor

The line-356 blanket override is the dispatch site; nine emitters
currently use save/restore workarounds (DD-50.7 Cluster 2: lines
1254, 1319, 1539, 1715, 1760, 1881, 1969, 2284, 2334 of
`crates/lykn-lang/src/emitter/forms.rs` pre-refactor).

**Before changing any code:**

1. For each of those nine emitters, record (in a workbench file —
   `workbench/verify/m16/value-override-audit.md`) the body
   context the emitter requires for its children — Statement or
   Value. The current save/restore exists precisely because the
   blanket override would otherwise inject the wrong context.
2. Run `cargo test -p lykn-lang -- dd_50` and `make test-lykn`;
   capture the pass-state.
3. **Then** make the refactor. Each emitter that needed save/restore
   now sets its own child context explicitly. The blanket override
   is removed.
4. Re-run the test commands; every test that passed must still pass.

If any emitter's required context isn't structurally obvious from
the existing save/restore site, stop and surface to CDC before
proceeding.

### M16-6 preflight — Direction-call gate BEFORE implementation

The ledger names the direction-call (auto-resolve vs require-explicit-
path). The default proposal is direction (a) — align JS to Rust's
`find_macro_entry` auto-resolve behaviour.

**Before changing either compiler's behaviour:** write a short
diagnosis (a paragraph or two in your closing-report's working
notes) confirming direction (a) is the right call, OR surfacing
structural reason to prefer direction (b). CDC reviews the
diagnosis before implementation lands.

---

## Iteration budget

**5 iterations.** Expected 2–3. If you reach iteration 5 without
convergence, **stop** — do not iterate a sixth time on the same
ledger in the same session. Options at that point:

- (a) Rework the milestone scope (most likely).
- (b) Start a new CC session with fresh context.
- (c) Escalate to a methodology review.

---

## Closing report requirements

When all rows reach final status, produce a closing report at
`workbench/2026-05-<date>-M16-closing-report.md`. The closing
report **MUST**:

1. Walk every ledger row by ID. For each row, state the final
   status (`done` / `deferred` / `no-op`) and the evidence (commit
   SHA + Verify command output). **No prose summaries.** No "all
   rows complete" without per-row walks.
2. Include a "Substrate-rule compliance" section addressing the
   six starter rules (CLAUDE.md safety gates, LEDGER_DISCIPLINE
   no-silent-rewrite, philosophy.md Principle 1, philosophy.md
   Principle 3, spec-softening check, partial-adoption check).
3. Include a "Findings for fast-follow" section logging any
   divergences M16-2 surfaced that weren't closed within the
   milestone, with disposition rationale.
4. Include a "What Worked" section per LEDGER_DISCIPLINE — Safety-II
   complement to the defect ledger.
5. Name any uncertainty explicitly. "Done with caveat X" is stronger
   than confident "done" that turns out softpedalled.

---

## What you do NOT need to do

- You do not need to redo DD-37 promotion work — it's at
  `docs/design/05-active/0047-*.md` with amendments already
  applied on the `cdc/compiler-coherence` branch.
- You do not need to scope DD-58 — that's parallel work in CDC's
  main context.
- You do not need to fix `emit_if_iife`'s separate latent bugs
  (DD-50.7 fast-follow #3) — that's deferred separately.
- You do not need to address the 8 remaining Class A1/A2 doctest
  failures from the cdc/dep-ergonomics Phase 1a triage — those
  resolve via DD-58, not via M16.

---

## Start

Once you've read the required materials, begin with row M16-1
(baseline capture), then proceed per the ledger's "Order of work"
in its CC instructions section. Surface anything that looks off
before working around it.
