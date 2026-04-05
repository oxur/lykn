pub mod func_check;
pub mod maranget;
pub mod match_check;
pub mod pattern;
pub mod prelude;
pub mod scope;
pub mod type_registry;

use crate::ast::surface::{Pattern, SurfaceForm};
use crate::diagnostics::{Diagnostic, Severity};
use scope::ScopeTracker;
use type_registry::TypeRegistry;

/// The result of running static analysis over a sequence of surface forms.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub diagnostics: Vec<Diagnostic>,
    pub type_registry: TypeRegistry,
    pub has_errors: bool,
}

// ---------------------------------------------------------------------------
// Analyze trait — per-form analysis dispatch (DD-20)
// ---------------------------------------------------------------------------

/// Trait for static analysis of surface forms.
///
/// Each phase of analysis is a separate method with a default no-op
/// implementation. Forms that participate in a phase override the
/// relevant method.
trait Analyze {
    /// Phase 1: Register types and constructors into the type registry
    /// and scope. Called once per form before any checks run.
    fn collect(
        &self,
        _registry: &mut TypeRegistry,
        _scope: &mut ScopeTracker,
    ) -> Vec<Diagnostic> {
        vec![]
    }

    /// Phase 2: Run semantic checks (exhaustiveness, overlap, etc.).
    fn check(&self, _registry: &TypeRegistry) -> Vec<Diagnostic> {
        vec![]
    }

    /// Phase 2: Track scope introductions and references.
    fn track_scope(&self, _scope: &mut ScopeTracker) {}
}

impl Analyze for SurfaceForm {
    fn collect(
        &self,
        registry: &mut TypeRegistry,
        scope: &mut ScopeTracker,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if let SurfaceForm::Type {
            name,
            name_span,
            constructors,
            span,
        } = self
        {
            let ctor_defs: Vec<type_registry::ConstructorDef> = constructors
                .iter()
                .map(|c| type_registry::ConstructorDef {
                    name: c.name.clone(),
                    fields: c
                        .fields
                        .iter()
                        .map(|f| type_registry::FieldDef {
                            name: f.name.clone(),
                            type_keyword: f.type_ann.name.clone(),
                        })
                        .collect(),
                    owning_type: name.clone(),
                    span: c.span,
                })
                .collect();

            let typedef = type_registry::TypeDef {
                name: name.clone(),
                module_path: None,
                constructors: ctor_defs,
                is_blessed: false,
                span: *span,
            };

            if let Err(diag) = registry.register_type(typedef) {
                diagnostics.push(diag);
            }

            for c in constructors {
                scope.introduce(&c.name, c.name_span, false, true);
            }
            scope.introduce(name, *name_span, false, false);
        }
        diagnostics
    }

    fn check(&self, registry: &TypeRegistry) -> Vec<Diagnostic> {
        match self {
            SurfaceForm::Match { .. } => match_check::check_match(self, registry),
            SurfaceForm::Func { clauses, .. } if clauses.len() > 1 => {
                func_check::check_func_overlap(self, registry)
            }
            _ => vec![],
        }
    }

