//! Exhaustiveness and reachability checking for `match` expressions.
//!
//! Wires the Maranget algorithm to the surface `Match` form. Includes
//! type inference from patterns and enhanced diagnostics for blessed types.

use crate::ast::surface::{MatchClause, Pattern, SurfaceForm};
use crate::diagnostics::{Diagnostic, Severity};

use super::maranget;
use super::pattern::{DeconPattern, deconstruct_pattern};
use super::type_registry::TypeRegistry;

/// Check a `Match` form for exhaustiveness and unreachable clauses.
pub fn check_match(form: &SurfaceForm, registry: &TypeRegistry) -> Vec<Diagnostic> {
    let (_target, clauses, span) = match form {
        SurfaceForm::Match {
            target,
            clauses,
            span,
        } => (target, clauses, *span),
        _ => return Vec::new(),
    };

    let mut diagnostics = Vec::new();

    // Infer the matched type from patterns
    let _inferred_type = infer_match_type(clauses, registry, &mut diagnostics);
    if !diagnostics.is_empty() {
        return diagnostics;
    }

    // Deconstruct patterns
    let mut decon_patterns: Vec<(DeconPattern, bool)> = Vec::new();
    for clause in clauses {
        match deconstruct_pattern(&clause.pattern, registry) {
            Ok(dp) => decon_patterns.push((dp, clause.guard.is_some())),
            Err(diag) => diagnostics.push(diag),
        }
    }
    if !diagnostics.is_empty() {
        return diagnostics;
    }

    // Build matrix for exhaustiveness (exclude guarded clauses since
    // guards can fail at runtime)
    let exhaust_matrix: Vec<Vec<DeconPattern>> = decon_patterns
        .iter()
        .filter(|(_, guarded)| !guarded)
        .map(|(p, _)| vec![p.clone()])
        .collect();

    // Check exhaustiveness
    if !maranget::is_exhaustive(&exhaust_matrix, 1, registry) {
        let uncovered = maranget::find_uncovered(&exhaust_matrix, 1, registry);
        if uncovered.is_empty() {
            // Generic non-exhaustive message when we cannot determine
            // specific witnesses (e.g., literals only)
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: "non-exhaustive match: not all cases are covered".into(),
                span,
                suggestion: Some("add a wildcard `_` clause or cover all variants".into()),
            });
        } else {
            for witness in &uncovered {
                let msg = format_uncovered_message(witness, registry);
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: msg,
                    span,
                    suggestion: Some("add a wildcard `_` clause or cover all variants".into()),
                });
            }
        }
    }

    // Check usefulness (unreachable clauses)
    let full_matrix: Vec<Vec<DeconPattern>> = decon_patterns
        .iter()
        .map(|(p, _)| vec![p.clone()])
        .collect();
    for i in 0..full_matrix.len() {
        let prior = full_matrix[..i].to_vec();
        let row = &full_matrix[i];
        if !maranget::is_useful(&prior, row, registry) {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                message: "unreachable match clause".into(),
                span: clauses[i].span,
                suggestion: Some("remove this clause or reorder patterns".into()),
            });
        }
    }

    diagnostics
}

/// Infer which type the match is dispatching on by inspecting patterns.
///
/// Five inference rules:
/// 1. Constructor pattern → owning type
/// 2. All clauses are wildcards/bindings → no type constraint (return None)
/// 3. Mixed constructors from different types → error
/// 4. Literal-only → no type constraint
/// 5. Combination of constructors and wildcards → use constructor type
fn infer_match_type(
    clauses: &[MatchClause],
    registry: &TypeRegistry,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let mut inferred: Option<String> = None;

    for clause in clauses {
        if let Some(type_name) = extract_type_from_pattern(&clause.pattern, registry) {
            match &inferred {
                None => {
                    inferred = Some(type_name);
                }
                Some(existing) if existing != &type_name => {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "mixed constructor types in match: expected '{existing}', found '{type_name}'"
                        ),
                        span: clause.span,
                        suggestion: Some(
                            "all constructor patterns in a match must belong to the same type"
                                .into(),
                        ),
                    });
                    return None;
                }
                _ => {}
            }
        }
    }

    inferred
}

