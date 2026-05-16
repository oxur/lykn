# If-Profile Audit Pattern for Special-Case Intercepts

**Number:** 0024
**Origin:** DD-50.7 CDC review Finding #1
**Date:** 2026-05-15

## The Pattern

When a kernel form has a **special-case context intercept** in
`emit_expr` — meaning the form is handled by a dedicated code path
in certain ExprContext values instead of going through the standard
`kernel_child_profile` dispatch — the profile assigned to that form
for the *non-intercepted* context paths **MUST** be independently
audited for correctness.

The canonical example is `if`:

- **Intercepted path:** When `ctx.expr_context` is `Value` or `Tail`,
  `emit_expr` dispatches to `emit_if_expression` (DD-50), which
  produces a ternary or IIFE wrapper. The kernel child profile is
  **not consulted** on this path.
- **Non-intercepted path:** When `ctx.expr_context` is `Statement`,
  `emit_expr` falls through to the standard kernel loop, which
  applies `kernel_child_profile("if")` to determine child contexts.

The bug DD-50.7 found: `if`'s profile was `AllValue` (all children
emitted in Value context), but the correct profile for the Statement
path is `Positional(&[V, S, S])` — condition=Value, then=Statement,
else=Statement. The intercepted path masked the incorrect profile
because Value/Tail `if` never reached the standard kernel loop.

## When to Apply This Audit

Apply this audit whenever:

1. A **new special-case intercept** is added to `emit_expr` for a
   kernel form (a `head_name == "..."` check that bypasses the
   standard profile dispatch).
2. A **profile is changed** for a form that has an existing intercept.
3. A **new ExprContext variant** is added that might change which path
   a form takes.

## How to Apply

For each form with a special-case intercept:

1. **Identify the intercepted contexts.** Which `ExprContext` values
   trigger the special-case path?
2. **Identify the non-intercepted contexts.** These are the contexts
   where the form falls through to `kernel_child_profile`.
3. **Verify the profile is correct for the non-intercepted path.**
   For each child position, confirm the assigned context (Value vs
   Statement) matches the JS semantics of that position when the
   form is used in the non-intercepted context.
4. **Add or verify a test** that exercises the form in the
   non-intercepted context. The test must fail if the profile is
   wrong (i.e., it must assert on emitted JS structure, not just
   "compiles without error").

## Current Intercept Inventory

See `workbench/verify/m16/kernel-profile-audit.md` for the M16
application of this pattern.
