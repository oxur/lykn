use crate::analysis::type_registry::TypeRegistry;
use crate::ast::sexpr::SExpr;
use crate::ast::surface::{
    Constructor, FuncClause, MatchClause, Pattern, SurfaceForm, ThreadingStep, TypedParam,
};
use crate::reader::source_loc::Span;

use super::context::EmitterContext;
use super::contracts::{emit_post_check, emit_pre_check};
use super::type_checks::emit_type_check;

// ---------------------------------------------------------------------------
// Kernel SExpr construction helpers
// ---------------------------------------------------------------------------

/// Create an `Atom` with a default span.
pub fn atom(s: &str) -> SExpr {
    SExpr::Atom {
        value: s.to_string(),
        span: Span::default(),
    }
}

/// Create a `String` literal with a default span.
pub fn str_lit(s: &str) -> SExpr {
    SExpr::String {
        value: s.to_string(),
        span: Span::default(),
    }
}

/// Create a `Number` literal with a default span.
pub fn num(n: f64) -> SExpr {
    SExpr::Number {
        value: n,
        span: Span::default(),
    }
}

/// Create a `List` with a default span.
pub fn list(items: Vec<SExpr>) -> SExpr {
    SExpr::List {
        values: items,
        span: Span::default(),
    }
}

/// Create a `Bool` with a default span.
fn bool_lit(b: bool) -> SExpr {
    SExpr::Bool {
        value: b,
        span: Span::default(),
    }
}

// ---------------------------------------------------------------------------
// Await detection
// ---------------------------------------------------------------------------

/// Recursively scan an `SExpr` tree for `(await ...)` forms.
fn contains_await(expr: &SExpr) -> bool {
    match expr {
        SExpr::List { values, .. } => {
            if let Some(SExpr::Atom { value, .. }) = values.first()
                && value == "await"
            {
                return true;
            }
            values.iter().any(contains_await)
        }
        _ => false,
    }
}

/// Check whether any expression in a slice contains an `(await ...)` form.
fn any_contains_await(exprs: &[SExpr]) -> bool {
    exprs.iter().any(contains_await)
}

/// Check whether a `ThreadingStep` contains an `(await ...)` form.
fn step_contains_await(step: &ThreadingStep) -> bool {
    match step {
        ThreadingStep::Bare(expr) => {
            // Bare symbol "await" or a list containing await
            expr.as_atom() == Some("await") || contains_await(expr)
        }
        ThreadingStep::Call(exprs) => {
            // Check if the call head is "await" or if any subexpression contains await
            if let Some(head) = exprs.first()
                && head.as_atom() == Some("await")
            {
                return true;
            }
            any_contains_await(exprs)
        }
    }
}

// ---------------------------------------------------------------------------
// IIFE async wrapping helpers
// ---------------------------------------------------------------------------

/// Wrap an IIFE `((=> () ...))` in `(await ((async (=> () ...))))`.
///
/// Takes the inner arrow body (the `vec![atom("=>"), list(vec![]), ...]`)
/// and returns the fully wrapped expression.
fn wrap_iife_async(arrow_body: Vec<SExpr>) -> SExpr {
    let arrow_fn = list(arrow_body);
    let async_arrow = list(vec![atom("async"), arrow_fn]);
    let call = list(vec![async_arrow]);
    list(vec![atom("await"), call])
}

// ---------------------------------------------------------------------------
// Top-level form dispatch
// ---------------------------------------------------------------------------

/// Emit one `SurfaceForm` to one or more kernel `SExpr` nodes.
pub fn emit_form(
    form: &SurfaceForm,
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> Vec<SExpr> {
    match form {
        SurfaceForm::Bind { name, value, .. } => emit_bind(name, value, ctx, registry),
        SurfaceForm::Obj { pairs, .. } => vec![emit_obj(pairs, ctx, registry)],
        SurfaceForm::Cell { value, .. } => vec![emit_cell(value, ctx, registry)],
        SurfaceForm::Express { target, .. } => vec![emit_express(target)],
        SurfaceForm::Swap {
            target,
            func,
            extra_args,
            ..
        } => vec![emit_swap(target, func, extra_args, ctx, registry)],
        SurfaceForm::Reset { target, value, .. } => vec![emit_reset(target, value, ctx, registry)],
        SurfaceForm::ThreadFirst { initial, steps, .. } => {
            vec![emit_thread_first(initial, steps, ctx, registry)]
        }
        SurfaceForm::ThreadLast { initial, steps, .. } => {
            vec![emit_thread_last(initial, steps, ctx, registry)]
        }
        SurfaceForm::SomeThreadFirst { initial, steps, .. } => {
            vec![emit_some_thread_first(initial, steps, ctx, registry)]
        }
        SurfaceForm::SomeThreadLast { initial, steps, .. } => {
            vec![emit_some_thread_last(initial, steps, ctx, registry)]
        }
        SurfaceForm::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
            ..
        } => vec![emit_if_let(
            pattern,
            expr,
            then_body,
            else_body.as_ref(),
            ctx,
            registry,
        )],
        SurfaceForm::WhenLet {
            pattern,
            expr,
            body,
            ..
        } => vec![emit_when_let(pattern, expr, body, ctx, registry)],
        SurfaceForm::Type { constructors, .. } => emit_type(constructors, ctx, registry),
        SurfaceForm::Fn { params, body, .. } => {
            vec![emit_fn_expr(params, body, ctx, registry)]
        }
        SurfaceForm::Lambda { params, body, .. } => {
            vec![emit_fn_expr(params, body, ctx, registry)]
        }
        SurfaceForm::Func {
            name,
            name_span,
            clauses,
            span,
            ..
        } => emit_func(name, *name_span, clauses, *span, ctx, registry),
        SurfaceForm::Match {
            target, clauses, ..
        } => vec![emit_match(target, clauses, ctx, registry)],
        SurfaceForm::KernelPassthrough { raw, .. } => vec![raw.clone()],
        SurfaceForm::FunctionCall {
            head,
            args,
            span: _,
        } => {
            // Check for js: namespace interop
            if let Some(name) = head.as_atom()
                && name.starts_with("js:") {
                    return vec![emit_js_interop(name, args, ctx, registry)];
                }
            vec![emit_function_call(head, args, ctx, registry)]
        }
        SurfaceForm::Conj { arr, value, .. } => vec![emit_conj(arr, value, ctx, registry)],
        SurfaceForm::Assoc { obj, pairs, .. } => vec![emit_assoc(obj, pairs, ctx, registry)],
        SurfaceForm::Dissoc { obj, keys, .. } => vec![emit_dissoc(obj, keys, ctx, registry)],
        SurfaceForm::MacroDef { raw, .. } => vec![raw.clone()],
        SurfaceForm::ImportMacros { raw, .. } => vec![raw.clone()],
    }
}

// ---------------------------------------------------------------------------
// Expression emission (recursive into sub-expressions)
// ---------------------------------------------------------------------------

/// Recursively emit a sub-expression.
///
/// If the expression is a list whose head is a surface form name, classify and
/// emit it so that nested surface forms (e.g. `(-> 1 (+ 2))` inside a `bind`)
/// are expanded correctly. Non-surface-form lists are recursed into so that
/// deeply nested surface forms are still discovered.
fn emit_expr(expr: &SExpr, ctx: &mut EmitterContext, registry: &TypeRegistry) -> SExpr {
    match expr {
        SExpr::List { values, span } if !values.is_empty() => {
            if let Some(head_name) = values[0].as_atom() {
                // js: namespace interop (DD-15) — handle before surface form check
                if head_name.starts_with("js:") {
                    return emit_js_interop(head_name, &values[1..], ctx, registry);
                }
                if crate::classifier::dispatch::is_surface_form(head_name) {
                    // This subexpression is a surface form — classify and emit it
                    match crate::classifier::classify_expr(expr) {
                        Ok(surface_form) => {
                            let emitted = emit_form(&surface_form, ctx, registry);
                            if emitted.len() == 1 {
                                emitted.into_iter().next().unwrap()
                            } else if emitted.is_empty() {
                                expr.clone()
                            } else {
                                // Multiple forms (e.g. type block) — wrap in block
                                let mut items = vec![atom("block")];
                                items.extend(emitted);
                                list(items)
                            }
                        }
                        Err(_) => expr.clone(),
                    }
                } else {
                    // Not a surface form — recursively process all subexpressions
                    let new_values: Vec<SExpr> =
                        values.iter().map(|v| emit_expr(v, ctx, registry)).collect();
                    SExpr::List {
                        values: new_values,
                        span: *span,
                    }
                }
            } else {
                // Head is not an atom (computed call) — recurse on all elements
                let new_values: Vec<SExpr> =
                    values.iter().map(|v| emit_expr(v, ctx, registry)).collect();
                SExpr::List {
                    values: new_values,
                    span: *span,
                }
            }
        }
        _ => expr.clone(),
    }
}

