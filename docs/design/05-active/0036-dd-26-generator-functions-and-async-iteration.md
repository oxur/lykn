---
number: 36
title: "DD-26: Generator Functions and Async Iteration"
author: "Duncan McGreggor"
component: All
tags: [change-me]
created: 2026-04-15
updated: 2026-04-15
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# DD-26: Generator Functions and Async Iteration

## Context

Chapter 21 (Iterators and Generators) needs generator functions. lykn can consume iterables but can't produce them. This adds kernel forms (`function*`, `yield`, `yield*`, `for-await-of`) plus surface forms (`genfunc`, `genfn`) with `:yields :type` runtime checks.

## Phase 1: Kernel Forms (JS Compiler)

**File**: `src/compiler.js`

### 1a. `function*` handler

Clone the `function` handler (line 295), set `generator: true`:

```javascript
'function*'(args) {
    // Same structure as 'function' but generator: true
}
```

### 1b. `yield` handler

```javascript
'yield'(args) {
    return {
        type: 'YieldExpression',
        argument: args.length > 0 ? compileExpr(args[0]) : null,
        delegate: false,
    };
}
```

### 1c. `yield*` handler

```javascript
'yield*'(args) {
    return {
        type: 'YieldExpression',
        argument: compileExpr(args[0]),
        delegate: true,
    };
}
```

### 1d. `for-await-of` handler

Clone `for-of` handler (line 599), set `await: true`.

### 1e. Update `async` handler (line 321)

Add `'function*'` to the allowed inner form heads alongside `'function'`, `'lambda'`, `'=>'`.

### 1f. Tests

`test/forms/generator.test.js`:

- `(function* gen () (yield 1) (yield 2))` → `function* gen() { yield 1; yield 2; }`
- `(yield value)` → `yield value`
- `(yield)` → `yield` (no argument)
- `(yield* other)` → `yield* other`
- `(for-await-of item stream (process item))` → `for await (const item of stream) { process(item); }`
- `(async (function* gen () ...))` → `async function* gen() { ... }`

## Phase 2: Kernel Forms (Rust Codegen)

**File**: `crates/lykn-lang/src/codegen/emit.rs`

### 2a. `emit_function_star` — insert `*` after `function`

### 2b. `emit_yield` — `yield` or `yield expr`

### 2c. `emit_yield_star` — `yield* expr`

### 2d. `emit_for_await_of` — `for await (const x of iter) { ... }`

### 2e. Update `emit_async` — add `"function*"` match arm

### 2f. Update dispatch in `emit_list`/`emit_expr` — add cases

### 2g. Update `STATEMENT_FORMS` — add `"function*"`, `"for-await-of"`

**File**: `crates/lykn-lang/src/classifier/dispatch.rs`

### 2h. Add to `is_kernel_form()`: `"function*"`, `"yield*"`, `"for-await-of"`

(`"yield"` already present.)

### 2i. Update `classify_async` — recognize `"function*"` as inner form

### 2j. Tests

Codegen tests for each new form + async generator combo.

## Phase 3: Surface Forms (JS)

**File**: `src/surface.js`

### 3a. `genfunc` macro

Parallels `func` — keyword-labeled clauses with `:args`, `:yields`, `:returns`, `:pre`, `:post`, `:body`.

```lisp
(genfunc fibonacci
  :yields :number
  :body
  (let a 0) (let b 1)
  (while true
    (yield a)
    (let temp a) (= a b) (= b (+ temp b))))

(genfunc range
  :args (:number start :number end)
  :yields :number
  :body
  (for (let i start) (< i end) (+= i 1)
    (yield i)))
```

**Compiled output**:

```javascript
function* fibonacci() {
  let a = 0;
  let b = 1;
  while (true) {
    {
      const __v = a;
      if (typeof __v !== "number" || Number.isNaN(__v))
        throw new TypeError("fibonacci: yield expected number, got " + typeof __v);
      yield __v;
    }
    let temp = a;
    a = b;
    b = temp + b;
  }
}
```

**Implementation**: `buildSingleClauseGenfunc` mirrors `buildSingleClauseFunc`:

