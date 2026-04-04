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
                let fields = parse_typed_params(&values[1..], *cspan)?;
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

fn parse_typed_params(values: &[SExpr], span: Span) -> Result<Vec<TypedParam>, Diagnostic> {
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
