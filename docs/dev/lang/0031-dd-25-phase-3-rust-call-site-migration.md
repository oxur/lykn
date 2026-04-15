# DD-25 Phase 3: Rust Call-Site Migration

## Context

Phase 2 changed `FuncClause::args` from `Vec<TypedParam>` to `Vec<ParamShape>` and `SurfaceForm::Fn::params` similarly. This broke every downstream site that accesses these fields directly. Phase 3 is a mechanical migration: replace each direct field access with the appropriate `ParamShape` accessor method. Every replacement is local — no replacement depends on another being done first.

---

## Milestone 1: Identify all migration sites

**Goal**: Catalog every compiler error from Phase 2.

### 1.1 — Run `cargo check` and collect errors

```bash
cargo check -p lykn-lang 2>&1 | grep "error\[E"
```

Expected error sites (based on codebase exploration):

| File | Line(s) | Current access | Required change |
|------|---------|---------------|-----------------|
| `emitter/forms.rs` ~988 | `clause.args.iter().map(\|p\| atom(&p.name))` | → `clause.args.iter().flat_map(\|p\| paramNameNodes(p))` |
| `emitter/forms.rs` ~992-1005 | Type check loop over `clause.args` accessing `.name`, `.type_ann` | → `p.typed_params()` + `emit_type_check` per field |
| `emitter/forms.rs` ~1123 | Multi-clause param names | → accessor methods |
| `emitter/forms.rs` ~1160-1167 | Multi-clause param binding from args array | → destructuring-aware binding |
| `emitter/forms.rs` ~1137-1147 | Dispatch type checks | → `p.dispatch_type()` |
| `emitter/forms.rs` ~1169-1182 | Post-binding type checks | → `p.typed_params()` loop |
| `emitter/forms.rs` ~928 | `emit_fn_expr` param name extraction | → accessor |
| `emitter/forms.rs` ~934-947 | `emit_fn_expr` type checks | → accessor |
| `emitter/type_checks.rs` ~13-30 | `emit_type_check` takes `&str` param_name | → called per field, no signature change needed |
| `analysis/scope.rs` | Binding introduction for func params | → `p.bound_names()` |
| `analysis/func_check.rs` ~54-62 | Overlap detection: `param.type_ann.name` | → `p.dispatch_type()` |
| `emitter/contracts.rs` | Pre/post condition param references | → check if accesses param names |

---

## Milestone 2: Emitter — `emit_fn_expr` (fn/lambda)

**File**: `crates/lykn-lang/src/emitter/forms.rs`, function `emit_fn_expr` (~line 922)

### 2.1 — Update param name nodes (line ~928)

Current:
```rust
let param_names: Vec<SExpr> = params.iter().map(|p| atom(&p.name)).collect();
```

Change to:
```rust
let param_names: Vec<SExpr> = params.iter().flat_map(|p| param_to_kernel_names(p)).collect();
```

Where `param_to_kernel_names` is a new helper (see Milestone 5).

### 2.2 — Update type check emission (lines ~934-947)

Current:
```rust
for param in params {
    if param.type_ann.name != "any" {
        if let Some(check) = emit_type_check(&param.name, &param.type_ann.name, ...) {
            body_stmts.push(check);
        }
    }
}
```

Change to:
```rust
for param in params {
    for tp in param.typed_params() {
        if tp.type_ann.name != "any" {
            if let Some(check) = emit_type_check(&tp.name, &tp.type_ann.name, ...) {
                body_stmts.push(check);
            }
        }
    }
}
```

### Verification

```bash
cargo test -p lykn-lang -- emit_fn
```

---

## Milestone 3: Emitter — `emit_func_single`

**File**: `crates/lykn-lang/src/emitter/forms.rs`, function `emit_func_single` (~line 981)

### 3.1 — Update param name nodes (line ~988)

Current:
```rust
let param_names: Vec<SExpr> = clause.args.iter().map(|p| atom(&p.name)).collect();
```

Change to:
```rust
let param_names: Vec<SExpr> = clause.args.iter().flat_map(|p| param_to_kernel_names(p)).collect();
```

### 3.2 — Update type check emission (lines ~992-1005)

Same pattern as Milestone 2.2 — iterate `p.typed_params()` for each `ParamShape`.

### 3.3 — Any other accesses to `clause.args` fields

Check for pre/post condition references to param names, return type emission, etc. Update to use accessors.