/// Emit a body (list of expressions), recursively expanding any nested surface
/// forms.
fn emit_body(body: &[SExpr], ctx: &mut EmitterContext, registry: &TypeRegistry) -> Vec<SExpr> {
    body.iter().map(|e| emit_expr(e, ctx, registry)).collect()
}

// ---------------------------------------------------------------------------
// js: namespace interop (DD-15)
// ---------------------------------------------------------------------------

/// Emit a `js:` namespace form. These are simple syntactic rewrites:
/// - `(js:call method args...)` → `(method args...)`
/// - `(js:bind obj:method obj)` → `(obj:method:bind obj)`
/// - `(js:eval code)` → `(eval code)`
/// - `(js:eq a b)` → `(== a b)`
/// - `(js:typeof x)` → `(typeof x)`
fn emit_js_interop(
    form: &str,
    args: &[SExpr],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    match form {
        "js:call" => {
            // (js:call method args...) → (method args...)
            let items: Vec<SExpr> = args.iter().map(|a| emit_expr(a, ctx, registry)).collect();
            list(items)
        }
        "js:bind" => {
            // (js:bind obj:method obj) → (obj:method:bind obj)
            if let Some(SExpr::Atom { value: method, .. }) = args.first() {
                let this_arg = if args.len() > 1 {
                    emit_expr(&args[1], ctx, registry)
                } else {
                    atom("undefined")
                };
                list(vec![atom(&format!("{method}:bind")), this_arg])
            } else {
                // Fallback: pass through
                let mut items = vec![atom(form)];
                items.extend(args.iter().map(|a| emit_expr(a, ctx, registry)));
                list(items)
            }
        }
        "js:eval" => {
            // (js:eval code) → (eval code)
            let code = args
                .first()
                .map(|a| emit_expr(a, ctx, registry))
                .unwrap_or_else(|| atom("undefined"));
            list(vec![atom("eval"), code])
        }
        "js:eq" => {
            // (js:eq a b) → (== a b)
            let a = args
                .first()
                .map(|x| emit_expr(x, ctx, registry))
                .unwrap_or_else(|| atom("undefined"));
            let b = args
                .get(1)
                .map(|x| emit_expr(x, ctx, registry))
                .unwrap_or_else(|| atom("undefined"));
            list(vec![atom("=="), a, b])
        }
        "js:typeof" => {
            // (js:typeof x) → (typeof x)
            let x = args
                .first()
                .map(|a| emit_expr(a, ctx, registry))
                .unwrap_or_else(|| atom("undefined"));
            list(vec![atom("typeof"), x])
        }
        _ => {
            // Unknown js: form — pass through as function call
            let mut items = vec![atom(form)];
            items.extend(args.iter().map(|a| emit_expr(a, ctx, registry)));
            list(items)
        }
    }
}

// ---------------------------------------------------------------------------
// Simple forms
// ---------------------------------------------------------------------------

