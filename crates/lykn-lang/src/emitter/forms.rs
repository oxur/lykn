use crate::analysis::type_registry::TypeRegistry;
use crate::ast::sexpr::SExpr;
use crate::ast::surface::{
    Constructor, FuncClause, MatchClause, Pattern, SurfaceForm, ThreadingStep, TypedParam,
};
use crate::reader::source_loc::Span;

use super::context::{EmitterContext, ExprContext};
use super::contracts::{emit_post_check, emit_pre_check};
use super::type_checks::{emit_return_type_check, emit_type_check};

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
        SurfaceForm::Express { target, .. } => vec![emit_express(target, ctx, registry)],
        SurfaceForm::Swap {
            target,
            func,
            extra_args,
            ..
        } => vec![emit_swap(target, func, extra_args, ctx, registry)],
        SurfaceForm::Reset { target, value, .. } => vec![emit_reset(target, value, ctx, registry)],
        SurfaceForm::Set { target, value, .. } => {
            // (set! target:prop value) → (= target:prop value)
            vec![list(vec![
                atom("="),
                emit_expr(target, ctx, registry),
                emit_expr(value, ctx, registry),
            ])]
        }
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
                && name.starts_with("js:")
            {
                return vec![emit_js_interop(name, args, ctx, registry)];
            }
            vec![emit_function_call(head, args, ctx, registry)]
        }
        SurfaceForm::Conj { arr, value, .. } => vec![emit_conj(arr, value, ctx, registry)],
        SurfaceForm::Assoc { obj, pairs, .. } => vec![emit_assoc(obj, pairs, ctx, registry)],
        SurfaceForm::Dissoc { obj, keys, .. } => vec![emit_dissoc(obj, keys, ctx, registry)],
        SurfaceForm::Eq { args, .. } => vec![emit_eq(args, ctx, registry)],
        SurfaceForm::NotEq { left, right, .. } => vec![emit_not_eq(left, right, ctx, registry)],
        SurfaceForm::And { args, .. } => vec![emit_and(args, ctx, registry)],
        SurfaceForm::Or { args, .. } => vec![emit_or(args, ctx, registry)],
        SurfaceForm::Not { operand, .. } => vec![emit_not(operand, ctx, registry)],
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
                    // This subexpression is a surface form — classify and emit it.
                    // Nested surface forms are always in Value position (they need
                    // to produce a result for the enclosing expression).
                    let saved_ctx = ctx.expr_context;
                    ctx.expr_context = ExprContext::Value;
                    let result = match crate::classifier::classify_expr(expr) {
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
                    };
                    ctx.expr_context = saved_ctx;
                    result
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

/// Convert a kernel `(if cond then else)` form to a ternary `(? cond then else)`.
///
/// In JavaScript, `if` is a statement and cannot appear in expression position
/// (e.g. `const x = if (...) ...` is invalid). When we need to assign the result
/// of a conditional to a variable, we must use a ternary expression instead.
///
/// If the expression is not an `(if ...)` form, it is returned unchanged.
fn if_to_ternary(expr: SExpr) -> SExpr {
    if let SExpr::List { values, span } = &expr
        && values.len() >= 3
        && values[0].as_atom() == Some("if")
    {
        // (if cond then) or (if cond then else)
        let mut new_values = vec![atom("?")];
        new_values.extend(values[1..].iter().cloned());
        return SExpr::List {
            values: new_values,
            span: *span,
        };
    }
    expr
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

/// Resolve a cell target to a `:value` accessor.
///
/// For atoms, returns `name:value` directly. For complex expressions,
/// emits the expression and appends `:value` via property access.
fn resolve_cell_target(target: &SExpr, ctx: &mut EmitterContext, registry: &TypeRegistry) -> SExpr {
    if let SExpr::Atom { value, .. } = target {
        atom(&format!("{value}:value"))
    } else {
        // Complex expression: emit and access :value property
        list(vec![
            atom("get"),
            emit_expr(target, ctx, registry),
            str_lit("value"),
        ])
    }
}

fn emit_express(target: &SExpr, ctx: &mut EmitterContext, registry: &TypeRegistry) -> SExpr {
    resolve_cell_target(target, ctx, registry)
}

fn emit_swap(
    target: &SExpr,
    func: &SExpr,
    extra_args: &[SExpr],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let target_val = resolve_cell_target(target, ctx, registry);

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
    let target_val = resolve_cell_target(target, ctx, registry);
    list(vec![atom("="), target_val, emit_expr(value, ctx, registry)])
}

// ---------------------------------------------------------------------------
// Equality and logical operators (DD-22)
// ---------------------------------------------------------------------------

/// `(= a b)` → `(=== a b)`
/// `(= a b c)` → `(&& (=== a b) (=== b c))` (variadic, pairwise, left-fold)
fn emit_eq(args: &[SExpr], ctx: &mut EmitterContext, registry: &TypeRegistry) -> SExpr {
    let emitted: Vec<SExpr> = args.iter().map(|a| emit_expr(a, ctx, registry)).collect();

    if emitted.len() == 2 {
        return list(vec![atom("==="), emitted[0].clone(), emitted[1].clone()]);
    }

    // Variadic: pairwise comparisons, left-folded with &&
    let mut checks: Vec<SExpr> = Vec::new();
    for i in 0..emitted.len() - 1 {
        checks.push(list(vec![
            atom("==="),
            emitted[i].clone(),
            emitted[i + 1].clone(),
        ]));
    }
    let mut result = checks[0].clone();
    for check in &checks[1..] {
        result = list(vec![atom("&&"), result, check.clone()]);
    }
    result
}

/// `(!= a b)` → `(!== a b)`
fn emit_not_eq(
    left: &SExpr,
    right: &SExpr,
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    list(vec![
        atom("!=="),
        emit_expr(left, ctx, registry),
        emit_expr(right, ctx, registry),
    ])
}

/// `(and a b)` → `(&& a b)`
/// `(and a b c d)` → `(&& (&& (&& a b) c) d)` (variadic, left-fold)
fn emit_and(args: &[SExpr], ctx: &mut EmitterContext, registry: &TypeRegistry) -> SExpr {
    let emitted: Vec<SExpr> = args.iter().map(|a| emit_expr(a, ctx, registry)).collect();
    let mut result = emitted[0].clone();
    for arg in &emitted[1..] {
        result = list(vec![atom("&&"), result, arg.clone()]);
    }
    result
}

/// `(or a b)` → `(|| a b)`
/// `(or a b c d)` → `(|| (|| (|| a b) c) d)` (variadic, left-fold)
fn emit_or(args: &[SExpr], ctx: &mut EmitterContext, registry: &TypeRegistry) -> SExpr {
    let emitted: Vec<SExpr> = args.iter().map(|a| emit_expr(a, ctx, registry)).collect();
    let mut result = emitted[0].clone();
    for arg in &emitted[1..] {
        result = list(vec![atom("||"), result, arg.clone()]);
    }
    result
}

/// `(not x)` → `(! x)`
fn emit_not(operand: &SExpr, ctx: &mut EmitterContext, registry: &TypeRegistry) -> SExpr {
    list(vec![atom("!"), emit_expr(operand, ctx, registry)])
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
///   (return _rest0)))
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
    let ret = list(vec![atom("return"), atom(&rest_var)]);
    let arrow = list(vec![atom("=>"), list(vec![]), binding, ret]);
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
        ThreadingStep::Bare(expr) => {
            if let SExpr::Keyword { value, .. } = expr {
                // Bare keyword: method call on acc — :method → (. acc method)
                list(vec![atom("."), acc, atom(value)])
            } else {
                list(vec![emit_expr(expr, ctx, registry), acc])
            }
        }
        ThreadingStep::Call(exprs) => {
            if exprs.is_empty() {
                return acc;
            }
            // Check for keyword-headed step: (:method args...) → (. acc method args...)
            if let SExpr::Keyword { value, .. } = &exprs[0] {
                let rest: Vec<SExpr> = exprs[1..]
                    .iter()
                    .map(|e| emit_expr(e, ctx, registry))
                    .collect();
                let mut items = vec![atom("."), acc, atom(value)];
                items.extend(rest);
                return list(items);
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
    let mut has_type_checks = false;
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
                has_type_checks = true;
            }
        }
    }

    // When type checks are present, the arrow gets a block body, so we must
    // wrap the last body expression in (return ...) to preserve the return value.
    let emitted_body = emit_body(body, ctx, registry);
    if has_type_checks && !emitted_body.is_empty() {
        items.extend(emitted_body[..emitted_body.len() - 1].iter().cloned());
        let last = emitted_body.last().unwrap().clone();
        items.push(list(vec![atom("return"), last]));
    } else {
        items.extend(emitted_body);
    }
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
        items.push(list(vec![
            atom("const"),
            atom(&result_var),
            if_to_ternary(last_expr),
        ]));
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
                items.push(list(vec![
                    atom("const"),
                    atom(&result_var),
                    if_to_ternary(last),
                ]));
                // Use "return value" in the error message instead of the
                // gensym variable name for a user-friendly message, but
                // still reference the gensym var for the typeof check.
                if let Some(check) = emit_return_type_check(&result_var, &ret.name, name, ret.span)
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
            block_items.push(list(vec![
                atom("const"),
                atom(&result_var),
                if_to_ternary(last),
            ]));
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
    match ctx.expr_context {
        ExprContext::Statement => emit_match_statement(target, clauses, ctx, registry),
        _ => emit_match_iife(target, clauses, ctx, registry),
    }
}

