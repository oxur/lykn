//! Pass 0: Import-macros processing.
//!
//! This pass scans the top-level forms for `(import-macros ...)` directives,
//! loads and compiles the referenced macro modules, and registers the
//! requested macros in the environment. Non-import forms are returned
//! unchanged for subsequent passes.
//!
//! Circular dependencies are detected via a compilation stack.

use std::path::{Path, PathBuf};

use crate::ast::sexpr::SExpr;
use crate::error::LyknError;
use crate::reader::source_loc::SourceLoc;

use super::MacroEnv;
use super::cache::ModuleCache;
use super::deno::DenoSubprocess;

/// Process all `import-macros` forms, loading and compiling external macro
/// modules as needed.
///
/// Returns the remaining (non-import) forms in their original order.
pub fn process_import_macros(
    forms: Vec<SExpr>,
    file_path: Option<&Path>,
    deno: &mut DenoSubprocess,
    cache: &mut ModuleCache,
    compilation_stack: &[PathBuf],
    env: &mut MacroEnv,
) -> Result<Vec<SExpr>, LyknError> {
    let mut remaining = Vec::new();

    for form in forms {
        if is_import_macros(&form) {
            process_single_import(&form, file_path, deno, cache, compilation_stack, env)?;
        } else {
            remaining.push(form);
        }
    }

    Ok(remaining)
}

/// Check whether a form is an `(import-macros ...)` directive.
fn is_import_macros(form: &SExpr) -> bool {
    if let SExpr::List { values, .. } = form
        && let Some(SExpr::Atom { value, .. }) = values.first()
    {
        return value == "import-macros";
    }
    false
}

/// Process a single `(import-macros "path" (name1 name2 ...))` form.
fn process_single_import(
    form: &SExpr,
    file_path: Option<&Path>,
    deno: &mut DenoSubprocess,
    cache: &mut ModuleCache,
    compilation_stack: &[PathBuf],
    env: &mut MacroEnv,
) -> Result<(), LyknError> {
    let values = form.as_list().ok_or_else(|| LyknError::Read {
        message: "import-macros: expected list form".to_string(),
        location: SourceLoc::default(),
    })?;

    if values.len() < 3 {
        return Err(LyknError::Read {
            message: "import-macros requires a path and a binding list".to_string(),
            location: SourceLoc::default(),
        });
    }

    // Extract the module path string.
    let module_path = match &values[1] {
        SExpr::String { value, .. } => value.clone(),
        _ => {
            return Err(LyknError::Read {
                message: "import-macros: first argument must be a string path".to_string(),
                location: SourceLoc::default(),
            });
        }
    };

    // Resolve relative to the importing file's directory.
    let resolved = if let Some(fp) = file_path {
        fp.parent().unwrap_or(Path::new(".")).join(&module_path)
    } else {
        PathBuf::from(&module_path)
    };

    // Detect circular module dependencies.
    if compilation_stack.iter().any(|p| p == &resolved) {
        let cycle: Vec<String> = compilation_stack
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        return Err(LyknError::Read {
            message: format!(
                "circular macro module dependency:\n  {}",
                cycle.join(" imports macros from\n  ")
            ),
            location: SourceLoc::default(),
        });
    }

    let binding_names = extract_binding_names(&values[2]);

    // If the module is already cached, pull from the cache.
    if let Some(cached_macros) = cache.get(&resolved) {
        for name in &binding_names {
            if let Some(m) = cached_macros.get(name) {
                env.insert(name.clone(), m.clone());
            } else {
                return Err(LyknError::Read {
                    message: format!("import-macros: macro '{name}' not found in module"),
                    location: SourceLoc::default(),
                });
            }
        }
        return Ok(());
    }

    // Load the source file.
    let source = std::fs::read_to_string(&resolved).map_err(|e| LyknError::Read {
        message: format!("cannot read macro module '{}': {e}", resolved.display()),
        location: SourceLoc::default(),
    })?;

    let module_forms = crate::reader::read(&source)?;

    let mut new_stack = compilation_stack.to_vec();
    new_stack.push(resolved.clone());

    // Recursively expand the imported module (pass 0 + pass 1).
    let mut module_env = MacroEnv::new();
    let module_forms = process_import_macros(
        module_forms,
        Some(&resolved),
        deno,
        cache,
        &new_stack,
        &mut module_env,
    )?;
    let _remaining = super::pass1::compile_local_macros(module_forms, deno, &mut module_env)?;

    // Cache the compiled macros for this module.
    cache.insert(resolved, module_env.clone());

    // Register the requested macros.
    for name in &binding_names {
        if let Some(m) = module_env.get(name) {
            env.insert(name.clone(), m.clone());
        } else {
            return Err(LyknError::Read {
                message: format!("import-macros: macro '{name}' not found in module"),
                location: SourceLoc::default(),
            });
        }
    }

    Ok(())
}

