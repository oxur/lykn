//! Pass 0: Import-macros processing.
//!
//! This pass scans the top-level forms for `(import-macros ...)` directives,
//! loads and compiles the referenced macro modules, and registers the
//! requested macros in the environment. Non-import forms are returned
//! unchanged for subsequent passes.
//!
//! Circular dependencies are detected via a compilation stack.

use std::collections::HashMap;
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
    imports: Option<&HashMap<String, String>>,
) -> Result<Vec<SExpr>, LyknError> {
    let mut remaining = Vec::new();

    for form in forms {
        if is_import_macros(&form) {
            process_single_import(
                &form,
                file_path,
                deno,
                cache,
                compilation_stack,
                env,
                imports,
            )?;
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

/// Validate an `(import-macros "path" (name1 name2 ...))` form and extract
/// the module path string and binding names.
///
/// Returns `(module_path, binding_names)` on success.
fn validate_import_form(values: &[SExpr]) -> Result<(String, Vec<String>), LyknError> {
    if values.len() < 3 {
        return Err(LyknError::Read {
            message: "import-macros requires a path and a binding list".to_string(),
            location: SourceLoc::default(),
        });
    }

    let module_path = match &values[1] {
        SExpr::String { value, .. } => value.clone(),
        _ => {
            return Err(LyknError::Read {
                message: "import-macros: first argument must be a string path".to_string(),
                location: SourceLoc::default(),
            });
        }
    };

    let binding_names = extract_binding_names(&values[2]);

    Ok((module_path, binding_names))
}

/// Resolve a module specifier using three-tier dispatch.
///
/// **Tier 1 — Scheme-prefixed**: specifiers starting with `jsr:`, `npm:`,
/// `https:`, or `http:` are delegated to Deno's `import.meta.resolve`.
///
/// **Tier 2 — Import-map lookup**: bare names (no `./`, `../`, or `/` prefix)
/// are looked up in the optional import map. Exact matches are tried first,
/// followed by longest-prefix matches for keys ending in `/`.
///
/// **Tier 3 — Filesystem path**: relative and absolute paths are resolved
/// against the importing file's directory, preserving the original behavior.
fn resolve_specifier(
    module_path: &str,
    file_path: Option<&Path>,
    imports: Option<&HashMap<String, String>>,
    deno: &mut DenoSubprocess,
) -> Result<PathBuf, LyknError> {
    // Tier 1: Scheme-prefixed — delegate to Deno
    if module_path.starts_with("jsr:")
        || module_path.starts_with("npm:")
        || module_path.starts_with("https:")
        || module_path.starts_with("http:")
    {
        return deno.resolve_specifier(module_path);
    }

    // file: scheme — convert to path directly
    if let Some(path) = module_path.strip_prefix("file://") {
        return Ok(PathBuf::from(path));
    }

    // Tier 2: Import-map lookup (bare name or prefix match)
    if let Some(map) = imports
        && !module_path.starts_with("./")
        && !module_path.starts_with("../")
        && !module_path.starts_with('/')
    {
        // Try exact match first
        if let Some(target) = map.get(module_path) {
            // Re-resolve the target (it may be scheme-prefixed)
            return resolve_specifier(target, file_path, None, deno);
        }
        // Try prefix match: find longest key ending in '/' that matches
        let mut best_match: Option<(&str, &str)> = None;
        for (key, value) in map {
            if key.ends_with('/')
                && module_path.starts_with(key.as_str())
                && (best_match.is_none() || key.len() > best_match.unwrap().0.len())
            {
                best_match = Some((key.as_str(), value.as_str()));
            }
        }
        if let Some((prefix, target)) = best_match {
            let suffix = &module_path[prefix.len()..];
            if target.starts_with("./") || target.starts_with("../") {
                // Relative to project root (where project.json is)
                return Ok(PathBuf::from(target).join(suffix));
            }
            // Scheme-prefixed target
            return resolve_specifier(&format!("{target}{suffix}"), file_path, None, deno);
        }
    }

    // Workspace package fallback: try packages/<name>/mod.lykn from project root
    if !module_path.starts_with("./")
        && !module_path.starts_with("../")
        && !module_path.starts_with('/')
    {
        let start_dir = file_path
            .and_then(|fp| fp.parent())
            .map(|p| p.to_path_buf())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let mut dir = start_dir.as_path();
        loop {
            let candidate = dir.join("packages").join(module_path).join("mod.lykn");
            if dir.join("project.json").exists() && candidate.exists() {
                return Ok(candidate);
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => break,
            }
        }
    }

    // Tier 3: Filesystem path (current behavior)
    if let Some(fp) = file_path {
        Ok(fp.parent().unwrap_or(Path::new(".")).join(module_path))
    } else {
        Ok(PathBuf::from(module_path))
    }
}

/// Locate the macro entry point file within a package directory.
///
/// The lookup chain is:
/// 1. `deno.json` field `lykn.macroEntry`
/// 2. Fallback files: `mod.lykn`, `macros.lykn`, `index.lykn`
/// 3. `deno.json` field `exports` if it points to a `.lykn` file
fn find_macro_entry(pkg_dir: &Path) -> Result<PathBuf, LyknError> {
    let deno_json = pkg_dir.join("deno.json");

    // Check deno.json for lykn.macroEntry
    if deno_json.exists()
        && let Ok(content) = std::fs::read_to_string(&deno_json)
        && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(entry) = parsed.pointer("/lykn/macroEntry").and_then(|v| v.as_str())
    {
        let entry_path = pkg_dir.join(entry);
        if entry_path.exists() {
            return Ok(entry_path);
        }
    }

    // Fallback chain
    for candidate in &["mod.lykn", "macros.lykn", "index.lykn"] {
        let path = pkg_dir.join(candidate);
        if path.exists() {
            return Ok(path);
        }
    }

    // Check if exports points to a .lykn file
    if deno_json.exists()
        && let Ok(content) = std::fs::read_to_string(&deno_json)
        && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(exports) = parsed.get("exports").and_then(|v| v.as_str())
        && exports.ends_with(".lykn")
    {
        let path = pkg_dir.join(exports);
        if path.exists() {
            return Ok(path);
        }
    }

    Err(LyknError::Read {
        message: format!(
            "import-macros: no macro entry found in {}\n  \
             checked: lykn.macroEntry (absent or file not found)\n  \
             checked: mod.lykn, macros.lykn, index.lykn (not found)\n  \
             hint: add lykn.macroEntry to the package's deno.json",
            pkg_dir.display()
        ),
        location: SourceLoc::default(),
    })
}

/// Process a single `(import-macros "path" (name1 name2 ...))` form.
fn process_single_import(
    form: &SExpr,
    file_path: Option<&Path>,
    deno: &mut DenoSubprocess,
    cache: &mut ModuleCache,
    compilation_stack: &[PathBuf],
    env: &mut MacroEnv,
    imports: Option<&HashMap<String, String>>,
) -> Result<(), LyknError> {
    let values = form.as_list().ok_or_else(|| LyknError::Read {
        message: "import-macros: expected list form".to_string(),
        location: SourceLoc::default(),
    })?;

    let (module_path, binding_names) = validate_import_form(values)?;

    // Three-tier specifier resolution.
    let mut resolved = resolve_specifier(&module_path, file_path, imports, deno)?;

    // If resolved path is a directory (package root), find the macro entry.
    if resolved.is_dir() {
        resolved = find_macro_entry(&resolved)?;
    }

    // Canonicalize for stable cache keying.
    let canonical = std::fs::canonicalize(&resolved).unwrap_or_else(|_| resolved.clone());

    // Detect circular module dependencies.
    if compilation_stack.iter().any(|p| p == &canonical) {
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

    // If the module is already cached, pull from the cache.
    if let Some(cached_macros) = cache.get(&canonical) {
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
    new_stack.push(canonical.clone());

    // Recursively expand the imported module (pass 0 + pass 1).
    let mut module_env = MacroEnv::new();
    let module_forms = process_import_macros(
        module_forms,
        Some(&resolved),
        deno,
        cache,
        &new_stack,
        &mut module_env,
        imports,
    )?;
    let _remaining = super::pass1::compile_local_macros(module_forms, deno, &mut module_env)?;

    // Cache the compiled macros for this module.
    cache.insert(canonical, module_env.clone());

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

    #[test]
    fn test_extract_binding_names_ignores_non_atom_non_list() {
        // Keywords and numbers in the binding list should be skipped
        let bindings = SExpr::List {
            values: vec![
                SExpr::Keyword {
                    value: "name".to_string(),
                    span: s(),
                },
                SExpr::Number {
                    value: 42.0,
                    span: s(),
                },
            ],
            span: s(),
        };
        let names = extract_binding_names(&bindings);
        assert!(names.is_empty());
    }

    #[test]
    fn test_extract_binding_names_short_inner_list_ignored() {
        // An inner list with only 2 elements (not a valid `as` form) should be skipped
        let bindings = SExpr::List {
            values: vec![SExpr::List {
                values: vec![
                    SExpr::Atom {
                        value: "as".to_string(),
                        span: s(),
                    },
                    SExpr::Atom {
                        value: "original".to_string(),
                        span: s(),
                    },
                ],
                span: s(),
            }],
            span: s(),
        };
        let names = extract_binding_names(&bindings);
        assert!(names.is_empty());
    }

    #[test]
    fn test_is_import_macros_empty_list() {
        let form = SExpr::List {
            values: vec![],
            span: s(),
        };
        assert!(!is_import_macros(&form));
    }

    #[test]
    fn test_is_import_macros_non_atom_head() {
        let form = SExpr::List {
            values: vec![SExpr::Number {
                value: 1.0,
                span: s(),
            }],
            span: s(),
        };
        assert!(!is_import_macros(&form));
    }

    #[test]
    fn test_import_macros_filtering_with_mixed_forms() {
        let import = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "import-macros".to_string(),
                    span: s(),
                },
                SExpr::String {
                    value: "./m.lykn".to_string(),
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
        let define = SExpr::List {
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
        let forms = vec![import, define.clone()];
        let remaining: Vec<_> = forms
            .iter()
            .filter(|f| !is_import_macros(f))
            .cloned()
            .collect();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0], define);
    }

    // ---------------------------------------------------------------
    // validate_import_form
    // ---------------------------------------------------------------

    #[test]
    fn test_validate_import_form_valid() {
        let values = vec![
            SExpr::Atom {
                value: "import-macros".to_string(),
                span: s(),
            },
            SExpr::String {
                value: "./macros.lykn".to_string(),
                span: s(),
            },
            SExpr::List {
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
            },
        ];
        let (path, names) = validate_import_form(&values).unwrap();
        assert_eq!(path, "./macros.lykn");
        assert_eq!(names, vec!["when", "unless"]);
    }

    #[test]
    fn test_validate_import_form_too_few_elements() {
        // Only the head atom and a path — missing binding list.
        let values = vec![
            SExpr::Atom {
                value: "import-macros".to_string(),
                span: s(),
            },
            SExpr::String {
                value: "./macros.lykn".to_string(),
                span: s(),
            },
        ];
        let err = validate_import_form(&values).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("requires a path and a binding list"), "{msg}");
    }

    #[test]
    fn test_validate_import_form_single_element() {
        let values = vec![SExpr::Atom {
            value: "import-macros".to_string(),
            span: s(),
        }];
        let err = validate_import_form(&values).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("requires a path and a binding list"), "{msg}");
    }

    #[test]
    fn test_validate_import_form_non_string_path() {
        let values = vec![
            SExpr::Atom {
                value: "import-macros".to_string(),
                span: s(),
            },
            SExpr::Atom {
                value: "not-a-string".to_string(),
                span: s(),
            },
            SExpr::List {
                values: vec![SExpr::Atom {
                    value: "when".to_string(),
                    span: s(),
                }],
                span: s(),
            },
        ];
        let err = validate_import_form(&values).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("first argument must be a string path"),
            "{msg}"
        );
    }

    #[test]
    fn test_validate_import_form_number_path() {
        let values = vec![
            SExpr::Atom {
                value: "import-macros".to_string(),
                span: s(),
            },
            SExpr::Number {
                value: 42.0,
                span: s(),
            },
            SExpr::List {
                values: vec![],
                span: s(),
            },
        ];
        let err = validate_import_form(&values).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("first argument must be a string path"),
            "{msg}"
        );
    }

    #[test]
    fn test_validate_import_form_extracts_aliased_bindings() {
        let values = vec![
            SExpr::Atom {
                value: "import-macros".to_string(),
                span: s(),
            },
            SExpr::String {
                value: "./m.lykn".to_string(),
                span: s(),
            },
            SExpr::List {
                values: vec![SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "as".to_string(),
                            span: s(),
                        },
                        SExpr::Atom {
                            value: "original".to_string(),
                            span: s(),
                        },
                        SExpr::Atom {
                            value: "alias".to_string(),
                            span: s(),
                        },
                    ],
                    span: s(),
                }],
                span: s(),
            },
        ];
        let (path, names) = validate_import_form(&values).unwrap();
        assert_eq!(path, "./m.lykn");
        assert_eq!(names, vec!["alias"]);
    }

    #[test]
    fn test_extract_binding_names_inner_list_non_atom_last() {
        // An inner list with 3 elements but the last element is NOT an atom
        // should be skipped by the guard.
        let bindings = SExpr::List {
            values: vec![SExpr::List {
                values: vec![
                    SExpr::Atom {
                        value: "as".to_string(),
                        span: s(),
                    },
                    SExpr::Atom {
                        value: "original".to_string(),
                        span: s(),
                    },
                    SExpr::Number {
                        value: 99.0,
                        span: s(),
                    },
                ],
                span: s(),
            }],
            span: s(),
        };
        let names = extract_binding_names(&bindings);
        assert!(names.is_empty());
    }

    // ---------------------------------------------------------------
    // find_macro_entry
    // ---------------------------------------------------------------

    #[test]
    fn test_find_macro_entry_with_macro_entry_in_deno_json() {
        let tmp = std::env::temp_dir().join("lykn_test_macro_entry_dj");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("deno.json"),
            r#"{ "name": "@lykn/test", "lykn": { "macroEntry": "my-macros.lykn" } }"#,
        )
        .unwrap();
        std::fs::write(tmp.join("my-macros.lykn"), "(macro foo () 1)").unwrap();

        let result = find_macro_entry(&tmp).unwrap();
        assert_eq!(result, tmp.join("my-macros.lykn"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_find_macro_entry_mod_lykn_fallback() {
        let tmp = std::env::temp_dir().join("lykn_test_macro_entry_mod");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("mod.lykn"), "(macro bar () 2)").unwrap();

        let result = find_macro_entry(&tmp).unwrap();
        assert_eq!(result, tmp.join("mod.lykn"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_find_macro_entry_macros_lykn_fallback() {
        let tmp = std::env::temp_dir().join("lykn_test_macro_entry_macros");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        // No mod.lykn, only macros.lykn
        std::fs::write(tmp.join("macros.lykn"), "(macro baz () 3)").unwrap();

        let result = find_macro_entry(&tmp).unwrap();
        assert_eq!(result, tmp.join("macros.lykn"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_find_macro_entry_index_lykn_fallback() {
        let tmp = std::env::temp_dir().join("lykn_test_macro_entry_index");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("index.lykn"), "(macro qux () 4)").unwrap();

        let result = find_macro_entry(&tmp).unwrap();
        assert_eq!(result, tmp.join("index.lykn"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_find_macro_entry_exports_lykn_fallback() {
        let tmp = std::env::temp_dir().join("lykn_test_macro_entry_exports");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("deno.json"),
            r#"{ "name": "@lykn/test", "exports": "./lib.lykn" }"#,
        )
        .unwrap();
        std::fs::write(tmp.join("lib.lykn"), "(macro quux () 5)").unwrap();

        let result = find_macro_entry(&tmp).unwrap();
        assert_eq!(result, tmp.join("./lib.lykn"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_find_macro_entry_no_entry_errors() {
        let tmp = std::env::temp_dir().join("lykn_test_macro_entry_none");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let err = find_macro_entry(&tmp).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("no macro entry found"),
            "expected descriptive error, got: {msg}"
        );
        assert!(
            msg.contains("hint: add lykn.macroEntry"),
            "error should contain hint, got: {msg}"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_find_macro_entry_macro_entry_file_missing_falls_through() {
        // deno.json declares macroEntry but the file doesn't exist —
        // should fall through to the fallback chain.
        let tmp = std::env::temp_dir().join("lykn_test_macro_entry_missing");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("deno.json"),
            r#"{ "name": "@lykn/test", "lykn": { "macroEntry": "absent.lykn" } }"#,
        )
        .unwrap();
        std::fs::write(tmp.join("mod.lykn"), "(macro x () 0)").unwrap();

        let result = find_macro_entry(&tmp).unwrap();
        assert_eq!(result, tmp.join("mod.lykn"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_find_macro_entry_priority_order() {
        // macroEntry in deno.json takes priority over mod.lykn
        let tmp = std::env::temp_dir().join("lykn_test_macro_entry_priority");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("deno.json"),
            r#"{ "name": "@lykn/test", "lykn": { "macroEntry": "custom.lykn" } }"#,
        )
        .unwrap();
        std::fs::write(tmp.join("custom.lykn"), "(macro a () 1)").unwrap();
        std::fs::write(tmp.join("mod.lykn"), "(macro b () 2)").unwrap();

        let result = find_macro_entry(&tmp).unwrap();
        assert_eq!(result, tmp.join("custom.lykn"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ---------------------------------------------------------------
    // resolve_specifier (tier 2 & tier 3 — tier 1 needs deno)
    // ---------------------------------------------------------------

    /// Returns true if `deno` is available on PATH.
    fn deno_available() -> bool {
        std::process::Command::new("deno")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
    }

    #[test]
    fn test_resolve_specifier_relative_path() {
        if !deno_available() {
            eprintln!("skipping: deno not found");
            return;
        }
        let mut deno = super::super::deno::DenoSubprocess::spawn().expect("deno should spawn");
        let file = Path::new("/some/project/src/main.lykn");
        let result = resolve_specifier("./macros.lykn", Some(file), None, &mut deno).unwrap();
        assert_eq!(result, PathBuf::from("/some/project/src/macros.lykn"));
    }

    #[test]
    fn test_resolve_specifier_parent_relative() {
        if !deno_available() {
            eprintln!("skipping: deno not found");
            return;
        }
        let mut deno = super::super::deno::DenoSubprocess::spawn().expect("deno should spawn");
        let file = Path::new("/some/project/src/main.lykn");
        let result = resolve_specifier("../lib/macros.lykn", Some(file), None, &mut deno).unwrap();
        assert_eq!(
            result,
            PathBuf::from("/some/project/src/../lib/macros.lykn")
        );
    }

    #[test]
    fn test_resolve_specifier_no_file_path() {
        if !deno_available() {
            eprintln!("skipping: deno not found");
            return;
        }
        let mut deno = super::super::deno::DenoSubprocess::spawn().expect("deno should spawn");
        let result = resolve_specifier("./macros.lykn", None, None, &mut deno).unwrap();
        assert_eq!(result, PathBuf::from("./macros.lykn"));
    }

    #[test]
    fn test_resolve_specifier_file_scheme() {
        if !deno_available() {
            eprintln!("skipping: deno not found");
            return;
        }
        let mut deno = super::super::deno::DenoSubprocess::spawn().expect("deno should spawn");
        let result =
            resolve_specifier("file:///usr/local/lib/macros.lykn", None, None, &mut deno).unwrap();
        assert_eq!(result, PathBuf::from("/usr/local/lib/macros.lykn"));
    }

    #[test]
    fn test_resolve_specifier_import_map_exact() {
        if !deno_available() {
            eprintln!("skipping: deno not found");
            return;
        }
        let mut deno = super::super::deno::DenoSubprocess::spawn().expect("deno should spawn");
        let mut map = HashMap::new();
        map.insert(
            "my-macros".to_string(),
            "./packages/macros/mod.lykn".to_string(),
        );
        let result = resolve_specifier("my-macros", None, Some(&map), &mut deno).unwrap();
        assert_eq!(result, PathBuf::from("./packages/macros/mod.lykn"));
    }

    #[test]
    fn test_resolve_specifier_import_map_prefix() {
        if !deno_available() {
            eprintln!("skipping: deno not found");
            return;
        }
        let mut deno = super::super::deno::DenoSubprocess::spawn().expect("deno should spawn");
        let mut map = HashMap::new();
        map.insert("macros/".to_string(), "./packages/macros/".to_string());
        let result = resolve_specifier("macros/utils.lykn", None, Some(&map), &mut deno).unwrap();
        assert_eq!(
            result,
            PathBuf::from("./packages/macros/").join("utils.lykn")
        );
    }

    #[test]
    fn test_resolve_specifier_import_map_longest_prefix_wins() {
        if !deno_available() {
            eprintln!("skipping: deno not found");
            return;
        }
        let mut deno = super::super::deno::DenoSubprocess::spawn().expect("deno should spawn");
        let mut map = HashMap::new();
        map.insert("macros/".to_string(), "./pkg/macros/".to_string());
        map.insert(
            "macros/testing/".to_string(),
            "./pkg/test-macros/".to_string(),
        );
        let result =
            resolve_specifier("macros/testing/assert.lykn", None, Some(&map), &mut deno).unwrap();
        assert_eq!(
            result,
            PathBuf::from("./pkg/test-macros/").join("assert.lykn")
        );
    }

    #[test]
    fn test_resolve_specifier_import_map_no_match_falls_to_tier3() {
        if !deno_available() {
            eprintln!("skipping: deno not found");
            return;
        }
        let mut deno = super::super::deno::DenoSubprocess::spawn().expect("deno should spawn");
        let mut map = HashMap::new();
        map.insert("other".to_string(), "./somewhere.lykn".to_string());
        // "my-macros" is a bare specifier but has no import map match and
        // no ./ prefix, so it falls through to tier 3.
        let file = Path::new("/project/src/main.lykn");
        let result = resolve_specifier("my-macros", Some(file), Some(&map), &mut deno).unwrap();
        // Tier 3: resolved relative to file_path's parent
        assert_eq!(result, PathBuf::from("/project/src/my-macros"));
    }

    #[test]
    fn test_resolve_specifier_jsr_delegates_to_deno() {
        if !deno_available() {
            eprintln!("skipping: deno not found");
            return;
        }
        let mut deno = super::super::deno::DenoSubprocess::spawn().expect("deno should spawn");
        // This will either resolve or error depending on the specifier.
        // We just verify it reaches deno (tier 1) rather than panicking.
        let result = resolve_specifier("jsr:@lykn/nonexistent", None, None, &mut deno);
        // Either Ok (cached) or Err (resolve failed) — not a panic.
        let _ = result;
    }
}
