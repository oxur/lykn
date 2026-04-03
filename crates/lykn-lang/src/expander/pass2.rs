//! Pass 2: Macro expansion walk.
//!
//! This pass recursively walks the S-expression tree and:
//! 1. Leaves `(quote ...)` forms untouched.
//! 2. Desugars built-in sugar forms (`cons`, `car`, `cdr`, `list`, etc.).
//! 3. Expands user-defined macro invocations by calling the compiled JS
//!    function body via the Deno subprocess. Expansion uses a fixed-point
//!    loop so that macros returning other macro invocations are expanded
//!    transitively.
//! 4. Recursively expands sub-forms of non-macro lists.

use crate::ast::sexpr::SExpr;
use crate::error::LyknError;
use crate::reader::source_loc::Span;

use super::MacroEnv;
use super::deno::DenoSubprocess;

/// Maximum number of expansion iterations before declaring runaway expansion.
const MAX_EXPAND_ITERATIONS: usize = 1000;

/// Expand all macro invocations in a list of top-level forms.
pub fn expand_all(
    forms: Vec<SExpr>,
    deno: &mut DenoSubprocess,
    env: &MacroEnv,
) -> Result<Vec<SExpr>, LyknError> {
    let mut result = Vec::new();
    for form in forms {
        if let Some(expr) = expand_expr(form, deno, env)? {
            result.push(expr);
        }
    }
    Ok(result)
}

/// Expand a single S-expression, returning `None` if the form should be
/// elided (currently all forms produce output).
fn expand_expr(
    form: SExpr,
    deno: &mut DenoSubprocess,
    env: &MacroEnv,
) -> Result<Option<SExpr>, LyknError> {
    match &form {
        // Leaf nodes — no expansion needed.
        SExpr::Atom { .. }
        | SExpr::Number { .. }
        | SExpr::String { .. }
        | SExpr::Keyword { .. }
        | SExpr::Bool { .. }
        | SExpr::Null { .. } => Ok(Some(form)),

        // Cons pairs — expand both halves.
        SExpr::Cons { car, cdr, span } => {
            let expanded_car = expand_expr(*car.clone(), deno, env)?;
            let expanded_cdr = expand_expr(*cdr.clone(), deno, env)?;
            match (expanded_car, expanded_cdr) {
                (Some(c), Some(d)) => Ok(Some(SExpr::Cons {
                    car: Box::new(c),
                    cdr: Box::new(d),
                    span: *span,
                })),
                _ => Ok(None),
            }
        }

        // Empty list — pass through.
        SExpr::List { values, .. } if values.is_empty() => Ok(Some(form)),

        // Non-empty list — the interesting case.
        SExpr::List { values, span } => {
            let head = &values[0];

            if let SExpr::Atom {
                value: head_name, ..
            } = head
            {
                // `(quote ...)` suppresses all expansion.
                if head_name == "quote" {
                    return Ok(Some(form));
                }

                // A `(macro ...)` in pass 2 is an error — they should have
                // been consumed in pass 1.
                if head_name == "macro" {
                    return Err(LyknError::Read {
                        message: "unexpected macro definition in expansion pass \
                                  (macros should be processed in Pass 1)"
                            .to_string(),
                        location: span.start,
                    });
                }

                // Sugar form desugaring.
                if let Some(desugared) = try_desugar(head_name, &values[1..], *span) {
                    return expand_expr(desugared, deno, env);
                }

                // User-defined macro expansion (fixed-point).
                if env.contains_key(head_name.as_str()) {
                    let mut current = form.clone();
                    let mut count: usize = 0;

                    loop {
                        if let SExpr::List { values: ref cv, .. } = current
                            && let Some(SExpr::Atom { value: name, .. }) = cv.first()
                            && let Some(macro_def) = env.get(name.as_str())
                        {
                            let args = &cv[1..];
                            current = deno.eval_macro(&macro_def.js_body, args)?;
                            count += 1;
                            if count > MAX_EXPAND_ITERATIONS {
                                return Err(LyknError::Read {
                                    message: format!(
                                        "expansion limit \
                                                 ({MAX_EXPAND_ITERATIONS}) \
                                                 exceeded expanding \
                                                 '{head_name}'"
                                    ),
                                    location: span.start,
                                });
                            }
                            continue;
                        }
                        break;
                    }

                    // Recursively expand the result.
                    return expand_expr(current, deno, env);
                }
            }

            // Default: recursively expand all sub-forms.
            let mut expanded_values = Vec::new();
            for sub in values.iter() {
                if let Some(expanded) = expand_expr(sub.clone(), deno, env)? {
                    expanded_values.push(expanded);
                }
            }
            Ok(Some(SExpr::List {
                values: expanded_values,
                span: *span,
            }))
        }
    }
}