/// Emit a match as a bare if/else chain (statement position — result unused).
fn emit_match_statement(
    target: &SExpr,
    clauses: &[MatchClause],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let target_var = ctx.gensym.next("target");
    let target_binding = list(vec![
        atom("const"),
        atom(&target_var),
        emit_expr(target, ctx, registry),
    ]);

    // Collect (condition, block) tuples; stop at wildcard
    let mut clause_data: Vec<(Option<SExpr>, SExpr)> = Vec::new();
    let mut wildcard_block: Option<SExpr> = None;

    for clause in clauses {
        let (condition, bindings) = compile_pattern(&clause.pattern, &target_var, registry);
        let clause_body = emit_body(&clause.body, ctx, registry);

        let mut block_items = vec![atom("block")];
        block_items.extend(bindings);

        if let Some(ref guard) = clause.guard {
            let mut inner_block = vec![atom("block")];
            inner_block.extend(clause_body);
            block_items.push(list(vec![atom("if"), guard.clone(), list(inner_block)]));
        } else {
            block_items.extend(clause_body);
        }

        let block = list(block_items);

        if condition.is_some() {
            clause_data.push((condition, block));
        } else {
            wildcard_block = Some(block);
            break;
        }
    }

    // Build the else-chain fallback
    let fallback = wildcard_block.unwrap_or_else(|| {
        list(vec![
            atom("block"),
            list(vec![
                atom("throw"),
                list(vec![
                    atom("new"),
                    atom("Error"),
                    str_lit("match: no matching pattern"),
                ]),
            ]),
        ])
    });

    // Build nested if/else from the end
    let mut chain = fallback;
    for (cond, block) in clause_data.into_iter().rev() {
        chain = list(vec![atom("if"), cond.unwrap(), block, chain]);
    }

    list(vec![atom("block"), target_binding, chain])
}

/// Emit a match as an IIFE (value position — result needed).
fn emit_match_iife(
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
            let mut inner_block = vec![atom("block")];
            for (i, expr) in clause_body.iter().enumerate() {
                if i == clause_body.len() - 1 {
                    inner_block.push(list(vec![atom("return"), expr.clone()]));
                } else {
                    inner_block.push(expr.clone());
                }
            }
            block_items.push(list(vec![atom("if"), guard.clone(), list(inner_block)]));
        } else {
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
            has_wildcard = true;
            body.push(list(block_items));
            break;
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
    match ctx.expr_context {
        ExprContext::Statement => {
            emit_if_let_statement(pattern, expr, then_body, else_body, ctx, registry)
        }
        _ => emit_if_let_iife(pattern, expr, then_body, else_body, ctx, registry),
    }
}

/// Emit if-let as a bare if/else (statement position).
fn emit_if_let_statement(
    pattern: &Pattern,
    expr: &SExpr,
    then_body: &SExpr,
    else_body: Option<&SExpr>,
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let t = ctx.gensym.next("t");
    let binding = list(vec![
        atom("const"),
        atom(&t),
        emit_expr(expr, ctx, registry),
    ]);

    let (condition, bindings) = compile_let_pattern(pattern, &t, registry);

    let mut then_block = vec![atom("block")];
    then_block.extend(bindings);
    then_block.push(emit_expr(then_body, ctx, registry));

    let if_form = if let Some(else_expr) = else_body {
        let else_block = list(vec![atom("block"), emit_expr(else_expr, ctx, registry)]);
        list(vec![atom("if"), condition, list(then_block), else_block])
    } else {
        list(vec![atom("if"), condition, list(then_block)])
    };

    list(vec![atom("block"), binding, if_form])
}

/// Emit if-let as an IIFE (value position).
fn emit_if_let_iife(
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

    let iife = if body_has_await {
        wrap_iife_async(body)
    } else {
        list(vec![list(body)])
    };

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
    match ctx.expr_context {
        ExprContext::Statement => emit_when_let_statement(pattern, expr, body_exprs, ctx, registry),
        _ => emit_when_let_iife(pattern, expr, body_exprs, ctx, registry),
    }
}

/// Emit when-let as a bare if (statement position).
fn emit_when_let_statement(
    pattern: &Pattern,
    expr: &SExpr,
    body_exprs: &[SExpr],
    ctx: &mut EmitterContext,
    registry: &TypeRegistry,
) -> SExpr {
    let t = ctx.gensym.next("t");
    let binding = list(vec![
        atom("const"),
        atom(&t),
        emit_expr(expr, ctx, registry),
    ]);

    let (condition, bindings) = compile_let_pattern(pattern, &t, registry);

    let mut then_block = vec![atom("block")];
    then_block.extend(bindings);
    then_block.extend(emit_body(body_exprs, ctx, registry));

    list(vec![
        atom("block"),
        binding,
        list(vec![atom("if"), condition, list(then_block)]),
    ])
}