fn emit_bind(
    name: &SExpr,
    value: &SExpr,
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> Vec<SExpr> {
    vec![list(vec![
        atom("const"),
        name.clone(),
        emit_expr(value, ctx, registry),
    ])]
}

fn emit_obj(pairs: &[(String, SExpr)], ctx: &mut EmitterContext, registry: &TypeRegistry) -> SExpr {
    let mut items = vec![atom("object")];
    for (key, val) in pairs {
        items.push(list(vec![atom(key), emit_expr(val, ctx, registry)]));
    }
    list(items)
}

fn emit_cell(value: &SExpr, ctx: &mut EmitterContext, registry: &TypeRegistry) -> SExpr {
    list(vec![
        atom("object"),
        list(vec![atom("value"), emit_expr(value, ctx, registry)]),
    ])
}

fn emit_express(target: &SExpr) -> SExpr {
    if let SExpr::Atom { value, .. } = target {
        atom(&format!("{value}:value"))
    } else {
        // Fallback: return target as-is (should not happen with valid input)
        target.clone()
    }
}

fn emit_swap(
    target: &SExpr,
    func: &SExpr,
    extra_args: &[SExpr],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let target_name = target
        .as_atom()
        .map(|s| format!("{s}:value"))
        .unwrap_or_default();
    let target_val = atom(&target_name);

    let mut call_args = vec![emit_expr(func, ctx, registry), target_val.clone()];
    for arg in extra_args {
        call_args.push(emit_expr(arg, ctx, registry));
    }

    list(vec![atom("="), target_val, list(call_args)])
}

fn emit_reset(
    target: &SExpr,
    value: &SExpr,
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let target_name = target
        .as_atom()
        .map(|s| format!("{s}:value"))
        .unwrap_or_default();
    list(vec![
        atom("="),
        atom(&target_name),
        emit_expr(value, ctx, registry),
    ])
}

// ---------------------------------------------------------------------------
// Conj / Assoc / Dissoc
// ---------------------------------------------------------------------------

/// `(conj arr value)` -> `(array (spread arr) value)`
fn emit_conj(
    arr: &SExpr,
    value: &SExpr,
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    list(vec![
        atom("array"),
        list(vec![atom("spread"), emit_expr(arr, ctx, registry)]),
        emit_expr(value, ctx, registry),
    ])
}

/// `(assoc obj :k1 v1 :k2 v2)` -> `(object (spread obj) (k1 v1) (k2 v2))`
fn emit_assoc(
    obj: &SExpr,
    pairs: &[(String, SExpr)],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let mut items = vec![
        atom("object"),
        list(vec![atom("spread"), emit_expr(obj, ctx, registry)]),
    ];
    for (key, val) in pairs {
        items.push(list(vec![atom(key), emit_expr(val, ctx, registry)]));
    }
    list(items)
}

/// `(dissoc obj :k1 :k2)` -> IIFE that destructures away the keys and returns the rest.
///
/// Produces:
/// ```text
/// ((=> ()
///   (const (object (alias k1 _discard0) (alias k2 _discard1) (rest _rest0)) obj)
///   _rest0))
/// ```
fn emit_dissoc(
    obj: &SExpr,
    keys: &[String],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let mut pattern_items = vec![atom("object")];
    for key in keys {
        let discard = ctx.gensym.next("_");
        pattern_items.push(list(vec![atom("alias"), atom(key), atom(&discard)]));
    }
    let rest_var = ctx.gensym.next("rest");
    pattern_items.push(list(vec![atom("rest"), atom(&rest_var)]));

    let pattern = list(pattern_items);
    let binding = list(vec![atom("const"), pattern, emit_expr(obj, ctx, registry)]);
    let arrow = list(vec![atom("=>"), list(vec![]), binding, atom(&rest_var)]);
    list(vec![arrow])
}

// ---------------------------------------------------------------------------
// Threading
// ---------------------------------------------------------------------------

fn apply_threading_step(
    acc: SExpr,
    step: &ThreadingStep,
    first: bool,
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    match step {
        ThreadingStep::Bare(expr) => list(vec![emit_expr(expr, ctx, registry), acc]),
        ThreadingStep::Call(exprs) => {
            if exprs.is_empty() {
                return acc;
            }
            let func = emit_expr(&exprs[0], ctx, registry);
            let rest: Vec<SExpr> = exprs[1..]
                .iter()
                .map(|e| emit_expr(e, ctx, registry))
                .collect();
            if first {
                // Thread-first: acc goes after func, before rest
                let mut items = vec![func, acc];
                items.extend(rest);
                list(items)
            } else {
                // Thread-last: acc goes at the end
                let mut items = vec![func];
                items.extend(rest);
                items.push(acc);
                list(items)
            }
        }
    }
}

fn emit_thread_first(
    initial: &SExpr,
    steps: &[ThreadingStep],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let mut acc = emit_expr(initial, ctx, registry);
    for step in steps {
        acc = apply_threading_step(acc, step, true, ctx, registry);
    }
    acc
}

fn emit_thread_last(
    initial: &SExpr,
    steps: &[ThreadingStep],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let mut acc = emit_expr(initial, ctx, registry);
    for step in steps {
        acc = apply_threading_step(acc, step, false, ctx, registry);
    }
    acc
}

// ---------------------------------------------------------------------------
// Some-threading (null-safe)
// ---------------------------------------------------------------------------

fn emit_some_thread(
    initial: &SExpr,
    steps: &[ThreadingStep],
    first: bool,
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    // Await detection
    let initial_has_await = contains_await(initial);
    let steps_have_await = steps.iter().any(step_contains_await);

    // Strategy 1: hoist initial if it contains await
    let (hoist_stmt, init_expr) = if initial_has_await {
        let hoisted_var = ctx.gensym.next("await");
        let hoist = list(vec![
            atom("const"),
            atom(&hoisted_var),
            emit_expr(initial, ctx, registry),
        ]);
        (Some(hoist), atom(&hoisted_var))
    } else {
        (None, emit_expr(initial, ctx, registry))
    };

    // Build as IIFE: ((=> () body...))
    let mut body = Vec::new();
    let t0 = ctx.gensym.next("t");
    body.push(list(vec![atom("const"), atom(&t0), init_expr]));

    let mut prev = t0;
    for (i, step) in steps.iter().enumerate() {
        let is_last = i == steps.len() - 1;

        if !is_last {
            // Null check: (if (== prev null) (return prev))
            body.push(list(vec![
                atom("if"),
                list(vec![atom("=="), atom(&prev), atom("null")]),
                list(vec![atom("return"), atom(&prev)]),
            ]));
        }

        let acc = atom(&prev);
        let result = apply_threading_step(acc, step, first, ctx, registry);

        if is_last {
            // Null check before final return
            body.push(list(vec![
                atom("if"),
                list(vec![atom("=="), atom(&prev), atom("null")]),
                list(vec![atom("return"), atom(&prev)]),
            ]));
            body.push(list(vec![atom("return"), result]));
        } else {
            let tn = ctx.gensym.next("t");
            body.push(list(vec![atom("const"), atom(&tn), result]));
            prev = tn;
        }
    }

    // If no steps, just return the initial value
    if steps.is_empty() {
        body.push(list(vec![atom("return"), atom(&prev)]));
    }

    // Wrap in IIFE, with async wrapping if steps contain await
    let mut arrow = vec![atom("=>"), list(vec![])];
    arrow.extend(body);

    let iife = if steps_have_await {
        wrap_iife_async(arrow)
    } else {
        list(vec![list(arrow)])
    };

    // Combine: if we hoisted, emit block with hoist + iife
    if let Some(hoist) = hoist_stmt {
        list(vec![atom("block"), hoist, iife])
    } else {
        iife
    }
}

fn emit_some_thread_first(
    initial: &SExpr,
    steps: &[ThreadingStep],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    emit_some_thread(initial, steps, true, ctx, registry)
}

fn emit_some_thread_last(
    initial: &SExpr,
    steps: &[ThreadingStep],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    emit_some_thread(initial, steps, false, ctx, registry)
}

// ---------------------------------------------------------------------------
// Type emission
// ---------------------------------------------------------------------------

fn emit_type(
    constructors: &[Constructor],
    ctx: &mut EmitterContext,
    _registry: &TypeRegistry,
) -> Vec<SExpr> {
    let mut forms: Vec<SExpr> = Vec::new();

    for ctor in constructors {
        if ctor.fields.is_empty() {
            // Zero-field: (const Name (object (tag "Name")))
            forms.push(list(vec![
                atom("const"),
                atom(&ctor.name),
                list(vec![
                    atom("object"),
                    list(vec![atom("tag"), str_lit(&ctor.name)]),
                ]),
            ]));
        } else {
            // With fields:
            // (function Name (params...) [type-checks] (return (object (tag "Name") (f f) ...)))
            let param_names: Vec<SExpr> = ctor.fields.iter().map(|f| atom(&f.name)).collect();

            let mut body_items = vec![atom("function"), atom(&ctor.name), list(param_names)];

            // Type checks
            if !ctx.strip_assertions {
                for field in &ctor.fields {
                    if let Some(check) = emit_type_check(
                        &field.name,
                        &field.type_ann.name,
                        &ctor.name,
                        "field",
                        field.name_span,
                    ) {
                        body_items.push(check);
                    }
                }
            }

            // Return object
            let mut obj_fields = vec![atom("object"), list(vec![atom("tag"), str_lit(&ctor.name)])];
            for field in &ctor.fields {
                obj_fields.push(list(vec![atom(&field.name), atom(&field.name)]));
            }
            body_items.push(list(vec![atom("return"), list(obj_fields)]));

            forms.push(list(body_items));
        }
    }

    if forms.len() > 1 {
        let mut block = vec![atom("block")];
        block.extend(forms);
        vec![list(block)]
    } else {
        forms
    }
}

// ---------------------------------------------------------------------------
// Fn / Lambda emission
// ---------------------------------------------------------------------------

fn emit_fn_expr(
    params: &[TypedParam],
    body: &[SExpr],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let param_names: Vec<SExpr> = params.iter().map(|p| atom(&p.name)).collect();

    let mut items = vec![atom("=>"), list(param_names)];

    // Type checks
    if !ctx.strip_assertions {
        for param in params {
            if let Some(check) = emit_type_check(
                &param.name,
                &param.type_ann.name,
                "anonymous",
                "arg",
                param.name_span,
            ) {
                items.push(check);
            }
        }
    }

    items.extend(emit_body(body, ctx, registry));
    list(items)
}

// ---------------------------------------------------------------------------
// Func emission
// ---------------------------------------------------------------------------

fn emit_func(
    name: &str,
    _name_span: Span,
    clauses: &[FuncClause],
    span: Span,
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> Vec<SExpr> {
    if clauses.len() == 1 {
        vec![emit_func_single(name, &clauses[0], span, ctx, registry)]
    } else {
        vec![emit_func_multi(name, clauses, span, ctx, registry)]
    }
}

fn emit_func_single(
    name: &str,
    clause: &FuncClause,
    _span: Span,
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let param_names: Vec<SExpr> = clause.args.iter().map(|p| atom(&p.name)).collect();

    let mut items = vec![atom("function"), atom(name), list(param_names)];

    // Type checks for parameters
    if !ctx.strip_assertions {
        for param in &clause.args {
            if let Some(check) = emit_type_check(
                &param.name,
                &param.type_ann.name,
                name,
                "arg",
                param.name_span,
            ) {
                items.push(check);
            }
        }
    }

    // Pre-condition
    if !ctx.strip_assertions
        && let Some(ref pre) = clause.pre
    {
        items.push(emit_pre_check(name, pre, clause.span));
    }

    let body = emit_body(&clause.body, ctx, registry);

    // Determine return handling
    let has_post = clause.post.is_some();
    let returns_void = clause.returns.as_ref().is_some_and(|r| r.name == "void");
    let returns_any = clause.returns.as_ref().is_some_and(|r| r.name == "any");
    let has_returns = clause.returns.is_some();

    if has_post && !ctx.strip_assertions {
        // Capture result in gensym, check post, return
        let result_var = ctx.gensym.next("result");
        let last_expr = if body.len() == 1 {
            body[0].clone()
        } else {
            // Multiple body exprs: emit all but last as statements, last as value
            for expr in &body[..body.len() - 1] {
                items.push(expr.clone());
            }
            body[body.len() - 1].clone()
        };
        items.push(list(vec![atom("const"), atom(&result_var), last_expr]));
        if let Some(ref post) = clause.post {
            items.push(emit_post_check(name, post, &result_var, clause.span));
        }
        items.push(list(vec![atom("return"), atom(&result_var)]));
    } else if returns_void {
        // Void: just body, no return
        items.extend(body);
    } else if has_returns && !returns_any {
        // Has a typed return: type check result, return
        if body.len() > 1 {
            for expr in &body[..body.len() - 1] {
                items.push(expr.clone());
            }
        }
        let last = if body.is_empty() {
            atom("undefined")
        } else {
            body[body.len() - 1].clone()
        };

        if !ctx.strip_assertions {
            if let Some(ref ret) = clause.returns {
                let result_var = ctx.gensym.next("result");
                items.push(list(vec![atom("const"), atom(&result_var), last]));
                if let Some(check) =
                    emit_type_check(&result_var, &ret.name, name, "return", ret.span)
                {
                    items.push(check);
                }
                items.push(list(vec![atom("return"), atom(&result_var)]));
            }
        } else {
            items.push(list(vec![atom("return"), last]));
        }
    } else {
        // Implicit return of last expression
        if body.len() > 1 {
            for expr in &body[..body.len() - 1] {
                items.push(expr.clone());
            }
        }
        let last = if body.is_empty() {
            atom("undefined")
        } else {
            body[body.len() - 1].clone()
        };
        items.push(list(vec![atom("return"), last]));
    }

    list(items)
}

fn emit_func_multi(
    name: &str,
    clauses: &[FuncClause],
    _span: Span,
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    // Sort clauses: longer arity first, more typed first
    let mut sorted_indices: Vec<usize> = (0..clauses.len()).collect();
    sorted_indices.sort_by(|&a, &b| {
        let ca = &clauses[a];
        let cb = &clauses[b];
        // Longer arity first
        let arity_cmp = cb.args.len().cmp(&ca.args.len());
        if arity_cmp != std::cmp::Ordering::Equal {
            return arity_cmp;
        }
        // More typed args first
        let typed_a = ca.args.iter().filter(|p| p.type_ann.name != "any").count();
        let typed_b = cb.args.iter().filter(|p| p.type_ann.name != "any").count();
        typed_b.cmp(&typed_a)
    });

    let mut items = vec![
        atom("function"),
        atom(name),
        list(vec![list(vec![atom("rest"), atom("args")])]),
    ];

    for &idx in &sorted_indices {
        let clause = &clauses[idx];
        let arity = clause.args.len();

        // Build dispatch condition: (=== args:length N) && type dispatch checks
        let mut conditions = vec![list(vec![
            atom("==="),
            atom("args:length"),
            num(arity as f64),
        ])];

        // Type dispatch checks (only for non-any types)
        for (i, param) in clause.args.iter().enumerate() {
            if param.type_ann.name != "any" {
                let arg_access = list(vec![atom("get"), atom("args"), num(i as f64)]);
                // For dispatch, just check typeof
                let dispatch_check = build_dispatch_check(&param.type_ann.name, &arg_access);
                if let Some(check) = dispatch_check {
                    conditions.push(check);
                }
            }
        }

        let condition = if conditions.len() == 1 {
            conditions.into_iter().next().unwrap()
        } else {
            let mut and_expr = vec![atom("&&")];
            and_expr.extend(conditions);
            list(and_expr)
        };

        // Build clause body block
        let mut block_items = vec![atom("block")];

        // Bind parameters from args array
        for (i, param) in clause.args.iter().enumerate() {
            block_items.push(list(vec![
                atom("const"),
                atom(&param.name),
                list(vec![atom("get"), atom("args"), num(i as f64)]),
            ]));
        }

        // Full type checks
        if !ctx.strip_assertions {
            for param in &clause.args {
                if let Some(check) = emit_type_check(
                    &param.name,
                    &param.type_ann.name,
                    name,
                    "arg",
                    param.name_span,
                ) {
                    block_items.push(check);
                }
            }
        }

        // Pre-condition
        if !ctx.strip_assertions
            && let Some(ref pre) = clause.pre
        {
            block_items.push(emit_pre_check(name, pre, clause.span));
        }

        // Body with return handling
        let body = emit_body(&clause.body, ctx, registry);
        let has_post = clause.post.is_some();

        if has_post && !ctx.strip_assertions {
            let result_var = ctx.gensym.next("result");
            if body.len() > 1 {
                block_items.extend(body[..body.len() - 1].to_vec());
            }
            let last = if body.is_empty() {
                atom("undefined")
            } else {
                body[body.len() - 1].clone()
            };
            block_items.push(list(vec![atom("const"), atom(&result_var), last]));
            if let Some(ref post) = clause.post {
                block_items.push(emit_post_check(name, post, &result_var, clause.span));
            }
            block_items.push(list(vec![atom("return"), atom(&result_var)]));
        } else {
            if body.len() > 1 {
                block_items.extend(body[..body.len() - 1].to_vec());
            }
            let last = if body.is_empty() {
                atom("undefined")
            } else {
                body[body.len() - 1].clone()
            };
            block_items.push(list(vec![atom("return"), last]));
        }

        items.push(list(vec![atom("if"), condition, list(block_items)]));
    }

    // Final throw for no matching clause
    items.push(list(vec![
        atom("throw"),
        list(vec![
            atom("new"),
            atom("TypeError"),
            str_lit(&format!("{name}: no matching clause for arguments")),
        ]),
    ]));

    list(items)
}

/// Build a dispatch type check for multi-clause function overloading.
///
/// This is a simpler check than the full type check — it only tests the
/// typeof for dispatch purposes, without building an error message.
fn build_dispatch_check(type_keyword: &str, expr: &SExpr) -> Option<SExpr> {
    match type_keyword {
        "any" => None,
        "number" => Some(list(vec![
            atom("==="),
            list(vec![atom("typeof"), expr.clone()]),
            str_lit("number"),
        ])),
        "string" => Some(list(vec![
            atom("==="),
            list(vec![atom("typeof"), expr.clone()]),
            str_lit("string"),
        ])),
        "boolean" => Some(list(vec![
            atom("==="),
            list(vec![atom("typeof"), expr.clone()]),
            str_lit("boolean"),
        ])),
        "function" => Some(list(vec![
            atom("==="),
            list(vec![atom("typeof"), expr.clone()]),
            str_lit("function"),
        ])),
        "object" => Some(list(vec![
            atom("&&"),
            list(vec![
                atom("==="),
                list(vec![atom("typeof"), expr.clone()]),
                str_lit("object"),
            ]),
            list(vec![atom("!=="), expr.clone(), atom("null")]),
        ])),
        "array" => Some(list(vec![atom("Array:isArray"), expr.clone()])),
        // User-defined type: check it's a tagged object
        _ => Some(list(vec![
            atom("&&"),
            list(vec![
                atom("==="),
                list(vec![atom("typeof"), expr.clone()]),
                str_lit("object"),
            ]),
            list(vec![
                atom("&&"),
                list(vec![atom("!=="), expr.clone(), atom("null")]),
                list(vec![atom("in"), str_lit("tag"), expr.clone()]),
            ]),
        ])),
    }
}

// ---------------------------------------------------------------------------
// Match emission
// ---------------------------------------------------------------------------

fn emit_match(
    target: &SExpr,
    clauses: &[MatchClause],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    // Await detection
    let target_has_await = contains_await(target);
    let body_has_await = clauses
        .iter()
        .any(|clause| any_contains_await(&clause.body));

    // Strategy 1: hoist target if it contains await
    let (hoist_stmt, target_init_expr) = if target_has_await {
        let hoisted_var = ctx.gensym.next("await");
        let hoist = list(vec![
            atom("const"),
            atom(&hoisted_var),
            emit_expr(target, ctx, registry),
        ]);
        (Some(hoist), atom(&hoisted_var))
    } else {
        (None, emit_expr(target, ctx, registry))
    };

    // Always IIFE: ((=> () (const _t expr) [if-chains] [throw]))
    let target_var = ctx.gensym.next("target");
    let mut body = vec![
        atom("=>"),
        list(vec![]),
        list(vec![atom("const"), atom(&target_var), target_init_expr]),
    ];

    let mut has_wildcard = false;

    for clause in clauses {
        let (condition, bindings) = compile_pattern(&clause.pattern, &target_var, registry);
        let clause_body = emit_body(&clause.body, ctx, registry);

        let mut block_items = vec![atom("block")];
        block_items.extend(bindings);

        if let Some(ref guard) = clause.guard {
            // With guard: wrap body in nested if
            let mut guarded = vec![atom("if"), guard.clone(), list(vec![atom("block")])];
            // Replace the empty block with one containing returns
            let mut inner_block = vec![atom("block")];
            for (i, expr) in clause_body.iter().enumerate() {
                if i == clause_body.len() - 1 {
                    inner_block.push(list(vec![atom("return"), expr.clone()]));
                } else {
                    inner_block.push(expr.clone());
                }
            }
            guarded[2] = list(inner_block);
            block_items.push(list(guarded));
        } else {
            // Without guard: add body with return for last expr
            for (i, expr) in clause_body.iter().enumerate() {
                if i == clause_body.len() - 1 {
                    block_items.push(list(vec![atom("return"), expr.clone()]));
                } else {
                    block_items.push(expr.clone());
                }
            }
        }

        if let Some(cond) = condition {
            body.push(list(vec![atom("if"), cond, list(block_items)]));
        } else {
            // Wildcard/binding with no condition — unconditional block
            has_wildcard = true;
            body.push(list(block_items));
            break; // No further clauses needed after wildcard
        }
    }

    if !has_wildcard {
        body.push(list(vec![
            atom("throw"),
            list(vec![
                atom("new"),
                atom("Error"),
                str_lit("match: no matching pattern"),
            ]),
        ]));
    }

    // Strategy 2: wrap IIFE in async + await if body contains await
    let iife = if body_has_await {
        wrap_iife_async(body)
    } else {
        list(vec![list(body)])
    };

    // Combine: if we hoisted, emit block with hoist + iife
    if let Some(hoist) = hoist_stmt {
        list(vec![atom("block"), hoist, iife])
    } else {
        iife
    }
}

/// Compile a pattern to a (condition, bindings) pair.
///
/// Returns `(None, bindings)` for wildcard/binding patterns (unconditional).
fn compile_pattern(
    pattern: &Pattern,
    target_var: &str,
    registry: &TypeRegistry,
) -> (Option<SExpr>, Vec<SExpr>) {
    match pattern {
        Pattern::Wildcard(_) => (None, vec![]),
        Pattern::Literal(expr) => {
            let condition = list(vec![atom("==="), atom(target_var), expr.clone()]);
            (Some(condition), vec![])
        }
        Pattern::Binding { name, .. } => {
            let binding = list(vec![atom("const"), atom(name), atom(target_var)]);
            (None, vec![binding])
        }
        Pattern::Constructor { name, bindings, .. } => {
            // Check: (=== target:tag "Name")
            let condition = list(vec![
                atom("==="),
                atom(&format!("{target_var}:tag")),
                str_lit(name),
            ]);

            // Bind fields using registry field names
            let mut field_bindings = Vec::new();
            if let Some(ctor_def) = registry.lookup_constructor(name) {
                for (i, binding_pat) in bindings.iter().enumerate() {
                    if i < ctor_def.fields.len() {
                        let field_name = &ctor_def.fields[i].name;
                        match binding_pat {
                            Pattern::Binding {
                                name: bind_name, ..
                            } => {
                                field_bindings.push(list(vec![
                                    atom("const"),
                                    atom(bind_name),
                                    atom(&format!("{target_var}:{field_name}")),
                                ]));
                            }
                            Pattern::Wildcard(_) => {
                                // Skip binding for wildcards
                            }
                            _ => {
                                // Nested patterns would need recursive compilation
                                // For now, treat as binding
                            }
                        }
                    }
                }
            }

            (Some(condition), field_bindings)
        }
        Pattern::Obj { pairs, .. } => {
            // Check: typeof/null/in checks for each key
            let mut conditions = vec![
                list(vec![
                    atom("==="),
                    list(vec![atom("typeof"), atom(target_var)]),
                    str_lit("object"),
                ]),
                list(vec![atom("!=="), atom(target_var), atom("null")]),
            ];

            let mut bindings = Vec::new();

            for (key, pat) in pairs {
                conditions.push(list(vec![atom("in"), str_lit(key), atom(target_var)]));

                match pat {
                    Pattern::Binding { name, .. } => {
                        bindings.push(list(vec![
                            atom("const"),
                            atom(name),
                            atom(&format!("{target_var}:{key}")),
                        ]));
                    }
                    Pattern::Wildcard(_) => {}
                    _ => {}
                }
            }

            let condition = if conditions.len() == 1 {
                conditions.into_iter().next().unwrap()
            } else {
                let mut and_expr = vec![atom("&&")];
                and_expr.extend(conditions);
                list(and_expr)
            };

            (Some(condition), bindings)
        }
    }
}

// ---------------------------------------------------------------------------
// IfLet / WhenLet emission
// ---------------------------------------------------------------------------

fn emit_if_let(
    pattern: &Pattern,
    expr: &SExpr,
    then_body: &SExpr,
    else_body: Option<&SExpr>,
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    // Await detection
    let expr_has_await = contains_await(expr);
    let body_has_await = contains_await(then_body) || else_body.is_some_and(contains_await);

    // Strategy 1: hoist expr if it contains await
    let (hoist_stmt, init_expr) = if expr_has_await {
        let hoisted_var = ctx.gensym.next("await");
        let hoist = list(vec![
            atom("const"),
            atom(&hoisted_var),
            emit_expr(expr, ctx, registry),
        ]);
        (Some(hoist), atom(&hoisted_var))
    } else {
        (None, emit_expr(expr, ctx, registry))
    };

    // IIFE: ((=> () (const _t expr) (if <check> (block [bindings] (return then)) (block (return else)))))
    let t = ctx.gensym.next("t");
    let mut body = vec![
        atom("=>"),
        list(vec![]),
        list(vec![atom("const"), atom(&t), init_expr]),
    ];

    let (condition, bindings) = compile_let_pattern(pattern, &t, registry);

    let mut then_block = vec![atom("block")];
    then_block.extend(bindings);
    then_block.push(list(vec![
        atom("return"),
        emit_expr(then_body, ctx, registry),
    ]));

    if let Some(else_expr) = else_body {
        let else_block = list(vec![
            atom("block"),
            list(vec![atom("return"), emit_expr(else_expr, ctx, registry)]),
        ]);
        body.push(list(vec![
            atom("if"),
            condition,
            list(then_block),
            else_block,
        ]));
    } else {
        body.push(list(vec![atom("if"), condition, list(then_block)]));
    }

    // Strategy 2: wrap IIFE in async + await if body contains await
    let iife = if body_has_await {
        wrap_iife_async(body)
    } else {
        list(vec![list(body)])
    };

    // Combine: if we hoisted, emit block with hoist + iife
    if let Some(hoist) = hoist_stmt {
        list(vec![atom("block"), hoist, iife])
    } else {
        iife
    }
}

fn emit_when_let(
    pattern: &Pattern,
    expr: &SExpr,
    body_exprs: &[SExpr],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    // Await detection
    let expr_has_await = contains_await(expr);
    let body_has_await = any_contains_await(body_exprs);

    // Strategy 1: hoist expr if it contains await
    let (hoist_stmt, init_expr) = if expr_has_await {
        let hoisted_var = ctx.gensym.next("await");
        let hoist = list(vec![
            atom("const"),
            atom(&hoisted_var),
            emit_expr(expr, ctx, registry),
        ]);
        (Some(hoist), atom(&hoisted_var))
    } else {
        (None, emit_expr(expr, ctx, registry))
    };

    // IIFE: ((=> () (const _t expr) (if <check> (block [bindings] (return body)))))
    let t = ctx.gensym.next("t");
    let mut body = vec![
        atom("=>"),
        list(vec![]),
        list(vec![atom("const"), atom(&t), init_expr]),
    ];

    let (condition, bindings) = compile_let_pattern(pattern, &t, registry);

    let mut then_block = vec![atom("block")];
    then_block.extend(bindings);

    let emitted = emit_body(body_exprs, ctx, registry);
    for (i, expr) in emitted.iter().enumerate() {
        if i == emitted.len() - 1 {
            then_block.push(list(vec![atom("return"), expr.clone()]));
        } else {
            then_block.push(expr.clone());
        }
    }

    body.push(list(vec![atom("if"), condition, list(then_block)]));

    // Strategy 2: wrap IIFE in async + await if body contains await
    let iife = if body_has_await {
        wrap_iife_async(body)
    } else {
        list(vec![list(body)])
    };

    // Combine: if we hoisted, emit block with hoist + iife
    if let Some(hoist) = hoist_stmt {
        list(vec![atom("block"), hoist, iife])
    } else {
        iife
    }
}

/// Compile a pattern for `if-let` / `when-let`.
///
/// Returns a (condition, bindings) pair. Unlike match patterns, these always
/// produce a condition (even for simple bindings, which check `!= null`).
fn compile_let_pattern(
    pattern: &Pattern,
    target_var: &str,
    registry: &TypeRegistry,
) -> (SExpr, Vec<SExpr>) {
    match pattern {
        Pattern::Constructor { name, bindings, .. } => {
            // ADT pattern: tag check + field bindings
            let condition = list(vec![
                atom("==="),
                atom(&format!("{target_var}:tag")),
                str_lit(name),
            ]);

            let mut field_bindings = Vec::new();
            if let Some(ctor_def) = registry.lookup_constructor(name) {
                for (i, binding_pat) in bindings.iter().enumerate() {
                    if i < ctor_def.fields.len() {
                        let field_name = &ctor_def.fields[i].name;
                        if let Pattern::Binding {
                            name: bind_name, ..
                        } = binding_pat
                        {
                            field_bindings.push(list(vec![
                                atom("const"),
                                atom(bind_name),
                                atom(&format!("{target_var}:{field_name}")),
                            ]));
                        }
                    }
                }
            }

            (condition, field_bindings)
        }
        Pattern::Obj { pairs, .. } => {
            // Object pattern: typeof/null/in checks
            let mut conditions = vec![
                list(vec![
                    atom("==="),
                    list(vec![atom("typeof"), atom(target_var)]),
                    str_lit("object"),
                ]),
                list(vec![atom("!=="), atom(target_var), atom("null")]),
            ];

            let mut bindings = Vec::new();

            for (key, pat) in pairs {
                conditions.push(list(vec![atom("in"), str_lit(key), atom(target_var)]));
                if let Pattern::Binding { name, .. } = pat {
                    bindings.push(list(vec![
                        atom("const"),
                        atom(name),
                        atom(&format!("{target_var}:{key}")),
                    ]));
                }
            }

            let condition = if conditions.len() == 1 {
                conditions.into_iter().next().unwrap()
            } else {
                let mut and_expr = vec![atom("&&")];
                and_expr.extend(conditions);
                list(and_expr)
            };

            (condition, bindings)
        }
        Pattern::Binding { name, .. } => {
            // Simple binding: (!= t null) check
            let condition = list(vec![atom("!="), atom(target_var), atom("null")]);
            let binding = list(vec![atom("const"), atom(name), atom(target_var)]);
            (condition, vec![binding])
        }
        Pattern::Wildcard(_) => {
            // Always true
            (bool_lit(true), vec![])
        }
        Pattern::Literal(expr) => {
            let condition = list(vec![atom("==="), atom(target_var), expr.clone()]);
            (condition, vec![])
        }
    }
}

// ---------------------------------------------------------------------------
// FunctionCall emission
// ---------------------------------------------------------------------------

fn emit_function_call(
    head: &SExpr,
    args: &[SExpr],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let mut items = vec![emit_expr(head, ctx, registry)];
    for arg in args {
        items.push(emit_expr(arg, ctx, registry));
    }
    list(items)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::type_registry::{ConstructorDef, FieldDef, TypeDef, TypeRegistry};
    use crate::ast::surface::{
        Constructor, FuncClause, MatchClause, Pattern, SurfaceForm, ThreadingStep, TypeAnnotation,
        TypedParam,
    };
    use crate::emitter::context::EmitterContext;

    fn s() -> Span {
        Span::default()
    }

    fn ta(name: &str) -> TypeAnnotation {
        TypeAnnotation {
            name: name.to_string(),
            span: s(),
        }
    }

    fn tp(type_name: &str, param_name: &str) -> TypedParam {
        TypedParam {
            type_ann: ta(type_name),
            name: param_name.to_string(),
            name_span: s(),
        }
    }

    fn ctx() -> EmitterContext {
        EmitterContext::new(false)
    }

    fn reg() -> TypeRegistry {
        TypeRegistry::default()
    }

    // --- Bind ---

    #[test]
    fn test_emit_bind() {
        let form = SurfaceForm::Bind {
            name: atom("x"),
            type_ann: None,
            value: num(42.0),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("const"));
            assert_eq!(values[1].as_atom(), Some("x"));
        } else {
            panic!("expected list");
        }
    }

    // --- Obj ---

    #[test]
    fn test_emit_obj() {
        let form = SurfaceForm::Obj {
            pairs: vec![("name".into(), str_lit("Alice")), ("age".into(), num(30.0))],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("object"));
            assert_eq!(values.len(), 3); // object + 2 pairs
        } else {
            panic!("expected list");
        }
    }

    // --- Cell ---

    #[test]
    fn test_emit_cell() {
        let form = SurfaceForm::Cell {
            value: num(0.0),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("object"));
            if let SExpr::List { values: pair, .. } = &values[1] {
                assert_eq!(pair[0].as_atom(), Some("value"));
            } else {
                panic!("expected pair");
            }
        } else {
            panic!("expected list");
        }
    }

    // --- Express ---

    #[test]
    fn test_emit_express() {
        let form = SurfaceForm::Express {
            target: atom("counter"),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_atom(), Some("counter:value"));
    }

    // --- Swap ---

    #[test]
    fn test_emit_swap() {
        let form = SurfaceForm::Swap {
            target: atom("counter"),
            func: atom("inc"),
            extra_args: vec![],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("="));
            assert_eq!(values[1].as_atom(), Some("counter:value"));
        } else {
            panic!("expected list");
        }
    }

    // --- Reset ---

    #[test]
    fn test_emit_reset() {
        let form = SurfaceForm::Reset {
            target: atom("counter"),
            value: num(0.0),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("="));
            assert_eq!(values[1].as_atom(), Some("counter:value"));
        } else {
            panic!("expected list");
        }
    }

    // --- ThreadFirst ---

    #[test]
    fn test_emit_thread_first_bare() {
        let form = SurfaceForm::ThreadFirst {
            initial: num(1.0),
            steps: vec![ThreadingStep::Bare(atom("inc"))],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (inc 1)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("inc"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_emit_thread_first_call() {
        let form = SurfaceForm::ThreadFirst {
            initial: num(1.0),
            steps: vec![ThreadingStep::Call(vec![atom("add"), num(2.0)])],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        // Should be (add 1 2)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("add"));
            // values[1] should be the initial (1.0)
            if let SExpr::Number { value, .. } = &values[1] {
                assert!((value - 1.0).abs() < f64::EPSILON);
            } else {
                panic!("expected number for threaded arg");
            }
        } else {
            panic!("expected list");
        }
    }

    // --- ThreadLast ---

    #[test]
    fn test_emit_thread_last_call() {
        let form = SurfaceForm::ThreadLast {
            initial: num(1.0),
            steps: vec![ThreadingStep::Call(vec![atom("add"), num(2.0)])],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        // Should be (add 2 1)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("add"));
            // Last arg should be the initial
            if let SExpr::Number { value, .. } = values.last().unwrap() {
                assert!((value - 1.0).abs() < f64::EPSILON);
            } else {
                panic!("expected number at last position");
            }
        } else {
            panic!("expected list");
        }
    }

    // --- Type ---

    #[test]
    fn test_emit_type_zero_fields() {
        let form = SurfaceForm::Type {
            name: "Color".into(),
            name_span: s(),
            constructors: vec![Constructor {
                name: "Red".into(),
                name_span: s(),
                fields: vec![],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // (const Red (object (tag "Red")))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("const"));
            assert_eq!(values[1].as_atom(), Some("Red"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_emit_type_with_fields() {
        let form = SurfaceForm::Type {
            name: "Pair".into(),
            name_span: s(),
            constructors: vec![Constructor {
                name: "Pair".into(),
                name_span: s(),
                fields: vec![tp("number", "x"), tp("number", "y")],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // (function Pair (x y) [checks] (return (object ...)))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("function"));
            assert_eq!(values[1].as_atom(), Some("Pair"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_emit_type_multiple_constructors_wrapped_in_block() {
        let form = SurfaceForm::Type {
            name: "Color".into(),
            name_span: s(),
            constructors: vec![
                Constructor {
                    name: "Red".into(),
                    name_span: s(),
                    fields: vec![],
                    span: s(),
                },
                Constructor {
                    name: "Green".into(),
                    name_span: s(),
                    fields: vec![],
                    span: s(),
                },
            ],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be wrapped in (block ...)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("block"));
            assert_eq!(values.len(), 3); // block + 2 constructors
        } else {
            panic!("expected block list");
        }
    }

    // --- Fn ---

    #[test]
    fn test_emit_fn() {
        let form = SurfaceForm::Fn {
            params: vec![tp("number", "x")],
            body: vec![list(vec![atom("+"), atom("x"), num(1.0)])],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("=>"));
        } else {
            panic!("expected arrow function");
        }
    }

    // --- Func single clause ---

    #[test]
    fn test_emit_func_single_clause() {
        let form = SurfaceForm::Func {
            name: "add".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![tp("number", "a"), tp("number", "b")],
                returns: None,
                pre: None,
                post: None,
                body: vec![list(vec![atom("+"), atom("a"), atom("b")])],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("function"));
            assert_eq!(values[1].as_atom(), Some("add"));
        } else {
            panic!("expected function");
        }
    }

    // --- Func multi-clause ---

    #[test]
    fn test_emit_func_multi_clause() {
        let form = SurfaceForm::Func {
            name: "f".into(),
            name_span: s(),
            clauses: vec![
                FuncClause {
                    args: vec![tp("number", "x")],
                    returns: None,
                    pre: None,
                    post: None,
                    body: vec![atom("x")],
                    span: s(),
                },
                FuncClause {
                    args: vec![tp("string", "s")],
                    returns: None,
                    pre: None,
                    post: None,
                    body: vec![atom("s")],
                    span: s(),
                },
            ],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("function"));
            assert_eq!(values[1].as_atom(), Some("f"));
            // Third element: ((rest args))
            if let SExpr::List { values: params, .. } = &values[2] {
                if let SExpr::List { values: rest, .. } = &params[0] {
                    assert_eq!(rest[0].as_atom(), Some("rest"));
                } else {
                    panic!("expected rest param");
                }
            } else {
                panic!("expected params list");
            }
        } else {
            panic!("expected function");
        }
    }

    // --- Match ---

    #[test]
    fn test_emit_match_literal() {
        let form = SurfaceForm::Match {
            target: atom("x"),
            clauses: vec![
                MatchClause {
                    pattern: Pattern::Literal(num(1.0)),
                    guard: None,
                    body: vec![str_lit("one")],
                    span: s(),
                },
                MatchClause {
                    pattern: Pattern::Wildcard(s()),
                    guard: None,
                    body: vec![str_lit("other")],
                    span: s(),
                },
            ],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // IIFE: ((=> () ...))
        if let SExpr::List { values, .. } = &result[0] {
            if let SExpr::List { values: arrow, .. } = &values[0] {
                assert_eq!(arrow[0].as_atom(), Some("=>"));
            } else {
                panic!("expected arrow inside IIFE");
            }
        } else {
            panic!("expected IIFE list");
        }
    }

    // --- Match with constructor ---

    #[test]
    fn test_emit_match_constructor() {
        let mut r = reg();
        r.register_type(TypeDef {
            name: "Option".into(),
            module_path: None,
            constructors: vec![
                ConstructorDef {
                    name: "Some".into(),
                    fields: vec![FieldDef {
                        name: "value".into(),
                        type_keyword: "any".into(),
                    }],
                    owning_type: "Option".into(),
                    span: s(),
                },
                ConstructorDef {
                    name: "None".into(),
                    fields: vec![],
                    owning_type: "Option".into(),
                    span: s(),
                },
            ],
            is_blessed: false,
            span: s(),
        })
        .unwrap();

        let form = SurfaceForm::Match {
            target: atom("x"),
            clauses: vec![
                MatchClause {
                    pattern: Pattern::Constructor {
                        name: "Some".into(),
                        name_span: s(),
                        bindings: vec![Pattern::Binding {
                            name: "v".into(),
                            span: s(),
                        }],
                        span: s(),
                    },
                    guard: None,
                    body: vec![atom("v")],
                    span: s(),
                },
                MatchClause {
                    pattern: Pattern::Wildcard(s()),
                    guard: None,
                    body: vec![atom("null")],
                    span: s(),
                },
            ],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &r);
        assert_eq!(result.len(), 1);
    }

    // --- KernelPassthrough ---

    #[test]
    fn test_emit_kernel_passthrough() {
        let raw = list(vec![atom("import"), str_lit("./foo.js")]);
        let form = SurfaceForm::KernelPassthrough {
            raw: raw.clone(),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], raw);
    }

    // --- FunctionCall ---

    #[test]
    fn test_emit_function_call() {
        let form = SurfaceForm::FunctionCall {
            head: atom("add"),
            args: vec![num(1.0), num(2.0)],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("add"));
            assert_eq!(values.len(), 3);
        } else {
            panic!("expected list");
        }
    }

    // --- SomeThreadFirst ---

    #[test]
    fn test_emit_some_thread_first() {
        let form = SurfaceForm::SomeThreadFirst {
            initial: atom("x"),
            steps: vec![ThreadingStep::Bare(atom("inc"))],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be IIFE
        if let SExpr::List { values, .. } = &result[0] {
            if let SExpr::List { values: arrow, .. } = &values[0] {
                assert_eq!(arrow[0].as_atom(), Some("=>"));
            } else {
                panic!("expected arrow inside IIFE");
            }
        } else {
            panic!("expected IIFE");
        }
    }

    // --- IfLet ---

    #[test]
    fn test_emit_if_let() {
        let form = SurfaceForm::IfLet {
            pattern: Pattern::Binding {
                name: "v".into(),
                span: s(),
            },
            expr: atom("x"),
            then_body: atom("v"),
            else_body: Some(num(0.0)),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // IIFE
        if let SExpr::List { values, .. } = &result[0] {
            if let SExpr::List { values: arrow, .. } = &values[0] {
                assert_eq!(arrow[0].as_atom(), Some("=>"));
            } else {
                panic!("expected arrow inside IIFE");
            }
        } else {
            panic!("expected IIFE");
        }
    }

    // --- WhenLet ---

    #[test]
    fn test_emit_when_let() {
        let form = SurfaceForm::WhenLet {
            pattern: Pattern::Binding {
                name: "v".into(),
                span: s(),
            },
            expr: atom("x"),
            body: vec![atom("v")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // IIFE
        if let SExpr::List { values, .. } = &result[0] {
            if let SExpr::List { values: arrow, .. } = &values[0] {
                assert_eq!(arrow[0].as_atom(), Some("=>"));
            } else {
                panic!("expected arrow inside IIFE");
            }
        } else {
            panic!("expected IIFE");
        }
    }

    // --- Strip assertions ---

    #[test]
    fn test_emit_func_strip_assertions_omits_type_checks() {
        let form = SurfaceForm::Func {
            name: "add".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![tp("number", "a"), tp("number", "b")],
                returns: None,
                pre: Some(list(vec![atom(">"), atom("a"), num(0.0)])),
                post: None,
                body: vec![list(vec![atom("+"), atom("a"), atom("b")])],
                span: s(),
            }],
            span: s(),
        };
        let mut c = EmitterContext::new(true);
        let result = emit_form(&form, &mut c, &reg());

        // With strip_assertions, no type checks or pre-checks
        if let SExpr::List { values, .. } = &result[0] {
            // function, name, params, (return ...)
            // No "if" nodes for type checks or pre
            let has_if = values.iter().any(|v| v.as_atom() == Some("if"));
            assert!(
                !has_if,
                "should not have if nodes when stripping assertions"
            );
        } else {
            panic!("expected list");
        }
    }

    // --- Lambda emits same as Fn ---

    #[test]
    fn test_emit_lambda_same_as_fn() {
        let params = vec![tp("any", "x")];
        let body = vec![atom("x")];

        let fn_form = SurfaceForm::Fn {
            params: params.clone(),
            body: body.clone(),
            span: s(),
        };
        let lambda_form = SurfaceForm::Lambda {
            params,
            body,
            span: s(),
        };

        let mut c1 = EmitterContext::new(true);
        let mut c2 = EmitterContext::new(true);
        let r = reg();

        let fn_result = emit_form(&fn_form, &mut c1, &r);
        let lambda_result = emit_form(&lambda_form, &mut c2, &r);

        assert_eq!(fn_result, lambda_result);
    }

    // --- contains_await detection ---

    #[test]
    fn test_contains_await_simple_atom() {
        assert!(!contains_await(&atom("x")));
        assert!(!contains_await(&num(42.0)));
        assert!(!contains_await(&str_lit("hello")));
    }

    #[test]
    fn test_contains_await_direct() {
        let expr = list(vec![atom("await"), list(vec![atom("fetch"), atom("url")])]);
        assert!(contains_await(&expr));
    }

    #[test]
    fn test_contains_await_nested() {
        let inner = list(vec![atom("await"), atom("p")]);
        let outer = list(vec![atom("then"), inner, atom("callback")]);
        assert!(contains_await(&outer));
    }

    #[test]
    fn test_contains_await_absent() {
        let expr = list(vec![atom("fetch"), atom("url")]);
        assert!(!contains_await(&expr));
    }

    #[test]
    fn test_contains_await_deeply_nested() {
        let deep = list(vec![
            atom("+"),
            num(1.0),
            list(vec![
                atom("*"),
                num(2.0),
                list(vec![atom("await"), list(vec![atom("get-value")])]),
            ]),
        ]);
        assert!(contains_await(&deep));
    }

    // --- Match with await in target (Strategy 1) ---

    #[test]
    fn test_match_with_await_in_target() {
        let form = SurfaceForm::Match {
            target: list(vec![atom("await"), list(vec![atom("fetch"), atom("url")])]),
            clauses: vec![MatchClause {
                pattern: Pattern::Wildcard(s()),
                guard: None,
                body: vec![str_lit("done")],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (block (const await__gensymN ...) ((=> () ...)))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("block"));
            // Second element: (const await__gensym0 (await (fetch url)))
            if let SExpr::List { values: hoist, .. } = &values[1] {
                assert_eq!(hoist[0].as_atom(), Some("const"));
                assert!(
                    hoist[1].as_atom().unwrap().starts_with("await__gensym"),
                    "hoisted var should start with await__gensym"
                );
            } else {
                panic!("expected hoist const");
            }
            // Third element: IIFE ((=> () ...))
            if let SExpr::List { values: iife, .. } = &values[2] {
                if let SExpr::List { values: arrow, .. } = &iife[0] {
                    assert_eq!(arrow[0].as_atom(), Some("=>"));
                } else {
                    panic!("expected arrow inside IIFE");
                }
            } else {
                panic!("expected IIFE");
            }
        } else {
            panic!("expected block wrapping hoist + IIFE");
        }
    }

    // --- Match with await in body (Strategy 2) ---

    #[test]
    fn test_match_with_await_in_body() {
        let form = SurfaceForm::Match {
            target: atom("response"),
            clauses: vec![MatchClause {
                pattern: Pattern::Wildcard(s()),
                guard: None,
                body: vec![list(vec![
                    atom("await"),
                    list(vec![atom("fetch"), atom("url")]),
                ])],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (await ((async (=> () ...))))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("await"), "outer should be await");
            // values[1] should be ((async (=> () ...)))
            if let SExpr::List { values: call, .. } = &values[1] {
                if let SExpr::List {
                    values: async_arrow,
                    ..
                } = &call[0]
                {
                    assert_eq!(async_arrow[0].as_atom(), Some("async"));
                } else {
                    panic!("expected async arrow");
                }
            } else {
                panic!("expected call expression");
            }
        } else {
            panic!("expected await expression");
        }
    }

    // --- Match with await in both target and body ---

    #[test]
    fn test_match_with_await_in_both() {
        let form = SurfaceForm::Match {
            target: list(vec![atom("await"), list(vec![atom("fetch"), atom("url")])]),
            clauses: vec![MatchClause {
                pattern: Pattern::Wildcard(s()),
                guard: None,
                body: vec![list(vec![atom("await"), list(vec![atom("process")])])],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (block (const await__gensym ...) (await ((async (=> () ...)))))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("block"));
            // Hoist
            if let SExpr::List { values: hoist, .. } = &values[1] {
                assert_eq!(hoist[0].as_atom(), Some("const"));
            } else {
                panic!("expected hoist");
            }
            // Async IIFE
            if let SExpr::List {
                values: await_expr, ..
            } = &values[2]
            {
                assert_eq!(await_expr[0].as_atom(), Some("await"));
            } else {
                panic!("expected await-wrapped IIFE");
            }
        } else {
            panic!("expected block");
        }
    }

    // --- some-> with await in initial (Strategy 1) ---

    #[test]
    fn test_some_thread_with_await_in_initial() {
        let form = SurfaceForm::SomeThreadFirst {
            initial: list(vec![atom("await"), list(vec![atom("fetch"), atom("url")])]),
            steps: vec![ThreadingStep::Bare(atom("inc"))],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (block (const await__gensym ...) ((=> () ...)))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("block"));
            if let SExpr::List { values: hoist, .. } = &values[1] {
                assert_eq!(hoist[0].as_atom(), Some("const"));
                assert!(hoist[1].as_atom().unwrap().starts_with("await__gensym"));
            } else {
                panic!("expected hoist const");
            }
        } else {
            panic!("expected block");
        }
    }

    // --- some-> with await in steps (Strategy 2) ---

    #[test]
    fn test_some_thread_with_await_in_steps() {
        let form = SurfaceForm::SomeThreadFirst {
            initial: atom("x"),
            steps: vec![ThreadingStep::Call(vec![
                atom("await"),
                list(vec![atom("fetch")]),
            ])],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (await ((async (=> () ...))))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("await"));
        } else {
            panic!("expected await-wrapped IIFE");
        }
    }

    // --- if-let with await in expr (Strategy 1) ---

    #[test]
    fn test_if_let_with_await_in_expr() {
        let form = SurfaceForm::IfLet {
            pattern: Pattern::Binding {
                name: "v".into(),
                span: s(),
            },
            expr: list(vec![atom("await"), list(vec![atom("fetch"), atom("url")])]),
            then_body: atom("v"),
            else_body: Some(num(0.0)),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (block (const await__gensym ...) ((=> () ...)))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("block"));
            if let SExpr::List { values: hoist, .. } = &values[1] {
                assert_eq!(hoist[0].as_atom(), Some("const"));
            } else {
                panic!("expected hoist");
            }
        } else {
            panic!("expected block");
        }
    }

    // --- if-let with await in body (Strategy 2) ---

    #[test]
    fn test_if_let_with_await_in_body() {
        let form = SurfaceForm::IfLet {
            pattern: Pattern::Binding {
                name: "v".into(),
                span: s(),
            },
            expr: atom("x"),
            then_body: list(vec![atom("await"), list(vec![atom("process"), atom("v")])]),
            else_body: None,
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (await ((async (=> () ...))))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("await"));
        } else {
            panic!("expected await-wrapped IIFE");
        }
    }

    // --- when-let with await in expr (Strategy 1) ---

    #[test]
    fn test_when_let_with_await_in_expr() {
        let form = SurfaceForm::WhenLet {
            pattern: Pattern::Binding {
                name: "v".into(),
                span: s(),
            },
            expr: list(vec![atom("await"), list(vec![atom("fetch"), atom("url")])]),
            body: vec![atom("v")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("block"));
        } else {
            panic!("expected block");
        }
    }

    // --- when-let with await in body (Strategy 2) ---

    #[test]
    fn test_when_let_with_await_in_body() {
        let form = SurfaceForm::WhenLet {
            pattern: Pattern::Binding {
                name: "v".into(),
                span: s(),
            },
            expr: atom("x"),
            body: vec![list(vec![
                atom("await"),
                list(vec![atom("process"), atom("v")]),
            ])],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("await"));
        } else {
            panic!("expected await-wrapped IIFE");
        }
    }

    // --- No-await cases still produce plain IIFEs ---

    #[test]
    fn test_match_without_await_unchanged() {
        let form = SurfaceForm::Match {
            target: atom("x"),
            clauses: vec![MatchClause {
                pattern: Pattern::Wildcard(s()),
                guard: None,
                body: vec![str_lit("done")],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be plain IIFE: ((=> () ...))
        if let SExpr::List { values, .. } = &result[0] {
            if let SExpr::List { values: arrow, .. } = &values[0] {
                assert_eq!(arrow[0].as_atom(), Some("=>"));
            } else {
                panic!("expected arrow inside IIFE");
            }
        } else {
            panic!("expected IIFE");
        }
    }
}