/// Attempt to desugar a known sugar form.
///
/// Returns `Some(desugared)` if `head` is a recognized sugar form with the
/// correct arity; `None` otherwise.
fn try_desugar(head: &str, args: &[SExpr], span: Span) -> Option<SExpr> {
    let s = Span::default();
    match head {
        "cons" if args.len() == 2 => Some(SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "array".to_string(),
                    span: s,
                },
                args[0].clone(),
                args[1].clone(),
            ],
            span,
        }),

        "car" if args.len() == 1 => Some(SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "get".to_string(),
                    span: s,
                },
                args[0].clone(),
                SExpr::Number {
                    value: 0.0,
                    span: s,
                },
            ],
            span,
        }),

        "cdr" if args.len() == 1 => Some(SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "get".to_string(),
                    span: s,
                },
                args[0].clone(),
                SExpr::Number {
                    value: 1.0,
                    span: s,
                },
            ],
            span,
        }),

        "cadr" if args.len() == 1 => Some(SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "get".to_string(),
                    span: s,
                },
                SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "get".to_string(),
                            span: s,
                        },
                        args[0].clone(),
                        SExpr::Number {
                            value: 1.0,
                            span: s,
                        },
                    ],
                    span: s,
                },
                SExpr::Number {
                    value: 0.0,
                    span: s,
                },
            ],
            span,
        }),

        "cddr" if args.len() == 1 => Some(SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "get".to_string(),
                    span: s,
                },
                SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "get".to_string(),
                            span: s,
                        },
                        args[0].clone(),
                        SExpr::Number {
                            value: 1.0,
                            span: s,
                        },
                    ],
                    span: s,
                },
                SExpr::Number {
                    value: 1.0,
                    span: s,
                },
            ],
            span,
        }),

        "list" if args.is_empty() => Some(SExpr::Atom {
            value: "null".to_string(),
            span,
        }),

        "list" => {
            // (list a b c) desugars to (array a (array b (array c null)))
            let mut result = SExpr::Atom {
                value: "null".to_string(),
                span: s,
            };
            for arg in args.iter().rev() {
                result = SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "array".to_string(),
                            span: s,
                        },
                        arg.clone(),
                        result,
                    ],
                    span: s,
                };
            }
            Some(result)
        }

        "as" if args.len() == 2 => {
            if args[0].is_atom() {
                Some(SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "alias".to_string(),
                            span: s,
                        },
                        args[0].clone(),
                        args[1].clone(),
                    ],
                    span,
                })
            } else {
                None
            }
        }

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::source_loc::Span;

    fn s() -> Span {
        Span::default()
    }

    fn atom(name: &str) -> SExpr {
        SExpr::Atom {
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

    fn list(vals: Vec<SExpr>) -> SExpr {
        SExpr::List {
            values: vals,
            span: s(),
        }
    }

    // ---------------------------------------------------------------
    // try_desugar
    // ---------------------------------------------------------------

    #[test]
    fn test_desugar_cons() {
        let result = try_desugar("cons", &[atom("a"), atom("b")], s()).unwrap();
        if let SExpr::List { values, .. } = result {
            assert_eq!(values[0].as_atom(), Some("array"));
            assert_eq!(values[1].as_atom(), Some("a"));
            assert_eq!(values[2].as_atom(), Some("b"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_desugar_cons_wrong_arity() {
        assert!(try_desugar("cons", &[atom("a")], s()).is_none());
        assert!(try_desugar("cons", &[atom("a"), atom("b"), atom("c")], s()).is_none());
    }

    #[test]
    fn test_desugar_car() {
        let result = try_desugar("car", &[atom("xs")], s()).unwrap();
        if let SExpr::List { values, .. } = result {
            assert_eq!(values[0].as_atom(), Some("get"));
            assert_eq!(values[1].as_atom(), Some("xs"));
            assert_eq!(values[2], num(0.0));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_desugar_cdr() {
        let result = try_desugar("cdr", &[atom("xs")], s()).unwrap();
        if let SExpr::List { values, .. } = result {
            assert_eq!(values[0].as_atom(), Some("get"));
            assert_eq!(values[1].as_atom(), Some("xs"));
            assert_eq!(values[2], num(1.0));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_desugar_cadr() {
        let result = try_desugar("cadr", &[atom("xs")], s()).unwrap();
        if let SExpr::List { values, .. } = result {
            assert_eq!(values[0].as_atom(), Some("get"));
            // Inner should be (get xs 1)
            if let SExpr::List { values: inner, .. } = &values[1] {
                assert_eq!(inner[0].as_atom(), Some("get"));
                assert_eq!(inner[1].as_atom(), Some("xs"));
                assert_eq!(inner[2], num(1.0));
            } else {
                panic!("expected inner list");
            }
            assert_eq!(values[2], num(0.0));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_desugar_cddr() {
        let result = try_desugar("cddr", &[atom("xs")], s()).unwrap();
        if let SExpr::List { values, .. } = result {
            assert_eq!(values[0].as_atom(), Some("get"));
            assert_eq!(values[2], num(1.0));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_desugar_list_empty() {
        let result = try_desugar("list", &[], s()).unwrap();
        assert_eq!(result.as_atom(), Some("null"));
    }

    #[test]
    fn test_desugar_list_single() {
        // (list a) -> (array a null)
        let result = try_desugar("list", &[atom("a")], s()).unwrap();
        if let SExpr::List { values, .. } = result {
            assert_eq!(values[0].as_atom(), Some("array"));
            assert_eq!(values[1].as_atom(), Some("a"));
            assert_eq!(values[2].as_atom(), Some("null"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_desugar_list_multiple() {
        // (list a b) -> (array a (array b null))
        let result = try_desugar("list", &[atom("a"), atom("b")], s()).unwrap();
        if let SExpr::List { values, .. } = result {
            assert_eq!(values[0].as_atom(), Some("array"));
            assert_eq!(values[1].as_atom(), Some("a"));
            if let SExpr::List { values: inner, .. } = &values[2] {
                assert_eq!(inner[0].as_atom(), Some("array"));
                assert_eq!(inner[1].as_atom(), Some("b"));
                assert_eq!(inner[2].as_atom(), Some("null"));
            } else {
                panic!("expected nested list");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_desugar_as() {
        let result = try_desugar("as", &[atom("foo"), atom("bar")], s()).unwrap();
        if let SExpr::List { values, .. } = result {
            assert_eq!(values[0].as_atom(), Some("alias"));
            assert_eq!(values[1].as_atom(), Some("foo"));
            assert_eq!(values[2].as_atom(), Some("bar"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_desugar_as_non_atom() {
        // (as (complex) bar) should not desugar.
        let complex = list(vec![atom("complex")]);
        assert!(try_desugar("as", &[complex, atom("bar")], s()).is_none());
    }

    #[test]
    fn test_desugar_unknown() {
        assert!(try_desugar("define", &[atom("x"), num(1.0)], s()).is_none());
    }

    // ---------------------------------------------------------------
    // expand_expr — leaf forms
    // ---------------------------------------------------------------

    #[test]
    fn test_expand_atom_passthrough() {
        // We can't construct a DenoSubprocess without deno, but for leaf
        // nodes the function never touches deno. We test indirectly via
        // the top-level `expand` which short-circuits when no macros are
        // present.
        let forms = vec![atom("x"), num(42.0)];
        let result = crate::expander::expand(forms, None).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].as_atom(), Some("x"));
    }

    // ---------------------------------------------------------------
    // expand via the top-level entry point (no macros)
    // ---------------------------------------------------------------

    #[test]
    fn test_expand_no_macros_passthrough() {
        let forms = vec![
            list(vec![atom("define"), atom("x"), num(1.0)]),
            list(vec![atom("console:log"), atom("x")]),
        ];
        let result = crate::expander::expand(forms.clone(), None).unwrap();
        assert_eq!(result, forms);
    }

    #[test]
    fn test_expand_empty_input() {
        let result = crate::expander::expand(vec![], None).unwrap();
        assert!(result.is_empty());
    }

    // ---------------------------------------------------------------
    // quote suppresses expansion
    // ---------------------------------------------------------------

    #[test]
    fn test_expand_quote_suppresses() {
        // (quote (macro foo () bar)) should pass through unchanged.
        let quoted = list(vec![
            atom("quote"),
            list(vec![atom("macro"), atom("foo"), list(vec![]), atom("bar")]),
        ]);
        // No macro/import-macros at top level, so expand returns as-is.
        let forms = vec![quoted.clone()];
        let result = crate::expander::expand(forms, None).unwrap();
        assert_eq!(result, vec![quoted]);
    }
}