/// Emit when-let as an IIFE (value position).
fn emit_when_let_iife(
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

    let iife = if body_has_await {
        wrap_iife_async(body)
    } else {
        list(vec![list(body)])
    };

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

    fn ctx_value() -> EmitterContext {
        let mut c = EmitterContext::new(false);
        c.expr_context = ExprContext::Value;
        c
    }

    fn reg() -> TypeRegistry {
        TypeRegistry::default()
    }

    // --- if_to_ternary ---

    #[test]
    fn test_if_to_ternary_converts_if_form() {
        let expr = list(vec![atom("if"), atom("cond"), atom("a"), atom("b")]);
        let result = if_to_ternary(expr);
        if let SExpr::List { values, .. } = &result {
            assert_eq!(values[0].as_atom(), Some("?"));
            assert_eq!(values[1].as_atom(), Some("cond"));
            assert_eq!(values[2].as_atom(), Some("a"));
            assert_eq!(values[3].as_atom(), Some("b"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_if_to_ternary_passes_through_non_if() {
        let expr = list(vec![atom("foo"), atom("bar")]);
        let result = if_to_ternary(expr.clone());
        assert_eq!(result, expr);
    }

    #[test]
    fn test_if_to_ternary_passes_through_atom() {
        let expr = atom("x");
        let result = if_to_ternary(expr.clone());
        assert_eq!(result, expr);
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
        // Statement context: bare if/else chain
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("block"));
            // Second element: (const target__gensymN x)
            if let SExpr::List {
                values: binding, ..
            } = &values[1]
            {
                assert_eq!(binding[0].as_atom(), Some("const"));
            } else {
                panic!("expected const binding");
            }
            // Third element: (if ...)
            if let SExpr::List {
                values: if_form, ..
            } = &values[2]
            {
                assert_eq!(if_form[0].as_atom(), Some("if"));
            } else {
                panic!("expected if form");
            }
        } else {
            panic!("expected block");
        }
    }

    #[test]
    fn test_emit_match_literal_value_context() {
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
        // Value context: IIFE
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
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
        // Statement context: bare if/else
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("block"));
        } else {
            panic!("expected block");
        }
    }

    #[test]
    fn test_emit_if_let_value_context() {
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
        // Value context: IIFE
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
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
        // Statement context: bare if
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("block"));
        } else {
            panic!("expected block");
        }
    }

    #[test]
    fn test_emit_when_let_value_context() {
        let form = SurfaceForm::WhenLet {
            pattern: Pattern::Binding {
                name: "v".into(),
                span: s(),
            },
            expr: atom("x"),
            body: vec![atom("v")],
            span: s(),
        };
        // Value context: IIFE
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
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
        let mut c = ctx_value();
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
        let mut c = ctx_value();
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
        let mut c = ctx_value();
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
        let mut c = ctx_value();
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
        let mut c = ctx_value();
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
        // Value context: plain IIFE (no async wrapper)
        let mut c = ctx_value();
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

    // =======================================================================
    // js: namespace interop forms
    // =======================================================================

    #[test]
    fn test_js_call() {
        let form = SurfaceForm::FunctionCall {
            head: atom("js:call"),
            args: vec![atom("console:log"), str_lit("hello")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // (js:call console:log "hello") -> (console:log "hello")
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("console:log"));
            if let SExpr::String { value, .. } = &values[1] {
                assert_eq!(value, "hello");
            } else {
                panic!("expected string arg");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_js_bind_with_this() {
        let form = SurfaceForm::FunctionCall {
            head: atom("js:bind"),
            args: vec![atom("obj:method"), atom("obj")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // (js:bind obj:method obj) -> (obj:method:bind obj)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("obj:method:bind"));
            assert_eq!(values[1].as_atom(), Some("obj"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_js_bind_without_this() {
        // Only method, no this arg -> uses "undefined"
        let form = SurfaceForm::FunctionCall {
            head: atom("js:bind"),
            args: vec![atom("obj:method")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("obj:method:bind"));
            assert_eq!(values[1].as_atom(), Some("undefined"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_js_bind_fallback_non_atom() {
        // First arg is not an atom -> fallback passthrough
        let form = SurfaceForm::FunctionCall {
            head: atom("js:bind"),
            args: vec![num(42.0)],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("js:bind"));
        } else {
            panic!("expected fallback list");
        }
    }

    #[test]
    fn test_js_eval() {
        let form = SurfaceForm::FunctionCall {
            head: atom("js:eval"),
            args: vec![str_lit("1 + 2")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // (js:eval "1 + 2") -> (eval "1 + 2")
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("eval"));
            if let SExpr::String { value, .. } = &values[1] {
                assert_eq!(value, "1 + 2");
            } else {
                panic!("expected string");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_js_eval_no_args() {
        let form = SurfaceForm::FunctionCall {
            head: atom("js:eval"),
            args: vec![],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("eval"));
            assert_eq!(values[1].as_atom(), Some("undefined"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_js_eq() {
        let form = SurfaceForm::FunctionCall {
            head: atom("js:eq"),
            args: vec![atom("a"), atom("b")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // (js:eq a b) -> (== a b)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("=="));
            assert_eq!(values[1].as_atom(), Some("a"));
            assert_eq!(values[2].as_atom(), Some("b"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_js_eq_no_args() {
        let form = SurfaceForm::FunctionCall {
            head: atom("js:eq"),
            args: vec![],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("=="));
            assert_eq!(values[1].as_atom(), Some("undefined"));
            assert_eq!(values[2].as_atom(), Some("undefined"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_js_typeof() {
        let form = SurfaceForm::FunctionCall {
            head: atom("js:typeof"),
            args: vec![atom("x")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // (js:typeof x) -> (typeof x)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("typeof"));
            assert_eq!(values[1].as_atom(), Some("x"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_js_typeof_no_args() {
        let form = SurfaceForm::FunctionCall {
            head: atom("js:typeof"),
            args: vec![],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("typeof"));
            assert_eq!(values[1].as_atom(), Some("undefined"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_js_unknown_form_passthrough() {
        let form = SurfaceForm::FunctionCall {
            head: atom("js:unknown"),
            args: vec![atom("x")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("js:unknown"));
            assert_eq!(values[1].as_atom(), Some("x"));
        } else {
            panic!("expected passthrough list");
        }
    }

    // =======================================================================
    // Conj / Assoc / Dissoc
    // =======================================================================

    #[test]
    fn test_emit_conj() {
        let form = SurfaceForm::Conj {
            arr: atom("xs"),
            value: num(42.0),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // (conj xs 42) -> (array (spread xs) 42)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("array"));
            if let SExpr::List {
                values: spread_vals,
                ..
            } = &values[1]
            {
                assert_eq!(spread_vals[0].as_atom(), Some("spread"));
                assert_eq!(spread_vals[1].as_atom(), Some("xs"));
            } else {
                panic!("expected spread");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_emit_assoc() {
        let form = SurfaceForm::Assoc {
            obj: atom("m"),
            pairs: vec![("name".into(), str_lit("Alice"))],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // (assoc m :name "Alice") -> (object (spread m) (name "Alice"))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("object"));
            if let SExpr::List {
                values: spread_vals,
                ..
            } = &values[1]
            {
                assert_eq!(spread_vals[0].as_atom(), Some("spread"));
                assert_eq!(spread_vals[1].as_atom(), Some("m"));
            } else {
                panic!("expected spread");
            }
            if let SExpr::List { values: pair, .. } = &values[2] {
                assert_eq!(pair[0].as_atom(), Some("name"));
            } else {
                panic!("expected pair");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_emit_dissoc() {
        let form = SurfaceForm::Dissoc {
            obj: atom("m"),
            keys: vec!["name".into()],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be IIFE: ((=> () (const (object ...) m) (return rest_var)))
        if let SExpr::List { values, .. } = &result[0] {
            if let SExpr::List { values: arrow, .. } = &values[0] {
                assert_eq!(arrow[0].as_atom(), Some("=>"));
                // Body should have const with destructuring pattern
                if let SExpr::List {
                    values: const_form, ..
                } = &arrow[2]
                {
                    assert_eq!(const_form[0].as_atom(), Some("const"));
                } else {
                    panic!("expected const form in arrow body");
                }
                // Last element should be (return rest_var)
                if let SExpr::List {
                    values: ret_form, ..
                } = &arrow[3]
                {
                    assert_eq!(ret_form[0].as_atom(), Some("return"));
                } else {
                    panic!("expected return form in arrow body");
                }
            } else {
                panic!("expected arrow inside IIFE");
            }
        } else {
            panic!("expected IIFE");
        }
    }

    // =======================================================================
    // resolve_cell_target with complex (non-atom) target
    // =======================================================================

    #[test]
    fn test_express_complex_target() {
        // A non-atom target should emit (get <expr> "value")
        let form = SurfaceForm::Express {
            target: list(vec![atom("get-cell")]),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("get"));
            if let SExpr::String { value, .. } = &values[2] {
                assert_eq!(value, "value");
            } else {
                panic!("expected 'value' string");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_swap_complex_target() {
        let form = SurfaceForm::Swap {
            target: list(vec![atom("get-cell")]),
            func: atom("inc"),
            extra_args: vec![num(1.0)],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("="));
            // target_val should be (get ... "value")
            if let SExpr::List {
                values: get_form, ..
            } = &values[1]
            {
                assert_eq!(get_form[0].as_atom(), Some("get"));
            } else {
                panic!("expected get form for complex target");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_reset_complex_target() {
        let form = SurfaceForm::Reset {
            target: list(vec![atom("get-cell")]),
            value: num(0.0),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("="));
            if let SExpr::List {
                values: get_form, ..
            } = &values[1]
            {
                assert_eq!(get_form[0].as_atom(), Some("get"));
            } else {
                panic!("expected get form");
            }
        } else {
            panic!("expected list");
        }
    }

    // =======================================================================
    // Threading with Call steps (thread-last bare, empty Call)
    // =======================================================================

    #[test]
    fn test_thread_last_bare() {
        let form = SurfaceForm::ThreadLast {
            initial: num(1.0),
            steps: vec![ThreadingStep::Bare(atom("inc"))],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // (inc 1)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("inc"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_thread_first_empty_call_returns_acc() {
        // An empty Call step should return the accumulator unchanged
        let form = SurfaceForm::ThreadFirst {
            initial: num(1.0),
            steps: vec![ThreadingStep::Call(vec![])],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::Number { value, .. } = &result[0] {
            assert!((value - 1.0).abs() < f64::EPSILON);
        } else {
            panic!("expected number (acc returned unchanged)");
        }
    }

    #[test]
    fn test_thread_last_call_with_extra_args() {
        // (thread-last 1 (add 2 3)) -> (add 2 3 1)
        let form = SurfaceForm::ThreadLast {
            initial: num(1.0),
            steps: vec![ThreadingStep::Call(vec![atom("add"), num(2.0), num(3.0)])],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("add"));
            assert_eq!(values.len(), 4); // add 2 3 1
            // Last should be 1.0
            if let SExpr::Number { value, .. } = values.last().unwrap() {
                assert!((value - 1.0).abs() < f64::EPSILON);
            } else {
                panic!("expected number at end");
            }
        } else {
            panic!("expected list");
        }
    }

    // =======================================================================
    // some-> with no steps
    // =======================================================================

    #[test]
    fn test_some_thread_no_steps() {
        let form = SurfaceForm::SomeThreadFirst {
            initial: atom("x"),
            steps: vec![],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // IIFE that just returns the initial value
        if let SExpr::List { values, .. } = &result[0] {
            if let SExpr::List { values: arrow, .. } = &values[0] {
                assert_eq!(arrow[0].as_atom(), Some("=>"));
                // Should contain a return
                let has_return = arrow.iter().any(|v| {
                    if let SExpr::List { values, .. } = v {
                        values.first().and_then(|f| f.as_atom()) == Some("return")
                    } else {
                        false
                    }
                });
                assert!(has_return, "should have a return statement");
            } else {
                panic!("expected arrow");
            }
        } else {
            panic!("expected IIFE");
        }
    }

    #[test]
    fn test_some_thread_last() {
        let form = SurfaceForm::SomeThreadLast {
            initial: atom("x"),
            steps: vec![ThreadingStep::Bare(atom("inc"))],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be IIFE with null checks
        if let SExpr::List { values, .. } = &result[0] {
            if let SExpr::List { values: arrow, .. } = &values[0] {
                assert_eq!(arrow[0].as_atom(), Some("=>"));
            } else {
                panic!("expected arrow");
            }
        } else {
            panic!("expected IIFE");
        }
    }

    #[test]
    fn test_some_thread_multiple_steps() {
        let form = SurfaceForm::SomeThreadFirst {
            initial: atom("x"),
            steps: vec![
                ThreadingStep::Bare(atom("inc")),
                ThreadingStep::Bare(atom("double")),
            ],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // IIFE with intermediate null checks
        if let SExpr::List { values, .. } = &result[0] {
            if let SExpr::List { values: arrow, .. } = &values[0] {
                assert_eq!(arrow[0].as_atom(), Some("=>"));
                // Should have multiple const bindings and null checks
                let const_count = arrow
                    .iter()
                    .filter(|v| {
                        if let SExpr::List { values, .. } = v {
                            values.first().and_then(|f| f.as_atom()) == Some("const")
                        } else {
                            false
                        }
                    })
                    .count();
                // At least t0 and t1 const bindings
                assert!(const_count >= 2, "should have at least 2 const bindings");
            } else {
                panic!("expected arrow");
            }
        } else {
            panic!("expected IIFE");
        }
    }

    // =======================================================================
    // Func with return types (void, typed, any)
    // =======================================================================

    #[test]
    fn test_func_returns_void() {
        let form = SurfaceForm::Func {
            name: "log".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![tp("string", "msg")],
                returns: Some(ta("void")),
                pre: None,
                post: None,
                body: vec![list(vec![atom("console:log"), atom("msg")])],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            // void function: no return statement at the end
            let last = values.last().unwrap();
            if let SExpr::List { values: last_v, .. } = last {
                assert_ne!(
                    last_v[0].as_atom(),
                    Some("return"),
                    "void func should not have return"
                );
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_func_returns_typed() {
        let form = SurfaceForm::Func {
            name: "double".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![tp("number", "x")],
                returns: Some(ta("number")),
                pre: None,
                post: None,
                body: vec![list(vec![atom("*"), atom("x"), num(2.0)])],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            // Should have a return-type check before the return
            let has_return = values.iter().any(|v| {
                if let SExpr::List { values, .. } = v {
                    values.first().and_then(|f| f.as_atom()) == Some("return")
                } else {
                    false
                }
            });
            assert!(has_return, "typed return func should have return");
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_func_returns_any() {
        let form = SurfaceForm::Func {
            name: "identity".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![tp("any", "x")],
                returns: Some(ta("any")),
                pre: None,
                post: None,
                body: vec![atom("x")],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            // returns any: implicit return, no type check
            let last = values.last().unwrap();
            if let SExpr::List { values: last_v, .. } = last {
                assert_eq!(last_v[0].as_atom(), Some("return"));
            } else {
                panic!("expected return");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_func_returns_typed_strip_assertions() {
        let form = SurfaceForm::Func {
            name: "double".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![tp("number", "x")],
                returns: Some(ta("number")),
                pre: None,
                post: None,
                body: vec![list(vec![atom("*"), atom("x"), num(2.0)])],
                span: s(),
            }],
            span: s(),
        };
        let mut c = EmitterContext::new(true);
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            // With strip_assertions, just a plain return
            let last = values.last().unwrap();
            if let SExpr::List { values: last_v, .. } = last {
                assert_eq!(last_v[0].as_atom(), Some("return"));
            } else {
                panic!("expected return");
            }
        } else {
            panic!("expected list");
        }
    }

    // =======================================================================
    // Func with pre/post conditions
    // =======================================================================

    #[test]
    fn test_func_with_pre_condition() {
        let form = SurfaceForm::Func {
            name: "sqrt".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![tp("number", "x")],
                returns: None,
                pre: Some(list(vec![atom(">="), atom("x"), num(0.0)])),
                post: None,
                body: vec![list(vec![atom("Math:sqrt"), atom("x")])],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should have an if block for pre-condition check
        if let SExpr::List { values, .. } = &result[0] {
            let has_if = values.iter().any(|v| {
                if let SExpr::List { values, .. } = v {
                    values.first().and_then(|f| f.as_atom()) == Some("if")
                } else {
                    false
                }
            });
            assert!(has_if, "pre-condition should generate an if check");
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_func_with_post_condition() {
        let form = SurfaceForm::Func {
            name: "abs".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![tp("number", "x")],
                returns: None,
                pre: None,
                post: Some(list(vec![atom(">="), atom("%"), num(0.0)])),
                body: vec![list(vec![atom("Math:abs"), atom("x")])],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Post-condition: should capture result in gensym, check, then return
        if let SExpr::List { values, .. } = &result[0] {
            let has_return = values.iter().any(|v| {
                if let SExpr::List { values, .. } = v {
                    values.first().and_then(|f| f.as_atom()) == Some("return")
                } else {
                    false
                }
            });
            assert!(has_return, "post-condition path should return");
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_func_with_post_and_if_body_uses_ternary() {
        // When the body is an (if ...) form and the function has a :post condition,
        // the result capture must use a ternary (? ...) instead of an if statement,
        // because `const x = if (...)` is invalid JavaScript.
        let form = SurfaceForm::Func {
            name: "abs-val".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![tp("number", "x")],
                returns: None,
                pre: None,
                post: Some(list(vec![atom(">="), atom("%"), num(0.0)])),
                body: vec![list(vec![
                    atom("if"),
                    list(vec![atom("<"), atom("x"), num(0.0)]),
                    list(vec![atom("-"), num(0.0), atom("x")]),
                    atom("x"),
                ])],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            // Find the const assignment: (const result__gensymN ...)
            let const_form = values.iter().find(|v| {
                if let SExpr::List { values, .. } = v {
                    values.first().and_then(|f| f.as_atom()) == Some("const")
                } else {
                    false
                }
            });
            assert!(const_form.is_some(), "should have a const binding");
            if let SExpr::List { values: cv, .. } = const_form.unwrap() {
                // The third element (the value) should be (? ...) not (if ...)
                if let SExpr::List { values: val, .. } = &cv[2] {
                    assert_eq!(
                        val[0].as_atom(),
                        Some("?"),
                        "if-form in value position should be converted to ternary"
                    );
                } else {
                    panic!("expected list as const value");
                }
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_func_with_post_and_multi_body() {
        let form = SurfaceForm::Func {
            name: "process".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![tp("any", "x")],
                returns: None,
                pre: None,
                post: Some(list(vec![atom("!="), atom("%"), atom("null")])),
                body: vec![
                    list(vec![atom("console:log"), atom("x")]),
                    list(vec![atom("transform"), atom("x")]),
                ],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Multi-body + post: all but last as statements, last captured
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("function"));
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_func_returns_typed_multi_body() {
        let form = SurfaceForm::Func {
            name: "process".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![tp("any", "x")],
                returns: Some(ta("number")),
                pre: None,
                post: None,
                body: vec![
                    list(vec![atom("console:log"), atom("x")]),
                    list(vec![atom("+"), atom("x"), num(1.0)]),
                ],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_func_returns_typed_empty_body() {
        let form = SurfaceForm::Func {
            name: "noop".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![],
                returns: Some(ta("number")),
                pre: None,
                post: None,
                body: vec![],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_func_implicit_return_empty_body() {
        let form = SurfaceForm::Func {
            name: "noop".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![],
                returns: None,
                pre: None,
                post: None,
                body: vec![],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            let last = values.last().unwrap();
            if let SExpr::List { values: ret, .. } = last {
                assert_eq!(ret[0].as_atom(), Some("return"));
                assert_eq!(ret[1].as_atom(), Some("undefined"));
            } else {
                panic!("expected return undefined");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_func_implicit_return_multi_body() {
        let form = SurfaceForm::Func {
            name: "process".into(),
            name_span: s(),
            clauses: vec![FuncClause {
                args: vec![],
                returns: None,
                pre: None,
                post: None,
                body: vec![
                    list(vec![atom("console:log"), str_lit("a")]),
                    list(vec![atom("console:log"), str_lit("b")]),
                    num(42.0),
                ],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            // Last should be (return 42)
            let last = values.last().unwrap();
            if let SExpr::List { values: ret, .. } = last {
                assert_eq!(ret[0].as_atom(), Some("return"));
            } else {
                panic!("expected return");
            }
        } else {
            panic!("expected list");
        }
    }

    // =======================================================================
    // Func multi-clause with typed dispatch
    // =======================================================================

    #[test]
    fn test_func_multi_clause_different_arity() {
        let form = SurfaceForm::Func {
            name: "f".into(),
            name_span: s(),
            clauses: vec![
                FuncClause {
                    args: vec![tp("any", "x")],
                    returns: None,
                    pre: None,
                    post: None,
                    body: vec![atom("x")],
                    span: s(),
                },
                FuncClause {
                    args: vec![tp("any", "x"), tp("any", "y")],
                    returns: None,
                    pre: None,
                    post: None,
                    body: vec![list(vec![atom("+"), atom("x"), atom("y")])],
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
            // Should have throw at the end
            let last = values.last().unwrap();
            if let SExpr::List { values: throw, .. } = last {
                assert_eq!(throw[0].as_atom(), Some("throw"));
            } else {
                panic!("expected throw");
            }
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_func_multi_clause_with_post() {
        let form = SurfaceForm::Func {
            name: "f".into(),
            name_span: s(),
            clauses: vec![
                FuncClause {
                    args: vec![tp("number", "x")],
                    returns: None,
                    pre: None,
                    post: Some(list(vec![atom(">"), atom("%"), num(0.0)])),
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
    }

    #[test]
    fn test_func_multi_clause_with_pre() {
        let form = SurfaceForm::Func {
            name: "f".into(),
            name_span: s(),
            clauses: vec![
                FuncClause {
                    args: vec![tp("number", "x")],
                    returns: None,
                    pre: Some(list(vec![atom(">"), atom("x"), num(0.0)])),
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
    }

    #[test]
    fn test_func_multi_clause_empty_body() {
        let form = SurfaceForm::Func {
            name: "f".into(),
            name_span: s(),
            clauses: vec![
                FuncClause {
                    args: vec![tp("number", "x")],
                    returns: None,
                    pre: None,
                    post: None,
                    body: vec![],
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
    }

    #[test]
    fn test_func_multi_clause_multi_body() {
        let form = SurfaceForm::Func {
            name: "f".into(),
            name_span: s(),
            clauses: vec![
                FuncClause {
                    args: vec![tp("number", "x")],
                    returns: None,
                    pre: None,
                    post: None,
                    body: vec![list(vec![atom("console:log"), atom("x")]), atom("x")],
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
    }

    #[test]
    fn test_func_multi_clause_post_empty_body() {
        let form = SurfaceForm::Func {
            name: "f".into(),
            name_span: s(),
            clauses: vec![
                FuncClause {
                    args: vec![tp("number", "x")],
                    returns: None,
                    pre: None,
                    post: Some(list(vec![atom(">"), atom("%"), num(0.0)])),
                    body: vec![],
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
    }

    #[test]
    fn test_func_multi_clause_post_multi_body() {
        let form = SurfaceForm::Func {
            name: "f".into(),
            name_span: s(),
            clauses: vec![
                FuncClause {
                    args: vec![tp("number", "x")],
                    returns: None,
                    pre: None,
                    post: Some(list(vec![atom(">"), atom("%"), num(0.0)])),
                    body: vec![list(vec![atom("console:log"), atom("x")]), atom("x")],
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
    }

    // =======================================================================
    // build_dispatch_check — various type keywords
    // =======================================================================

    #[test]
    fn test_dispatch_check_boolean() {
        let expr = atom("x");
        let result = build_dispatch_check("boolean", &expr);
        assert!(result.is_some());
        if let Some(SExpr::List { values, .. }) = result {
            assert_eq!(values[0].as_atom(), Some("==="));
        }
    }

    #[test]
    fn test_dispatch_check_function() {
        let expr = atom("x");
        let result = build_dispatch_check("function", &expr);
        assert!(result.is_some());
    }

    #[test]
    fn test_dispatch_check_object() {
        let expr = atom("x");
        let result = build_dispatch_check("object", &expr);
        assert!(result.is_some());
        // Should be (&& (=== typeof "object") (!== x null))
        if let Some(SExpr::List { values, .. }) = result {
            assert_eq!(values[0].as_atom(), Some("&&"));
        }
    }

    #[test]
    fn test_dispatch_check_array() {
        let expr = atom("x");
        let result = build_dispatch_check("array", &expr);
        assert!(result.is_some());
        if let Some(SExpr::List { values, .. }) = result {
            assert_eq!(values[0].as_atom(), Some("Array:isArray"));
        }
    }

    #[test]
    fn test_dispatch_check_any() {
        let result = build_dispatch_check("any", &atom("x"));
        assert!(result.is_none());
    }

    #[test]
    fn test_dispatch_check_user_type() {
        let expr = atom("x");
        let result = build_dispatch_check("MyType", &expr);
        assert!(result.is_some());
        // User-defined: typeof "object" && !== null && "tag" in x
        if let Some(SExpr::List { values, .. }) = result {
            assert_eq!(values[0].as_atom(), Some("&&"));
        }
    }

    // =======================================================================
    // Match with Obj pattern
    // =======================================================================

    #[test]
    fn test_match_obj_pattern_statement() {
        let form = SurfaceForm::Match {
            target: atom("x"),
            clauses: vec![
                MatchClause {
                    pattern: Pattern::Obj {
                        pairs: vec![
                            (
                                "name".into(),
                                Pattern::Binding {
                                    name: "n".into(),
                                    span: s(),
                                },
                            ),
                            ("age".into(), Pattern::Wildcard(s())),
                        ],
                        span: s(),
                    },
                    guard: None,
                    body: vec![atom("n")],
                    span: s(),
                },
                MatchClause {
                    pattern: Pattern::Wildcard(s()),
                    guard: None,
                    body: vec![str_lit("unknown")],
                    span: s(),
                },
            ],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_match_obj_pattern_value() {
        let form = SurfaceForm::Match {
            target: atom("x"),
            clauses: vec![MatchClause {
                pattern: Pattern::Obj {
                    pairs: vec![(
                        "name".into(),
                        Pattern::Binding {
                            name: "n".into(),
                            span: s(),
                        },
                    )],
                    span: s(),
                },
                guard: None,
                body: vec![atom("n")],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    // =======================================================================
    // Match with guard clauses
    // =======================================================================

    #[test]
    fn test_match_with_guard_statement() {
        let form = SurfaceForm::Match {
            target: atom("x"),
            clauses: vec![
                MatchClause {
                    pattern: Pattern::Binding {
                        name: "v".into(),
                        span: s(),
                    },
                    guard: Some(list(vec![atom(">"), atom("v"), num(0.0)])),
                    body: vec![str_lit("positive")],
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
    }

    #[test]
    fn test_match_with_guard_value() {
        let form = SurfaceForm::Match {
            target: atom("x"),
            clauses: vec![
                MatchClause {
                    pattern: Pattern::Binding {
                        name: "v".into(),
                        span: s(),
                    },
                    guard: Some(list(vec![atom(">"), atom("v"), num(0.0)])),
                    body: vec![str_lit("positive")],
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
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_match_with_guard_multi_body_value() {
        let form = SurfaceForm::Match {
            target: atom("x"),
            clauses: vec![MatchClause {
                pattern: Pattern::Binding {
                    name: "v".into(),
                    span: s(),
                },
                guard: Some(list(vec![atom(">"), atom("v"), num(0.0)])),
                body: vec![
                    list(vec![atom("console:log"), atom("v")]),
                    str_lit("positive"),
                ],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    // =======================================================================
    // Match without wildcard (should have throw fallback)
    // =======================================================================

    #[test]
    fn test_match_no_wildcard_statement() {
        let form = SurfaceForm::Match {
            target: atom("x"),
            clauses: vec![MatchClause {
                pattern: Pattern::Literal(num(1.0)),
                guard: None,
                body: vec![str_lit("one")],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should have a throw fallback in the else chain
    }

    #[test]
    fn test_match_no_wildcard_value() {
        let form = SurfaceForm::Match {
            target: atom("x"),
            clauses: vec![MatchClause {
                pattern: Pattern::Literal(num(1.0)),
                guard: None,
                body: vec![str_lit("one")],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    // =======================================================================
    // Match with binding pattern (non-wildcard, non-literal)
    // =======================================================================

    #[test]
    fn test_match_binding_pattern() {
        let form = SurfaceForm::Match {
            target: atom("x"),
            clauses: vec![MatchClause {
                pattern: Pattern::Binding {
                    name: "v".into(),
                    span: s(),
                },
                guard: None,
                body: vec![atom("v")],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    // =======================================================================
    // compile_let_pattern — additional pattern types
    // =======================================================================

    #[test]
    fn test_if_let_constructor_pattern() {
        let mut r = reg();
        r.register_type(TypeDef {
            name: "Option".into(),
            module_path: None,
            constructors: vec![ConstructorDef {
                name: "Some".into(),
                fields: vec![FieldDef {
                    name: "value".into(),
                    type_keyword: "any".into(),
                }],
                owning_type: "Option".into(),
                span: s(),
            }],
            is_blessed: false,
            span: s(),
        })
        .unwrap();

        let form = SurfaceForm::IfLet {
            pattern: Pattern::Constructor {
                name: "Some".into(),
                name_span: s(),
                bindings: vec![Pattern::Binding {
                    name: "v".into(),
                    span: s(),
                }],
                span: s(),
            },
            expr: atom("x"),
            then_body: atom("v"),
            else_body: Some(num(0.0)),
            span: s(),
        };
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &r);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_if_let_obj_pattern() {
        let form = SurfaceForm::IfLet {
            pattern: Pattern::Obj {
                pairs: vec![(
                    "name".into(),
                    Pattern::Binding {
                        name: "n".into(),
                        span: s(),
                    },
                )],
                span: s(),
            },
            expr: atom("x"),
            then_body: atom("n"),
            else_body: None,
            span: s(),
        };
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_if_let_wildcard_pattern() {
        let form = SurfaceForm::IfLet {
            pattern: Pattern::Wildcard(s()),
            expr: atom("x"),
            then_body: str_lit("matched"),
            else_body: None,
            span: s(),
        };
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_if_let_literal_pattern() {
        let form = SurfaceForm::IfLet {
            pattern: Pattern::Literal(num(42.0)),
            expr: atom("x"),
            then_body: str_lit("matched"),
            else_body: Some(str_lit("nope")),
            span: s(),
        };
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_if_let_statement_no_else() {
        let form = SurfaceForm::IfLet {
            pattern: Pattern::Binding {
                name: "v".into(),
                span: s(),
            },
            expr: atom("x"),
            then_body: atom("v"),
            else_body: None,
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Statement context without else: should be (block (const ...) (if cond then))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("block"));
        } else {
            panic!("expected block");
        }
    }

    #[test]
    fn test_if_let_value_no_else() {
        let form = SurfaceForm::IfLet {
            pattern: Pattern::Binding {
                name: "v".into(),
                span: s(),
            },
            expr: atom("x"),
            then_body: atom("v"),
            else_body: None,
            span: s(),
        };
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    // =======================================================================
    // when-let with constructor/obj patterns
    // =======================================================================

    #[test]
    fn test_when_let_constructor_pattern() {
        let mut r = reg();
        r.register_type(TypeDef {
            name: "Option".into(),
            module_path: None,
            constructors: vec![ConstructorDef {
                name: "Some".into(),
                fields: vec![FieldDef {
                    name: "value".into(),
                    type_keyword: "any".into(),
                }],
                owning_type: "Option".into(),
                span: s(),
            }],
            is_blessed: false,
            span: s(),
        })
        .unwrap();

        let form = SurfaceForm::WhenLet {
            pattern: Pattern::Constructor {
                name: "Some".into(),
                name_span: s(),
                bindings: vec![Pattern::Binding {
                    name: "v".into(),
                    span: s(),
                }],
                span: s(),
            },
            expr: atom("x"),
            body: vec![atom("v")],
            span: s(),
        };
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &r);
        assert_eq!(result.len(), 1);
    }

    // =======================================================================
    // step_contains_await
    // =======================================================================

    #[test]
    fn test_step_contains_await_bare_atom() {
        assert!(step_contains_await(&ThreadingStep::Bare(atom("await"))));
        assert!(!step_contains_await(&ThreadingStep::Bare(atom("inc"))));
    }

    #[test]
    fn test_step_contains_await_bare_list() {
        let step = ThreadingStep::Bare(list(vec![atom("await"), atom("p")]));
        assert!(step_contains_await(&step));
    }

    #[test]
    fn test_step_contains_await_call_head() {
        let step = ThreadingStep::Call(vec![atom("await"), atom("p")]);
        assert!(step_contains_await(&step));
    }

    #[test]
    fn test_step_contains_await_call_nested() {
        let step = ThreadingStep::Call(vec![atom("process"), list(vec![atom("await"), atom("p")])]);
        assert!(step_contains_await(&step));
    }

    #[test]
    fn test_step_contains_await_call_absent() {
        let step = ThreadingStep::Call(vec![atom("process"), atom("x")]);
        assert!(!step_contains_await(&step));
    }

    // =======================================================================
    // any_contains_await
    // =======================================================================

    #[test]
    fn test_any_contains_await_true() {
        let exprs = vec![atom("x"), list(vec![atom("await"), atom("p")])];
        assert!(any_contains_await(&exprs));
    }

    #[test]
    fn test_any_contains_await_false() {
        let exprs = vec![atom("x"), num(1.0)];
        assert!(!any_contains_await(&exprs));
    }

    // =======================================================================
    // bool_lit helper
    // =======================================================================

    #[test]
    fn test_bool_lit() {
        if let SExpr::Bool { value, .. } = bool_lit(true) {
            assert!(value);
        } else {
            panic!("expected bool");
        }
        if let SExpr::Bool { value, .. } = bool_lit(false) {
            assert!(!value);
        } else {
            panic!("expected bool");
        }
    }

    // =======================================================================
    // MacroDef / ImportMacros passthrough
    // =======================================================================

    #[test]
    fn test_emit_macro_def() {
        let raw = list(vec![atom("defmacro"), atom("my-macro")]);
        let form = SurfaceForm::MacroDef {
            name: "my-macro".into(),
            raw: raw.clone(),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], raw);
    }

    #[test]
    fn test_emit_import_macros() {
        let raw = list(vec![atom("import-macros"), str_lit("./macros.lykn")]);
        let form = SurfaceForm::ImportMacros {
            raw: raw.clone(),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], raw);
    }

    // =======================================================================
    // emit_expr — nested surface form inside non-surface-form list
    // =======================================================================

    #[test]
    fn test_emit_expr_nested_surface_form_in_bind() {
        // A bind whose value contains a threading form
        let form = SurfaceForm::Bind {
            name: atom("x"),
            type_ann: None,
            value: list(vec![atom("->"), num(1.0), atom("inc")]),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // The nested (-> 1 inc) should be expanded to (inc 1)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("const"));
            if let SExpr::List {
                values: call_vals, ..
            } = &values[2]
            {
                assert_eq!(call_vals[0].as_atom(), Some("inc"));
            } else {
                panic!("expected expanded threading form");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_emit_expr_computed_call() {
        // Head is not an atom (computed call) — recurse on all elements
        let form = SurfaceForm::FunctionCall {
            head: list(vec![atom("get-fn")]),
            args: vec![num(1.0)],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            // Head should be the list (get-fn), recursed
            assert!(matches!(&values[0], SExpr::List { .. }));
        } else {
            panic!("expected list");
        }
    }

    // =======================================================================
    // some-> with Call steps (not just Bare)
    // =======================================================================

    #[test]
    fn test_some_thread_first_call_step() {
        let form = SurfaceForm::SomeThreadFirst {
            initial: atom("x"),
            steps: vec![ThreadingStep::Call(vec![atom("add"), num(2.0)])],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    // =======================================================================
    // Match with multi-body clauses in value context
    // =======================================================================

    #[test]
    fn test_match_value_multi_body() {
        let form = SurfaceForm::Match {
            target: atom("x"),
            clauses: vec![MatchClause {
                pattern: Pattern::Wildcard(s()),
                guard: None,
                body: vec![list(vec![atom("console:log"), atom("x")]), str_lit("done")],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    // =======================================================================
    // Match constructor in value context with wildcard bindings
    // =======================================================================

    #[test]
    fn test_match_constructor_wildcard_binding() {
        let mut r = reg();
        r.register_type(TypeDef {
            name: "Pair".into(),
            module_path: None,
            constructors: vec![ConstructorDef {
                name: "Pair".into(),
                fields: vec![
                    FieldDef {
                        name: "x".into(),
                        type_keyword: "any".into(),
                    },
                    FieldDef {
                        name: "y".into(),
                        type_keyword: "any".into(),
                    },
                ],
                owning_type: "Pair".into(),
                span: s(),
            }],
            is_blessed: false,
            span: s(),
        })
        .unwrap();

        let form = SurfaceForm::Match {
            target: atom("p"),
            clauses: vec![MatchClause {
                pattern: Pattern::Constructor {
                    name: "Pair".into(),
                    name_span: s(),
                    bindings: vec![
                        Pattern::Binding {
                            name: "a".into(),
                            span: s(),
                        },
                        Pattern::Wildcard(s()),
                    ],
                    span: s(),
                },
                guard: None,
                body: vec![atom("a")],
                span: s(),
            }],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &r);
        assert_eq!(result.len(), 1);
    }

    // =======================================================================
    // when-let with multi-body in value context
    // =======================================================================

    #[test]
    fn test_when_let_value_multi_body() {
        let form = SurfaceForm::WhenLet {
            pattern: Pattern::Binding {
                name: "v".into(),
                span: s(),
            },
            expr: atom("x"),
            body: vec![list(vec![atom("console:log"), atom("v")]), atom("v")],
            span: s(),
        };
        let mut c = ctx_value();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    // =======================================================================
    // Type emission with strip_assertions
    // =======================================================================

    #[test]
    fn test_emit_type_with_fields_strip_assertions() {
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
        let mut c = EmitterContext::new(true);
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // With strip_assertions, no type checks inside the function
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("function"));
            // Should not have any if nodes (type checks)
            let has_if = values.iter().any(|v| {
                if let SExpr::List { values, .. } = v {
                    values.first().and_then(|f| f.as_atom()) == Some("if")
                } else {
                    false
                }
            });
            assert!(!has_if, "should not have type checks when stripping");
        } else {
            panic!("expected list");
        }
    }

    // =======================================================================
    // Fn with strip_assertions
    // =======================================================================

    #[test]
    fn test_fn_strip_assertions() {
        let form = SurfaceForm::Fn {
            params: vec![tp("number", "x")],
            body: vec![atom("x")],
            span: s(),
        };
        let mut c = EmitterContext::new(true);
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("=>"));
            // No type check if nodes
            let has_if = values.iter().any(|v| {
                if let SExpr::List { values, .. } = v {
                    values.first().and_then(|f| f.as_atom()) == Some("if")
                } else {
                    false
                }
            });
            assert!(!has_if, "should not have type checks");
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_fn_typed_returns_last_expression() {
        // (fn (:number x) (* x 2)) should emit (=> (x) <type-check> (return (* x 2)))
        let form = SurfaceForm::Fn {
            params: vec![tp("number", "x")],
            body: vec![list(vec![
                atom("*"),
                atom("x"),
                SExpr::Number {
                    value: 2.0,
                    span: s(),
                },
            ])],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("=>"));
            // Last item should be (return (* x 2))
            let last = values.last().unwrap();
            if let SExpr::List {
                values: ret_vals, ..
            } = last
            {
                assert_eq!(ret_vals[0].as_atom(), Some("return"));
            } else {
                panic!("expected last item to be a return list");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_fn_typed_multi_body_returns_last() {
        // (fn (:number x) (console:log x) (+ x 1)) should return last expr
        let form = SurfaceForm::Fn {
            params: vec![tp("number", "x")],
            body: vec![
                list(vec![atom("console.log"), atom("x")]),
                list(vec![
                    atom("+"),
                    atom("x"),
                    SExpr::Number {
                        value: 1.0,
                        span: s(),
                    },
                ]),
            ],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("=>"));
            // Last item should be (return (+ x 1))
            let last = values.last().unwrap();
            if let SExpr::List {
                values: ret_vals, ..
            } = last
            {
                assert_eq!(ret_vals[0].as_atom(), Some("return"));
            } else {
                panic!("expected last item to be a return list");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_fn_any_no_return_wrapper() {
        // (fn (:any x) x) should NOT get a return wrapper (concise arrow body)
        let form = SurfaceForm::Fn {
            params: vec![tp("any", "x")],
            body: vec![atom("x")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("=>"));
            // Should be (=> (x) x) — no return wrapper
            let last = values.last().unwrap();
            assert_eq!(
                last.as_atom(),
                Some("x"),
                "should be bare atom, not return wrapper"
            );
        } else {
            panic!("expected list");
        }
    }

    // =======================================================================
    // js: interop via emit_expr (nested in value expressions)
    // =======================================================================

    #[test]
    fn test_js_interop_nested_in_bind() {
        // Bind whose value is a js:typeof call
        let form = SurfaceForm::Bind {
            name: atom("t"),
            type_ann: None,
            value: list(vec![atom("js:typeof"), atom("x")]),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("const"));
            if let SExpr::List {
                values: typeof_form,
                ..
            } = &values[2]
            {
                assert_eq!(typeof_form[0].as_atom(), Some("typeof"));
            } else {
                panic!("expected typeof form");
            }
        } else {
            panic!("expected list");
        }
    }

    // =======================================================================
    // Multi-clause func strip_assertions
    // =======================================================================

    #[test]
    fn test_func_multi_clause_strip_assertions() {
        let form = SurfaceForm::Func {
            name: "f".into(),
            name_span: s(),
            clauses: vec![
                FuncClause {
                    args: vec![tp("number", "x")],
                    returns: None,
                    pre: Some(list(vec![atom(">"), atom("x"), num(0.0)])),
                    post: Some(list(vec![atom(">"), atom("%"), num(0.0)])),
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
        let mut c = EmitterContext::new(true);
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
    }

    // =======================================================================
    // Single-arity condition in multi-clause (no &&, just arity check)
    // =======================================================================

    #[test]
    fn test_func_multi_clause_any_types_single_condition() {
        let form = SurfaceForm::Func {
            name: "f".into(),
            name_span: s(),
            clauses: vec![
                FuncClause {
                    args: vec![tp("any", "x")],
                    returns: None,
                    pre: None,
                    post: None,
                    body: vec![atom("x")],
                    span: s(),
                },
                FuncClause {
                    args: vec![tp("any", "x"), tp("any", "y")],
                    returns: None,
                    pre: None,
                    post: None,
                    body: vec![list(vec![atom("+"), atom("x"), atom("y")])],
                    span: s(),
                },
            ],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // When all params are "any", dispatch condition is just arity check (no &&)
        if let SExpr::List { values, .. } = &result[0] {
            // Find the first if block
            for v in values.iter() {
                if let SExpr::List {
                    values: if_vals, ..
                } = v
                {
                    if if_vals.first().and_then(|f| f.as_atom()) == Some("if") {
                        // The condition should be (=== args:length N) directly
                        if let SExpr::List {
                            values: cond_vals, ..
                        } = &if_vals[1]
                        {
                            assert_eq!(
                                cond_vals[0].as_atom(),
                                Some("==="),
                                "single condition should be ==="
                            );
                        }
                        break;
                    }
                }
            }
        }
    }

    // =======================================================================
    // Match statement with multiple literal clauses (builds else-chain)
    // =======================================================================

    #[test]
    fn test_match_statement_multiple_literals_else_chain() {
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
                    pattern: Pattern::Literal(num(2.0)),
                    guard: None,
                    body: vec![str_lit("two")],
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
        // Should be nested if/else chain
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("block"));
            // The chain element should be (if ... ... (if ... ... ...))
            if let SExpr::List {
                values: if_form, ..
            } = &values[2]
            {
                assert_eq!(if_form[0].as_atom(), Some("if"));
                // The else branch should also be an if
                if let SExpr::List {
                    values: else_form, ..
                } = &if_form[3]
                {
                    assert_eq!(else_form[0].as_atom(), Some("if"));
                } else {
                    panic!("expected nested if in else");
                }
            } else {
                panic!("expected if form");
            }
        } else {
            panic!("expected block");
        }
    }

    // =======================================================================
    // Swap with extra_args
    // =======================================================================

    #[test]
    fn test_swap_with_extra_args() {
        let form = SurfaceForm::Swap {
            target: atom("counter"),
            func: atom("add"),
            extra_args: vec![num(5.0)],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // (= counter:value (add counter:value 5))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("="));
            if let SExpr::List { values: call, .. } = &values[2] {
                assert_eq!(call[0].as_atom(), Some("add"));
                assert_eq!(call.len(), 3); // add counter:value 5
            } else {
                panic!("expected call");
            }
        } else {
            panic!("expected list");
        }
    }

    // =======================================================================
    // Dissoc with multiple keys
    // =======================================================================

    #[test]
    fn test_dissoc_multiple_keys() {
        let form = SurfaceForm::Dissoc {
            obj: atom("m"),
            keys: vec!["name".into(), "age".into()],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // IIFE with destructuring pattern that has 2 alias entries + rest
        if let SExpr::List { values, .. } = &result[0] {
            if let SExpr::List { values: arrow, .. } = &values[0] {
                // arrow[2] should be the const with object pattern
                if let SExpr::List {
                    values: const_form, ..
                } = &arrow[2]
                {
                    if let SExpr::List {
                        values: pattern, ..
                    } = &const_form[1]
                    {
                        assert_eq!(pattern[0].as_atom(), Some("object"));
                        // 2 alias + 1 rest = 3 entries + "object" head = 4
                        assert_eq!(pattern.len(), 4);
                    }
                }
            }
        }
    }

    // --- Eq (DD-22) ---

    #[test]
    fn test_emit_eq_binary() {
        let form = SurfaceForm::Eq {
            args: vec![atom("a"), atom("b")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (=== a b)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("==="));
            assert_eq!(values[1].as_atom(), Some("a"));
            assert_eq!(values[2].as_atom(), Some("b"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_emit_eq_variadic_three() {
        let form = SurfaceForm::Eq {
            args: vec![atom("a"), atom("b"), atom("c")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (&& (=== a b) (=== b c))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("&&"));
            if let SExpr::List { values: left, .. } = &values[1] {
                assert_eq!(left[0].as_atom(), Some("==="));
                assert_eq!(left[1].as_atom(), Some("a"));
                assert_eq!(left[2].as_atom(), Some("b"));
            } else {
                panic!("expected left === pair");
            }
            if let SExpr::List { values: right, .. } = &values[2] {
                assert_eq!(right[0].as_atom(), Some("==="));
                assert_eq!(right[1].as_atom(), Some("b"));
                assert_eq!(right[2].as_atom(), Some("c"));
            } else {
                panic!("expected right === pair");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_emit_eq_variadic_four() {
        let form = SurfaceForm::Eq {
            args: vec![atom("a"), atom("b"), atom("c"), atom("d")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (&& (&& (=== a b) (=== b c)) (=== c d))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("&&"));
            // Left side is (&& (=== a b) (=== b c))
            if let SExpr::List { values: inner, .. } = &values[1] {
                assert_eq!(inner[0].as_atom(), Some("&&"));
            } else {
                panic!("expected inner && for left-fold");
            }
            // Right side is (=== c d)
            if let SExpr::List { values: right, .. } = &values[2] {
                assert_eq!(right[0].as_atom(), Some("==="));
                assert_eq!(right[1].as_atom(), Some("c"));
                assert_eq!(right[2].as_atom(), Some("d"));
            } else {
                panic!("expected right === pair");
            }
        } else {
            panic!("expected list");
        }
    }

    // --- NotEq (DD-22) ---

    #[test]
    fn test_emit_not_eq() {
        let form = SurfaceForm::NotEq {
            left: atom("a"),
            right: atom("b"),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (!== a b)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("!=="));
            assert_eq!(values[1].as_atom(), Some("a"));
            assert_eq!(values[2].as_atom(), Some("b"));
        } else {
            panic!("expected list");
        }
    }

    // --- And (DD-22) ---

    #[test]
    fn test_emit_and_binary() {
        let form = SurfaceForm::And {
            args: vec![atom("a"), atom("b")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (&& a b)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("&&"));
            assert_eq!(values[1].as_atom(), Some("a"));
            assert_eq!(values[2].as_atom(), Some("b"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_emit_and_variadic() {
        let form = SurfaceForm::And {
            args: vec![atom("a"), atom("b"), atom("c"), atom("d")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (&& (&& (&& a b) c) d)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("&&"));
            assert_eq!(values[2].as_atom(), Some("d"));
            // Inner: (&& (&& a b) c)
            if let SExpr::List { values: mid, .. } = &values[1] {
                assert_eq!(mid[0].as_atom(), Some("&&"));
                assert_eq!(mid[2].as_atom(), Some("c"));
                // Innermost: (&& a b)
                if let SExpr::List { values: inner, .. } = &mid[1] {
                    assert_eq!(inner[0].as_atom(), Some("&&"));
                    assert_eq!(inner[1].as_atom(), Some("a"));
                    assert_eq!(inner[2].as_atom(), Some("b"));
                } else {
                    panic!("expected innermost && list");
                }
            } else {
                panic!("expected middle && list");
            }
        } else {
            panic!("expected list");
        }
    }

    // --- Or (DD-22) ---

    #[test]
    fn test_emit_or_binary() {
        let form = SurfaceForm::Or {
            args: vec![atom("a"), atom("b")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (|| a b)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("||"));
            assert_eq!(values[1].as_atom(), Some("a"));
            assert_eq!(values[2].as_atom(), Some("b"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_emit_or_variadic() {
        let form = SurfaceForm::Or {
            args: vec![atom("a"), atom("b"), atom("c"), atom("d")],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (|| (|| (|| a b) c) d)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("||"));
            assert_eq!(values[2].as_atom(), Some("d"));
            // Inner: (|| (|| a b) c)
            if let SExpr::List { values: mid, .. } = &values[1] {
                assert_eq!(mid[0].as_atom(), Some("||"));
                assert_eq!(mid[2].as_atom(), Some("c"));
            } else {
                panic!("expected middle || list");
            }
        } else {
            panic!("expected list");
        }
    }

    // --- Not (DD-22) ---

    #[test]
    fn test_emit_not() {
        let form = SurfaceForm::Not {
            operand: atom("x"),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (! x)
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("!"));
            assert_eq!(values[1].as_atom(), Some("x"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_emit_not_nested() {
        // (not (not x)) → (! (! x))
        // Construct the inner (not x) form as a surface form nested in the SExpr.
        // Since emit uses emit_expr which classifies nested surface forms,
        // we test the emitter directly by constructing the kernel form.
        let inner_not = SurfaceForm::Not {
            operand: atom("x"),
            span: s(),
        };
        let mut c = ctx();
        let inner_result = emit_form(&inner_not, &mut c, &reg());
        // inner_result = [(! x)]

        let outer = SurfaceForm::Not {
            operand: inner_result[0].clone(),
            span: s(),
        };
        let result = emit_form(&outer, &mut c, &reg());
        assert_eq!(result.len(), 1);
        // Should be (! (! x))
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("!"));
            if let SExpr::List {
                values: inner_vals, ..
            } = &values[1]
            {
                assert_eq!(inner_vals[0].as_atom(), Some("!"));
                assert_eq!(inner_vals[1].as_atom(), Some("x"));
            } else {
                panic!("expected inner (! x)");
            }
        } else {
            panic!("expected list");
        }
    }

    // --- DD-22 regression: kernel = in emitter output is not re-intercepted ---

    #[test]
    fn test_emit_reset_still_uses_kernel_assignment() {
        // reset! emits kernel `=` (assignment). This must not be intercepted
        // by the surface `=` (equality) form.
        let form = SurfaceForm::Reset {
            target: atom("counter"),
            value: num(0.0),
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(
                values[0].as_atom(),
                Some("="),
                "reset! should still emit kernel ="
            );
            assert_eq!(values[1].as_atom(), Some("counter:value"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_emit_swap_still_uses_kernel_assignment() {
        // swap! emits kernel `=` (assignment). This must not be intercepted.
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
            assert_eq!(
                values[0].as_atom(),
                Some("="),
                "swap! should still emit kernel ="
            );
        } else {
            panic!("expected list");
        }
    }

    // --- DD-22 regression: nested surface forms in equality/logical ops ---

    #[test]
    fn test_emit_eq_with_nested_surface_form() {
        // (= (cell 1) (cell 2)) — the args contain surface forms that should be
        // recursively emitted
        let form = SurfaceForm::Eq {
            args: vec![
                list(vec![atom("cell"), num(1.0)]),
                list(vec![atom("cell"), num(2.0)]),
            ],
            span: s(),
        };
        let mut c = ctx();
        let result = emit_form(&form, &mut c, &reg());
        assert_eq!(result.len(), 1);
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("==="));
            // Both sides should be emitted (cell → object with value)
            assert!(matches!(values[1], SExpr::List { .. }));
            assert!(matches!(values[2], SExpr::List { .. }));
        } else {
            panic!("expected list");
        }
    }
}