    fn track_scope(&self, scope: &mut ScopeTracker) {
        match self {
            SurfaceForm::Bind { name, span, .. } => {
                if let Some(atom) = name.as_atom() {
                    scope.introduce(atom, *span, false, false);
                }
            }
            SurfaceForm::Func {
                name,
                name_span,
                clauses,
                ..
            } => {
                scope.introduce(name, *name_span, false, false);
                for clause in clauses {
                    scope.enter_scope();
                    for param in &clause.args {
                        scope.introduce(&param.name, param.name_span, false, false);
                    }
                    scope.exit_scope();
                }
            }
            SurfaceForm::Match { clauses, .. } => {
                for clause in clauses {
                    scope.enter_scope();
                    introduce_pattern_bindings(&clause.pattern, scope);
                    scope.exit_scope();
                }
            }
            SurfaceForm::Fn { params, .. } | SurfaceForm::Lambda { params, .. } => {
                scope.enter_scope();
                for param in params {
                    scope.introduce(&param.name, param.name_span, false, false);
                }
                scope.exit_scope();
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run all analysis passes over a slice of surface forms.
///
/// Phase 1 (collection): register prelude types and user-defined types.
/// Phase 2 (analysis): check match exhaustiveness, function clause overlap,
/// and scope usage.
pub fn analyze(forms: &[SurfaceForm]) -> AnalysisResult {
    let mut registry = TypeRegistry::default();
    let mut scope = ScopeTracker::new();
    let mut diagnostics = Vec::new();

    // Phase 1: Collection
    prelude::register_prelude_types(&mut registry);
    for form in forms {
        diagnostics.extend(form.collect(&mut registry, &mut scope));
    }

    // Phase 2: Checks + scope tracking
    for form in forms {
        diagnostics.extend(form.check(&registry));
        form.track_scope(&mut scope);
    }

    diagnostics.extend(scope.collect_diagnostics());

    let has_errors = diagnostics.iter().any(|d| d.severity == Severity::Error);
    AnalysisResult {
        diagnostics,
        type_registry: registry,
        has_errors,
    }
}

/// Introduce all binding names from a pattern into the current scope.
fn introduce_pattern_bindings(pat: &Pattern, scope: &mut ScopeTracker) {
    match pat {
        Pattern::Binding { name, span } => {
            scope.introduce(name, *span, false, false);
        }
        Pattern::Constructor { bindings, .. } => {
            for b in bindings {
                introduce_pattern_bindings(b, scope);
            }
        }
        Pattern::Obj { pairs, .. } => {
            for (_, p) in pairs {
                introduce_pattern_bindings(p, scope);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::sexpr::SExpr;
    use crate::ast::surface::{Constructor, MatchClause, Pattern, SurfaceForm};
    use crate::reader::source_loc::Span;

    fn span() -> Span {
        Span::default()
    }

    #[test]
    fn test_analyze_empty_forms() {
        let result = analyze(&[]);
        assert!(!result.has_errors);
        assert!(result.diagnostics.is_empty());
        // Prelude types should be registered
        assert!(result.type_registry.lookup_type("Option").is_some());
        assert!(result.type_registry.lookup_type("Result").is_some());
    }

    #[test]
    fn test_analyze_registers_user_type() {
        let forms = vec![SurfaceForm::Type {
            name: "Color".into(),
            name_span: span(),
            constructors: vec![
                Constructor {
                    name: "Red".into(),
                    name_span: span(),
                    fields: vec![],
                    span: span(),
                },
                Constructor {
                    name: "Green".into(),
                    name_span: span(),
                    fields: vec![],
                    span: span(),
                },
                Constructor {
                    name: "Blue".into(),
                    name_span: span(),
                    fields: vec![],
                    span: span(),
                },
            ],
            span: span(),
        }];
        let result = analyze(&forms);
        assert!(!result.has_errors);
        assert!(result.type_registry.lookup_type("Color").is_some());
        assert!(result.type_registry.lookup_constructor("Red").is_some());
    }

    #[test]
    fn test_analyze_detects_non_exhaustive_match() {
        // Register Option type via prelude, then match only Some
        let forms = vec![SurfaceForm::Match {
            target: SExpr::Atom {
                value: "x".into(),
                span: span(),
            },
            clauses: vec![MatchClause {
                pattern: Pattern::Constructor {
                    name: "Some".into(),
                    name_span: span(),
                    bindings: vec![Pattern::Wildcard(span())],
                    span: span(),
                },
                guard: None,
                body: vec![SExpr::Atom {
                    value: "1".into(),
                    span: span(),
                }],
                span: span(),
            }],
            span: span(),
        }];
        let result = analyze(&forms);
        assert!(result.has_errors);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("None"))
        );
    }
}
