# DD-25 Phase 5: Book Update — Chapter 15, Section 3

## Context

Book Chapter 15 section 3 (`/Users/oubiwann/lab/cnbb/lykn/src/part3/chapter15/3-parameter-destructuring.md`) currently documents that surface `func` doesn't support destructured params and recommends body destructuring as a workaround. With DD-25 implemented, this section needs updating to show the clean surface syntax.

---

## Milestone 1: Rewrite section 3 content

**File**: `/Users/oubiwann/lab/cnbb/lykn/src/part3/chapter15/3-parameter-destructuring.md`

### 1.1 — Remove "Why Not `func`?" and "The Surface Alternative" sections

These sections (lines 32-51) explain the old limitation. They should be replaced with the new syntax.

### 1.2 — Add surface `func` destructuring examples

Replace with content showing the new surface syntax:

**Object destructuring in `func`:**
```lisp
(func process
  :args ((object :string name :number age) :string action)
  :returns :string
  :body (template name " (" age ") — " action))
```
→
```javascript
function process({name, age}, action) {
  if (typeof name !== "string")
    throw new TypeError("process: arg 'name' expected string, got " + typeof name);
  if (typeof age !== "number" || Number.isNaN(age))
    throw new TypeError("process: arg 'age' expected number, got " + typeof age);
  if (typeof action !== "string")
    throw new TypeError("process: arg 'action' expected string, got " + typeof action);
  return `${name} (${age}) — ${action}`;
}
```

**Array destructuring in `func`:**
```lisp
(func head-tail
  :args ((array :number first (rest :number remaining)))
  :body (console:log first remaining))
```

**`fn` with destructured params:**
```lisp
(bind f (fn ((object :string name :number age))
  (console:log name age)))
```

### 1.3 — Explain the typing rule

Every field inside a destructuring pattern requires a type keyword. `:any` opts out of the type check for that field. This matches the surface principle: "if you name it in `:args`, you type it."

### 1.4 — Mention what's deferred

Brief note that nested destructuring and `default` in destructured fields are designed but deferred — the compiler provides helpful error messages pointing to workarounds.

### 1.5 — Keep kernel examples

The existing kernel function examples (lines 1-30) are still valid — kernel forms support destructuring independently. Keep them as-is, or note that surface `func` now makes these unnecessary for most use cases.

### 1.6 — Update the body-destructuring section

Reframe it as "an alternative approach" rather than "the recommended workaround." Both are valid; surface destructuring is now the primary path.

---

## Milestone 2: Verify book examples compile

### 2.1 — Test each lykn code example

Run every code snippet from the updated section through the compiler:

```bash
# Each example should compile without error
echo '(func process :args ((object :string name :number age) :string action) :returns :string :body (template name " (" age ") — " action))' | deno run src/index.js
```

### 2.2 — Verify compiled JS matches shown output

The JavaScript output in the book must match what the compiler actually produces. No hand-edited JS.

---

## Milestone 3: Check cross-references

### 3.1 — Section 1 (object destructuring)

`/Users/oubiwann/lab/cnbb/lykn/src/part3/chapter15/1-object-destructuring.md`

Check if it references surface `func` limitations. If so, update or add a forward reference to section 3.

### 3.2 — Section 4 (spread/rest)

`/Users/oubiwann/lab/cnbb/lykn/src/part3/chapter15/4-spread-rest.md`

If it discusses rest parameters, add a note about `(rest :type name)` in array destructuring.

### 3.3 — Section 6 (edge cases)

`/Users/oubiwann/lab/cnbb/lykn/src/part3/chapter15/6-edge-cases.md`

Add or update edge cases:
- Empty destructuring pattern → error
- `:any` field in destructured param → no type check
- Nested destructuring → deferred with helpful error
- Multi-clause overlap with destructured params

### 3.4 — Closing section

`/Users/oubiwann/lab/cnbb/lykn/src/part3/chapter15/7-closing.md`

Update if it references the surface `func` limitation as something to look forward to.

---

## Files modified

| File | Change |
|------|--------|
| `/Users/oubiwann/lab/cnbb/lykn/src/part3/chapter15/3-parameter-destructuring.md` | Major rewrite — show new surface syntax, remove limitation notes |
| `/Users/oubiwann/lab/cnbb/lykn/src/part3/chapter15/6-edge-cases.md` | Add destructuring edge cases (if applicable) |
| `/Users/oubiwann/lab/cnbb/lykn/src/part3/chapter15/7-closing.md` | Update if needed |
