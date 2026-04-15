//! Overlap detection for multi-clause function definitions.
//!
//! Uses the Maranget usefulness algorithm to detect when two clauses of the
//! same function accept the same set of argument types.

use std::collections::HashMap;

use crate::ast::surface::{FuncClause, SurfaceForm};
use crate::diagnostics::{Diagnostic, Severity};

use super::maranget;
use super::pattern::DeconPattern;
use super::type_registry::TypeRegistry;

/// Check a multi-clause `Func` form for overlapping clauses.
///
/// Clauses are grouped by arity. Within each group, pairwise overlap is
/// detected by checking whether each clause's type-keyword pattern vector is
/// useful with respect to the other.
pub fn check_func_overlap(form: &SurfaceForm, registry: &TypeRegistry) -> Vec<Diagnostic> {
    let (name, clauses, _span) = match form {
        SurfaceForm::Func {
            name,
            clauses,
            span,
            ..
        } => (name, clauses, *span),
        _ => return Vec::new(),
    };
    if clauses.len() < 2 {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();

    // Group clauses by arity
    let mut by_arity: HashMap<usize, Vec<(usize, &FuncClause)>> = HashMap::new();
    for (i, clause) in clauses.iter().enumerate() {
        by_arity
            .entry(clause.args.len())
            .or_default()
            .push((i, clause));
    }

    for (arity, group) in &by_arity {
        if group.len() < 2 {
            continue;
        }

        // Convert each clause's type annotations to a pattern row
        let rows: Vec<Vec<DeconPattern>> = group
            .iter()
            .map(|(_, clause)| {
                clause
                    .args
                    .iter()
                    .map(|param| {
                        let dtype = param.dispatch_type();
                        if dtype == "any" {
                            DeconPattern::Wildcard
                        } else {
                            DeconPattern::TypeKeyword(dtype.to_string())
                        }
                    })
                    .collect()
            })
            .collect();

        // Check each pair for overlap
        for i in 0..rows.len() {
            for j in (i + 1)..rows.len() {
                let matrix_i = vec![rows[i].clone()];
                let matrix_j = vec![rows[j].clone()];
                let j_useful_wrt_i = maranget::is_useful(&matrix_i, &rows[j], registry);
                let i_useful_wrt_j = maranget::is_useful(&matrix_j, &rows[i], registry);
                if !j_useful_wrt_i || !i_useful_wrt_j {
                    let (orig_i, _) = group[i];
                    let (orig_j, _) = group[j];
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "{name}: clauses {orig_i} and {orig_j} overlap \
                             (same arity {arity}, compatible types)"
                        ),
                        span: clauses[orig_j].span,
                        suggestion: Some(
                            "ensure each clause matches a distinct set of argument types".into(),
                        ),
                    });
                }
            }
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::prelude::register_prelude_types;
    use crate::analysis::type_registry::TypeRegistry;
    use crate::ast::surface::{FuncClause, ParamShape, SurfaceForm, TypeAnnotation, TypedParam};
    use crate::reader::source_loc::Span;

    fn span() -> Span {
        Span::default()
    }

    fn registry() -> TypeRegistry {
        let mut r = TypeRegistry::default();
        register_prelude_types(&mut r);
        r
    }

    fn typed_param(name: &str, type_name: &str) -> TypedParam {
        TypedParam {
            type_ann: TypeAnnotation {
                name: type_name.into(),
                span: span(),
            },
            name: name.into(),
            name_span: span(),
        }
    }

    fn func_clause(params: Vec<TypedParam>) -> FuncClause {
        FuncClause {
            args: params.into_iter().map(ParamShape::from).collect(),
            returns: None,
            pre: None,
            post: None,
            body: vec![],
            span: span(),
        }
    }

    fn func_form(name: &str, clauses: Vec<FuncClause>) -> SurfaceForm {
        SurfaceForm::Func {
            name: name.into(),
            name_span: span(),
            clauses,
            span: span(),
        }
    }

    #[test]
    fn test_no_overlap_different_types() {
        let reg = registry();
        let form = func_form(
            "add",
            vec![
                func_clause(vec![typed_param("a", "number"), typed_param("b", "number")]),
                func_clause(vec![typed_param("a", "string"), typed_param("b", "string")]),
            ],
        );
        let diags = check_func_overlap(&form, &reg);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn test_overlap_same_types() {
        let reg = registry();
        let form = func_form(
            "add",
            vec![
                func_clause(vec![typed_param("a", "number"), typed_param("b", "number")]),
                func_clause(vec![typed_param("x", "number"), typed_param("y", "number")]),
            ],
        );
        let diags = check_func_overlap(&form, &reg);
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("overlap"));
    }

    #[test]
    fn test_overlap_any_subsumes() {
        let reg = registry();
        let form = func_form(
            "process",
            vec![
                func_clause(vec![typed_param("a", "any")]),
                func_clause(vec![typed_param("a", "number")]),
            ],
        );
        let diags = check_func_overlap(&form, &reg);
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("overlap"));
    }

    #[test]
    fn test_no_overlap_different_arity() {
        let reg = registry();
        let form = func_form(
            "f",
            vec![
                func_clause(vec![typed_param("a", "number")]),
                func_clause(vec![typed_param("a", "number"), typed_param("b", "number")]),
            ],
        );
        let diags = check_func_overlap(&form, &reg);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_single_clause_no_check() {
        let reg = registry();
        let form = func_form("f", vec![func_clause(vec![typed_param("a", "number")])]);
        let diags = check_func_overlap(&form, &reg);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_non_func_form_returns_empty() {
        let reg = registry();
        let form = SurfaceForm::KernelPassthrough {
            raw: crate::ast::sexpr::SExpr::Atom {
                value: "x".into(),
                span: span(),
            },
            span: span(),
        };
        assert!(check_func_overlap(&form, &reg).is_empty());
    }

    // ---------------------------------------------------------------
    // Destructured parameter overlap detection (DD-25)
    // ---------------------------------------------------------------

    fn func_clause_with_shapes(params: Vec<ParamShape>) -> FuncClause {
        FuncClause {
            args: params,
            returns: None,
            pre: None,
            post: None,
            body: vec![],
            span: span(),
        }
    }

    #[test]
    fn test_overlap_two_object_destructures() {
        let reg = registry();
        let form = func_form(
            "f",
            vec![
                func_clause_with_shapes(vec![ParamShape::DestructuredObject {
                    fields: vec![typed_param("name", "string"), typed_param("age", "number")],
                    span: span(),
                }]),
                func_clause_with_shapes(vec![ParamShape::DestructuredObject {
                    fields: vec![typed_param("label", "string")],
                    span: span(),
                }]),
            ],
        );
        let diags = check_func_overlap(&form, &reg);
        assert!(
            !diags.is_empty(),
            "two object destructures at same position should overlap"
        );
        assert!(diags[0].message.contains("overlap"));
    }

    #[test]
    fn test_no_overlap_object_vs_string() {
        let reg = registry();
        let form = func_form(
            "f",
            vec![
                func_clause_with_shapes(vec![ParamShape::DestructuredObject {
                    fields: vec![typed_param("name", "string")],
                    span: span(),
                }]),
                func_clause(vec![typed_param("s", "string")]),
            ],
        );
        let diags = check_func_overlap(&form, &reg);
        assert!(
            diags.is_empty(),
            "object vs string should not overlap, got: {diags:?}"
        );
    }

    #[test]
    fn test_no_overlap_object_vs_array_destructure() {
        let reg = registry();
        let form = func_form(
            "f",
            vec![
                func_clause_with_shapes(vec![ParamShape::DestructuredObject {
                    fields: vec![typed_param("name", "string")],
                    span: span(),
                }]),
                func_clause_with_shapes(vec![ParamShape::DestructuredArray {
                    elements: vec![crate::ast::surface::ArrayParamElement::Typed(typed_param(
                        "first", "number",
                    ))],
                    span: span(),
                }]),
            ],
        );
        let diags = check_func_overlap(&form, &reg);
        assert!(
            diags.is_empty(),
            "object vs array should not overlap, got: {diags:?}"
        );
    }

    #[test]
    fn test_three_clauses_partial_overlap() {
        let reg = registry();
        let form = func_form(
            "f",
            vec![
                func_clause(vec![typed_param("a", "number")]),
                func_clause(vec![typed_param("a", "string")]),
                func_clause(vec![typed_param("a", "number")]), // overlaps with clause 0
            ],
        );
        let diags = check_func_overlap(&form, &reg);
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("clauses 0 and 2"));
    }
}
