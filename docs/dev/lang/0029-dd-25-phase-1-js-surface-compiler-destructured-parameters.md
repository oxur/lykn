# DD-25 Phase 1: JS Surface Compiler — Destructured Parameters

## Context

Book Chapter 15 section 3 (`/Users/oubiwann/lab/cnbb/lykn/src/part3/chapter15/3-parameter-destructuring.md`) currently documents that surface `func` doesn't support destructured params and recommends body destructuring as a workaround. This phase adds destructured parameter support to the JS surface compiler, producing canonical test fixtures that the Rust implementation (Phase 2-3) will match.

All changes are in `src/surface.js`. Three new functions, updates to three existing call sites, plus tests and fixtures.

---

## Milestone 1: `parseDestructuredParam` function

**Goal**: Parse a list node starting with `object` or `array` into a destructured param descriptor.

**File**: `src/surface.js` — insert after `parseTypedParams` (after line 232)

### 1.1 — Object destructuring parser

Add `parseDestructuredParam(listNode)`:

```javascript
function parseDestructuredParam(listNode) {
    const values = listNode.values;
    if (values.length === 0) {
        throw new Error("empty destructuring pattern — at least one field required");
    }
    const head = values[0];
    if (!isAtom(head) || (head.value !== "object" && head.value !== "array")) {
        throw new Error(`expected 'object' or 'array' at head of destructuring pattern, got '${head.value}'`);
    }
    const kind = head.value; // "object" or "array"
    // ... dispatch to object vs array parsing
}
```

For `object` kind:
- Iterate `values[1..]` as `:type name` pairs (same alternation as `parseTypedParams`)
- Each pair: validate keyword at even position, atom at odd position
- **Deferred feature detection**:
  - If `values[i]` is a list starting with `default` → throw: `"default values in destructured params are not yet supported — use a typed param with body destructuring and default"`
  - If `values[i+1]` (name position) is a list starting with `object` or `array` → throw: `"nested destructuring in func/fn params is not yet supported — use a typed param with body destructuring"`
- If bare name without preceding keyword → throw: `"field 'x' missing type annotation (use :any to opt out)"`
- Return: `{ destructured: true, kind: "object", fields: [{typeKw, name}, ...] }`

### 1.2 — Array destructuring parser

For `array` kind, iterate `values[1..]` with variable-step logic:
- If `values[i]` is a keyword → `:type name` pair (step 2)
- If `values[i]` is a list starting with `rest` → parse `(rest :type name)` → `{ rest: true, typeKw, name }` (step 1). Must be last element.
- If `values[i]` is atom `_` → skip element (step 1)
- **Deferred features**: same detection as object (nested, default)
- Return: `{ destructured: true, kind: "array", fields: [{typeKw, name}, ...], rest: {typeKw, name} | null, skips: [indices...] }`

### 1.3 — Edge case validation

In `parseDestructuredParam`:
- Empty pattern `(object)` or `(array)` → `"empty destructuring pattern — at least one field required"`
- `(rest ...)` not at end of array → `"rest element must be last in array destructuring"`
- Multiple rest elements → error

### Verification

- Unit test each error message
- Unit test successful parse of object and array patterns
- No compilation yet — just parsing

---

## Milestone 2: Update `parseTypedParams` to variable-step

**Goal**: Make `parseTypedParams` accept both simple `:type name` pairs and destructuring pattern lists.

**File**: `src/surface.js`, lines 217-232

### 2.1 — Change loop from fixed step-2 to variable-step

Current code uses `i += 2` unconditionally. Change to:

