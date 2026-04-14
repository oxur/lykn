---
number: 32
title: "DD-23: `set!` — External Property Mutation"
author: "an explicit"
component: All
tags: [change-me]
created: 2026-04-13
updated: 2026-04-13
state: Final
supersedes: null
superseded-by: null
version: 1.0
---

# DD-23: `set!` — External Property Mutation

**Status**: Decided
**Date**: 2026-04-13
**Amends**: DD-15 (Language Architecture), DD-22 (Surface Equality)
**Release**: v0.4.0

## Summary

`set!` is a new surface form for mutating properties on objects the
programmer does not own — DOM elements, platform APIs, third-party
library objects. It requires a colon-syntax property path in the
target position and emits plain JS assignment. `set!` on a cell or
a bare binding is a compile error. This completes the mutation model
that DD-22 opened by removing `=` as assignment from the surface
language.

## Problem

DD-22 repurposed `=` as strict equality in surface syntax. This
eliminated the surface-level assignment operator — correctly, since
`bind`, `reset!`, `swap!`, and `assoc`/`dissoc`/`conj` cover all
lykn-native mutation patterns.

However, one legitimate mutation case was left without a surface form:
**property assignment on external objects**. The most common example
is DOM property mutation:

```js
// This JS has no surface-language equivalent after DD-22
el.textContent = "Hello from lykn!";
canvas.width = 800;
style.display = "none";
audioNode.gain.value = 0.5;
```

These are not cells (no `{ value: x }` wrapper), not new bindings
(the object already exists), and not immutable updates (you cannot
`assoc` a DOM element). The kernel `=` is no longer reachable through
the surface compiler because DD-22 intercepts it as equality.

Without `set!`, the only option is the `js:` escape hatch, which is
poor ergonomics for DOM-heavy code.

## Decisions

### 1. `set!` assigns a property on an existing object

**Decision**: `(set! target:property value)` compiles to
`target.property = value`. The target must use colon syntax
(a member expression).

**Syntax**:

```lisp
(set! el:text-content "Hello from lykn!")
```

```javascript
el.textContent = "Hello from lykn!";
```

```lisp
(set! canvas:width 800)
```

```javascript
canvas.width = 800;
```

```lisp
;; Chained property access
(set! ctx:shadow-color "rgba(0,0,0,0.5)")
```

```javascript
ctx.shadowColor = "rgba(0,0,0,0.5)";
```

```lisp
;; Nested property path
(set! audio-node:gain:value 0.5)
```

```javascript
audioNode.gain.value = 0.5;
```

**ESTree nodes**: `ExpressionStatement` containing
`AssignmentExpression` (`=`) with a `MemberExpression` left-hand side.

**Rationale**: The `!` suffix follows the mutation convention
established by `swap!` and `reset!`. Every mutation site in surface
lykn is marked with `!` and is greppable. `set!` is honest about what
it does: mutate a property on an existing object.

### 2. `set!` on a bare binding is a compile error

**Decision**: The target of `set!` must be a colon-syntax member
expression. Bare identifiers are rejected.

**Syntax**:

```lisp
;; Compile error — bare binding
(set! x 1)
;; Error: set! requires a property path (e.g., obj:prop), not a bare binding.
;; Use (bind x 1) for new bindings, (reset! x val) for cells.

;; Compile error — bare identifier
(set! counter 0)
;; Error: set! requires a property path. Use (reset! counter 0) for cells.
```

**Rationale**: `set!` is not a back door to variable reassignment.
Variable binding is `bind`. Cell update is `reset!`/`swap!`. `set!`
is exclusively for property mutation on existing objects. Requiring
colon syntax in the target enforces this structurally.

### 3. `set!` on a cell is a compile error

**Decision**: If the compiler can determine that the target binding
is a cell (created via `(bind name (cell ...))`), `set!` is rejected
with a clear error message.

**Syntax**:

```lisp
(bind counter (cell 0))

;; Compile error — target is a cell
(set! counter:value 1)
;; Error: 'counter' is a cell. Use (reset! counter 1) or (swap! counter f).
;; (set! counter:value ...) would bypass cell semantics.
```

**Rationale**: `(set! counter:value 1)` would technically produce
correct JS (`counter.value = 1`), but it bypasses the cell protocol.
`reset!` is the correct form — it signals "this is a cell update" and
is consistent with the `!` mutation convention. Allowing both `set!`
and `reset!` on cells creates two paths for the same operation,
defeating the auditability goal.

**Scope of detection**: The compiler checks the local scope for `bind`
+ `cell` patterns. It does NOT trace cells across module boundaries
or through function returns. The check catches the common case; edge
cases fall through to working (but non-idiomatic) code.