### Verification

Existing test `test_emit_func_single_clause` must pass. Add new test with destructured object param.

---

## Milestone 4: Emitter — `emit_func_multi`

**File**: `crates/lykn-lang/src/emitter/forms.rs`, function `emit_func_multi` (~line 1097)

### 4.1 — Update dispatch condition building (lines ~1137-1147)

Current:
```rust
for (i, param) in clause.args.iter().enumerate() {
    if param.type_ann.name != "any" {
        // build dispatch check using param.type_ann.name
    }
}
```

Change to use `param.dispatch_type()`:
```rust
for (i, param) in clause.args.iter().enumerate() {
    let dtype = param.dispatch_type();
    if dtype != "any" {
        // build dispatch check using dtype
        // "object" and "array" are already handled by existing switch cases
    }
}
```

### 4.2 — Update parameter binding (lines ~1160-1167)

Current:
```rust
for (i, param) in clause.args.iter().enumerate() {
    block_items.push(list(vec![
        atom("const"),
        atom(&param.name),
        list(vec![atom("get"), atom("args"), num(i as f64)]),
    ]));
}
```

Change to:
```rust
for (i, param) in clause.args.iter().enumerate() {
    let arg_access = list(vec![atom("get"), atom("args"), num(i as f64)]);
    match param {
        ParamShape::Simple(tp) => {
            block_items.push(list(vec![atom("const"), atom(&tp.name), arg_access]));
        }
        ParamShape::DestructuredObject { fields, .. } => {
            // const (object name1 name2 ...) = get(args, i)
            let pattern = list(
                std::iter::once(atom("object"))
                    .chain(fields.iter().map(|f| atom(&f.name)))
                    .collect(),
            );
            block_items.push(list(vec![atom("const"), pattern, arg_access]));
        }
        ParamShape::DestructuredArray { elements, .. } => {
            // const (array name1 _ name2 (rest rest_name)) = get(args, i)
            let pattern = list(
                std::iter::once(atom("array"))
                    .chain(elements.iter().map(|e| match e {
                        ArrayParamElement::Typed(tp) => atom(&tp.name),
                        ArrayParamElement::Rest(tp) => {
                            list(vec![atom("rest"), atom(&tp.name)])
                        }
                        ArrayParamElement::Skip(_) => atom("_"),
                    }))
                    .collect(),
            );
            block_items.push(list(vec![atom("const"), pattern, arg_access]));
        }
    }
}
```

### 4.3 — Update post-binding type checks (lines ~1169-1182)

Same accessor pattern as Milestone 2.2.

### 4.4 — Update arity calculation

Currently `clause.args.len()` — this is correct because each `ParamShape` (simple or destructured) occupies one positional argument slot. No change needed.

### Verification

Existing test `test_emit_func_multi_clause` must pass. Add new test with mixed destructured/simple clauses.

---

## Milestone 5: Helper functions for kernel emission

**Goal**: Shared helpers used by Milestones 2-4.

**File**: `crates/lykn-lang/src/emitter/forms.rs` (or a new `emitter/param_helpers.rs` if cleaner)

### 5.1 — `param_to_kernel_names(p: &ParamShape) -> Vec<SExpr>`

Returns the kernel parameter nodes for the function signature:

```rust
fn param_to_kernel_names(p: &ParamShape) -> Vec<SExpr> {
    match p {
        ParamShape::Simple(tp) => vec![atom(&tp.name)],
        ParamShape::DestructuredObject { fields, .. } => {
            let names: Vec<SExpr> = fields.iter().map(|f| atom(&f.name)).collect();
            vec![list(std::iter::once(atom("object")).chain(names).collect())]
        }
        ParamShape::DestructuredArray { elements, .. } => {
            let elems: Vec<SExpr> = elements
                .iter()
                .map(|e| match e {
                    ArrayParamElement::Typed(tp) => atom(&tp.name),
                    ArrayParamElement::Rest(tp) => list(vec![atom("rest"), atom(&tp.name)]),
                    ArrayParamElement::Skip(_) => atom("_"),
                })
                .collect();
            vec![list(std::iter::once(atom("array")).chain(elems).collect())]
        }
    }
}
```

### 5.2 — `param_type_checks(p: &ParamShape, func_name: &str, label: &str, span: Span) -> Vec<SExpr>`

Returns type check assertions for all fields:

