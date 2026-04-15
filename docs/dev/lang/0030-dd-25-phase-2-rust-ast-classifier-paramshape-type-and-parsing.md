# DD-25 Phase 2: Rust AST + Classifier — `ParamShape` Type and Parsing

## Context

Phase 1 (JS) produces canonical kernel JSON fixtures. Phase 2 introduces the `ParamShape` enum and its accessor methods into the Rust AST, updates the classifier's param parser to recognize destructuring patterns, and wraps existing `TypedParam` construction sites with `ParamShape::Simple`. At the end of this phase, the code will have type errors at every emitter/analysis call site — these are the migration targets for Phase 3.

---

## Milestone 1: `ParamShape` and `ArrayParamElement` enums

**Goal**: Define the new types and all accessor methods.

**File**: `crates/lykn-lang/src/ast/surface.rs`

### 1.1 — Add `ArrayParamElement` enum (after `TypedParam`, ~line 15)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayParamElement {
    /// A typed element: `:type name`
    Typed(TypedParam),
    /// A rest element: `(rest :type name)`
    Rest(TypedParam),
    /// A skip: `_`
    Skip(Span),
}
```

### 1.2 — Add `ParamShape` enum (after `ArrayParamElement`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ParamShape {
    Simple(TypedParam),
    DestructuredObject {
        fields: Vec<TypedParam>,
        span: Span,
    },
    DestructuredArray {
        elements: Vec<ArrayParamElement>,
        span: Span,
    },
}
```

### 1.3 — `From<TypedParam> for ParamShape`

```rust
impl From<TypedParam> for ParamShape {
    fn from(tp: TypedParam) -> Self {
        ParamShape::Simple(tp)
    }
}
```

This lets existing code that constructs `TypedParam` values wrap them trivially: `tp.into()`.

### 1.4 — Accessor methods on `ParamShape`

```rust
impl ParamShape {
    /// All typed params — flattened. Most downstream code needs this:
    /// "give me the names and types, I don't care about structural shape."
    pub fn typed_params(&self) -> Vec<&TypedParam> {
        match self {
            Self::Simple(tp) => vec![tp],
            Self::DestructuredObject { fields, .. } => fields.iter().collect(),
            Self::DestructuredArray { elements, .. } => elements
                .iter()
                .filter_map(|e| match e {
                    ArrayParamElement::Typed(tp) | ArrayParamElement::Rest(tp) => Some(tp),
                    ArrayParamElement::Skip(_) => None,
                })
                .collect(),
        }
    }

    /// All bound names — for scope tracking.
    pub fn bound_names(&self) -> Vec<&str> {
        self.typed_params()
            .iter()
            .map(|tp| tp.name.as_str())
            .collect()
    }

    /// The type keyword for dispatch purposes.
    /// Simple: the actual type keyword.
    /// Destructured object: synthetic "object".
    /// Destructured array: synthetic "array".
    pub fn dispatch_type(&self) -> &str {
        match self {
            Self::Simple(tp) => &tp.type_ann.name,
            Self::DestructuredObject { .. } => "object",
            Self::DestructuredArray { .. } => "array",
        }
    }

    /// The span of this param shape.
    pub fn span(&self) -> Span {
        match self {
            Self::Simple(tp) => tp.name_span,
            Self::DestructuredObject { span, .. } => *span,
            Self::DestructuredArray { span, .. } => *span,
        }
    }
}
```

### 1.5 — Unit tests for accessors

In `crates/lykn-lang/src/ast/surface.rs` or a dedicated test module:

- `typed_params()` on Simple → 1 param
- `typed_params()` on DestructuredObject with 3 fields → 3 params
- `typed_params()` on DestructuredArray with Typed + Rest + Skip → 2 params (skip excluded)
- `bound_names()` matches expected names
- `dispatch_type()` returns correct synthetic types
- `From<TypedParam>` conversion works

### Verification

```bash
cargo test -p lykn-lang -- ast::surface
```

---

## Milestone 2: Change `FuncClause::args` and `SurfaceForm::Fn::params`