### 4. Computed property assignment requires `js:` escape hatch

**Decision**: `set!` does not support computed (dynamic) property
targets. For `obj[key] = value`, use the `js:` escape hatch or
kernel passthrough.

**Syntax**:

```lisp
;; set! — static property path only
(set! el:text-content "Hello")    ;; ✓

;; Computed — use kernel = or js: escape hatch
;; (= (get obj key) value)        ;; kernel passthrough
```

**Rationale**: Static property paths cover the vast majority of DOM
and platform API usage. Computed property assignment is rare in
surface lykn and better served by an explicit escape hatch that
signals "I'm doing something unusual."

### 5. Complete mutation model summary

| What you're mutating | Form | Compiled output | Notes |
|---|---|---|---|
| New binding | `bind` | `const x = ...` | Not mutation — creates binding |
| Cell value (replace) | `reset!` | `counter.value = ...` | Cell protocol |
| Cell value (update) | `swap!` | `counter.value = f(counter.value)` | Cell protocol |
| Object data (new copy) | `assoc`/`dissoc`/`conj` | `{...obj, key: val}` | Not mutation — creates new value |
| External object property | `set!` | `el.textContent = ...` | Colon-syntax target required |
| Dynamic/computed property | `js:` or kernel | `obj[key] = ...` | Escape hatch |

**The `!` convention holds**: Every form that mutates existing state
has a `!` suffix (`set!`, `reset!`, `swap!`). Forms that create new
values (`bind`, `assoc`, `dissoc`, `conj`) do not.

## Rejected Alternatives

### Extend `reset!` to work on any property, not just cells

**What**: `(reset! el:text-content "Hello")` would detect that the
target isn't a cell and emit plain assignment.

**Why rejected**: Muddies `reset!`'s semantics. `reset!` means
"replace a cell's value" — making it polymorphic between cells and
arbitrary properties is confusing. A developer reading `reset!` should
know exactly what data model is in play (cell), not have to determine
whether the target is a cell or a DOM element.

### Allow `set!` on bare bindings (general-purpose assignment)

**What**: `(set! x 1)` compiles to `x = 1`, providing a surface-level
assignment operator alongside `=` as equality.

**Why rejected**: Reintroduces the assignment hazard that DD-22
eliminated. Surface lykn has no mutable bindings — `bind` is `const`.
If you need to change a value over time, use `cell`. A general-purpose
`set!` would let developers bypass the `cell` model entirely.

### Use `js:=` for all property assignment

**What**: `(js:= el:text-content "Hello")` — use the escape hatch.

**Why rejected**: DOM manipulation is not an edge case. In browser
code, property assignment is one of the most common operations.
Requiring `js:` for every DOM mutation is poor ergonomics and signals
"you're doing something unusual" when in fact you're doing something
routine.

### Allow `set!` with computed targets via `get`

**What**: `(set! (get obj key) value)` compiles to `obj[key] = value`.

**Why rejected**: Overly permissive. Computed property assignment is
rare and better served by the `js:` escape hatch. Restricting `set!`
to colon-syntax targets keeps it tightly scoped and auditable. Can be
revisited if real-world usage reveals sufficient demand.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| Bare binding target | Compile error | `(set! x 1)` → error |
| Cell target (known) | Compile error | `(set! counter:value 1)` → error, use `reset!` |
| Cell target (unknown) | Passes — produces `counter.value = 1` | Cell from another module |
| Chained property path | Allowed | `(set! a:b:c 1)` → `a.b.c = 1` |
| Private field via `-` | Allowed (in class bodies) | `(set! this:-count 1)` → `this.#_count = 1` |
| Computed target | Not supported | Use kernel or `js:` |
| Return value of `set!` | `undefined` (statement, not expression) | Cannot use as expression |

## Implementation

`set!` is a surface form macro. In the JS surface compiler
(`src/surface.js`), add a case for `set!` that:

1. Checks the target is a colon-syntax member expression (not a bare
   symbol)
2. Checks the target binding is not a known cell
3. Emits kernel `=` (assignment) with the member expression as the
   left-hand side

In the Rust surface compiler, add a surface form handler in the
classifier and emitter that performs the same checks and emits the
kernel assignment s-expression.

**No kernel changes.** The kernel `=` (assignment) is unchanged.
`set!` simply provides a surface-level entry point to it with
compile-time constraints.

Estimated scope: ~20 lines per compiler. The compile-error checks
are the bulk of the work.

## Testing

### New test fixtures

Add `test/fixtures/surface/set.json`:

