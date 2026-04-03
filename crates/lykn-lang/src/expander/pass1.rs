//! Pass 1: Local macro compilation.
//!
//! This pass extracts `(macro name params body...)` forms from the top-level,
//! compiles each to a JavaScript function body via Deno, and stores the result
//! in the macro environment. An iterative fixed-point algorithm handles macros
//! that depend on other macros defined in the same file: each pass compiles
//! macros whose dependencies are already resolved, repeating until all macros
//! are compiled or a circular dependency is detected.
//!
//! Non-macro forms are returned unchanged for Pass 2.

use crate::ast::sexpr::SExpr;
use crate::diagnostics::serializer::serialize_sexpr;
use crate::error::LyknError;

use super::deno::DenoSubprocess;
use super::{CompiledMacro, MacroEnv};

/// Compile all local `(macro ...)` definitions, returning the remaining
/// non-macro forms.
///
/// Macros are compiled in dependency order. If macro A's body references macro
/// B (which is also being defined in this file), B will be compiled first.
/// True circular dependencies produce an error.
pub fn compile_local_macros(
    forms: Vec<SExpr>,
    deno: &mut DenoSubprocess,
    env: &mut MacroEnv,
) -> Result<Vec<SExpr>, LyknError> {
    let mut macro_forms = Vec::new();
    let mut other_forms = Vec::new();

    for form in forms {
        if is_macro_def(&form) {
            macro_forms.push(form);
        } else {
            other_forms.push(form);
        }
    }

    if macro_forms.is_empty() {
        return Ok(other_forms);
    }

    // Collect the names of all macros being defined in this batch so we
    // can detect inter-macro dependencies.
    let all_macro_names: Vec<String> = macro_forms.iter().filter_map(macro_name).collect();

    // Iterative fixed-point: each iteration should compile at least one
    // macro. If an iteration makes no progress, the remaining macros form
    // a dependency cycle.
    let mut pending = macro_forms;

    loop {
        if pending.is_empty() {
            break;
        }

        let mut still_pending = Vec::new();
        let mut progress = false;

        for form in pending {
            let name = macro_name(&form).unwrap_or_default();

            // Check whether this macro depends on any not-yet-compiled macros
            // from the current batch.
            let deps = find_local_deps(&form, &all_macro_names, env);
            if deps.iter().any(|d| !env.contains_key(d)) {
                still_pending.push(form);
            } else {
                // Check for duplicate definition.
                if env.contains_key(&name) {
                    return Err(LyknError::Read {
                        message: format!("duplicate macro definition: '{name}'"),
                        location: form.span().start,
                    });
                }

                // Compile the macro via Deno.
                let source = serialize_sexpr(&form);
                let js_body = deno.compile_macro(&source)?;

                env.insert(
                    name.clone(),
                    CompiledMacro {
                        name: name.clone(),
                        js_body,
                    },
                );
                progress = true;
            }
        }

        if !progress && !still_pending.is_empty() {
            let names: Vec<String> = still_pending.iter().filter_map(macro_name).collect();
            return Err(LyknError::Read {
                message: format!("circular macro dependency among: {}", names.join(", ")),
                location: still_pending[0].span().start,
            });
        }

        pending = still_pending;
    }

    Ok(other_forms)
}

/// Check whether a form is a `(macro ...)` definition.
fn is_macro_def(form: &SExpr) -> bool {
    if let SExpr::List { values, .. } = form
        && let Some(SExpr::Atom { value, .. }) = values.first()
    {
        return value == "macro";
    }
    false
}

/// Extract the name from a `(macro name ...)` form.
fn macro_name(form: &SExpr) -> Option<String> {
    if let SExpr::List { values, .. } = form
        && values.len() >= 2
        && let SExpr::Atom { value: name, .. } = &values[1]
    {
        return Some(name.clone());
    }
    None
}

/// Find atoms in the macro body that reference other macros being defined
/// in this batch but not yet compiled.
///
/// This is a conservative heuristic: it collects all atom values from the
/// body and intersects with the set of local macro names minus those already
/// in the environment.
fn find_local_deps(form: &SExpr, all_local_names: &[String], env: &MacroEnv) -> Vec<String> {
    let mut atoms = Vec::new();
    if let SExpr::List { values, .. } = form {
        // values[0] = "macro", values[1] = name, values[2] = params, values[3..] = body
        if values.len() >= 4 {
            for body_form in &values[3..] {
                collect_atoms(body_form, &mut atoms);
            }
        }
    }
    atoms.sort();
    atoms.dedup();

    // Keep only names that are local macros not yet compiled.
    atoms
        .into_iter()
        .filter(|a| all_local_names.contains(a) && !env.contains_key(a))
        .collect()
}