**Goal**: Switch the param vector types from `Vec<TypedParam>` to `Vec<ParamShape>`.

**File**: `crates/lykn-lang/src/ast/surface.rs`

### 2.1 — Update `FuncClause` (line 18-25)

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct FuncClause {
    pub args: Vec<ParamShape>,  // was: Vec<TypedParam>
    pub returns: Option<TypeAnnotation>,
    pub pre: Option<SExpr>,
    pub post: Option<SExpr>,
    pub body: Vec<SExpr>,
    pub span: Span,
}
```

### 2.2 — Update `SurfaceForm::Fn` (line 155-159)

```rust
Fn {
    params: Vec<ParamShape>,  // was: Vec<TypedParam>
    body: Vec<SExpr>,
    span: Span,
},
```

### 2.3 — Update `SurfaceForm::Lambda` (same change)

Check if Lambda has the same structure — if so, update identically.

### 2.4 — Leave `Constructor::fields` as `Vec<TypedParam>`

Per the DD: destructuring doesn't apply to type constructors. `Constructor` at line 27-33 stays unchanged.

### 2.5 — Expected breakage

After this change, `cargo check` will produce type errors at every site that accesses `clause.args` or `params` as `Vec<TypedParam>`. These are Phase 3 migration targets. **Do NOT fix them in this phase** — list them for Phase 3.

### Verification

```bash
cargo check -p lykn-lang 2>&1 | grep "error\[" | wc -l  # count migration sites
cargo check -p lykn-lang 2>&1 | grep "error\[" > /tmp/phase2-migration-sites.txt
```

---

## Milestone 3: Split `parse_typed_params` and add `parse_destructured_param`

**Goal**: Classifier parses both simple and destructured params. Constructor parsing preserved separately.

**File**: `crates/lykn-lang/src/classifier/forms.rs`

### 3.1 — Rename existing `parse_typed_params` → `parse_simple_typed_params`

The original function at line 835-867 stays as-is but gets renamed. This is used by `classify_type` for constructor fields (which remain `Vec<TypedParam>`).

### 3.2 — New `parse_typed_params` → returns `Vec<ParamShape>`

Variable-step loop matching the JS implementation:

```rust
fn parse_typed_params(values: &[SExpr], span: Span) -> Result<Vec<ParamShape>, Diagnostic> {
    let mut params = Vec::new();
    let mut i = 0;
    while i < values.len() {
        match &values[i] {
            // Destructuring pattern: a list at position i
            SExpr::List { values: inner, span: lspan, .. } => {
                params.push(parse_destructured_param(inner, *lspan)?);
                i += 1;
            }
            // Simple param: keyword at i, atom at i+1
            SExpr::Keyword { value: type_name, span: kspan } => {
                if i + 1 >= values.len() {
                    return Err(err(
                        format!("type keyword :{type_name} has no parameter name"),
                        span,
                    ));
                }
                match &values[i + 1] {
                    SExpr::Atom { value: name, span: nspan } => {
                        params.push(ParamShape::Simple(TypedParam {
                            type_ann: TypeAnnotation {
                                name: type_name.clone(),
                                span: *kspan,
                            },
                            name: name.clone(),
                            name_span: *nspan,
                        }));
                    }
                    _ => return Err(err("parameter name must be an atom", span)),
                }
                i += 2;
            }
            _ => {
                return Err(err(
                    format!("expected type keyword or destructuring pattern at position {i}"),
                    span,
                ));
            }
        }
    }
    Ok(params)
}
```

### 3.3 — `parse_destructured_param` function

```rust
fn parse_destructured_param(values: &[SExpr], span: Span) -> Result<ParamShape, Diagnostic> {
    if values.is_empty() {
        return Err(err("empty destructuring pattern — at least one field required", span));
    }
    let head = match &values[0] {
        SExpr::Atom { value, .. } => value.as_str(),
        _ => return Err(err("destructuring pattern must start with 'object' or 'array'", span)),
    };
    match head {
        "object" => parse_object_destructure(&values[1..], span),
        "array" => parse_array_destructure(&values[1..], span),
        _ => Err(err(
            format!("destructuring pattern must start with 'object' or 'array', got '{head}'"),
            span,
        )),
    }
}
```

### 3.4 — `parse_object_destructure`

```rust
fn parse_object_destructure(values: &[SExpr], span: Span) -> Result<ParamShape, Diagnostic> {
    if values.is_empty() {
        return Err(err("empty destructuring pattern — at least one field required", span));
    }
    let mut fields = Vec::new();
    let mut i = 0;
    while i < values.len() {
        match &values[i] {
            // Check for deferred features
            SExpr::List { values: inner, .. } => {
                let head_name = match inner.first() {
                    Some(SExpr::Atom { value, .. }) => value.as_str(),
                    _ => "",
                };
                if head_name == "default" {
                    return Err(err(
                        "default values in destructured params are not yet supported \
                         — use a typed param with body destructuring and default",
                        span,
                    ));
                }
                // Any other list in type position → unexpected
                return Err(err(
                    format!("expected type keyword at position {i} in destructuring pattern"),
                    span,
                ));
            }
            SExpr::Keyword { value: type_name, span: kspan } => {
                if i + 1 >= values.len() {
                    return Err(err(
                        format!("type keyword :{type_name} has no field name in destructuring pattern"),
                        span,
                    ));
                }
                match &values[i + 1] {
                    SExpr::Atom { value: name, span: nspan } => {
                        fields.push(TypedParam {
                            type_ann: TypeAnnotation {
                                name: type_name.clone(),
                                span: *kspan,
                            },
                            name: name.clone(),
                            name_span: *nspan,
                        });
                    }
                    // Nested destructuring in name position
                    SExpr::List { values: inner, .. } => {
                        let head_name = match inner.first() {
                            Some(SExpr::Atom { value, .. }) => value.as_str(),
                            _ => "",
                        };
                        if head_name == "object" || head_name == "array" || head_name == "alias" {
                            return Err(err(
                                "nested destructuring in func/fn params is not yet supported \
                                 — use a typed param with body destructuring",
                                span,
                            ));
                        }
                        return Err(err("field name must be an atom", span));
                    }
                    _ => return Err(err("field name must be an atom", span)),
                }
                i += 2;
            }
            SExpr::Atom { value: name, .. } => {
                // Bare name without type keyword
                return Err(err(
                    format!("field '{name}' missing type annotation (use :any to opt out)"),
                    span,
                ));
            }
            _ => {
                return Err(err(
                    format!("expected type keyword at position {i} in destructuring pattern"),
                    span,
                ));
            }
        }
    }
    Ok(ParamShape::DestructuredObject { fields, span })
}
```

### 3.5 — `parse_array_destructure`

```rust
fn parse_array_destructure(values: &[SExpr], span: Span) -> Result<ParamShape, Diagnostic> {
    if values.is_empty() {
        return Err(err("empty destructuring pattern — at least one field required", span));
    }
    let mut elements = Vec::new();
    let mut i = 0;
    while i < values.len() {
        match &values[i] {
            // Skip element: _
            SExpr::Atom { value, span: aspan } if value == "_" => {
                elements.push(ArrayParamElement::Skip(*aspan));
                i += 1;
            }
            // Rest element: (rest :type name)
            SExpr::List { values: inner, span: lspan, .. } => {
                let head_name = match inner.first() {
                    Some(SExpr::Atom { value, .. }) => value.as_str(),
                    _ => "",
                };
                match head_name {
                    "rest" => {
                        // (rest :type name) — must be 3 elements
                        if inner.len() != 3 {
                            return Err(err("rest element must be (rest :type name)", *lspan));
                        }
                        let tp = parse_rest_element(&inner[1..], *lspan)?;
                        // Rest must be last
                        if i + 1 != values.len() {
                            return Err(err("rest element must be last in array destructuring", span));
                        }
                        elements.push(ArrayParamElement::Rest(tp));
                        i += 1;
                    }
                    "default" => {
                        return Err(err(
                            "default values in destructured params are not yet supported \
                             — use a typed param with body destructuring and default",
                            span,
                        ));
                    }
                    "object" | "array" | "alias" => {
                        return Err(err(
                            "nested destructuring in func/fn params is not yet supported \
                             — use a typed param with body destructuring",
                            span,
                        ));
                    }
                    _ => {
                        return Err(err(
                            format!("unexpected list in array destructuring at position {i}"),
                            span,
                        ));
                    }
                }
            }
            // Typed element: :type name
            SExpr::Keyword { value: type_name, span: kspan } => {
                if i + 1 >= values.len() {
                    return Err(err(
                        format!("type keyword :{type_name} has no element name"),
                        span,
                    ));
                }
                match &values[i + 1] {
                    SExpr::Atom { value: name, span: nspan } => {
                        elements.push(ArrayParamElement::Typed(TypedParam {
                            type_ann: TypeAnnotation {
                                name: type_name.clone(),
                                span: *kspan,
                            },
                            name: name.clone(),
                            name_span: *nspan,
                        }));
                    }
                    _ => return Err(err("element name must be an atom", span)),
                }
                i += 2;
            }
            _ => {
                return Err(err(
                    format!("expected type keyword, _, or (rest ...) at position {i} in array destructuring"),
                    span,
                ));
            }
        }
    }
    Ok(ParamShape::DestructuredArray { elements, span })
}
```

### 3.6 — `parse_rest_element` helper

```rust
fn parse_rest_element(values: &[SExpr], span: Span) -> Result<TypedParam, Diagnostic> {
    // values = [:type, name]
    match (&values[0], &values[1]) {
        (
            SExpr::Keyword { value: type_name, span: kspan },
            SExpr::Atom { value: name, span: nspan },
        ) => Ok(TypedParam {
            type_ann: TypeAnnotation {
                name: type_name.clone(),
                span: *kspan,
            },
            name: name.clone(),
            name_span: *nspan,
        }),
        _ => Err(err("rest element must be (rest :type name)", span)),
    }
}
```

### 3.7 — Update constructor field parsing call sites

Search for all calls to the old `parse_typed_params` that parse constructor fields (in `classify_type` or similar). Change those to call `parse_simple_typed_params` instead.

### 3.8 — Update `classify_fn` and `classify_lambda`

These call `parse_typed_params` to get `Vec<TypedParam>` for `SurfaceForm::Fn::params`. Since `parse_typed_params` now returns `Vec<ParamShape>`, the type change in Milestone 2 (which changed `params: Vec<ParamShape>`) makes this compatible automatically.

### Verification

```bash
# This will still have errors from Phase 3 migration sites,
# but the classifier itself should compile
cargo check -p lykn-lang 2>&1 | grep "classifier" | grep "error" | wc -l  # should be 0