```json
[
  { "input": "(set! el:text-content \"hello\")",
    "output": "el.textContent = \"hello\";\n" },
  { "input": "(set! canvas:width 800)",
    "output": "canvas.width = 800;\n" },
  { "input": "(set! a:b:c 1)",
    "output": "a.b.c = 1;\n" },
  { "input": "(set! this:-count 0)",
    "output": "this.#_count = 0;\n" }
]
```

### Error tests

- `(set! x 1)` → compile error: bare binding
- `(bind c (cell 0)) (set! c:value 1)` → compile error: cell target
- `(set! (get obj key) 1)` → compile error: computed target

### Regression tests

- `reset!` still works on cells
- `swap!` still works on cells
- `=` still compiles to `===` (DD-22 unchanged)
- Kernel `=` in class bodies still emits assignment
- `for` loop counters still work (kernel `=`, `++`, `+=`)

## Dependencies

- **Depends on**: DD-15 (language architecture — mutation convention),
  DD-22 (surface equality — `=` is no longer assignment)
- **Affects**: DD-22 (completes the mutation model), all guides and
  SKILL.md (documentation updates below), README (browser example)

## Guide and Documentation Updates

The following documents must be updated after `set!` is implemented.

### README.md

Update the browser `<script>` example:

```lisp
;; Before (broken after DD-22)
(= el:text-content "Hello from lykn!")

;; After
(set! el:text-content "Hello from lykn!")
```

Add `set!` to the "Surface forms — Bindings & mutation" table:

```
| `(set! el:prop value)` | `el.prop = value;` |
```

### Guide 00: `00-lykn-surface-forms.md`

Add a `set!` entry in the "Bindings & Mutation" section after
`reset!`:

```markdown
### set! (surface) — external property mutation

Assign a value to a property on an existing object. The target must
use colon syntax. Intended for DOM elements, platform APIs, and
third-party library objects — not for lykn-owned data.

‎```lykn
(set! el:text-content "Hello from lykn!")
(set! canvas:width 800)
(set! style:display "none")
‎```
‎```js
el.textContent = "Hello from lykn!";
canvas.width = 800;
style.display = "none";
‎```

`set!` on a bare binding or a known cell is a compile error.
Use `bind` for new bindings, `reset!` for cell updates.
```

Add `set!` to the Assignment kernel section note:

```
> Surface code uses `set!` for property mutation, `reset!` for cells,
> and `bind` for new bindings. The kernel `=` (assignment) is used
> internally by these forms and in kernel passthrough for classes
> and `for` loops.
```

### Guide 01: `01-core-idioms.md`

Update ID-01 (bind by default) to mention `set!` in the mutation
strategy table:

```
| External property mutation | `set!` | DOM, canvas, platform APIs |
```

Update ID-02 (= means equality) to note that property assignment
uses `set!`:

```
For property assignment on external objects (DOM, platform APIs),
use `set!`: `(set! el:text-content "Hello")`. See DD-23.
```

### Guide 04: `04-values-references.md`

Add a note in ID-05 (no reassignment) or a new entry explaining
that `set!` provides property mutation for external objects while
maintaining the "no reassignment" principle for bindings.

### Guide 09: `09-anti-patterns.md`

Add a new entry in the lykn-specific anti-patterns section:

```markdown
## ID-42: Using `set!` on lykn-Owned Data

**Strength**: SHOULD-AVOID

**Summary**: `set!` is for external objects (DOM, platform APIs).
For lykn-owned data, use `assoc` (immutable update) or `cell` +
`reset!` (controlled mutation).

‎```lykn
;; Anti-pattern — set! on your own objects
(bind user (obj :name "Alice"))
(set! user:name "Bob")        ;; works but bypasses immutable model

;; Fix — immutable update
(bind updated (assoc user :name "Bob"))

;; Fix — cell for state
(bind user (cell (obj :name "Alice")))
(swap! user (fn (:object u) (assoc u :name "Bob")))
‎```
```

### SKILL.md

Add `set!` to the Bindings & Mutation section bullet list:

```
- **`set!` for external property mutation**: `(set! el:text-content "Hello")`.
  For DOM, canvas, platform APIs. NOT for lykn-owned data. **SHOULD** use
  only for objects you don't own.
```

Add to the anti-patterns table:

```
| Using `set!` on lykn-owned data | Use `assoc` for immutable update
  or `cell` + `reset!` for controlled mutation |
```

Update the mutation model summary in the Kernel vs Surface section
or add a mutation reference table.

Update the Document Selection Guide to include Guide 15:

```
| **lykn CLI** | `docs/guides/15-lykn-cli.md` |
```

### CC Prompt Template

Add `set!` to the syntax translation table:

```
| `el.textContent = "x"` | `(set! el:text-content "x")` |
  Property mutation on external objects |
```

## Open Questions

None.
