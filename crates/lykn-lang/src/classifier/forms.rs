use std::collections::HashMap;

use crate::ast::sexpr::SExpr;
use crate::ast::surface::*;
use crate::diagnostics::{Diagnostic, Severity};
use crate::reader::source_loc::Span;

use super::dispatch;

pub fn classify_form(expr: &SExpr) -> Result<SurfaceForm, Diagnostic> {
    match expr {
        SExpr::List { values, span } if !values.is_empty() => {
            if let Some(head_name) = values[0].as_atom() {
                let args = &values[1..];
                if dispatch::is_surface_form(head_name) {
                    classify_surface_form(head_name, args, *span)
                } else if dispatch::is_kernel_form(head_name) {
                    Ok(SurfaceForm::KernelPassthrough {
                        raw: expr.clone(),
                        span: *span,
                    })
                } else {
                    // Function call
                    Ok(SurfaceForm::FunctionCall {
                        head: values[0].clone(),
                        args: args.to_vec(),
                        span: *span,
                    })
                }
            } else {
                // Head is not an atom — function call with computed head
                Ok(SurfaceForm::FunctionCall {
                    head: values[0].clone(),
                    args: values[1..].to_vec(),
                    span: *span,
                })
            }
        }
        // Non-list top-level forms are kernel passthroughs
        _ => Ok(SurfaceForm::KernelPassthrough {
            raw: expr.clone(),
            span: expr.span(),
        }),
    }
}

fn classify_surface_form(
    name: &str,
    args: &[SExpr],
    span: Span,
) -> Result<SurfaceForm, Diagnostic> {
    match name {
        "bind" => classify_bind(args, span),
        "obj" => classify_obj(args, span),
        "cell" => classify_cell(args, span),
        "express" => classify_express(args, span),
        "swap!" => classify_swap(args, span),
        "reset!" => classify_reset(args, span),
        "set!" => classify_set(args, span),
        "->" => classify_threading(args, span, false, false),
        "->>" => classify_threading(args, span, true, false),
        "some->" => classify_threading(args, span, false, true),
        "some->>" => classify_threading(args, span, true, true),
        "type" => classify_type(args, span),
        "func" => classify_func(args, span),
        "match" => classify_match(args, span),
        "if-let" => classify_if_let(args, span),
        "when-let" => classify_when_let(args, span),
        "fn" => classify_fn(args, span),
        "lambda" => classify_lambda(args, span),
        "conj" => classify_conj(args, span),
        "assoc" => classify_assoc(args, span),
        "dissoc" => classify_dissoc(args, span),
        "macro" => classify_macro_def(args, span),
        "import-macros" => classify_import_macros(args, span),
        "=" => classify_eq(args, span),
        "!=" => classify_not_eq(args, span),
        "and" => classify_and(args, span),
        "or" => classify_or(args, span),
        "not" => classify_not(args, span),
        _ => Err(Diagnostic {
            severity: Severity::Error,
            message: format!("unknown surface form: {name}"),
            span,
            suggestion: None,
        }),
    }
}

fn err(msg: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        message: msg.into(),
        span,
        suggestion: None,
    }
}

fn classify_bind(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    match args.len() {
        2 => Ok(SurfaceForm::Bind {
            name: args[0].clone(),
            type_ann: None,
            value: args[1].clone(),
            span,
        }),
        3 => {
            let type_ann = match &args[0] {
                SExpr::Keyword { value, span: kspan } => Some(TypeAnnotation {
                    name: value.clone(),
                    span: *kspan,
                }),
                _ => {
                    return Err(err(
                        "bind with 3 arguments requires a type keyword as first argument",
                        span,
                    ));
                }
            };
            Ok(SurfaceForm::Bind {
                name: args[1].clone(),
                type_ann,
                value: args[2].clone(),
                span,
            })
        }
        _ => Err(err(
            "bind requires 2 or 3 arguments: (bind name value) or (bind :type name value)",
            span,
        )),
    }
}

fn classify_obj(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if !args.len().is_multiple_of(2) {
        return Err(err(
            "obj requires an even number of arguments (keyword-value pairs)",
            span,
        ));
    }
    let mut pairs = Vec::new();
    for i in (0..args.len()).step_by(2) {
        match &args[i] {
            SExpr::Keyword { value, .. } => {
                pairs.push((value.clone(), args[i + 1].clone()));
            }
            _ => return Err(err(format!("obj: expected keyword at position {i}"), span)),
        }
    }
    Ok(SurfaceForm::Obj { pairs, span })
}

fn classify_cell(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() != 1 {
        return Err(err("cell requires exactly 1 argument", span));
    }
    Ok(SurfaceForm::Cell {
        value: args[0].clone(),
        span,
    })
}

fn classify_express(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() != 1 {
        return Err(err("express requires exactly 1 argument", span));
    }
    Ok(SurfaceForm::Express {
        target: args[0].clone(),
        span,
    })
}

fn classify_swap(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 2 {
        return Err(err("swap! requires at least 2 arguments", span));
    }
    Ok(SurfaceForm::Swap {
        target: args[0].clone(),
        func: args[1].clone(),
        extra_args: args[2..].to_vec(),
        span,
    })
}

fn classify_reset(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() != 2 {
        return Err(err("reset! requires exactly 2 arguments", span));
    }
    Ok(SurfaceForm::Reset {
        target: args[0].clone(),
        value: args[1].clone(),
        span,
    })
}

fn classify_set(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() != 2 {
        return Err(err(
            "set! requires exactly 2 arguments: (set! target:prop value)",
            span,
        ));
    }
    // Target must be colon-syntax (member expression)
    match &args[0] {
        SExpr::Atom { value, .. } if value.contains(':') => {}
        _ => {
            return Err(err(
                "set! requires a property path (e.g., obj:prop), not a bare binding. \
                 Use (bind x val) for new bindings, (reset! cell val) for cells.",
                span,
            ));
        }
    }
    Ok(SurfaceForm::Set {
        target: args[0].clone(),
        value: args[1].clone(),
        span,
    })
}

fn classify_threading(
    args: &[SExpr],
    span: Span,
    is_last: bool,
    is_some: bool,
) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 2 {
        return Err(err("threading macro requires at least 2 arguments", span));
    }
    let initial = args[0].clone();
    let steps = args[1..]
        .iter()
        .map(|s| match s {
            SExpr::List { values, .. } => ThreadingStep::Call(values.clone()),
            _ => ThreadingStep::Bare(s.clone()),
        })
        .collect();

    match (is_last, is_some) {
        (false, false) => Ok(SurfaceForm::ThreadFirst {
            initial,
            steps,
            span,
        }),
        (true, false) => Ok(SurfaceForm::ThreadLast {
            initial,
            steps,
            span,
        }),
        (false, true) => Ok(SurfaceForm::SomeThreadFirst {
            initial,
            steps,
            span,
        }),
        (true, true) => Ok(SurfaceForm::SomeThreadLast {
            initial,
            steps,
            span,
        }),
    }
}

fn classify_type(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 2 {
        return Err(err(
            "type requires a name and at least one constructor",
            span,
        ));
    }
    let name = match &args[0] {
        SExpr::Atom { value, span: nspan } => (value.clone(), *nspan),
        _ => return Err(err("type: first argument must be a type name", span)),
    };

    let mut constructors = Vec::new();
    for ctor in &args[1..] {
        match ctor {
            SExpr::Atom { value, span: cspan } => {
                constructors.push(Constructor {
                    name: value.clone(),
                    name_span: *cspan,
                    fields: Vec::new(),
                    span: *cspan,
                });
            }
            SExpr::List {
                values,
                span: cspan,
            } if !values.is_empty() => {
                let ctor_name = match &values[0] {
                    SExpr::Atom { value, span: nspan } => (value.clone(), *nspan),
                    _ => return Err(err("constructor name must be an atom", *cspan)),
                };
                let fields = parse_simple_typed_params(&values[1..], *cspan)?;
                constructors.push(Constructor {
                    name: ctor_name.0,
                    name_span: ctor_name.1,
                    fields,
                    span: *cspan,
                });
            }
            _ => return Err(err("type: constructor must be an atom or list", span)),
        }
    }

    Ok(SurfaceForm::Type {
        name: name.0,
        name_span: name.1,
        constructors,
        span,
    })
}

fn classify_func(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.is_empty() {
        return Err(err("func requires at least a name", span));
    }
    let name = match &args[0] {
        SExpr::Atom { value, span: nspan } => (value.clone(), *nspan),
        _ => return Err(err("func: first argument must be a function name", span)),
    };

    let rest = &args[1..];
    if rest.is_empty() {
        return Err(err("func requires a body", span));
    }

    // Detect mode: multi-clause, single-clause keyword, or zero-arg
    let first = &rest[0];

    // Multi-clause: first arg is a list starting with a keyword
    if let SExpr::List { values, .. } = first
        && !values.is_empty()
        && let SExpr::Keyword { value, .. } = &values[0]
        && is_func_clause_key(value)
    {
        // Multi-clause
        let mut clauses = Vec::new();
        for clause_expr in rest {
            clauses.push(parse_func_clause(clause_expr, span)?);
        }
        return Ok(SurfaceForm::Func {
            name: name.0,
            name_span: name.1,
            clauses,
            span,
        });
    }

    // Single-clause keyword mode
    if let SExpr::Keyword { value, .. } = first
        && is_func_clause_key(value)
    {
        let clause = parse_func_clause_from_args(rest, span)?;
        return Ok(SurfaceForm::Func {
            name: name.0,
            name_span: name.1,
            clauses: vec![clause],
            span,
        });
    }

    // Zero-arg shorthand: (func name body...)
    let clause = FuncClause {
        args: Vec::new(),
        returns: None,
        pre: None,
        post: None,
        body: rest.to_vec(),
        span,
    };
    Ok(SurfaceForm::Func {
        name: name.0,
        name_span: name.1,
        clauses: vec![clause],
        span,
    })
}