1. Parse `:args` with `parseTypedParams` (same as `func`)
2. Parse `:yields` — a single type keyword
3. Emit param type checks (same as `func`)
4. **Yield instrumentation**: Walk the body AST recursively. For every `(yield expr)` found, replace with a block that captures the value, type-checks it, then yields it. For `(yield)` with no arg, skip the check.
5. Emit `:returns` check on final return value (if any)
6. Emit `(function* name (params) ...body...)` kernel form

**Yield walk function** (`instrumentYields(bodyExpr, yieldsType, funcName)`):

- Recurse into all list nodes
- When a list has head `"yield"`: wrap the argument with a type check
- When a list has head `"yield*"`: leave as-is (delegated yields are the other generator's responsibility)
- Leave everything else unchanged

### 3b. `genfn` macro

Parallels `fn` — anonymous typed generator expression:

```lisp
(bind gen (genfn (:number start :number end)
  :yields :number
  (for (let i start) (< i end) (+= i 1)
    (yield i))))
```

Syntax: `(genfn (params) :yields :type body...)`

Compiles to `(function* (params) ...instrumented-body...)` — a generator expression (anonymous `FunctionExpression` with `generator: true`).

Note: the JS compiler needs a `function*` expression form too. The `lambda` handler can be cloned → `lambda*` with `generator: true`, or `function*` can handle both named and anonymous (check if first arg is an atom or a param list).

### 3c. Update dispatch

Add `"genfunc"` and `"genfn"` to `is_surface_form()` in Rust classifier.

### 3d. Tests

`test/surface/genfunc.test.js`:

- `genfunc` with `:yields :number` — verify `function*` output + yield checks
- `genfunc` with `:args` + `:yields` — param checks + yield checks
- `genfunc` with `:yields :any` — no yield check
- `genfn` — anonymous generator with yield checks
- `(async (genfunc ...))` — async generator
- `(export (genfunc ...))` — exported generator
- `--strip-assertions` — yield checks stripped, `function*` and `yield` preserved
- Error: `:yields` missing → compile error

## Phase 4: Surface Forms (Rust)

**File**: `crates/lykn-lang/src/ast/surface.rs`

### 4a. Add `Genfunc` and `Genfn` variants to `SurfaceForm`

```rust
Genfunc {
    name: String,
    name_span: Span,
    clauses: Vec<GenfuncClause>,
    span: Span,
},
Genfn {
    params: Vec<ParamShape>,
    yields: Option<TypeAnnotation>,
    body: Vec<SExpr>,
    span: Span,
},
```

`GenfuncClause` is like `FuncClause` plus `yields: Option<TypeAnnotation>`.

### 4b. Classifier — `classify_genfunc`, `classify_genfn`

Parse keyword clauses same as `func`/`fn` + `:yields` keyword.

### 4c. Emitter — `emit_genfunc`, `emit_genfn`

Same as `emit_func`/`emit_fn_expr` but:

- Emit `function*` instead of `function`
- Walk body to instrument yields with type checks
- Handle `:yields :any` (no checks)

### 4d. Analysis — scope tracking, overlap detection

Same patterns as `func` — params introduce bindings, `:yields` and `:returns` are metadata.

## Phase 5: Tests + Verification

- Full JS test suite
- Full Rust test suite
- Cross-compiler verification for kernel forms
- `cargo clippy` clean

## Key files

| File | Change |
|------|--------|
| `src/compiler.js` | `function*`, `yield`, `yield*`, `for-await-of` handlers; update `async` |
| `src/surface.js` | `genfunc`, `genfn` macros with yield instrumentation |
| `crates/lykn-lang/src/codegen/emit.rs` | 4 new emitters, update async, dispatch |
| `crates/lykn-lang/src/classifier/dispatch.rs` | New kernel + surface forms |
| `crates/lykn-lang/src/classifier/forms.rs` | `classify_genfunc`, `classify_genfn` |
| `crates/lykn-lang/src/ast/surface.rs` | `Genfunc`, `Genfn` variants |
| `crates/lykn-lang/src/emitter/forms.rs` | `emit_genfunc`, `emit_genfn` with yield instrumentation |
| `test/forms/generator.test.js` | Kernel generator tests |
| `test/surface/genfunc.test.js` | Surface generator tests |