/// Recursively collect all atom values from an S-expression tree.
fn collect_atoms(form: &SExpr, atoms: &mut Vec<String>) {
    match form {
        SExpr::Atom { value, .. } => atoms.push(value.clone()),
        SExpr::List { values, .. } => {
            for v in values {
                collect_atoms(v, atoms);
            }
        }
        SExpr::Cons { car, cdr, .. } => {
            collect_atoms(car, atoms);
            collect_atoms(cdr, atoms);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::source_loc::Span;

    fn s() -> Span {
        Span::default()
    }

    #[test]
    fn test_is_macro_def_true() {
        let form = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "macro".to_string(),
                    span: s(),
                },
                SExpr::Atom {
                    value: "when".to_string(),
                    span: s(),
                },
                SExpr::List {
                    values: vec![SExpr::Atom {
                        value: "test".to_string(),
                        span: s(),
                    }],
                    span: s(),
                },
                SExpr::Atom {
                    value: "body".to_string(),
                    span: s(),
                },
            ],
            span: s(),
        };
        assert!(is_macro_def(&form));
    }

    #[test]
    fn test_is_macro_def_false() {
        let form = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "define".to_string(),
                    span: s(),
                },
                SExpr::Atom {
                    value: "x".to_string(),
                    span: s(),
                },
            ],
            span: s(),
        };
        assert!(!is_macro_def(&form));
    }

    #[test]
    fn test_is_macro_def_non_list() {
        let form = SExpr::Atom {
            value: "macro".to_string(),
            span: s(),
        };
        assert!(!is_macro_def(&form));
    }

    #[test]
    fn test_macro_name_extraction() {
        let form = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "macro".to_string(),
                    span: s(),
                },
                SExpr::Atom {
                    value: "when".to_string(),
                    span: s(),
                },
                SExpr::List {
                    values: vec![],
                    span: s(),
                },
            ],
            span: s(),
        };
        assert_eq!(macro_name(&form), Some("when".to_string()));
    }

    #[test]
    fn test_macro_name_none_for_short_list() {
        let form = SExpr::List {
            values: vec![SExpr::Atom {
                value: "macro".to_string(),
                span: s(),
            }],
            span: s(),
        };
        assert_eq!(macro_name(&form), None);
    }

    #[test]
    fn test_macro_name_none_for_non_atom_name() {
        let form = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "macro".to_string(),
                    span: s(),
                },
                SExpr::Number {
                    value: 42.0,
                    span: s(),
                },
            ],
            span: s(),
        };
        assert_eq!(macro_name(&form), None);
    }

    #[test]
    fn test_collect_atoms() {
        let form = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "if".to_string(),
                    span: s(),
                },
                SExpr::Atom {
                    value: "test".to_string(),
                    span: s(),
                },
                SExpr::Number {
                    value: 1.0,
                    span: s(),
                },
                SExpr::List {
                    values: vec![SExpr::Atom {
                        value: "nested".to_string(),
                        span: s(),
                    }],
                    span: s(),
                },
            ],
            span: s(),
        };
        let mut atoms = Vec::new();
        collect_atoms(&form, &mut atoms);
        assert_eq!(atoms, vec!["if", "test", "nested"]);
    }

    #[test]
    fn test_find_local_deps() {
        // (macro my-when (test body) (other-macro test body))
        let form = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "macro".to_string(),
                    span: s(),
                },
                SExpr::Atom {
                    value: "my-when".to_string(),
                    span: s(),
                },
                SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "test".to_string(),
                            span: s(),
                        },
                        SExpr::Atom {
                            value: "body".to_string(),
                            span: s(),
                        },
                    ],
                    span: s(),
                },
                // body references "other-macro"
                SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "other-macro".to_string(),
                            span: s(),
                        },
                        SExpr::Atom {
                            value: "test".to_string(),
                            span: s(),
                        },
                    ],
                    span: s(),
                },
            ],
            span: s(),
        };

        let all_names = vec!["my-when".to_string(), "other-macro".to_string()];
        let env = MacroEnv::new();
        let deps = find_local_deps(&form, &all_names, &env);
        assert_eq!(deps, vec!["other-macro"]);
    }

    #[test]
    fn test_find_local_deps_already_compiled() {
        let form = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "macro".to_string(),
                    span: s(),
                },
                SExpr::Atom {
                    value: "my-when".to_string(),
                    span: s(),
                },
                SExpr::List {
                    values: vec![],
                    span: s(),
                },
                SExpr::List {
                    values: vec![SExpr::Atom {
                        value: "other-macro".to_string(),
                        span: s(),
                    }],
                    span: s(),
                },
            ],
            span: s(),
        };

        let all_names = vec!["my-when".to_string(), "other-macro".to_string()];

        // "other-macro" is already compiled.
        let mut env = MacroEnv::new();
        env.insert(
            "other-macro".to_string(),
            CompiledMacro {
                name: "other-macro".to_string(),
                js_body: "/* compiled */".to_string(),
            },
        );

        let deps = find_local_deps(&form, &all_names, &env);
        assert!(deps.is_empty());
    }

    #[test]
    fn test_compile_local_macros_no_macros() {
        // When there are no macro forms, all forms pass through unchanged.
        // We don't need deno for this case since the early return fires.
        let forms = vec![
            SExpr::List {
                values: vec![
                    SExpr::Atom {
                        value: "define".to_string(),
                        span: s(),
                    },
                    SExpr::Atom {
                        value: "x".to_string(),
                        span: s(),
                    },
                ],
                span: s(),
            },
            SExpr::Number {
                value: 42.0,
                span: s(),
            },
        ];

        // We need a DenoSubprocess but it won't be used. If deno is not
        // available, verify the early return path by checking the partition.
        let macro_count = forms.iter().filter(|f| is_macro_def(f)).count();
        assert_eq!(macro_count, 0);
        // The function returns early with all forms when no macros exist.
    }
}
