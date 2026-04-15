# DD-25 Phase 4: Rust Tests + Cross-Compiler Verification

## Context

Phases 2-3 built the Rust implementation. Phase 4 adds comprehensive tests and verifies that the Rust compiler produces identical kernel JSON to the JS compiler (using the fixtures captured in Phase 1, milestone 7).

---

## Milestone 1: Classifier tests for destructured param parsing

**File**: `crates/lykn-lang/src/classifier/forms.rs`, `#[cfg(test)] mod tests`

### 1.1 — Object destructuring classification

Test that classifying:
```lisp
(func process
  :args ((object :string name :number age) :string action)
  :returns :string
  :body (template name " (" age ") — " action))
```
produces a `SurfaceForm::Func` with one clause where `args` contains:
- `ParamShape::DestructuredObject { fields: [TypedParam("string", "name"), TypedParam("number", "age")] }`
- `ParamShape::Simple(TypedParam("string", "action"))`

### 1.2 — Array destructuring classification

Test:
```lisp
(func head-tail
  :args ((array :number first (rest :number remaining)))
  :body (console:log first remaining))
```
produces:
- `ParamShape::DestructuredArray { elements: [Typed("number", "first"), Rest("number", "remaining")] }`

### 1.3 — Array with skip

Test:
```lisp
(func f :args ((array :number first _ :number third)) :body ...)
```
produces:
- `ParamShape::DestructuredArray { elements: [Typed("number", "first"), Skip, Typed("number", "third")] }`

### 1.4 — fn with destructured params

Test:
```lisp
(fn ((object :string name :number age)) (console:log name age))
```
produces `SurfaceForm::Fn { params: [ParamShape::DestructuredObject { ... }], ... }`

### 1.5 — Error cases

Each error case produces a `Diagnostic` with the expected message:

| Input | Expected error substring |
|-------|------------------------|
| `(func f :args ((object)) :body ...)` | "empty destructuring pattern" |
| `(func f :args ((object name)) :body ...)` | "missing type annotation" |
| `(func f :args ((object :string name (alias :any addr (object :string city)))) :body ...)` | "nested destructuring.*not yet supported" |
| `(func f :args ((object (default :string name "anon") :number age)) :body ...)` | "default values.*not yet supported" |
| `(func f :args ((array (rest :number r) :number x)) :body ...)` | "rest element must be last" |

### 1.6 — Regression: simple params still work

Re-run all existing classifier tests to ensure no regression.

### Verification

```bash
cargo test -p lykn-lang -- classifier::forms::tests
```

---

## Milestone 2: Emitter tests for destructured param emission

**File**: `crates/lykn-lang/src/emitter/forms.rs`, test module (near existing `test_emit_func_single_clause`)

### 2.1 — Single-clause func with object destructuring

Construct a `SurfaceForm::Func` with one clause containing a `ParamShape::DestructuredObject`. Verify emitted kernel S-expression:

```lisp
(function process ((object name age) action)
  (if (|| (!== (typeof name) "string") ...)
    (throw (new TypeError ...)))
  (if (|| (!== (typeof age) "number") (Number:isNaN age))
    (throw (new TypeError ...)))
  (if (!== (typeof action) "string")
    (throw (new TypeError ...)))
  (return (template name " (" age ") — " action)))
```

Key assertions:
- Parameter list contains `(object name age)` kernel pattern, not individual atoms
- Type checks reference individual field names (name, age), not the pattern
- Type checks appear as body statements (per-field)

### 2.2 — Single-clause func with array destructuring + rest

Verify:
- Parameter list contains `(array first (rest remaining))`
- Type checks for `first` and `remaining`

### 2.3 — fn with destructured object

Verify:
- Arrow syntax: `(=> ((object name age)) ...)`
- Type checks for each field
- Block body when checks present, expression body when all `:any`

### 2.4 — Multi-clause with destructured dispatch

Two clauses: one with `(object :string name)`, one with `(:string raw)`:
- Dispatch condition for clause 1: `typeof args[0] === "object" && args[0] !== null`
- Dispatch condition for clause 2: `typeof args[0] === "string"`
- Binding for clause 1: `const (object name) = get(args, 0)`
- Binding for clause 2: `const raw = get(args, 0)`

### 2.5 — `:any` fields produce no type checks

Object destructure with `:any name :number age` → only `age` gets a type check.

### 2.6 — strip-assertions mode

If the emitter has a strip-assertions flag/context, verify:
- Destructuring patterns preserved in params
- Type checks removed from body

### Verification

```bash
cargo test -p lykn-lang -- emitter::forms::tests
```

---

## Milestone 3: Overlap detection tests

**File**: `crates/lykn-lang/src/analysis/func_check.rs`, test module

### 3.1 — Overlapping destructured objects at same position

Two clauses:
- `(:args ((object :string name)) :body ...)`
- `(:args ((object :number id)) :body ...)`

Both dispatch as `:object` at position 0 → overlap error.

### 3.2 — Non-overlapping: object destructure vs `:string`

- `(:args ((object :string name) :string action) :body ...)`
- `(:args (:string raw-input :string action) :body ...)`

Clause 1 dispatches `:object`, clause 2 `:string` → no overlap.

### 3.3 — Non-overlapping: object vs array destructure

- `(:args ((object :string name)) :body ...)`
- `(:args ((array :number first)) :body ...)`

`:object` vs `:array` → no overlap.

### 3.4 — Mixed: destructured + simple, different arities

Different arities never overlap (existing behavior). Verify destructured params don't break arity counting.

### Verification

```bash
cargo test -p lykn-lang -- analysis::func_check
```

---

## Milestone 4: Cross-compiler verification

**Goal**: Verify Rust kernel JSON matches JS fixtures from Phase 1.

**File**: `crates/lykn-lang/tests/cross_compiler.rs` (or existing `e2e_tests.rs`)

### 4.1 — Load JS fixtures

Read `test/fixtures/surface/func-destructuring.json` (created in Phase 1, Milestone 7.2).

### 4.2 — For each fixture entry:

1. Parse the source with the Rust reader
2. Classify into `SurfaceForm`
3. Emit to kernel S-expression
4. Serialize to JSON
5. Compare with JS fixture JSON

### 4.3 — Test cases from fixtures

Every entry in `func-destructuring.json`:
- Object destructuring single-clause
- Array destructuring single-clause
- Array with rest
- Array with skip
- Mixed destructured + simple
- `:any` field (no type check)
- fn with object destructuring
- Multi-clause dispatch

### 4.4 — Round-trip consistency

For each test: `parse → classify → emit → serialize` in both JS and Rust must produce byte-identical JSON.

### Verification

```bash
cargo test -p lykn-lang -- cross_compiler
# or
cargo test -- func_destructuring
```

---

## Milestone 5: Full test suite pass

### 5.1 — Rust full suite

```bash
cargo test                   # all workspace crates
cargo clippy                 # no warnings
cargo fmt --check            # formatted
```

### 5.2 — JS full suite

```bash
deno test                    # all JS tests
deno lint src/               # no lint errors
```

### 5.3 — Cross-compiler suite

```bash
cargo test -- cross_compiler  # all cross-compiler tests
```

---

## Files modified

| File | Change |
|------|--------|
| `crates/lykn-lang/src/classifier/forms.rs` | Add ~8 classifier tests |
| `crates/lykn-lang/src/emitter/forms.rs` | Add ~6 emitter tests |
| `crates/lykn-lang/src/analysis/func_check.rs` | Add ~4 overlap detection tests |
| `crates/lykn-lang/tests/cross_compiler.rs` | Add fixture-based cross-compiler tests |