/// Extract the owning type name from a pattern, if it contains a constructor.
fn extract_type_from_pattern(pat: &Pattern, registry: &TypeRegistry) -> Option<String> {
    match pat {
        Pattern::Constructor { name, .. } => {
            registry.owning_type_of(name).map(|td| td.name.clone())
        }
        _ => None,
    }
}

/// Format a human-readable message for uncovered patterns, with enhanced
/// messages for blessed types like `Option` and `Result`.
fn format_uncovered_message(witness: &[DeconPattern], registry: &TypeRegistry) -> String {
    let parts: Vec<String> = witness
        .iter()
        .map(|p| format_pattern(p, registry))
        .collect();
    let pattern_str = parts.join(", ");

    // Check if any uncovered constructor belongs to a blessed type
    for p in witness {
        if let DeconPattern::Constructor { type_name, .. } = p
            && let Some(td) = registry.lookup_type(type_name)
            && td.is_blessed
        {
            return format!(
                "non-exhaustive match: missing '{pattern_str}' \
                         (the {type_name} type must be fully destructured)"
            );
        }
    }

    format!("non-exhaustive match: missing '{pattern_str}'")
}

/// Format a single deconstructed pattern for display.
#[expect(
    clippy::only_used_in_recursion,
    reason = "registry needed for recursive field formatting"
)]
fn format_pattern(pat: &DeconPattern, registry: &TypeRegistry) -> String {
    match pat {
        DeconPattern::Constructor {
            ctor_name, fields, ..
        } => {
            if fields.is_empty() {
                ctor_name.clone()
            } else {
                let inner: Vec<String> =
                    fields.iter().map(|f| format_pattern(f, registry)).collect();
                format!("({} {})", ctor_name, inner.join(" "))
            }
        }
        DeconPattern::Literal(lit) => format!("{lit:?}"),
        DeconPattern::Wildcard => "_".into(),
        DeconPattern::Structural { keys } => {
            let pairs: Vec<String> = keys
                .iter()
                .map(|(k, v)| format!("{k}: {}", format_pattern(v, registry)))
                .collect();
            format!("{{{}}}", pairs.join(", "))
        }
        DeconPattern::TypeKeyword(kw) => format!(":{kw}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::prelude::register_prelude_types;
    use crate::analysis::type_registry::{ConstructorDef, FieldDef, TypeDef, TypeRegistry};
    use crate::ast::sexpr::SExpr;
    use crate::ast::surface::{MatchClause, Pattern, SurfaceForm};
    use crate::reader::source_loc::Span;

    fn span() -> Span {
        Span::default()
    }

    fn registry_with_option() -> TypeRegistry {
        let mut r = TypeRegistry::default();
        register_prelude_types(&mut r);
        r
    }

    fn registry_with_color() -> TypeRegistry {
        let mut r = registry_with_option();
        r.register_type(TypeDef {
            name: "Color".into(),
            module_path: None,
            constructors: vec![
                ConstructorDef {
                    name: "Red".into(),
                    fields: vec![],
                    owning_type: "Color".into(),
                    span: span(),
                },
                ConstructorDef {
                    name: "Green".into(),
                    fields: vec![],
                    owning_type: "Color".into(),
                    span: span(),
                },
                ConstructorDef {
                    name: "Blue".into(),
                    fields: vec![],
                    owning_type: "Color".into(),
                    span: span(),
                },
            ],
            is_blessed: false,
            span: span(),
        })
        .unwrap();
        r
    }

    fn match_form(clauses: Vec<MatchClause>) -> SurfaceForm {
        SurfaceForm::Match {
            target: SExpr::Atom {
                value: "x".into(),
                span: span(),
            },
            clauses,
            span: span(),
        }
    }

    fn clause(pattern: Pattern, guard: Option<SExpr>) -> MatchClause {
        MatchClause {
            pattern,
            guard,
            body: vec![SExpr::Atom {
                value: "body".into(),
                span: span(),
            }],
            span: span(),
        }
    }

    #[test]
    fn test_exhaustive_option_match() {
        let reg = registry_with_option();
        let form = match_form(vec![
            clause(
                Pattern::Constructor {
                    name: "Some".into(),
                    name_span: span(),
                    bindings: vec![Pattern::Wildcard(span())],
                    span: span(),
                },
                None,
            ),
            clause(
                Pattern::Constructor {
                    name: "None".into(),
                    name_span: span(),
                    bindings: vec![],
                    span: span(),
                },
                None,
            ),
        ]);
        let diags = check_match(&form, &reg);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn test_non_exhaustive_option_missing_none() {
        let reg = registry_with_option();
        let form = match_form(vec![clause(
            Pattern::Constructor {
                name: "Some".into(),
                name_span: span(),
                bindings: vec![Pattern::Wildcard(span())],
                span: span(),
            },
            None,
        )]);
        let diags = check_match(&form, &reg);
        assert!(!diags.is_empty());
        assert!(diags.iter().any(|d| d.severity == Severity::Error));
        assert!(diags.iter().any(|d| d.message.contains("None")));
    }

    #[test]
    fn test_non_exhaustive_option_missing_some() {
        let reg = registry_with_option();
        let form = match_form(vec![clause(
            Pattern::Constructor {
                name: "None".into(),
                name_span: span(),
                bindings: vec![],
                span: span(),
            },
            None,
        )]);
        let diags = check_match(&form, &reg);
        assert!(diags.iter().any(|d| d.message.contains("Some")));
    }

    #[test]
    fn test_wildcard_makes_exhaustive() {
        let reg = registry_with_option();
        let form = match_form(vec![
            clause(
                Pattern::Constructor {
                    name: "Some".into(),
                    name_span: span(),
                    bindings: vec![Pattern::Wildcard(span())],
                    span: span(),
                },
                None,
            ),
            clause(Pattern::Wildcard(span()), None),
        ]);
        let diags = check_match(&form, &reg);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
    }

    #[test]
    fn test_unreachable_clause_after_wildcard() {
        let reg = registry_with_option();
        let form = match_form(vec![
            clause(Pattern::Wildcard(span()), None),
            clause(
                Pattern::Constructor {
                    name: "Some".into(),
                    name_span: span(),
                    bindings: vec![Pattern::Wildcard(span())],
                    span: span(),
                },
                None,
            ),
        ]);
        let diags = check_match(&form, &reg);
        assert!(diags.iter().any(|d| d.message.contains("unreachable")));
    }

    #[test]
    fn test_duplicate_clause_unreachable() {
        let reg = registry_with_option();
        let form = match_form(vec![
            clause(
                Pattern::Constructor {
                    name: "None".into(),
                    name_span: span(),
                    bindings: vec![],
                    span: span(),
                },
                None,
            ),
            clause(
                Pattern::Constructor {
                    name: "None".into(),
                    name_span: span(),
                    bindings: vec![],
                    span: span(),
                },
                None,
            ),
            clause(
                Pattern::Constructor {
                    name: "Some".into(),
                    name_span: span(),
                    bindings: vec![Pattern::Wildcard(span())],
                    span: span(),
                },
                None,
            ),
        ]);
        let diags = check_match(&form, &reg);
        let warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("unreachable"));
    }

    #[test]
    fn test_guarded_clause_excluded_from_exhaustiveness() {
        let reg = registry_with_option();
        // Some with guard, None without guard -- not exhaustive because
        // the guarded Some could fail
        let form = match_form(vec![
            clause(
                Pattern::Constructor {
                    name: "Some".into(),
                    name_span: span(),
                    bindings: vec![Pattern::Wildcard(span())],
                    span: span(),
                },
                Some(SExpr::Atom {
                    value: "guard".into(),
                    span: span(),
                }),
            ),
            clause(
                Pattern::Constructor {
                    name: "None".into(),
                    name_span: span(),
                    bindings: vec![],
                    span: span(),
                },
                None,
            ),
        ]);
        let diags = check_match(&form, &reg);
        assert!(
            diags
                .iter()
                .any(|d| d.severity == Severity::Error && d.message.contains("non-exhaustive"))
        );
    }

    #[test]
    fn test_color_exhaustive() {
        let reg = registry_with_color();
        let form = match_form(vec![
            clause(
                Pattern::Constructor {
                    name: "Red".into(),
                    name_span: span(),
                    bindings: vec![],
                    span: span(),
                },
                None,
            ),
            clause(
                Pattern::Constructor {
                    name: "Green".into(),
                    name_span: span(),
                    bindings: vec![],
                    span: span(),
                },
                None,
            ),
            clause(
                Pattern::Constructor {
                    name: "Blue".into(),
                    name_span: span(),
                    bindings: vec![],
                    span: span(),
                },
                None,
            ),
        ]);
        let diags = check_match(&form, &reg);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn test_mixed_type_constructors_error() {
        let reg = registry_with_color();
        let form = match_form(vec![
            clause(
                Pattern::Constructor {
                    name: "Some".into(),
                    name_span: span(),
                    bindings: vec![Pattern::Wildcard(span())],
                    span: span(),
                },
                None,
            ),
            clause(
                Pattern::Constructor {
                    name: "Red".into(),
                    name_span: span(),
                    bindings: vec![],
                    span: span(),
                },
                None,
            ),
        ]);
        let diags = check_match(&form, &reg);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("mixed constructor types"))
        );
    }

    #[test]
    fn test_blessed_type_enhanced_message() {
        let reg = registry_with_option();
        let form = match_form(vec![clause(
            Pattern::Constructor {
                name: "Some".into(),
                name_span: span(),
                bindings: vec![Pattern::Wildcard(span())],
                span: span(),
            },
            None,
        )]);
        let diags = check_match(&form, &reg);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("must be fully destructured"))
        );
    }

    #[test]
    fn test_non_match_form_returns_empty() {
        let reg = registry_with_option();
        let form = SurfaceForm::KernelPassthrough {
            raw: SExpr::Atom {
                value: "x".into(),
                span: span(),
            },
            span: span(),
        };
        assert!(check_match(&form, &reg).is_empty());
    }

    #[test]
    fn test_literal_only_match_not_exhaustive() {
        let reg = registry_with_option();
        let form = match_form(vec![
            clause(
                Pattern::Literal(SExpr::Number {
                    value: 1.0,
                    span: span(),
                }),
                None,
            ),
            clause(
                Pattern::Literal(SExpr::Number {
                    value: 2.0,
                    span: span(),
                }),
                None,
            ),
        ]);
        let diags = check_match(&form, &reg);
        assert!(
            diags
                .iter()
                .any(|d| d.severity == Severity::Error && d.message.contains("non-exhaustive"))
        );
    }

    #[test]
    fn test_literal_with_wildcard_exhaustive() {
        let reg = registry_with_option();
        let form = match_form(vec![
            clause(
                Pattern::Literal(SExpr::Number {
                    value: 1.0,
                    span: span(),
                }),
                None,
            ),
            clause(Pattern::Wildcard(span()), None),
        ]);
        let diags = check_match(&form, &reg);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
    }

    #[test]
    fn test_unknown_constructor_error() {
        let reg = registry_with_option();
        let form = match_form(vec![clause(
            Pattern::Constructor {
                name: "Bogus".into(),
                name_span: span(),
                bindings: vec![],
                span: span(),
            },
            None,
        )]);
        let diags = check_match(&form, &reg);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unknown constructor"))
        );
    }
}