# Run classifier-specific tests
cargo test -p lykn-lang -- classifier
```

### Classifier test cases

Add to `crates/lykn-lang/src/classifier/forms.rs` tests module:

1. **Object destructure parse**: `(func f :args ((object :string name :number age)) :body ...)` → `FuncClause` with one `ParamShape::DestructuredObject`
2. **Array destructure parse**: `(func f :args ((array :number first (rest :number rest))) :body ...)` → `FuncClause` with one `ParamShape::DestructuredArray`
3. **Mixed params**: destructured + simple → correct types in args vec
4. **Error: empty pattern**: `(object)` → helpful error
5. **Error: bare name**: `(object name)` → "missing type annotation"
6. **Error: nested**: → "not yet supported"
7. **Error: default**: → "not yet supported"
8. **Simple params still work** (regression)

---

## Files modified

| File | Change |
|------|--------|
| `crates/lykn-lang/src/ast/surface.rs` | Add `ParamShape`, `ArrayParamElement`, accessors, `From` impl; change `FuncClause::args` and `Fn::params` types |
| `crates/lykn-lang/src/classifier/forms.rs` | Rename old parser, add new `parse_typed_params`, `parse_destructured_param`, `parse_object_destructure`, `parse_array_destructure`, `parse_rest_element`; update constructor sites; add tests |

## Dependencies

- Phase 1 complete (JS canonical fixtures exist)
- No emitter changes yet — Phase 3 handles those