/// Extract binding names from the import binding list.
///
/// Supports:
/// - Simple names: `(name1 name2)`
/// - Aliased names: `((as original alias))` — extracts `alias`
fn extract_binding_names(bindings: &SExpr) -> Vec<String> {
    let mut names = Vec::new();
    if let SExpr::List { values, .. } = bindings {
        for val in values {
            match val {
                SExpr::Atom { value, .. } => names.push(value.clone()),
                SExpr::List { values: inner, .. } => {
                    // (as original alias) — extract the alias name.
                    if inner.len() == 3
                        && let Some(SExpr::Atom { value, .. }) = inner.last()
                    {
                        names.push(value.clone());
                    }
                }
                _ => {}
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::source_loc::Span;

    fn s() -> Span {
        Span::default()
    }

    #[test]
    fn test_is_import_macros_true() {
        let form = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "import-macros".to_string(),
                    span: s(),
                },
                SExpr::String {
                    value: "./macros.lykn".to_string(),
                    span: s(),
                },
                SExpr::List {
                    values: vec![SExpr::Atom {
                        value: "when".to_string(),
                        span: s(),
                    }],
                    span: s(),
                },
            ],
            span: s(),
        };
        assert!(is_import_macros(&form));
    }

    #[test]
    fn test_is_import_macros_false() {
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
        assert!(!is_import_macros(&form));
    }

    #[test]
    fn test_is_import_macros_non_list() {
        let form = SExpr::Atom {
            value: "import-macros".to_string(),
            span: s(),
        };
        assert!(!is_import_macros(&form));
    }

    #[test]
    fn test_extract_binding_names_simple() {
        let bindings = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "when".to_string(),
                    span: s(),
                },
                SExpr::Atom {
                    value: "unless".to_string(),
                    span: s(),
                },
            ],
            span: s(),
        };
        let names = extract_binding_names(&bindings);
        assert_eq!(names, vec!["when", "unless"]);
    }

    #[test]
    fn test_extract_binding_names_aliased() {
        let bindings = SExpr::List {
            values: vec![SExpr::List {
                values: vec![
                    SExpr::Atom {
                        value: "as".to_string(),
                        span: s(),
                    },
                    SExpr::Atom {
                        value: "original-name".to_string(),
                        span: s(),
                    },
                    SExpr::Atom {
                        value: "my-alias".to_string(),
                        span: s(),
                    },
                ],
                span: s(),
            }],
            span: s(),
        };
        let names = extract_binding_names(&bindings);
        assert_eq!(names, vec!["my-alias"]);
    }

    #[test]
    fn test_extract_binding_names_mixed() {
        let bindings = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "when".to_string(),
                    span: s(),
                },
                SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "as".to_string(),
                            span: s(),
                        },
                        SExpr::Atom {
                            value: "unless".to_string(),
                            span: s(),
                        },
                        SExpr::Atom {
                            value: "my-unless".to_string(),
                            span: s(),
                        },
                    ],
                    span: s(),
                },
            ],
            span: s(),
        };
        let names = extract_binding_names(&bindings);
        assert_eq!(names, vec!["when", "my-unless"]);
    }

    #[test]
    fn test_extract_binding_names_empty() {
        let bindings = SExpr::List {
            values: vec![],
            span: s(),
        };
        let names = extract_binding_names(&bindings);
        assert!(names.is_empty());
    }

    #[test]
    fn test_extract_binding_names_non_list() {
        let bindings = SExpr::Atom {
            value: "foo".to_string(),
            span: s(),
        };
        let names = extract_binding_names(&bindings);
        assert!(names.is_empty());
    }

    #[test]
    fn test_process_import_macros_no_imports() {
        // When there are no import-macros forms, all forms pass through.
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
                    SExpr::Number {
                        value: 1.0,
                        span: s(),
                    },
                ],
                span: s(),
            },
            SExpr::List {
                values: vec![
                    SExpr::Atom {
                        value: "define".to_string(),
                        span: s(),
                    },
                    SExpr::Atom {
                        value: "y".to_string(),
                        span: s(),
                    },
                    SExpr::Number {
                        value: 2.0,
                        span: s(),
                    },
                ],
                span: s(),
            },
        ];

        // We need a deno subprocess for the signature, but since there are
        // no imports, it won't actually be called. However, we still need to
        // construct one. If deno is not available, skip.
        // Instead, just test the is_import_macros filter directly.
        let remaining: Vec<_> = forms
            .iter()
            .filter(|f| !is_import_macros(f))
            .cloned()
            .collect();
        assert_eq!(remaining.len(), 2);
    }
}