fn is_func_clause_key(name: &str) -> bool {
    matches!(name, "args" | "returns" | "pre" | "post" | "body")
}

fn parse_func_clause(expr: &SExpr, outer_span: Span) -> Result<FuncClause, Diagnostic> {
    match expr {
        SExpr::List { values, span } => parse_func_clause_from_args(values, *span),
        _ => Err(err("func clause must be a list", outer_span)),
    }
}

fn parse_func_clause_from_args(args: &[SExpr], span: Span) -> Result<FuncClause, Diagnostic> {
    let kw_map = parse_keyword_clauses(args);
    let typed_args = if let Some(args_val) = kw_map.get("args") {
        if let Some(SExpr::List {
            values,
            span: pspan,
        }) = args_val.first()
        {
            parse_typed_params(values, *pspan)?
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let returns = kw_map.get("returns").and_then(|v| v.first()).and_then(|e| {
        if let SExpr::Keyword { value, span } = e {
            Some(TypeAnnotation {
                name: value.clone(),
                span: *span,
            })
        } else {
            None
        }
    });

    let pre = kw_map.get("pre").and_then(|v| v.first().cloned());
    let post = kw_map.get("post").and_then(|v| v.first().cloned());
    let body = kw_map.get("body").cloned().unwrap_or_default();

    if body.is_empty() {
        return Err(err("func clause requires :body", span));
    }

    Ok(FuncClause {
        args: typed_args,
        returns,
        pre,
        post,
        body,
        span,
    })
}

fn parse_keyword_clauses(args: &[SExpr]) -> HashMap<String, Vec<SExpr>> {
    let mut map = HashMap::new();
    let mut current_key: Option<String> = None;
    let mut current_values: Vec<SExpr> = Vec::new();

    for arg in args {
        if let SExpr::Keyword { value, .. } = arg
            && is_func_clause_key(value)
        {
            if let Some(key) = current_key.take() {
                map.insert(key, std::mem::take(&mut current_values));
            }
            current_key = Some(value.clone());
            continue;
        }
        current_values.push(arg.clone());
    }
    if let Some(key) = current_key {
        map.insert(key, current_values);
    }
    map
}

fn classify_match(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 2 {
        return Err(err("match requires a target and at least one clause", span));
    }
    let target = args[0].clone();
    let mut clauses = Vec::new();

    for clause_expr in &args[1..] {
        match clause_expr {
            SExpr::List {
                values,
                span: cspan,
            } if values.len() >= 2 => {
                let pattern = classify_pattern(&values[0])?;
                let mut guard = None;
                let mut body_start = 1;

                // Check for :when guard
                if values.len() >= 3
                    && let SExpr::Keyword { value, .. } = &values[1]
                    && value == "when"
                {
                    guard = Some(values[2].clone());
                    body_start = 3;
                }

                let body = values[body_start..].to_vec();
                if body.is_empty() {
                    return Err(err("match clause must have a body", *cspan));
                }

                clauses.push(MatchClause {
                    pattern,
                    guard,
                    body,
                    span: *cspan,
                });
            }
            _ => {
                return Err(err(
                    "match clause must be a list with at least a pattern and body",
                    span,
                ));
            }
        }
    }

    Ok(SurfaceForm::Match {
        target,
        clauses,
        span,
    })
}

fn classify_pattern(expr: &SExpr) -> Result<Pattern, Diagnostic> {
    match expr {
        SExpr::Atom { value, span } if value == "_" => Ok(Pattern::Wildcard(*span)),
        SExpr::Atom { value, .. }
            if value == "true" || value == "false" || value == "null" || value == "undefined" =>
        {
            Ok(Pattern::Literal(expr.clone()))
        }
        SExpr::Atom { value, span } => {
            if value.starts_with(|c: char| c.is_uppercase()) {
                // PascalCase — zero-field constructor
                Ok(Pattern::Constructor {
                    name: value.clone(),
                    name_span: *span,
                    bindings: Vec::new(),
                    span: *span,
                })
            } else {
                // lowercase — binding
                Ok(Pattern::Binding {
                    name: value.clone(),
                    span: *span,
                })
            }
        }
        SExpr::Number { .. }
        | SExpr::String { .. }
        | SExpr::Keyword { .. }
        | SExpr::Bool { .. } => Ok(Pattern::Literal(expr.clone())),
        SExpr::List { values, span } if !values.is_empty() => {
            let head = &values[0];
            // Structural obj pattern
            if let SExpr::Atom { value, .. } = head {
                if value == "obj" {
                    return classify_obj_pattern(&values[1..], *span);
                }
                if value.starts_with(|c: char| c.is_uppercase()) {
                    // ADT constructor pattern
                    let bindings = values[1..]
                        .iter()
                        .map(classify_pattern)
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(Pattern::Constructor {
                        name: value.clone(),
                        name_span: head.span(),
                        bindings,
                        span: *span,
                    });
                }
            }
            Err(err("unrecognized pattern", *span))
        }
        _ => Err(err("invalid pattern", expr.span())),
    }
}

fn classify_obj_pattern(args: &[SExpr], span: Span) -> Result<Pattern, Diagnostic> {
    if !args.len().is_multiple_of(2) {
        return Err(err("obj pattern requires keyword-binding pairs", span));
    }
    let mut pairs = Vec::new();
    for i in (0..args.len()).step_by(2) {
        match &args[i] {
            SExpr::Keyword { value, .. } => {
                let binding = classify_pattern(&args[i + 1])?;
                pairs.push((value.clone(), binding));
            }
            _ => return Err(err("obj pattern: expected keyword", span)),
        }
    }
    Ok(Pattern::Obj { pairs, span })
}

fn classify_if_let(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 2 || args.len() > 3 {
        return Err(err("if-let requires 2-3 arguments", span));
    }
    let binding = match &args[0] {
        SExpr::List { values, .. } if values.len() == 2 => {
            let pattern = classify_pattern(&values[0])?;
            let expr = values[1].clone();
            (pattern, expr)
        }
        _ => {
            return Err(err("if-let: first argument must be (pattern expr)", span));
        }
    };
    let then_body = args[1].clone();
    let else_body = args.get(2).cloned();

    Ok(SurfaceForm::IfLet {
        pattern: binding.0,
        expr: binding.1,
        then_body,
        else_body,
        span,
    })
}

fn classify_when_let(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 2 {
        return Err(err("when-let requires at least 2 arguments", span));
    }
    let binding = match &args[0] {
        SExpr::List { values, .. } if values.len() == 2 => {
            let pattern = classify_pattern(&values[0])?;
            let expr = values[1].clone();
            (pattern, expr)
        }
        _ => {
            return Err(err("when-let: first argument must be (pattern expr)", span));
        }
    };

    Ok(SurfaceForm::WhenLet {
        pattern: binding.0,
        expr: binding.1,
        body: args[1..].to_vec(),
        span,
    })
}

fn classify_fn(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 2 {
        return Err(err("fn requires params and body", span));
    }
    let params = match &args[0] {
        SExpr::List {
            values,
            span: pspan,
        } => parse_typed_params(values, *pspan)?,
        _ => {
            return Err(err("fn: first argument must be a parameter list", span));
        }
    };
    Ok(SurfaceForm::Fn {
        params,
        body: args[1..].to_vec(),
        span,
    })
}

fn classify_lambda(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 2 {
        return Err(err("lambda requires params and body", span));
    }
    let params = match &args[0] {
        SExpr::List {
            values,
            span: pspan,
        } => parse_typed_params(values, *pspan)?,
        _ => {
            return Err(err("lambda: first argument must be a parameter list", span));
        }
    };
    Ok(SurfaceForm::Lambda {
        params,
        body: args[1..].to_vec(),
        span,
    })
}

fn classify_conj(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() != 2 {
        return Err(err(
            "conj requires exactly 2 arguments: (conj array value)",
            span,
        ));
    }
    Ok(SurfaceForm::Conj {
        arr: args[0].clone(),
        value: args[1].clone(),
        span,
    })
}

fn classify_assoc(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 3 {
        return Err(err(
            "assoc requires at least 3 arguments: (assoc obj :key value)",
            span,
        ));
    }
    let obj = args[0].clone();
    let rest = &args[1..];
    if !rest.len().is_multiple_of(2) {
        return Err(err(
            "assoc requires keyword-value pairs after the object",
            span,
        ));
    }
    let mut pairs = Vec::new();
    for i in (0..rest.len()).step_by(2) {
        match &rest[i] {
            SExpr::Keyword { value, .. } => {
                pairs.push((value.clone(), rest[i + 1].clone()));
            }
            _ => {
                return Err(err(
                    format!("assoc: expected keyword at position {}", i + 1),
                    span,
                ));
            }
        }
    }
    Ok(SurfaceForm::Assoc { obj, pairs, span })
}

fn classify_dissoc(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 2 {
        return Err(err(
            "dissoc requires at least 2 arguments: (dissoc obj :key)",
            span,
        ));
    }
    let obj = args[0].clone();
    let mut keys = Vec::new();
    for (i, arg) in args[1..].iter().enumerate() {
        match arg {
            SExpr::Keyword { value, .. } => {
                keys.push(value.clone());
            }
            _ => {
                return Err(err(
                    format!("dissoc: expected keyword at position {}", i + 1),
                    span,
                ));
            }
        }
    }
    Ok(SurfaceForm::Dissoc { obj, keys, span })
}

fn classify_macro_def(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.is_empty() {
        return Err(err("macro requires a name", span));
    }
    let name = match &args[0] {
        SExpr::Atom { value, .. } => value.clone(),
        _ => return Err(err("macro: name must be an atom", span)),
    };
    // Store the entire original expression as raw
    let raw = SExpr::List {
        values: std::iter::once(SExpr::Atom {
            value: "macro".to_string(),
            span,
        })
        .chain(args.iter().cloned())
        .collect(),
        span,
    };
    Ok(SurfaceForm::MacroDef { name, raw, span })
}

fn classify_import_macros(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    let raw = SExpr::List {
        values: std::iter::once(SExpr::Atom {
            value: "import-macros".to_string(),
            span,
        })
        .chain(args.iter().cloned())
        .collect(),
        span,
    };
    Ok(SurfaceForm::ImportMacros { raw, span })
}

// ---------------------------------------------------------------------------
// Equality and logical operators (DD-22)
// ---------------------------------------------------------------------------

fn classify_eq(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 2 {
        return Err(err("= requires at least 2 arguments: (= a b)", span));
    }
    Ok(SurfaceForm::Eq {
        args: args.to_vec(),
        span,
    })
}

fn classify_not_eq(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() != 2 {
        return Err(err("!= requires exactly 2 arguments: (!= a b)", span));
    }
    Ok(SurfaceForm::NotEq {
        left: args[0].clone(),
        right: args[1].clone(),
        span,
    })
}

fn classify_and(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 2 {
        return Err(err("and requires at least 2 arguments: (and a b)", span));
    }
    Ok(SurfaceForm::And {
        args: args.to_vec(),
        span,
    })
}

fn classify_or(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() < 2 {
        return Err(err("or requires at least 2 arguments: (or a b)", span));
    }
    Ok(SurfaceForm::Or {
        args: args.to_vec(),
        span,
    })
}

fn classify_not(args: &[SExpr], span: Span) -> Result<SurfaceForm, Diagnostic> {
    if args.len() != 1 {
        return Err(err("not requires exactly 1 argument: (not x)", span));
    }
    Ok(SurfaceForm::Not {
        operand: args[0].clone(),
        span,
    })
}

/// Parse simple typed params (keyword-name pairs only). Used for type constructors.
fn parse_simple_typed_params(values: &[SExpr], span: Span) -> Result<Vec<TypedParam>, Diagnostic> {
    if !values.len().is_multiple_of(2) {
        return Err(err("typed parameters must be keyword-name pairs", span));
    }
    let mut params = Vec::new();
    for i in (0..values.len()).step_by(2) {
        match &values[i] {
            SExpr::Keyword {
                value: type_name,
                span: kspan,
            } => match &values[i + 1] {
                SExpr::Atom {
                    value: name,
                    span: nspan,
                } => {
                    params.push(TypedParam {
                        type_ann: TypeAnnotation {
                            name: type_name.clone(),
                            span: *kspan,
                        },
                        name: name.clone(),
                        name_span: *nspan,
                    });
                }
                _ => return Err(err("parameter name must be an atom", span)),
            },
            _ => {
                return Err(err(format!("expected type keyword at position {i}"), span));
            }
        }
    }
    Ok(params)
}

/// Parse typed params with support for destructuring patterns.
/// Returns Vec<ParamShape> — each element is either Simple or Destructured.
fn parse_typed_params(values: &[SExpr], span: Span) -> Result<Vec<ParamShape>, Diagnostic> {
    let mut params = Vec::new();
    let mut i = 0;
    while i < values.len() {
        match &values[i] {
            // Destructuring pattern: a list at position i
            SExpr::List {
                values: inner,
                span: lspan,
                ..
            } => {
                params.push(parse_destructured_param(inner, *lspan)?);
                i += 1;
            }
            // Simple param: keyword at i, atom at i+1
            SExpr::Keyword {
                value: type_name,
                span: kspan,
            } => {
                if i + 1 >= values.len() {
                    return Err(err(
                        format!("type keyword :{type_name} has no parameter name"),
                        span,
                    ));
                }
                match &values[i + 1] {
                    SExpr::Atom {
                        value: name,
                        span: nspan,
                    } => {
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

fn parse_destructured_param(values: &[SExpr], span: Span) -> Result<ParamShape, Diagnostic> {
    if values.is_empty() {
        return Err(err(
            "empty destructuring pattern — at least one field required",
            span,
        ));
    }
    let head = match &values[0] {
        SExpr::Atom { value, .. } => value.as_str(),
        _ => {
            return Err(err(
                "destructuring pattern must start with 'object' or 'array'",
                span,
            ));
        }
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

fn parse_object_destructure(values: &[SExpr], span: Span) -> Result<ParamShape, Diagnostic> {
    if values.is_empty() {
        return Err(err(
            "empty destructuring pattern — at least one field required",
            span,
        ));
    }
    let mut fields = Vec::new();
    let mut i = 0;
    while i < values.len() {
        match &values[i] {
            // Check for deferred features in type position
            SExpr::List { values: inner, .. } => {
                let head_name = inner.first().and_then(|e| e.as_atom()).unwrap_or("");
                if head_name == "default" {
                    return Err(err(
                        "default values in destructured params are not yet supported \
                         — use a typed param with body destructuring and default",
                        span,
                    ));
                }
                if head_name == "object" || head_name == "array" || head_name == "alias" {
                    return Err(err(
                        "nested destructuring in func/fn params is not yet supported \
                         — use a typed param with body destructuring",
                        span,
                    ));
                }
                return Err(err(
                    format!("expected type keyword at position {i} in destructuring pattern"),
                    span,
                ));
            }
            SExpr::Keyword {
                value: type_name,
                span: kspan,
            } => {
                if i + 1 >= values.len() {
                    return Err(err(
                        format!(
                            "type keyword :{type_name} has no field name in destructuring pattern"
                        ),
                        span,
                    ));
                }
                match &values[i + 1] {
                    SExpr::Atom {
                        value: name,
                        span: nspan,
                    } => {
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
                        let head_name = inner.first().and_then(|e| e.as_atom()).unwrap_or("");
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

fn parse_array_destructure(values: &[SExpr], span: Span) -> Result<ParamShape, Diagnostic> {
    if values.is_empty() {
        return Err(err(
            "empty destructuring pattern — at least one field required",
            span,
        ));
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
            // Rest or deferred feature list
            SExpr::List {
                values: inner,
                span: lspan,
                ..
            } => {
                let head_name = inner.first().and_then(|e| e.as_atom()).unwrap_or("");
                match head_name {
                    "rest" => {
                        if inner.len() != 3 {
                            return Err(err("rest element must be (rest :type name)", *lspan));
                        }
                        if i + 1 != values.len() {
                            return Err(err(
                                "rest element must be last in array destructuring",
                                span,
                            ));
                        }
                        let tp = parse_rest_element(&inner[1..], *lspan)?;
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
            SExpr::Keyword {
                value: type_name,
                span: kspan,
            } => {
                if i + 1 >= values.len() {
                    return Err(err(
                        format!("type keyword :{type_name} has no element name"),
                        span,
                    ));
                }
                match &values[i + 1] {
                    SExpr::Atom {
                        value: name,
                        span: nspan,
                    } => {
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
            // Bare name without type keyword
            SExpr::Atom { value: name, .. } => {
                return Err(err(
                    format!("field '{name}' missing type annotation (use :any to opt out)"),
                    span,
                ));
            }
            _ => {
                return Err(err(
                    format!(
                        "expected type keyword, _, or (rest ...) at position {i} in array destructuring"
                    ),
                    span,
                ));
            }
        }
    }
    Ok(ParamShape::DestructuredArray { elements, span })
}

fn parse_rest_element(values: &[SExpr], span: Span) -> Result<TypedParam, Diagnostic> {
    match (&values[0], &values[1]) {
        (
            SExpr::Keyword {
                value: type_name,
                span: kspan,
            },
            SExpr::Atom {
                value: name,
                span: nspan,
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------

    fn s() -> Span {
        Span::default()
    }

    fn atom(name: &str) -> SExpr {
        SExpr::Atom {
            value: name.to_string(),
            span: s(),
        }
    }

    fn kw(name: &str) -> SExpr {
        SExpr::Keyword {
            value: name.to_string(),
            span: s(),
        }
    }

    fn num(n: f64) -> SExpr {
        SExpr::Number {
            value: n,
            span: s(),
        }
    }

    fn string(v: &str) -> SExpr {
        SExpr::String {
            value: v.to_string(),
            span: s(),
        }
    }

    fn boolean(v: bool) -> SExpr {
        SExpr::Bool {
            value: v,
            span: s(),
        }
    }

    fn list(vals: Vec<SExpr>) -> SExpr {
        SExpr::List {
            values: vals,
            span: s(),
        }
    }

    /// Shorthand: builds `(head arg0 arg1 ...)` and runs `classify_form`.
    fn form(head: &str, args: Vec<SExpr>) -> Result<SurfaceForm, Diagnostic> {
        let mut vals = vec![atom(head)];
        vals.extend(args);
        classify_form(&list(vals))
    }

    // ---------------------------------------------------------------
    // classify_form -- top-level dispatch
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_form_non_list_is_kernel_passthrough() {
        let expr = atom("x");
        let result = classify_form(&expr).unwrap();
        assert!(matches!(result, SurfaceForm::KernelPassthrough { .. }));
    }

    #[test]
    fn test_classify_form_empty_list_is_kernel_passthrough() {
        let expr = list(vec![]);
        let result = classify_form(&expr).unwrap();
        assert!(matches!(result, SurfaceForm::KernelPassthrough { .. }));
    }

    #[test]
    fn test_classify_form_kernel_form_head() {
        // "const" is a kernel form
        let expr = list(vec![atom("const"), atom("x"), num(1.0)]);
        let result = classify_form(&expr).unwrap();
        assert!(matches!(result, SurfaceForm::KernelPassthrough { .. }));
    }

    #[test]
    fn test_classify_form_function_call() {
        // "my-fn" is not a surface or kernel form
        let result = form("my-fn", vec![num(1.0), num(2.0)]).unwrap();
        match result {
            SurfaceForm::FunctionCall { head, args, .. } => {
                assert_eq!(head.as_atom(), Some("my-fn"));
                assert_eq!(args.len(), 2);
            }
            other => panic!("expected FunctionCall, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_form_computed_head() {
        // Head is a list (not an atom) -- computed function call
        let expr = list(vec![list(vec![atom("get-fn")]), num(42.0)]);
        let result = classify_form(&expr).unwrap();
        match result {
            SurfaceForm::FunctionCall { head, args, .. } => {
                assert!(matches!(head, SExpr::List { .. }));
                assert_eq!(args.len(), 1);
            }
            other => panic!("expected FunctionCall, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_form_number_is_kernel_passthrough() {
        let result = classify_form(&num(42.0)).unwrap();
        assert!(matches!(result, SurfaceForm::KernelPassthrough { .. }));
    }

    #[test]
    fn test_classify_form_string_is_kernel_passthrough() {
        let result = classify_form(&string("hello")).unwrap();
        assert!(matches!(result, SurfaceForm::KernelPassthrough { .. }));
    }

    // ---------------------------------------------------------------
    // classify_bind
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_bind_two_args() {
        let result = form("bind", vec![atom("x"), num(42.0)]).unwrap();
        match result {
            SurfaceForm::Bind {
                name,
                type_ann,
                value,
                ..
            } => {
                assert_eq!(name.as_atom(), Some("x"));
                assert!(type_ann.is_none());
                assert!(matches!(value, SExpr::Number { value: v, .. } if v == 42.0));
            }
            other => panic!("expected Bind, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_bind_three_args_with_type() {
        let result = form("bind", vec![kw("string"), atom("name"), string("alice")]).unwrap();
        match result {
            SurfaceForm::Bind {
                name,
                type_ann,
                value,
                ..
            } => {
                assert_eq!(name.as_atom(), Some("name"));
                assert_eq!(type_ann.unwrap().name, "string");
                assert!(matches!(value, SExpr::String { .. }));
            }
            other => panic!("expected Bind, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_bind_three_args_non_keyword_type_error() {
        let result = form("bind", vec![atom("bad"), atom("name"), num(1.0)]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("type keyword as first argument")
        );
    }

    #[test]
    fn test_classify_bind_wrong_arg_count() {
        let result = form("bind", vec![atom("x")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("2 or 3 arguments"));

        let result = form("bind", vec![atom("a"), atom("b"), atom("c"), atom("d")]);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // classify_obj
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_obj_valid_pairs() {
        let result = form(
            "obj",
            vec![kw("name"), string("alice"), kw("age"), num(30.0)],
        )
        .unwrap();
        match result {
            SurfaceForm::Obj { pairs, .. } => {
                assert_eq!(pairs.len(), 2);
                assert_eq!(pairs[0].0, "name");
                assert_eq!(pairs[1].0, "age");
            }
            other => panic!("expected Obj, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_obj_empty() {
        let result = form("obj", vec![]).unwrap();
        match result {
            SurfaceForm::Obj { pairs, .. } => assert!(pairs.is_empty()),
            other => panic!("expected Obj, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_obj_odd_count_error() {
        let result = form("obj", vec![kw("name"), string("alice"), kw("age")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("even number"));
    }

    #[test]
    fn test_classify_obj_non_keyword_error() {
        let result = form("obj", vec![atom("name"), string("alice")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("expected keyword"));
    }

    // ---------------------------------------------------------------
    // classify_cell
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_cell_valid() {
        let result = form("cell", vec![num(0.0)]).unwrap();
        assert!(matches!(result, SurfaceForm::Cell { .. }));
    }

    #[test]
    fn test_classify_cell_zero_args_error() {
        let result = form("cell", vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("exactly 1"));
    }

    #[test]
    fn test_classify_cell_too_many_args_error() {
        let result = form("cell", vec![num(1.0), num(2.0)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("exactly 1"));
    }

    // ---------------------------------------------------------------
    // classify_express
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_express_valid() {
        let result = form("express", vec![atom("my-cell")]).unwrap();
        match result {
            SurfaceForm::Express { target, .. } => {
                assert_eq!(target.as_atom(), Some("my-cell"));
            }
            other => panic!("expected Express, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_express_zero_args_error() {
        let result = form("express", vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("exactly 1"));
    }

    #[test]
    fn test_classify_express_too_many_args_error() {
        let result = form("express", vec![atom("a"), atom("b")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("exactly 1"));
    }

    // ---------------------------------------------------------------
    // classify_swap
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_swap_two_args() {
        let result = form("swap!", vec![atom("c"), atom("inc")]).unwrap();
        match result {
            SurfaceForm::Swap {
                target,
                func,
                extra_args,
                ..
            } => {
                assert_eq!(target.as_atom(), Some("c"));
                assert_eq!(func.as_atom(), Some("inc"));
                assert!(extra_args.is_empty());
            }
            other => panic!("expected Swap, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_swap_with_extra_args() {
        let result = form("swap!", vec![atom("c"), atom("add"), num(5.0)]).unwrap();
        match result {
            SurfaceForm::Swap { extra_args, .. } => {
                assert_eq!(extra_args.len(), 1);
            }
            other => panic!("expected Swap, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_swap_too_few_args() {
        let result = form("swap!", vec![atom("c")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("at least 2"));
    }

    #[test]
    fn test_classify_swap_zero_args_error() {
        let result = form("swap!", vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("at least 2"));
    }

    // ---------------------------------------------------------------
    // classify_reset
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_reset_valid() {
        let result = form("reset!", vec![atom("c"), num(42.0)]).unwrap();
        match result {
            SurfaceForm::Reset { target, value, .. } => {
                assert_eq!(target.as_atom(), Some("c"));
                assert!(matches!(value, SExpr::Number { value: v, .. } if v == 42.0));
            }
            other => panic!("expected Reset, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_reset_one_arg_error() {
        let result = form("reset!", vec![atom("c")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("exactly 2"));
    }

    #[test]
    fn test_classify_reset_three_args_error() {
        let result = form("reset!", vec![atom("c"), num(1.0), num(2.0)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("exactly 2"));
    }

    // ---------------------------------------------------------------
    // classify_threading -- all four variants
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_thread_first() {
        let result = form("->", vec![atom("x"), atom("inc"), atom("double")]).unwrap();
        match result {
            SurfaceForm::ThreadFirst { initial, steps, .. } => {
                assert_eq!(initial.as_atom(), Some("x"));
                assert_eq!(steps.len(), 2);
                assert!(matches!(&steps[0], ThreadingStep::Bare(_)));
            }
            other => panic!("expected ThreadFirst, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_thread_last() {
        let result = form("->>", vec![atom("x"), list(vec![atom("map"), atom("f")])]).unwrap();
        match result {
            SurfaceForm::ThreadLast { steps, .. } => {
                assert_eq!(steps.len(), 1);
                assert!(matches!(&steps[0], ThreadingStep::Call(_)));
            }
            other => panic!("expected ThreadLast, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_some_thread_first() {
        let result = form("some->", vec![atom("x"), atom("f")]).unwrap();
        assert!(matches!(result, SurfaceForm::SomeThreadFirst { .. }));
    }

    #[test]
    fn test_classify_some_thread_last() {
        let result = form("some->>", vec![atom("x"), atom("f")]).unwrap();
        assert!(matches!(result, SurfaceForm::SomeThreadLast { .. }));
    }

    #[test]
    fn test_classify_threading_too_few_args() {
        let result = form("->", vec![atom("x")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("at least 2"));
    }

    #[test]
    fn test_classify_threading_zero_args_error() {
        let result = form("->>", vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("at least 2"));
    }

    #[test]
    fn test_classify_threading_mixed_steps() {
        let result = form(
            "->",
            vec![
                atom("x"),
                atom("inc"),
                list(vec![atom("add"), num(5.0)]),
                atom("double"),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::ThreadFirst { steps, .. } => {
                assert_eq!(steps.len(), 3);
                assert!(matches!(&steps[0], ThreadingStep::Bare(_)));
                assert!(matches!(&steps[1], ThreadingStep::Call(_)));
                assert!(matches!(&steps[2], ThreadingStep::Bare(_)));
            }
            other => panic!("expected ThreadFirst, got {other:?}"),
        }
    }

    // ---------------------------------------------------------------
    // classify_type
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_type_atom_constructors() {
        let result = form(
            "type",
            vec![atom("Color"), atom("Red"), atom("Green"), atom("Blue")],
        )
        .unwrap();
        match result {
            SurfaceForm::Type {
                name, constructors, ..
            } => {
                assert_eq!(name, "Color");
                assert_eq!(constructors.len(), 3);
                assert_eq!(constructors[0].name, "Red");
                assert!(constructors[0].fields.is_empty());
            }
            other => panic!("expected Type, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_type_list_constructor_with_fields() {
        let result = form(
            "type",
            vec![
                atom("Shape"),
                list(vec![atom("Circle"), kw("number"), atom("radius")]),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Type { constructors, .. } => {
                assert_eq!(constructors.len(), 1);
                assert_eq!(constructors[0].name, "Circle");
                assert_eq!(constructors[0].fields.len(), 1);
                assert_eq!(constructors[0].fields[0].name, "radius");
                assert_eq!(constructors[0].fields[0].type_ann.name, "number");
            }
            other => panic!("expected Type, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_type_too_few_args() {
        let result = form("type", vec![atom("Color")]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("at least one constructor")
        );
    }

    #[test]
    fn test_classify_type_zero_args_error() {
        let result = form("type", vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_classify_type_non_atom_name() {
        let result = form("type", vec![num(42.0), atom("X")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("type name"));
    }

    #[test]
    fn test_classify_type_empty_list_constructor_error() {
        let result = form("type", vec![atom("T"), list(vec![])]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("constructor must be an atom or list")
        );
    }

    #[test]
    fn test_classify_type_non_atom_constructor_name_error() {
        let result = form("type", vec![atom("T"), list(vec![num(1.0)])]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("constructor name must be an atom")
        );
    }

    #[test]
    fn test_classify_type_invalid_constructor_form() {
        // A keyword as a constructor is neither atom nor list-with-values
        let result = form("type", vec![atom("T"), kw("bad")]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("constructor must be an atom or list")
        );
    }

    #[test]
    fn test_classify_type_mixed_constructors() {
        let result = form(
            "type",
            vec![
                atom("Result"),
                atom("Loading"),
                list(vec![atom("Ok"), kw("any"), atom("value")]),
                list(vec![atom("Err"), kw("string"), atom("msg")]),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Type { constructors, .. } => {
                assert_eq!(constructors.len(), 3);
                assert!(constructors[0].fields.is_empty());
                assert_eq!(constructors[1].fields.len(), 1);
                assert_eq!(constructors[2].fields.len(), 1);
                assert_eq!(constructors[2].fields[0].type_ann.name, "string");
            }
            other => panic!("expected Type, got {other:?}"),
        }
    }

    // ---------------------------------------------------------------
    // classify_func
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_func_zero_arg_shorthand() {
        let result = form(
            "func",
            vec![atom("main"), list(vec![atom("console.log"), string("hi")])],
        )
        .unwrap();
        match result {
            SurfaceForm::Func { name, clauses, .. } => {
                assert_eq!(name, "main");
                assert_eq!(clauses.len(), 1);
                assert!(clauses[0].args.is_empty());
                assert_eq!(clauses[0].body.len(), 1);
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_func_single_clause_keyword_mode() {
        let result = form(
            "func",
            vec![
                atom("add"),
                kw("args"),
                list(vec![kw("number"), atom("a"), kw("number"), atom("b")]),
                kw("body"),
                list(vec![atom("+"), atom("a"), atom("b")]),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Func { name, clauses, .. } => {
                assert_eq!(name, "add");
                assert_eq!(clauses.len(), 1);
                assert_eq!(clauses[0].args.len(), 2);
                assert_eq!(clauses[0].args[0].bound_names(), vec!["a"]);
                assert_eq!(clauses[0].args[1].bound_names(), vec!["b"]);
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_func_multi_clause() {
        let result = form(
            "func",
            vec![
                atom("greet"),
                list(vec![kw("args"), list(vec![]), kw("body"), atom("a")]),
                list(vec![
                    kw("args"),
                    list(vec![kw("string"), atom("name")]),
                    kw("body"),
                    atom("b"),
                ]),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Func { clauses, .. } => {
                assert_eq!(clauses.len(), 2);
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_func_with_returns_pre_post() {
        let result = form(
            "func",
            vec![
                atom("double"),
                kw("args"),
                list(vec![kw("number"), atom("x")]),
                kw("returns"),
                kw("number"),
                kw("pre"),
                list(vec![atom(">"), atom("x"), num(0.0)]),
                kw("post"),
                list(vec![atom(">"), atom("result"), atom("x")]),
                kw("body"),
                list(vec![atom("*"), atom("x"), num(2.0)]),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Func { clauses, .. } => {
                let c = &clauses[0];
                assert!(c.returns.is_some());
                assert_eq!(c.returns.as_ref().unwrap().name, "number");
                assert!(c.pre.is_some());
                assert!(c.post.is_some());
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_func_no_name_error() {
        let result = form("func", vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("at least a name"));
    }

    #[test]
    fn test_classify_func_non_atom_name_error() {
        let result = form("func", vec![num(1.0)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("function name"));
    }

    #[test]
    fn test_classify_func_name_only_no_body_error() {
        let result = form("func", vec![atom("f")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("body"));
    }

    #[test]
    fn test_classify_func_clause_missing_body_error() {
        let result = form("func", vec![atom("f"), kw("args"), list(vec![])]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains(":body"));
    }

    #[test]
    fn test_classify_func_multi_clause_non_list_error() {
        let result = form(
            "func",
            vec![
                atom("f"),
                list(vec![kw("args"), list(vec![]), kw("body"), atom("a")]),
                atom("not-a-clause"),
            ],
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("clause must be a list")
        );
    }

    #[test]
    fn test_classify_func_zero_arg_multiple_body_exprs() {
        let result = form("func", vec![atom("main"), atom("expr1"), atom("expr2")]).unwrap();
        match result {
            SurfaceForm::Func { clauses, .. } => {
                assert_eq!(clauses[0].body.len(), 2);
                assert!(clauses[0].args.is_empty());
                assert!(clauses[0].returns.is_none());
                assert!(clauses[0].pre.is_none());
                assert!(clauses[0].post.is_none());
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_func_single_clause_returns_non_keyword_ignored() {
        // :returns followed by a non-keyword should yield None for returns
        let result = form(
            "func",
            vec![
                atom("f"),
                kw("args"),
                list(vec![]),
                kw("returns"),
                atom("not-a-keyword"),
                kw("body"),
                atom("x"),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Func { clauses, .. } => {
                assert!(clauses[0].returns.is_none());
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_func_single_clause_args_not_list() {
        // :args followed by a non-list should yield empty args
        let result = form(
            "func",
            vec![
                atom("f"),
                kw("args"),
                atom("not-a-list"),
                kw("body"),
                atom("x"),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Func { clauses, .. } => {
                assert!(clauses[0].args.is_empty());
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    // ---------------------------------------------------------------
    // classify_match
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_match_basic() {
        let result = form(
            "match",
            vec![
                atom("x"),
                list(vec![num(1.0), string("one")]),
                list(vec![num(2.0), string("two")]),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Match {
                target, clauses, ..
            } => {
                assert_eq!(target.as_atom(), Some("x"));
                assert_eq!(clauses.len(), 2);
                assert!(matches!(&clauses[0].pattern, Pattern::Literal(_)));
            }
            other => panic!("expected Match, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_match_with_guard() {
        let result = form(
            "match",
            vec![
                atom("x"),
                list(vec![
                    atom("n"),
                    kw("when"),
                    list(vec![atom(">"), atom("n"), num(0.0)]),
                    string("positive"),
                ]),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Match { clauses, .. } => {
                assert_eq!(clauses.len(), 1);
                assert!(clauses[0].guard.is_some());
                assert!(
                    matches!(&clauses[0].pattern, Pattern::Binding { name, .. } if name == "n")
                );
            }
            other => panic!("expected Match, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_match_too_few_args() {
        let result = form("match", vec![atom("x")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("at least one clause"));
    }

    #[test]
    fn test_classify_match_zero_args_error() {
        let result = form("match", vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_classify_match_clause_not_list() {
        let result = form("match", vec![atom("x"), atom("bad")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("must be a list"));
    }

    #[test]
    fn test_classify_match_clause_too_short() {
        let result = form("match", vec![atom("x"), list(vec![atom("y")])]);
        assert!(result.is_err());
    }

    #[test]
    fn test_classify_match_guard_no_body_error() {
        // (match x (n :when guard)) -- guard consumes positions, no body left
        let result = form(
            "match",
            vec![atom("x"), list(vec![atom("n"), kw("when"), atom("guard")])],
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("must have a body"));
    }

    #[test]
    fn test_classify_match_multiple_body_exprs() {
        let result = form(
            "match",
            vec![
                atom("x"),
                list(vec![atom("n"), atom("body1"), atom("body2")]),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Match { clauses, .. } => {
                assert_eq!(clauses[0].body.len(), 2);
                assert!(clauses[0].guard.is_none());
            }
            other => panic!("expected Match, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_match_nested_constructor_pattern() {
        let result = form(
            "match",
            vec![
                atom("x"),
                list(vec![
                    list(vec![atom("Some"), list(vec![atom("Just"), atom("v")])]),
                    atom("body"),
                ]),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Match { clauses, .. } => match &clauses[0].pattern {
                Pattern::Constructor { name, bindings, .. } => {
                    assert_eq!(name, "Some");
                    assert_eq!(bindings.len(), 1);
                    assert!(matches!(
                        &bindings[0],
                        Pattern::Constructor { name, .. } if name == "Just"
                    ));
                }
                other => panic!("expected Constructor, got {other:?}"),
            },
            other => panic!("expected Match, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_match_wildcard_pattern() {
        let result = form(
            "match",
            vec![atom("x"), list(vec![atom("_"), string("default")])],
        )
        .unwrap();
        match result {
            SurfaceForm::Match { clauses, .. } => {
                assert!(matches!(&clauses[0].pattern, Pattern::Wildcard(_)));
            }
            other => panic!("expected Match, got {other:?}"),
        }
    }

    // ---------------------------------------------------------------
    // classify_pattern
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_pattern_wildcard() {
        let pat = classify_pattern(&atom("_")).unwrap();
        assert!(matches!(pat, Pattern::Wildcard(_)));
    }

    #[test]
    fn test_classify_pattern_binding() {
        let pat = classify_pattern(&atom("x")).unwrap();
        assert!(matches!(pat, Pattern::Binding { name, .. } if name == "x"));
    }

    #[test]
    fn test_classify_pattern_literal_true() {
        let pat = classify_pattern(&atom("true")).unwrap();
        assert!(matches!(pat, Pattern::Literal(_)));
    }

    #[test]
    fn test_classify_pattern_literal_false() {
        let pat = classify_pattern(&atom("false")).unwrap();
        assert!(matches!(pat, Pattern::Literal(_)));
    }

    #[test]
    fn test_classify_pattern_literal_null() {
        let pat = classify_pattern(&atom("null")).unwrap();
        assert!(matches!(pat, Pattern::Literal(_)));
    }

    #[test]
    fn test_classify_pattern_literal_undefined() {
        let pat = classify_pattern(&atom("undefined")).unwrap();
        assert!(matches!(pat, Pattern::Literal(_)));
    }

    #[test]
    fn test_classify_pattern_number_literal() {
        let pat = classify_pattern(&num(42.0)).unwrap();
        assert!(matches!(pat, Pattern::Literal(_)));
    }

    #[test]
    fn test_classify_pattern_string_literal() {
        let pat = classify_pattern(&string("hi")).unwrap();
        assert!(matches!(pat, Pattern::Literal(_)));
    }

    #[test]
    fn test_classify_pattern_keyword_literal() {
        let pat = classify_pattern(&kw("status")).unwrap();
        assert!(matches!(pat, Pattern::Literal(_)));
    }

    #[test]
    fn test_classify_pattern_bool_literal() {
        let pat = classify_pattern(&boolean(true)).unwrap();
        assert!(matches!(pat, Pattern::Literal(_)));
    }

    #[test]
    fn test_classify_pattern_zero_field_constructor() {
        let pat = classify_pattern(&atom("None")).unwrap();
        match pat {
            Pattern::Constructor { name, bindings, .. } => {
                assert_eq!(name, "None");
                assert!(bindings.is_empty());
            }
            other => panic!("expected Constructor, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_pattern_constructor_with_bindings() {
        let pat = classify_pattern(&list(vec![atom("Some"), atom("x")])).unwrap();
        match pat {
            Pattern::Constructor { name, bindings, .. } => {
                assert_eq!(name, "Some");
                assert_eq!(bindings.len(), 1);
                assert!(matches!(&bindings[0], Pattern::Binding { name, .. } if name == "x"));
            }
            other => panic!("expected Constructor, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_pattern_constructor_multiple_bindings() {
        // (Pair a b)
        let pat = classify_pattern(&list(vec![atom("Pair"), atom("a"), atom("b")])).unwrap();
        match pat {
            Pattern::Constructor { bindings, .. } => {
                assert_eq!(bindings.len(), 2);
            }
            other => panic!("expected Constructor, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_pattern_obj() {
        let pat = classify_pattern(&list(vec![
            atom("obj"),
            kw("name"),
            atom("n"),
            kw("age"),
            atom("a"),
        ]))
        .unwrap();
        match pat {
            Pattern::Obj { pairs, .. } => {
                assert_eq!(pairs.len(), 2);
                assert_eq!(pairs[0].0, "name");
                assert_eq!(pairs[1].0, "age");
            }
            other => panic!("expected Obj pattern, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_pattern_obj_odd_count_error() {
        let pat = classify_pattern(&list(vec![atom("obj"), kw("name")]));
        assert!(pat.is_err());
        assert!(pat.unwrap_err().message.contains("keyword-binding pairs"));
    }

    #[test]
    fn test_classify_pattern_obj_non_keyword_error() {
        let pat = classify_pattern(&list(vec![atom("obj"), atom("bad"), atom("x")]));
        assert!(pat.is_err());
        assert!(pat.unwrap_err().message.contains("expected keyword"));
    }

    #[test]
    fn test_classify_pattern_unrecognized_list_error() {
        let pat = classify_pattern(&list(vec![atom("foo"), atom("bar")]));
        assert!(pat.is_err());
        assert!(pat.unwrap_err().message.contains("unrecognized pattern"));
    }

    #[test]
    fn test_classify_pattern_empty_list_error() {
        let pat = classify_pattern(&list(vec![]));
        assert!(pat.is_err());
        assert!(pat.unwrap_err().message.contains("invalid pattern"));
    }

    #[test]
    fn test_classify_pattern_null_sexpr() {
        let expr = SExpr::Null { span: s() };
        let pat = classify_pattern(&expr);
        assert!(pat.is_err());
        assert!(pat.unwrap_err().message.contains("invalid pattern"));
    }

    // ---------------------------------------------------------------
    // classify_if_let
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_if_let_two_args() {
        let result = form(
            "if-let",
            vec![list(vec![atom("x"), atom("val")]), string("then")],
        )
        .unwrap();
        match result {
            SurfaceForm::IfLet {
                pattern,
                then_body,
                else_body,
                ..
            } => {
                assert!(matches!(pattern, Pattern::Binding { name, .. } if name == "x"));
                assert!(matches!(then_body, SExpr::String { .. }));
                assert!(else_body.is_none());
            }
            other => panic!("expected IfLet, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_if_let_three_args() {
        let result = form(
            "if-let",
            vec![
                list(vec![atom("x"), atom("val")]),
                string("then"),
                string("else"),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::IfLet { else_body, .. } => {
                assert!(else_body.is_some());
            }
            other => panic!("expected IfLet, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_if_let_too_few_args() {
        let result = form("if-let", vec![list(vec![atom("x"), atom("v")])]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("2-3 arguments"));
    }

    #[test]
    fn test_classify_if_let_too_many_args() {
        let result = form(
            "if-let",
            vec![
                list(vec![atom("x"), atom("v")]),
                atom("a"),
                atom("b"),
                atom("c"),
            ],
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("2-3 arguments"));
    }

    #[test]
    fn test_classify_if_let_non_list_binding_error() {
        let result = form("if-let", vec![atom("bad"), atom("body")]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("first argument must be (pattern expr)")
        );
    }

    #[test]
    fn test_classify_if_let_binding_wrong_length_error() {
        let result = form("if-let", vec![list(vec![atom("x")]), atom("body")]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("first argument must be (pattern expr)")
        );
    }

    #[test]
    fn test_classify_if_let_binding_three_elements_error() {
        let result = form(
            "if-let",
            vec![list(vec![atom("x"), atom("y"), atom("z")]), atom("body")],
        );
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // classify_when_let
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_when_let_valid() {
        let result = form(
            "when-let",
            vec![
                list(vec![atom("x"), atom("val")]),
                atom("body1"),
                atom("body2"),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::WhenLet { pattern, body, .. } => {
                assert!(matches!(pattern, Pattern::Binding { name, .. } if name == "x"));
                assert_eq!(body.len(), 2);
            }
            other => panic!("expected WhenLet, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_when_let_too_few_args() {
        let result = form("when-let", vec![list(vec![atom("x"), atom("v")])]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("at least 2"));
    }

    #[test]
    fn test_classify_when_let_non_list_binding_error() {
        let result = form("when-let", vec![atom("bad"), atom("body")]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("first argument must be (pattern expr)")
        );
    }

    #[test]
    fn test_classify_when_let_zero_args_error() {
        let result = form("when-let", vec![]);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // classify_fn
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_fn_valid() {
        let result = form(
            "fn",
            vec![
                list(vec![kw("number"), atom("x")]),
                list(vec![atom("*"), atom("x"), num(2.0)]),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Fn { params, body, .. } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].bound_names(), vec!["x"]);
                assert_eq!(body.len(), 1);
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_fn_empty_params() {
        let result = form("fn", vec![list(vec![]), atom("body")]).unwrap();
        match result {
            SurfaceForm::Fn { params, body, .. } => {
                assert!(params.is_empty());
                assert_eq!(body.len(), 1);
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_fn_multiple_body_exprs() {
        let result = form("fn", vec![list(vec![]), atom("a"), atom("b"), atom("c")]).unwrap();
        match result {
            SurfaceForm::Fn { body, .. } => {
                assert_eq!(body.len(), 3);
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_fn_too_few_args() {
        let result = form("fn", vec![list(vec![])]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("params and body"));
    }

    #[test]
    fn test_classify_fn_zero_args_error() {
        let result = form("fn", vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_classify_fn_non_list_params_error() {
        let result = form("fn", vec![atom("bad"), atom("body")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("parameter list"));
    }

    // ---------------------------------------------------------------
    // classify_lambda
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_lambda_valid() {
        let result = form(
            "lambda",
            vec![
                list(vec![kw("string"), atom("s")]),
                list(vec![atom("console.log"), atom("s")]),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Lambda { params, body, .. } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].bound_names(), vec!["s"]);
                assert_eq!(body.len(), 1);
            }
            other => panic!("expected Lambda, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_lambda_empty_params() {
        let result = form("lambda", vec![list(vec![]), atom("body")]).unwrap();
        match result {
            SurfaceForm::Lambda { params, .. } => {
                assert!(params.is_empty());
            }
            other => panic!("expected Lambda, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_lambda_too_few_args() {
        let result = form("lambda", vec![list(vec![])]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("params and body"));
    }

    #[test]
    fn test_classify_lambda_zero_args_error() {
        let result = form("lambda", vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_classify_lambda_non_list_params_error() {
        let result = form("lambda", vec![atom("bad"), atom("body")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("parameter list"));
    }

    // ---------------------------------------------------------------
    // classify_conj
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_conj_valid() {
        let result = form("conj", vec![atom("arr"), num(42.0)]).unwrap();
        match result {
            SurfaceForm::Conj { arr, value, .. } => {
                assert_eq!(arr.as_atom(), Some("arr"));
                assert!(matches!(value, SExpr::Number { value: v, .. } if v == 42.0));
            }
            other => panic!("expected Conj, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_conj_one_arg_error() {
        let result = form("conj", vec![atom("arr")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("exactly 2"));
    }

    #[test]
    fn test_classify_conj_three_args_error() {
        let result = form("conj", vec![atom("a"), atom("b"), atom("c")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("exactly 2"));
    }

    #[test]
    fn test_classify_conj_zero_args_error() {
        let result = form("conj", vec![]);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // classify_assoc
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_assoc_valid() {
        let result = form("assoc", vec![atom("obj"), kw("name"), string("alice")]).unwrap();
        match result {
            SurfaceForm::Assoc { obj, pairs, .. } => {
                assert_eq!(obj.as_atom(), Some("obj"));
                assert_eq!(pairs.len(), 1);
                assert_eq!(pairs[0].0, "name");
            }
            other => panic!("expected Assoc, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_assoc_multiple_pairs() {
        let result = form(
            "assoc",
            vec![atom("o"), kw("a"), num(1.0), kw("b"), num(2.0)],
        )
        .unwrap();
        match result {
            SurfaceForm::Assoc { pairs, .. } => assert_eq!(pairs.len(), 2),
            other => panic!("expected Assoc, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_assoc_too_few_args() {
        let result = form("assoc", vec![atom("obj"), kw("k")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("at least 3"));
    }

    #[test]
    fn test_classify_assoc_zero_args_error() {
        let result = form("assoc", vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_classify_assoc_odd_pair_count_error() {
        let result = form("assoc", vec![atom("obj"), kw("a"), num(1.0), kw("b")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("keyword-value pairs"));
    }

    #[test]
    fn test_classify_assoc_non_keyword_error() {
        let result = form("assoc", vec![atom("obj"), atom("bad"), num(1.0)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("expected keyword"));
    }

    // ---------------------------------------------------------------
    // classify_dissoc
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_dissoc_valid() {
        let result = form("dissoc", vec![atom("obj"), kw("name")]).unwrap();
        match result {
            SurfaceForm::Dissoc { obj, keys, .. } => {
                assert_eq!(obj.as_atom(), Some("obj"));
                assert_eq!(keys, vec!["name"]);
            }
            other => panic!("expected Dissoc, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_dissoc_multiple_keys() {
        let result = form("dissoc", vec![atom("obj"), kw("a"), kw("b"), kw("c")]).unwrap();
        match result {
            SurfaceForm::Dissoc { keys, .. } => assert_eq!(keys.len(), 3),
            other => panic!("expected Dissoc, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_dissoc_too_few_args() {
        let result = form("dissoc", vec![atom("obj")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("at least 2"));
    }

    #[test]
    fn test_classify_dissoc_zero_args_error() {
        let result = form("dissoc", vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_classify_dissoc_non_keyword_key_error() {
        let result = form("dissoc", vec![atom("obj"), atom("bad")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("expected keyword"));
    }

    // ---------------------------------------------------------------
    // classify_macro_def
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_macro_def_valid() {
        let result = form("macro", vec![atom("my-macro"), atom("body")]).unwrap();
        match result {
            SurfaceForm::MacroDef { name, raw, .. } => {
                assert_eq!(name, "my-macro");
                if let SExpr::List { values, .. } = &raw {
                    assert_eq!(values[0].as_atom(), Some("macro"));
                    assert_eq!(values.len(), 3);
                } else {
                    panic!("expected raw to be a list");
                }
            }
            other => panic!("expected MacroDef, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_macro_def_name_only() {
        let result = form("macro", vec![atom("m")]).unwrap();
        match result {
            SurfaceForm::MacroDef { name, .. } => assert_eq!(name, "m"),
            other => panic!("expected MacroDef, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_macro_def_no_name_error() {
        let result = form("macro", vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("requires a name"));
    }

    #[test]
    fn test_classify_macro_def_non_atom_name_error() {
        let result = form("macro", vec![num(1.0)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("name must be an atom"));
    }

    // ---------------------------------------------------------------
    // classify_import_macros
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_import_macros_valid() {
        let result = form("import-macros", vec![string("./macros.lykn")]).unwrap();
        match result {
            SurfaceForm::ImportMacros { raw, .. } => {
                if let SExpr::List { values, .. } = &raw {
                    assert_eq!(values[0].as_atom(), Some("import-macros"));
                    assert_eq!(values.len(), 2);
                } else {
                    panic!("expected raw to be a list");
                }
            }
            other => panic!("expected ImportMacros, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_import_macros_empty() {
        let result = form("import-macros", vec![]).unwrap();
        match result {
            SurfaceForm::ImportMacros { raw, .. } => {
                if let SExpr::List { values, .. } = &raw {
                    assert_eq!(values.len(), 1);
                } else {
                    panic!("expected raw to be a list");
                }
            }
            other => panic!("expected ImportMacros, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_import_macros_multiple_args() {
        let result = form(
            "import-macros",
            vec![string("./a.lykn"), string("./b.lykn")],
        )
        .unwrap();
        assert!(matches!(result, SurfaceForm::ImportMacros { .. }));
    }

    // ---------------------------------------------------------------
    // parse_typed_params (exercised through fn/lambda/type)
    // ---------------------------------------------------------------

    #[test]
    fn test_typed_params_trailing_keyword_error() {
        let result = form("fn", vec![list(vec![kw("number")]), atom("body")]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("has no parameter name")
        );
    }

    #[test]
    fn test_typed_params_non_keyword_type_error() {
        let result = form(
            "fn",
            vec![list(vec![atom("badtype"), atom("x")]), atom("body")],
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("expected type keyword")
        );
    }

    #[test]
    fn test_typed_params_non_atom_name_error() {
        let result = form(
            "fn",
            vec![list(vec![kw("number"), num(42.0)]), atom("body")],
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("parameter name must be an atom")
        );
    }

    #[test]
    fn test_typed_params_multiple_params() {
        let result = form(
            "fn",
            vec![
                list(vec![
                    kw("number"),
                    atom("x"),
                    kw("string"),
                    atom("y"),
                    kw("bool"),
                    atom("z"),
                ]),
                atom("body"),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Fn { params, .. } => {
                assert_eq!(params.len(), 3);
                assert_eq!(params[0].dispatch_type(), "number");
                assert_eq!(params[0].bound_names(), vec!["x"]);
                assert_eq!(params[1].dispatch_type(), "string");
                assert_eq!(params[1].bound_names(), vec!["y"]);
                assert_eq!(params[2].dispatch_type(), "bool");
                assert_eq!(params[2].bound_names(), vec!["z"]);
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    // ---------------------------------------------------------------
    // Destructured parameters (DD-25)
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_func_object_destructure() {
        // (func f :args ((object :string name :number age)) :body x)
        let result = form(
            "func",
            vec![
                atom("f"),
                kw("args"),
                list(vec![list(vec![
                    atom("object"),
                    kw("string"),
                    atom("name"),
                    kw("number"),
                    atom("age"),
                ])]),
                kw("body"),
                atom("x"),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Func { clauses, .. } => {
                assert_eq!(clauses.len(), 1);
                assert_eq!(clauses[0].args.len(), 1);
                match &clauses[0].args[0] {
                    ParamShape::DestructuredObject { fields, .. } => {
                        assert_eq!(fields.len(), 2);
                        assert_eq!(fields[0].name, "name");
                        assert_eq!(fields[0].type_ann.name, "string");
                        assert_eq!(fields[1].name, "age");
                        assert_eq!(fields[1].type_ann.name, "number");
                    }
                    other => panic!("expected DestructuredObject, got {other:?}"),
                }
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_func_array_destructure_with_rest() {
        // (func f :args ((array :number first (rest :number remaining))) :body x)
        let result = form(
            "func",
            vec![
                atom("f"),
                kw("args"),
                list(vec![list(vec![
                    atom("array"),
                    kw("number"),
                    atom("first"),
                    list(vec![atom("rest"), kw("number"), atom("remaining")]),
                ])]),
                kw("body"),
                atom("x"),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Func { clauses, .. } => {
                assert_eq!(clauses.len(), 1);
                assert_eq!(clauses[0].args.len(), 1);
                match &clauses[0].args[0] {
                    ParamShape::DestructuredArray { elements, .. } => {
                        assert_eq!(elements.len(), 2);
                        match &elements[0] {
                            ArrayParamElement::Typed(tp) => {
                                assert_eq!(tp.name, "first");
                                assert_eq!(tp.type_ann.name, "number");
                            }
                            other => panic!("expected Typed, got {other:?}"),
                        }
                        match &elements[1] {
                            ArrayParamElement::Rest(tp) => {
                                assert_eq!(tp.name, "remaining");
                                assert_eq!(tp.type_ann.name, "number");
                            }
                            other => panic!("expected Rest, got {other:?}"),
                        }
                    }
                    other => panic!("expected DestructuredArray, got {other:?}"),
                }
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_func_mixed_destructured_and_simple() {
        // (func f :args ((object :string name) :string action) :body x)
        let result = form(
            "func",
            vec![
                atom("f"),
                kw("args"),
                list(vec![
                    list(vec![atom("object"), kw("string"), atom("name")]),
                    kw("string"),
                    atom("action"),
                ]),
                kw("body"),
                atom("x"),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Func { clauses, .. } => {
                assert_eq!(clauses.len(), 1);
                assert_eq!(clauses[0].args.len(), 2);
                assert!(matches!(
                    &clauses[0].args[0],
                    ParamShape::DestructuredObject { fields, .. } if fields.len() == 1
                ));
                assert!(matches!(
                    &clauses[0].args[1],
                    ParamShape::Simple(tp) if tp.name == "action" && tp.type_ann.name == "string"
                ));
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_fn_with_object_destructuring() {
        // (fn ((object :string name)) x)
        let result = form(
            "fn",
            vec![
                list(vec![list(vec![atom("object"), kw("string"), atom("name")])]),
                atom("x"),
            ],
        )
        .unwrap();
        match result {
            SurfaceForm::Fn { params, body, .. } => {
                assert_eq!(params.len(), 1);
                match &params[0] {
                    ParamShape::DestructuredObject { fields, .. } => {
                        assert_eq!(fields.len(), 1);
                        assert_eq!(fields[0].name, "name");
                    }
                    other => panic!("expected DestructuredObject, got {other:?}"),
                }
                assert_eq!(body.len(), 1);
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_func_empty_object_destructure_error() {
        // (func f :args ((object)) :body x)
        let result = form(
            "func",
            vec![
                atom("f"),
                kw("args"),
                list(vec![list(vec![atom("object")])]),
                kw("body"),
                atom("x"),
            ],
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("empty destructuring pattern")
        );
    }

    #[test]
    fn test_classify_func_bare_name_in_object_destructure_error() {
        // (func f :args ((object name)) :body x)
        let result = form(
            "func",
            vec![
                atom("f"),
                kw("args"),
                list(vec![list(vec![atom("object"), atom("name")])]),
                kw("body"),
                atom("x"),
            ],
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("missing type annotation")
        );
    }

    #[test]
    fn test_classify_func_nested_destructure_error() {
        // (func f :args ((object :string (object :string inner))) :body x)
        let result = form(
            "func",
            vec![
                atom("f"),
                kw("args"),
                list(vec![list(vec![
                    atom("object"),
                    kw("string"),
                    list(vec![atom("object"), kw("string"), atom("inner")]),
                ])]),
                kw("body"),
                atom("x"),
            ],
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("nested destructuring"));
    }

    #[test]
    fn test_classify_func_default_in_destructure_error() {
        // (func f :args ((object (default :string name "anon"))) :body x)
        let result = form(
            "func",
            vec![
                atom("f"),
                kw("args"),
                list(vec![list(vec![
                    atom("object"),
                    list(vec![
                        atom("default"),
                        kw("string"),
                        atom("name"),
                        string("anon"),
                    ]),
                ])]),
                kw("body"),
                atom("x"),
            ],
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("default values"));
    }

    // ---------------------------------------------------------------
    // classify_surface_form -- unknown surface form
    // ---------------------------------------------------------------

    #[test]
    fn test_unknown_surface_form_error() {
        let result = classify_surface_form("definitely-unknown", &[], Span::default());
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("unknown surface form"));
    }

    // ---------------------------------------------------------------
    // Computed head edge cases
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_form_keyword_head_is_function_call() {
        let expr = list(vec![kw("dynamic"), atom("arg")]);
        let result = classify_form(&expr).unwrap();
        match result {
            SurfaceForm::FunctionCall { head, .. } => {
                assert!(matches!(head, SExpr::Keyword { .. }));
            }
            other => panic!("expected FunctionCall, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_form_number_head_is_function_call() {
        let expr = list(vec![num(42.0), atom("arg")]);
        let result = classify_form(&expr).unwrap();
        assert!(matches!(result, SurfaceForm::FunctionCall { .. }));
    }

    #[test]
    fn test_classify_form_computed_head_no_args() {
        let expr = list(vec![list(vec![atom("get-fn")])]);
        let result = classify_form(&expr).unwrap();
        match result {
            SurfaceForm::FunctionCall { args, .. } => {
                assert!(args.is_empty());
            }
            other => panic!("expected FunctionCall, got {other:?}"),
        }
    }

    // ---------------------------------------------------------------
    // Kernel form passthrough variants
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_form_various_kernel_forms() {
        for kf in &["let", "if", "while", "for", "class", "import", "+", "==="] {
            let expr = list(vec![atom(kf), atom("x")]);
            let result = classify_form(&expr).unwrap();
            assert!(
                matches!(result, SurfaceForm::KernelPassthrough { .. }),
                "expected KernelPassthrough for {kf}"
            );
        }
    }

    #[test]
    fn test_classify_form_bool_is_kernel_passthrough() {
        let result = classify_form(&boolean(true)).unwrap();
        assert!(matches!(result, SurfaceForm::KernelPassthrough { .. }));
    }

    #[test]
    fn test_classify_form_keyword_is_kernel_passthrough() {
        let result = classify_form(&kw("something")).unwrap();
        assert!(matches!(result, SurfaceForm::KernelPassthrough { .. }));
    }

    // ---------------------------------------------------------------
    // classify_eq (DD-22)
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_eq_binary() {
        let result = form("=", vec![atom("a"), atom("b")]).unwrap();
        match result {
            SurfaceForm::Eq { args, .. } => {
                assert_eq!(args.len(), 2);
                assert_eq!(args[0].as_atom(), Some("a"));
                assert_eq!(args[1].as_atom(), Some("b"));
            }
            other => panic!("expected Eq, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_eq_variadic() {
        let result = form("=", vec![atom("a"), atom("b"), atom("c")]).unwrap();
        match result {
            SurfaceForm::Eq { args, .. } => {
                assert_eq!(args.len(), 3);
            }
            other => panic!("expected Eq, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_eq_too_few_args() {
        let result = form("=", vec![atom("a")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("at least 2 arguments"));
    }

    // ---------------------------------------------------------------
    // classify_not_eq (DD-22)
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_not_eq_binary() {
        let result = form("!=", vec![atom("a"), atom("b")]).unwrap();
        match result {
            SurfaceForm::NotEq { left, right, .. } => {
                assert_eq!(left.as_atom(), Some("a"));
                assert_eq!(right.as_atom(), Some("b"));
            }
            other => panic!("expected NotEq, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_not_eq_wrong_arg_count() {
        let result = form("!=", vec![atom("a")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("exactly 2 arguments"));

        let result = form("!=", vec![atom("a"), atom("b"), atom("c")]);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // classify_and (DD-22)
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_and_binary() {
        let result = form("and", vec![atom("a"), atom("b")]).unwrap();
        match result {
            SurfaceForm::And { args, .. } => {
                assert_eq!(args.len(), 2);
                assert_eq!(args[0].as_atom(), Some("a"));
                assert_eq!(args[1].as_atom(), Some("b"));
            }
            other => panic!("expected And, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_and_variadic() {
        let result = form("and", vec![atom("a"), atom("b"), atom("c"), atom("d")]).unwrap();
        match result {
            SurfaceForm::And { args, .. } => {
                assert_eq!(args.len(), 4);
            }
            other => panic!("expected And, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_and_too_few_args() {
        let result = form("and", vec![atom("a")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("at least 2 arguments"));
    }

    // ---------------------------------------------------------------
    // classify_or (DD-22)
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_or_binary() {
        let result = form("or", vec![atom("a"), atom("b")]).unwrap();
        match result {
            SurfaceForm::Or { args, .. } => {
                assert_eq!(args.len(), 2);
                assert_eq!(args[0].as_atom(), Some("a"));
                assert_eq!(args[1].as_atom(), Some("b"));
            }
            other => panic!("expected Or, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_or_variadic() {
        let result = form("or", vec![atom("a"), atom("b"), atom("c"), atom("d")]).unwrap();
        match result {
            SurfaceForm::Or { args, .. } => {
                assert_eq!(args.len(), 4);
            }
            other => panic!("expected Or, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_or_too_few_args() {
        let result = form("or", vec![atom("a")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("at least 2 arguments"));
    }

    // ---------------------------------------------------------------
    // classify_not (DD-22)
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_not_unary() {
        let result = form("not", vec![atom("x")]).unwrap();
        match result {
            SurfaceForm::Not { operand, .. } => {
                assert_eq!(operand.as_atom(), Some("x"));
            }
            other => panic!("expected Not, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_not_wrong_arg_count() {
        let result = form("not", vec![atom("a"), atom("b")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("exactly 1 argument"));

        let result = form("not", vec![]);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // DD-22: =, != are surface forms, not kernel passthroughs
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_eq_is_surface_form_not_kernel() {
        // `=` should be classified as a surface Eq form, NOT a kernel passthrough
        let expr = list(vec![atom("="), atom("a"), atom("b")]);
        let result = classify_form(&expr).unwrap();
        assert!(
            matches!(result, SurfaceForm::Eq { .. }),
            "expected Eq surface form, got {result:?}"
        );
    }

    #[test]
    fn test_classify_ne_is_surface_form_not_kernel() {
        // `!=` should be classified as a surface NotEq form, NOT a kernel passthrough
        let expr = list(vec![atom("!="), atom("a"), atom("b")]);
        let result = classify_form(&expr).unwrap();
        assert!(
            matches!(result, SurfaceForm::NotEq { .. }),
            "expected NotEq surface form, got {result:?}"
        );
    }

    #[test]
    fn test_classify_and_or_not_are_surface_forms() {
        // and, or, not should be surface forms, NOT function calls
        let expr = list(vec![atom("and"), atom("a"), atom("b")]);
        let result = classify_form(&expr).unwrap();
        assert!(matches!(result, SurfaceForm::And { .. }));

        let expr = list(vec![atom("or"), atom("a"), atom("b")]);
        let result = classify_form(&expr).unwrap();
        assert!(matches!(result, SurfaceForm::Or { .. }));

        let expr = list(vec![atom("not"), atom("x")]);
        let result = classify_form(&expr).unwrap();
        assert!(matches!(result, SurfaceForm::Not { .. }));
    }
}