```rust
fn param_type_checks(p: &ParamShape, func_name: &str, label: &str, span: Span) -> Vec<SExpr> {
    p.typed_params()
        .iter()
        .filter_map(|tp| {
            if tp.type_ann.name == "any" {
                None
            } else {
                emit_type_check(&tp.name, &tp.type_ann.name, func_name, label, span)
            }
        })
        .collect()
}
```

---

## Milestone 6: Analysis — scope tracking

**File**: `crates/lykn-lang/src/analysis/scope.rs`

### 6.1 — Find all sites that introduce func/fn param bindings

Search for where `TypedParam.name` is used to introduce bindings into scope. Change to iterate `param.bound_names()`:

Current pattern:
```rust
for param in &clause.args {
    scope.introduce(&param.name, param.name_span);
}
```

Change to:
```rust
for param in &clause.args {
    for tp in param.typed_params() {
        scope.introduce(&tp.name, tp.name_span);
    }
}
```

### Verification

```bash
cargo test -p lykn-lang -- analysis::scope
```

---

## Milestone 7: Analysis — func overlap detection

**File**: `crates/lykn-lang/src/analysis/func_check.rs`

### 7.1 — Update pattern row construction (lines ~51-66)

Current:
```rust
clause.args.iter().map(|param| {
    if param.type_ann.name == "any" {
        DeconPattern::Wildcard
    } else {
        DeconPattern::TypeKeyword(param.type_ann.name.clone())
    }
}).collect()
```

Change to:
```rust
clause.args.iter().map(|param| {
    let dtype = param.dispatch_type();
    if dtype == "any" {
        DeconPattern::Wildcard
    } else {
        DeconPattern::TypeKeyword(dtype.to_string())
    }
}).collect()
```

This is the key change: destructured objects dispatch as `"object"`, destructured arrays as `"array"`. The Maranget algorithm treats these as constructors and correctly identifies overlaps (two `object` destructures at same position = overlap).

### 7.2 — Arity grouping (line ~40)

`clause.args.len()` still correct — each `ParamShape` is one positional slot.

### Verification

Add test: two clauses both destructuring objects at position 0 → overlap detected.
Add test: object destructure vs `:string` at position 0 → no overlap.

---

## Milestone 8: Emitter — contracts (pre/post conditions)

**File**: `crates/lykn-lang/src/emitter/contracts.rs` (if it exists, otherwise in `forms.rs`)

### 8.1 — Check if pre/post condition emission accesses param names

If pre/post conditions reference param names for error messages or assertion building, update to use accessors. Pre/post conditions likely reference the param names within the user's expressions (which are already S-expressions), so the emitter may not directly access `TypedParam` fields for this purpose.

### 8.2 — Return type check emission

If `returns` type check references param info, update. Likely independent of params.

### Verification

Existing pre/post condition tests must pass.

---

## Milestone 9: Compilation verification

**Goal**: Full green build.

```bash
cargo check -p lykn-lang    # zero errors
cargo clippy -p lykn-lang   # zero warnings
cargo test -p lykn-lang     # all pass
cargo fmt -p lykn-lang      # formatted
```

---

## Files modified

| File | Change |
|------|--------|
| `crates/lykn-lang/src/emitter/forms.rs` | Update `emit_fn_expr`, `emit_func_single`, `emit_func_multi`; add `param_to_kernel_names`, `param_type_checks` helpers |
| `crates/lykn-lang/src/emitter/type_checks.rs` | No signature change — called per-field by new helpers |
| `crates/lykn-lang/src/analysis/scope.rs` | Use `param.typed_params()` / `bound_names()` for binding introduction |
| `crates/lykn-lang/src/analysis/func_check.rs` | Use `param.dispatch_type()` for overlap detection |
| `crates/lykn-lang/src/emitter/contracts.rs` | If applicable — accessor migration |

## Migration pattern summary

Every change follows one of these shapes:

1. **Name access** (`param.name`) → `param.bound_names()` or `param.typed_params().iter().map(|tp| &tp.name)`
2. **Type access** (`param.type_ann.name`) → `param.dispatch_type()` (for dispatch) or `tp.type_ann.name` (per-field via `typed_params()`)
3. **Kernel emission** (`atom(&param.name)`) → `param_to_kernel_names(param)` (returns vec)
4. **Type check emission** (per param) → `param_type_checks(param, ...)` (returns vec of checks)

Each replacement is local and independent. The compiler guides you to every site.