```javascript
function parseTypedParams(paramList) {
    const params = [];
    const values = paramList.values;
    let i = 0;
    while (i < values.length) {
        if (isArray(values[i])) {
            // Destructured param — the list IS the param (no preceding type keyword)
            params.push(parseDestructuredParam(values[i]));
            i += 1; // step 1: the list consumed one position
        } else if (isKeyword(values[i])) {
            // Simple param — :type name pair
            if (i + 1 >= values.length) {
                throw new Error(`type keyword :${values[i].value} has no parameter name`);
            }
            params.push({ typeKw: values[i], name: values[i + 1] });
            i += 2; // step 2: keyword + name
        } else {
            throw new Error(
                `expected type keyword or destructuring pattern at position ${i}, got ${values[i]?.type ?? "nothing"}`
            );
        }
    }
    return params;
}
```

### 2.2 — Return type now mixed

`parseTypedParams` returns `Array<SimpleParam | DestructuredParam>`:
- Simple: `{ typeKw, name }` (no `destructured` property)
- Destructured: `{ destructured: true, kind, fields, ... }`

Downstream code must check `p.destructured` to distinguish.

### Verification

- Parse `(:string name :number age)` → two simple params (regression)
- Parse `((object :string name :number age) :string action)` → one destructured + one simple
- Parse `((array :number first (rest :number remaining)))` → one destructured with rest
- All existing `func` and `fn` tests must still pass

---

## Milestone 3: Helper functions `paramToKernel` and `paramTypeChecks`

**Goal**: Bridge destructured params to kernel emission without modifying `buildTypeCheck`.

**File**: `src/surface.js` — insert after `parseDestructuredParam`

### 3.1 — `paramNames(p)` helper

Returns the kernel param node(s) for the function signature:

```javascript
function paramNames(p) {
    if (p.destructured) {
        if (p.kind === "object") {
            // Kernel form: (object name1 name2 ...)
            return [array(sym("object"), ...p.fields.map(f => f.name))];
        }
        if (p.kind === "array") {
            // Kernel form: (array name1 name2 ...) or (array name1 _ name2 (rest remaining))
            const elems = [];
            for (const f of p.fields) {
                elems.push(f.name);
            }
            // Insert skips as _ atoms at correct positions
            // Handle rest: (rest name)
            if (p.rest) {
                elems.push(array(sym("rest"), p.rest.name));
            }
            return [array(sym("array"), ...elems)];
        }
    }
    return [p.name]; // simple param — just the name node
}
```

This produces the kernel destructuring patterns that DD-06 already handles.

### 3.2 — `paramTypeChecks(p, funcName)` helper

Returns type check assertions for a param:

```javascript
function paramTypeChecks(p, funcName) {
    if (p.destructured) {
        // Each field gets its own type check
        const checks = [];
        const allFields = [...p.fields, ...(p.rest ? [p.rest] : [])];
        for (const f of allFields) {
            const check = buildTypeCheck(f.name, f.typeKw, funcName, "arg");
            if (check) checks.push(check);
        }
        return checks;
    }
    // Simple param
    const check = buildTypeCheck(p.name, p.typeKw, funcName, "arg");
    return check ? [check] : [];
}
```

### 3.3 — `paramDispatchType(p)` helper

For multi-clause dispatch:

```javascript
function paramDispatchType(p) {
    if (p.destructured) {
        return p.kind; // "object" or "array"
    }
    return p.typeKw.value; // the actual type keyword
}
```

### 3.4 — `paramBoundNames(p)` helper

For scope tracking / name extraction:

```javascript
function paramBoundNames(p) {
    if (p.destructured) {
        const names = p.fields.map(f => f.name);
        if (p.rest) names.push(p.rest.name);
        return names;
    }
    return [p.name];
}
```

### Verification

- Unit test each helper with simple and destructured params
- `paramNames` for object destructured → produces `(object name age)` kernel form
- `paramTypeChecks` for `(object :string name :number age)` → two type checks
- `paramTypeChecks` for `:any` fields → no checks for those fields

---

## Milestone 4: Update `buildSingleClauseFunc`

**Goal**: Single-clause `func` emits destructured params correctly.

**File**: `src/surface.js`, lines 989-1131

### 4.1 — Update param name extraction (line 1006)

