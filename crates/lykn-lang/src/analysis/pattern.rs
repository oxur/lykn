use super::type_registry::TypeRegistry;
use crate::ast::sexpr::SExpr;
use crate::ast::surface::Pattern;
use crate::diagnostics::{Diagnostic, Severity};

/// A literal value in a pattern. Uses `f64::to_bits()` for numeric equality
/// so that `LiteralKind` can implement `Eq`.
#[derive(Debug, Clone)]
pub enum LiteralKind {
    Number(u64),
    String(String),
    Bool(bool),
    Null,
    Keyword(String),
}

impl PartialEq for LiteralKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LiteralKind::Number(a), LiteralKind::Number(b)) => a == b,
            (LiteralKind::String(a), LiteralKind::String(b)) => a == b,
            (LiteralKind::Bool(a), LiteralKind::Bool(b)) => a == b,
            (LiteralKind::Null, LiteralKind::Null) => true,
            (LiteralKind::Keyword(a), LiteralKind::Keyword(b)) => a == b,
            _ => false,
        }
    }
}
impl Eq for LiteralKind {}

/// Internal deconstructed pattern representation used by the Maranget
/// algorithm. Surface-level `Pattern` nodes are lowered into this form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeconPattern {
    /// A constructor with its type name, constructor name, and sub-patterns
    /// for each field.
    Constructor {
        type_name: String,
        ctor_name: String,
        fields: Vec<DeconPattern>,
    },
    /// A literal value pattern.
    Literal(LiteralKind),
    /// Matches anything (wildcard or variable binding).
    Wildcard,
    /// An object/structural pattern with key-value pairs.
    Structural { keys: Vec<(String, DeconPattern)> },
    /// A type keyword pattern (used in function clause dispatch).
    TypeKeyword(String),
}

/// Lower a surface `Pattern` into a `DeconPattern` suitable for the
/// exhaustiveness / usefulness checker.
pub fn deconstruct_pattern(
    pat: &Pattern,
    registry: &TypeRegistry,
) -> Result<DeconPattern, Diagnostic> {
    match pat {
        Pattern::Wildcard(_) => Ok(DeconPattern::Wildcard),
        Pattern::Binding { .. } => Ok(DeconPattern::Wildcard),
        Pattern::Literal(sexpr) => deconstruct_literal(sexpr),
        Pattern::Constructor {
            name,
            bindings,
            span,
            ..
        } => {
            let ctor = registry
                .lookup_constructor(name)
                .ok_or_else(|| Diagnostic {
                    severity: Severity::Error,
                    message: format!("unknown constructor '{name}'"),
                    span: *span,
                    suggestion: None,
                })?;
            let type_name = ctor.owning_type.clone();
            let fields = bindings
                .iter()
                .map(|b| deconstruct_pattern(b, registry))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(DeconPattern::Constructor {
                type_name,
                ctor_name: name.clone(),
                fields,
            })
        }
        Pattern::Obj { pairs, .. } => {
            let keys = pairs
                .iter()
                .map(|(k, p)| deconstruct_pattern(p, registry).map(|dp| (k.clone(), dp)))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(DeconPattern::Structural { keys })
        }
    }
}

