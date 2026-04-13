pub mod func_check;
pub mod maranget;
pub mod match_check;
pub mod pattern;
pub mod prelude;
pub mod scope;
pub mod type_registry;

use crate::ast::sexpr::SExpr;
use crate::ast::surface::{Pattern, SurfaceForm, ThreadingStep};
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
    fn collect(&self, _registry: &mut TypeRegistry, _scope: &mut ScopeTracker) -> Vec<Diagnostic> {
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
    fn collect(&self, registry: &mut TypeRegistry, scope: &mut ScopeTracker) -> Vec<Diagnostic> {
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
            SurfaceForm::Bind {
                name, value, span, ..
            } => {
                // Walk the value expression first so references to earlier
                // bindings are recorded before we introduce this one.
                track_references_in_expr(value, scope);
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
                    // Walk the body so references to params (and outer
                    // bindings) are recorded inside this scope.
                    for expr in &clause.body {
                        track_references_in_expr(expr, scope);
                    }
                    if let Some(pre) = &clause.pre {
                        track_references_in_expr(pre, scope);
                    }
                    if let Some(post) = &clause.post {
                        track_references_in_expr(post, scope);
                    }
                    scope.exit_scope();
                }
            }
            SurfaceForm::Match {
                target, clauses, ..
            } => {
                track_references_in_expr(target, scope);
                for clause in clauses {
                    scope.enter_scope();
                    introduce_pattern_bindings(&clause.pattern, scope);
                    if let Some(guard) = &clause.guard {
                        track_references_in_expr(guard, scope);
                    }
                    for expr in &clause.body {
                        track_references_in_expr(expr, scope);
                    }
                    scope.exit_scope();
                }
            }
            SurfaceForm::Fn { params, body, .. } | SurfaceForm::Lambda { params, body, .. } => {
                scope.enter_scope();
                for param in params {
                    scope.introduce(&param.name, param.name_span, false, false);
                }
                for expr in body {
                    track_references_in_expr(expr, scope);
                }
                scope.exit_scope();
            }
            SurfaceForm::FunctionCall { head, args, .. } => {
                track_references_in_expr(head, scope);
                for arg in args {
                    track_references_in_expr(arg, scope);
                }
            }
            SurfaceForm::IfLet {
                expr,
                pattern,
                then_body,
                else_body,
                ..
            } => {
                track_references_in_expr(expr, scope);
                scope.enter_scope();
                introduce_pattern_bindings(pattern, scope);
                track_references_in_expr(then_body, scope);
                scope.exit_scope();
                if let Some(eb) = else_body {
                    track_references_in_expr(eb, scope);
                }
            }
            SurfaceForm::WhenLet {
                expr,
                pattern,
                body,
                ..
            } => {
                track_references_in_expr(expr, scope);
                scope.enter_scope();
                introduce_pattern_bindings(pattern, scope);
                for e in body {
                    track_references_in_expr(e, scope);
                }
                scope.exit_scope();
            }
            SurfaceForm::Express { target, .. } => {
                track_references_in_expr(target, scope);
            }
            SurfaceForm::Cell { value, .. } => {
                track_references_in_expr(value, scope);
            }
            SurfaceForm::Swap {
                target,
                func,
                extra_args,
                ..
            } => {
                track_references_in_expr(target, scope);
                track_references_in_expr(func, scope);
                for arg in extra_args {
                    track_references_in_expr(arg, scope);
                }
            }
            SurfaceForm::Reset { target, value, .. } => {
                track_references_in_expr(target, scope);
                track_references_in_expr(value, scope);
            }
            SurfaceForm::ThreadFirst { initial, steps, .. }
            | SurfaceForm::ThreadLast { initial, steps, .. }
            | SurfaceForm::SomeThreadFirst { initial, steps, .. }
            | SurfaceForm::SomeThreadLast { initial, steps, .. } => {
                track_references_in_expr(initial, scope);
                for step in steps {
                    match step {
                        ThreadingStep::Bare(e) => track_references_in_expr(e, scope),
                        ThreadingStep::Call(exprs) => {
                            for e in exprs {
                                track_references_in_expr(e, scope);
                            }
                        }
                    }
                }
            }
            SurfaceForm::Obj { pairs, .. } => {
                for (_, expr) in pairs {
                    track_references_in_expr(expr, scope);
                }
            }
            SurfaceForm::Conj { arr, value, .. } => {
                track_references_in_expr(arr, scope);
                track_references_in_expr(value, scope);
            }
            SurfaceForm::Assoc { obj, pairs, .. } => {
                track_references_in_expr(obj, scope);
                for (_, expr) in pairs {
                    track_references_in_expr(expr, scope);
                }
            }
            SurfaceForm::Dissoc { obj, .. } => {
                track_references_in_expr(obj, scope);
            }
            SurfaceForm::Eq { args, .. }
            | SurfaceForm::And { args, .. }
            | SurfaceForm::Or { args, .. } => {
                for arg in args {
                    track_references_in_expr(arg, scope);
                }
            }
            SurfaceForm::NotEq { left, right, .. } => {
                track_references_in_expr(left, scope);
                track_references_in_expr(right, scope);
            }
            SurfaceForm::Not { operand, .. } => {
                track_references_in_expr(operand, scope);
            }
            // Type, MacroDef, ImportMacros, KernelPassthrough — no user
            // references to track beyond what `collect` already handles.
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

/// Recursively walk an S-expression, recording every atom reference in the
/// scope tracker. This is how usages of bindings are detected — each `Atom`
/// node that matches an in-scope binding marks that binding as used.
fn track_references_in_expr(expr: &SExpr, scope: &mut ScopeTracker) {
    match expr {
        SExpr::Atom { value, span } => {
            scope.reference(value, *span);
        }
        SExpr::List { values, .. } => {
            for v in values {
                track_references_in_expr(v, scope);
            }
        }
        SExpr::Cons { car, cdr, .. } => {
            track_references_in_expr(car, scope);
            track_references_in_expr(cdr, scope);
        }
        // Keywords, strings, numbers, bools, null — no references.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::sexpr::SExpr;
    use crate::ast::surface::{
        Constructor, FuncClause, MatchClause, Pattern, SurfaceForm, TypeAnnotation, TypedParam,
    };
    use crate::reader::source_loc::Span;

    fn span() -> Span {
        Span::default()
    }

    // --- Cross-form reference tracking tests ---

    #[test]
    fn test_bind_then_function_call_no_unused_warning() {
        // (bind greeting "hello")
        // (console:log greeting)
        let forms = vec![
            SurfaceForm::Bind {
                name: SExpr::Atom {
                    value: "greeting".into(),
                    span: span(),
                },
                type_ann: None,
                value: SExpr::String {
                    value: "hello".into(),
                    span: span(),
                },
                span: span(),
            },
            SurfaceForm::FunctionCall {
                head: SExpr::Atom {
                    value: "console:log".into(),
                    span: span(),
                },
                args: vec![SExpr::Atom {
                    value: "greeting".into(),
                    span: span(),
                }],
                span: span(),
            },
        ];
        let result = analyze(&forms);
        let unused: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.message.contains("unused"))
            .collect();
        assert!(
            unused.is_empty(),
            "expected no unused warnings, got: {unused:?}"
        );
    }

    #[test]
    fn test_bind_without_reference_warns_unused() {
        // (bind x 1) — with no reference anywhere
        let forms = vec![SurfaceForm::Bind {
            name: SExpr::Atom {
                value: "x".into(),
                span: span(),
            },
            type_ann: None,
            value: SExpr::Number {
                value: 1.0,
                span: span(),
            },
            span: span(),
        }];
        let result = analyze(&forms);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("unused binding 'x'")),
            "expected unused warning for 'x'"
        );
    }

    #[test]
    fn test_func_params_referenced_in_body_no_unused_warning() {
        // (func add :args (:number a :number b) :returns :number :body (+ a b))
        // (console:log (add 1 2))
        let forms = vec![
            SurfaceForm::Func {
                name: "add".into(),
                name_span: span(),
                clauses: vec![FuncClause {
                    args: vec![
                        TypedParam {
                            type_ann: TypeAnnotation {
                                name: "number".into(),
                                span: span(),
                            },
                            name: "a".into(),
                            name_span: span(),
                        },
                        TypedParam {
                            type_ann: TypeAnnotation {
                                name: "number".into(),
                                span: span(),
                            },
                            name: "b".into(),
                            name_span: span(),
                        },
                    ],
                    returns: Some(TypeAnnotation {
                        name: "number".into(),
                        span: span(),
                    }),
                    pre: None,
                    post: None,
                    body: vec![SExpr::List {
                        values: vec![
                            SExpr::Atom {
                                value: "+".into(),
                                span: span(),
                            },
                            SExpr::Atom {
                                value: "a".into(),
                                span: span(),
                            },
                            SExpr::Atom {
                                value: "b".into(),
                                span: span(),
                            },
                        ],
                        span: span(),
                    }],
                    span: span(),
                }],
                span: span(),
            },
            SurfaceForm::FunctionCall {
                head: SExpr::Atom {
                    value: "console:log".into(),
                    span: span(),
                },
                args: vec![SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "add".into(),
                            span: span(),
                        },
                        SExpr::Number {
                            value: 1.0,
                            span: span(),
                        },
                        SExpr::Number {
                            value: 2.0,
                            span: span(),
                        },
                    ],
                    span: span(),
                }],
                span: span(),
            },
        ];
        let result = analyze(&forms);
        let unused: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.message.contains("unused"))
            .collect();
        assert!(
            unused.is_empty(),
            "expected no unused warnings, got: {unused:?}"
        );
    }

    #[test]
    fn test_func_unused_param_still_warns() {
        // A function where param 'b' is never referenced in the body
        let forms = vec![
            SurfaceForm::Func {
                name: "f".into(),
                name_span: span(),
                clauses: vec![FuncClause {
                    args: vec![
                        TypedParam {
                            type_ann: TypeAnnotation {
                                name: "number".into(),
                                span: span(),
                            },
                            name: "a".into(),
                            name_span: span(),
                        },
                        TypedParam {
                            type_ann: TypeAnnotation {
                                name: "number".into(),
                                span: span(),
                            },
                            name: "b".into(),
                            name_span: span(),
                        },
                    ],
                    returns: None,
                    pre: None,
                    post: None,
                    body: vec![SExpr::Atom {
                        value: "a".into(),
                        span: span(),
                    }],
                    span: span(),
                }],
                span: span(),
            },
            SurfaceForm::FunctionCall {
                head: SExpr::Atom {
                    value: "f".into(),
                    span: span(),
                },
                args: vec![SExpr::Number {
                    value: 1.0,
                    span: span(),
                }],
                span: span(),
            },
        ];
        let result = analyze(&forms);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("unused binding 'b'")),
            "expected unused warning for 'b'"
        );
        assert!(
            !result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("unused binding 'a'")),
            "should NOT warn about 'a'"
        );
    }

    // --- Original tests ---

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