Change:
```javascript
const paramNames = params.map((p) => p.name);
```
To:
```javascript
const paramNameNodes = params.flatMap((p) => paramNames(p));
```

### 4.2 — Update type check emission (lines 1012-1015)

Change:
```javascript
for (const p of params) {
    const check = buildTypeCheck(p.name, p.typeKw, funcName, "arg");
    if (check) bodyStmts.push(check);
}
```
To:
```javascript
for (const p of params) {
    bodyStmts.push(...paramTypeChecks(p, funcName));
}
```

### 4.3 — Update function node construction

The `(function funcName (params...) body...)` form uses `paramNameNodes` instead of `paramNames`.

### Verification

**Test case 1 — Object destructured single-clause:**
```lisp
(func process
  :args ((object :string name :number age) :string action)
  :returns :string
  :body (template name " (" age ") — " action))
```
Expected kernel output:
```javascript
function process({name, age}, action) {
  if (typeof name !== "string") throw new TypeError(...);
  if (typeof age !== "number" || Number.isNaN(age)) throw new TypeError(...);
  if (typeof action !== "string") throw new TypeError(...);
  return `${name} (${age}) — ${action}`;
}
```

**Test case 2 — Array destructured:**
```lisp
(func head-tail
  :args ((array :number first (rest :number remaining)))
  :body (console:log first remaining))
```

**Test case 3 — Mixed destructured + simple params**

**Test case 4 — `:any` fields in destructured param (no type checks for those)**

---

## Milestone 5: Update `buildMultiClauseFunc`

**Goal**: Multi-clause `func` handles destructured params in dispatch and binding.

**File**: `src/surface.js`, lines 1133-1338

### 5.1 — Update dispatch condition building (lines 1173-1232)

Currently uses `p.typeKw.value` for dispatch. Change to use `paramDispatchType(p)`:

For destructured object params, the dispatch check at position `i` should be:
```javascript
// typeof args[i] === "object" && args[i] !== null
```
For destructured array params:
```javascript
// Array.isArray(args[i])
```

This means adding `"object"` and `"array"` cases (which already exist in the switch) — the `paramDispatchType` helper maps destructured params to these values, so the existing switch cases handle them naturally.

### 5.2 — Update arity calculation (line 1146)

Currently: `arity: params.length` — each `ParamShape` (simple or destructured) counts as one positional argument. This is correct: a destructured object occupies one argument position.

### 5.3 — Update parameter binding (lines 1240-1248)

Currently binds `const name = get(args, i)` for each param. For destructured params, bind with kernel destructuring pattern:

```javascript
for (let i = 0; i < params.length; i++) {
    const p = params[i];
    const argAccess = array(sym("get"), argsVar, { type: "number", value: i });
    if (p.destructured) {
        // const (object name age) = get(args, i)
        // or: const (array first ...) = get(args, i)
        clauseBody.push(
            array(sym("const"), paramNames(p)[0], argAccess)
        );
    } else {
        clauseBody.push(
            array(sym("const"), p.name, argAccess)
        );
    }
}
```

### 5.4 — Update type checks after binding (around lines 1251-1254)

Change to use `paramTypeChecks(p, funcName)` for each param.

### Verification

**Test case 1 — Two clauses, one with destructured object, one with string:**
```lisp
(func process
  (:args ((object :string name) :string action)
   :body (template name ": " action))
  (:args (:string raw-input :string action)
   :body (template raw-input " — " action)))
```
Clause 1 dispatches on `:object`, clause 2 on `:string` — no overlap.

**Test case 2 — Object vs array destructuring (no overlap):**
```lisp
(func transform
  (:args ((object :string name)) :body ...)
  (:args ((array :number first)) :body ...))
```

**Test case 3 — Overlapping destructured objects (compile error):**
```lisp
(func bad
  (:args ((object :string name)) :body ...)
  (:args ((object :number id)) :body ...))
```
Should produce overlap error (both dispatch as `:object` at position 0).

---

## Milestone 6: Update `fnMacro`