fn deconstruct_literal(sexpr: &SExpr) -> Result<DeconPattern, Diagnostic> {
    match sexpr {
        SExpr::Number { value, .. } => {
            Ok(DeconPattern::Literal(LiteralKind::Number(value.to_bits())))
        }
        SExpr::String { value, .. } => {
            Ok(DeconPattern::Literal(LiteralKind::String(value.clone())))
        }
        SExpr::Bool { value, .. } => Ok(DeconPattern::Literal(LiteralKind::Bool(*value))),
        SExpr::Null { .. } => Ok(DeconPattern::Literal(LiteralKind::Null)),
        SExpr::Keyword { value, .. } => {
            Ok(DeconPattern::Literal(LiteralKind::Keyword(value.clone())))
        }
        SExpr::Atom { value, .. } => match value.as_str() {
            "true" => Ok(DeconPattern::Literal(LiteralKind::Bool(true))),
            "false" => Ok(DeconPattern::Literal(LiteralKind::Bool(false))),
            "null" | "undefined" => Ok(DeconPattern::Literal(LiteralKind::Null)),
            _ => Ok(DeconPattern::Wildcard), // variable binding in literal position
        },
        _ => Err(Diagnostic {
            severity: Severity::Error,
            message: "invalid literal in pattern".into(),
            span: sexpr.span(),
            suggestion: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::prelude::register_prelude_types;
    use crate::reader::source_loc::Span;

    fn span() -> Span {
        Span::default()
    }

    fn registry() -> TypeRegistry {
        let mut r = TypeRegistry::default();
        register_prelude_types(&mut r);
        r
    }

    #[test]
    fn test_wildcard_pattern() {
        let reg = registry();
        let dp = deconstruct_pattern(&Pattern::Wildcard(span()), &reg).unwrap();
        assert_eq!(dp, DeconPattern::Wildcard);
    }

    #[test]
    fn test_binding_becomes_wildcard() {
        let reg = registry();
        let dp = deconstruct_pattern(
            &Pattern::Binding {
                name: "x".into(),
                span: span(),
            },
            &reg,
        )
        .unwrap();
        assert_eq!(dp, DeconPattern::Wildcard);
    }

    #[test]
    fn test_constructor_pattern() {
        let reg = registry();
        let pat = Pattern::Constructor {
            name: "Some".into(),
            name_span: span(),
            bindings: vec![Pattern::Wildcard(span())],
            span: span(),
        };
        let dp = deconstruct_pattern(&pat, &reg).unwrap();
        match dp {
            DeconPattern::Constructor {
                type_name,
                ctor_name,
                fields,
            } => {
                assert_eq!(type_name, "Option");
                assert_eq!(ctor_name, "Some");
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0], DeconPattern::Wildcard);
            }
            other => panic!("expected Constructor, got {other:?}"),
        }
    }

    #[test]
    fn test_unknown_constructor_error() {
        let reg = registry();
        let pat = Pattern::Constructor {
            name: "Bogus".into(),
            name_span: span(),
            bindings: vec![],
            span: span(),
        };
        let err = deconstruct_pattern(&pat, &reg).unwrap_err();
        assert!(err.message.contains("unknown constructor 'Bogus'"));
    }

    #[test]
    fn test_literal_number() {
        let reg = registry();
        let pat = Pattern::Literal(SExpr::Number {
            value: 42.0,
            span: span(),
        });
        let dp = deconstruct_pattern(&pat, &reg).unwrap();
        assert_eq!(
            dp,
            DeconPattern::Literal(LiteralKind::Number(42.0_f64.to_bits()))
        );
    }

    #[test]
    fn test_literal_string() {
        let reg = registry();
        let pat = Pattern::Literal(SExpr::String {
            value: "hello".into(),
            span: span(),
        });
        let dp = deconstruct_pattern(&pat, &reg).unwrap();
        assert_eq!(
            dp,
            DeconPattern::Literal(LiteralKind::String("hello".into()))
        );
    }

    #[test]
    fn test_literal_bool() {
        let reg = registry();
        let pat = Pattern::Literal(SExpr::Bool {
            value: true,
            span: span(),
        });
        let dp = deconstruct_pattern(&pat, &reg).unwrap();
        assert_eq!(dp, DeconPattern::Literal(LiteralKind::Bool(true)));
    }

    #[test]
    fn test_literal_null() {
        let reg = registry();
        let pat = Pattern::Literal(SExpr::Null { span: span() });
        let dp = deconstruct_pattern(&pat, &reg).unwrap();
        assert_eq!(dp, DeconPattern::Literal(LiteralKind::Null));
    }

    #[test]
    fn test_literal_keyword() {
        let reg = registry();
        let pat = Pattern::Literal(SExpr::Keyword {
            value: "status".into(),
            span: span(),
        });
        let dp = deconstruct_pattern(&pat, &reg).unwrap();
        assert_eq!(
            dp,
            DeconPattern::Literal(LiteralKind::Keyword("status".into()))
        );
    }

    #[test]
    fn test_atom_true_becomes_bool() {
        let reg = registry();
        let pat = Pattern::Literal(SExpr::Atom {
            value: "true".into(),
            span: span(),
        });
        let dp = deconstruct_pattern(&pat, &reg).unwrap();
        assert_eq!(dp, DeconPattern::Literal(LiteralKind::Bool(true)));
    }

    #[test]
    fn test_atom_null_becomes_null() {
        let reg = registry();
        let pat = Pattern::Literal(SExpr::Atom {
            value: "null".into(),
            span: span(),
        });
        let dp = deconstruct_pattern(&pat, &reg).unwrap();
        assert_eq!(dp, DeconPattern::Literal(LiteralKind::Null));
    }

    #[test]
    fn test_atom_variable_becomes_wildcard() {
        let reg = registry();
        let pat = Pattern::Literal(SExpr::Atom {
            value: "x".into(),
            span: span(),
        });
        let dp = deconstruct_pattern(&pat, &reg).unwrap();
        assert_eq!(dp, DeconPattern::Wildcard);
    }

    #[test]
    fn test_obj_pattern() {
        let reg = registry();
        let pat = Pattern::Obj {
            pairs: vec![
                ("name".into(), Pattern::Wildcard(span())),
                ("age".into(), Pattern::Wildcard(span())),
            ],
            span: span(),
        };
        let dp = deconstruct_pattern(&pat, &reg).unwrap();
        match dp {
            DeconPattern::Structural { keys } => {
                assert_eq!(keys.len(), 2);
                assert_eq!(keys[0].0, "name");
                assert_eq!(keys[1].0, "age");
            }
            other => panic!("expected Structural, got {other:?}"),
        }
    }

    #[test]
    fn test_invalid_literal_list() {
        let reg = registry();
        let pat = Pattern::Literal(SExpr::List {
            values: vec![],
            span: span(),
        });
        let err = deconstruct_pattern(&pat, &reg).unwrap_err();
        assert!(err.message.contains("invalid literal"));
    }
}