**Goal**: `fn` / `lambda` supports destructured params.

**File**: `src/surface.js`, lines 829-861

### 6.1 — Update param name extraction (line 843)

Change:
```javascript
const paramNames = params.map((p) => p.name);
```
To:
```javascript
const paramNameNodes = params.flatMap((p) => paramNames(p));
```

### 6.2 — Update type check building (lines 846-850)

Change:
```javascript
for (const p of params) {
    const check = buildTypeCheck(p.name, p.typeKw, "anonymous", "arg");
    if (check) typeChecks.push(check);
}
```
To:
```javascript
for (const p of params) {
    typeChecks.push(...paramTypeChecks(p, "anonymous"));
}
```

### 6.3 — Update arrow construction (lines 854-860)

Use `paramNameNodes` instead of `paramNames` in the `(=> (params...) ...)` form.

### Verification

**Test case:**
```lisp
(bind f (fn ((object :string name :number age))
  (console:log name age)))
```
Expected:
```javascript
const f = ({name, age}) => {
  if (typeof name !== "string") throw new TypeError(...);
  if (typeof age !== "number" || Number.isNaN(age)) throw new TypeError(...);
  console.log(name, age);
};
```

---

## Milestone 7: Tests and fixtures

**Goal**: Comprehensive test coverage and cross-compiler fixtures.

### 7.1 — Create `test/surface/func-destructuring.test.js`

Test cases (each verifies kernel JSON output):

1. **Object destructuring — single clause**: `(func f :args ((object :string name :number age)) :body ...)`
2. **Array destructuring — single clause**: `(func f :args ((array :number first :number second)) :body ...)`
3. **Array with rest**: `(func f :args ((array :number first (rest :number remaining))) :body ...)`
4. **Array with skip (_)**: `(func f :args ((array :number first _ :number third)) :body ...)`
5. **Mixed destructured + simple**: `(func f :args ((object :string name) :string action) :body ...)`
6. **`:any` field — no type check**: `(func f :args ((object :any name :number age)) :body ...)`
7. **fn with object destructuring**: `(bind f (fn ((object :string name)) (console:log name)))`
8. **Multi-clause with destructured + simple dispatch**: Different types at same position
9. **Multi-clause with object vs array**: No overlap
10. **Error: empty pattern `(object)`**: Should throw
11. **Error: bare name without type**: `(object name)` → error
12. **Error: nested destructuring**: Should throw with helpful message
13. **Error: default in destructured**: Should throw with helpful message
14. **Error: overlapping destructured clauses**: Two `(object ...)` at position 0

### 7.2 — Create `test/fixtures/surface/func-destructuring.json`

Capture kernel JSON output from each passing test case. Format matches existing fixtures (e.g., `test/fixtures/surface/func.json`).

Structure:
```json
{
  "objectDestructuringSingle": {
    "source": "(func f :args ((object :string name :number age)) :body (console:log name age))",
    "kernel": [ ... ESTree-like kernel JSON ... ]
  },
  ...
}
```

### 7.3 — Regression: run all existing surface tests

Ensure `deno test test/surface/func.test.js` and `deno test test/surface/fn-lambda.test.js` still pass — no simple-param regressions.

### Verification

```bash
deno test test/surface/func-destructuring.test.js
deno test test/surface/func.test.js
deno test test/surface/fn-lambda.test.js
deno test  # full suite
```

---

## Files modified

| File | Change |
|------|--------|
| `src/surface.js` | Add `parseDestructuredParam`, update `parseTypedParams`, add 4 helpers, update 3 call sites (~50 lines net) |
| `test/surface/func-destructuring.test.js` | New — ~14 test cases |
| `test/fixtures/surface/func-destructuring.json` | New — kernel JSON fixtures for cross-compiler verification |

## Dependencies

- DD-06 kernel destructuring must work (it does — tested in `test/forms/destructuring-params.test.js`)
- `buildTypeCheck` unchanged — field-level checks reuse it as-is
