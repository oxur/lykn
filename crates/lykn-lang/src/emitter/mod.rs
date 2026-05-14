//! Kernel emitter — transforms typed surface AST to kernel S-expression forms.
//!
//! The emitter is the final compilation stage in the Rust pipeline. It takes
//! classified `SurfaceForm` nodes and produces kernel `SExpr` nodes that the
//! JS kernel compiler can then lower to JavaScript.

pub mod context;
pub mod contracts;
pub mod dts;
pub mod forms;
pub mod gensym;
pub mod json;
pub mod type_checks;

use crate::analysis::type_registry::TypeRegistry;
use crate::ast::sexpr::SExpr;
use crate::ast::surface::SurfaceForm;

use context::EmitterContext;
use forms::emit_form;
use json::emit_module_json;

/// Emit a slice of surface forms to kernel `SExpr` nodes.
///
/// Each surface form may produce one or more kernel forms. The results are
/// collected in order. When `strip_assertions` is `true`, type-check
/// assertions and contract checks are omitted from the output.
pub fn emit(forms: &[SurfaceForm], registry: &TypeRegistry, strip_assertions: bool) -> Vec<SExpr> {
    let mut ctx = EmitterContext::new(strip_assertions);
    let mut result = Vec::new();
    for form in forms {
        result.extend(emit_form(form, &mut ctx, registry));
    }
    result
}

/// Emit a slice of surface forms to a pretty-printed JSON string.
///
/// This is a convenience function that calls [`emit`] followed by JSON
/// serialization.
pub fn emit_json(forms: &[SurfaceForm], registry: &TypeRegistry, strip_assertions: bool) -> String {
    let kernel = emit(forms, registry, strip_assertions);
    emit_module_json(&kernel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::surface::SurfaceForm;
    use crate::reader::source_loc::Span;

    use forms::{atom, list, num, str_lit};

    fn s() -> Span {
        Span::default()
    }

    #[test]
    fn test_emit_empty_input() {
        let registry = TypeRegistry::default();
        let result = emit(&[], &registry, false);
        assert!(result.is_empty());
    }

    #[test]
    fn test_emit_multiple_forms() {
        let registry = TypeRegistry::default();
        let forms = vec![
            SurfaceForm::Bind {
                name: atom("x"),
                type_ann: None,
                value: num(1.0),
                span: s(),
            },
            SurfaceForm::Bind {
                name: atom("y"),
                type_ann: None,
                value: num(2.0),
                span: s(),
            },
        ];
        let result = emit(&forms, &registry, false);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_emit_json_produces_valid_json() {
        let registry = TypeRegistry::default();
        let forms = vec![SurfaceForm::Bind {
            name: atom("x"),
            type_ann: None,
            value: num(42.0),
            span: s(),
        }];
        let json_str = emit_json(&forms, &registry, false);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_array());
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        // {"type": "list", "values": [{"type": "atom", "value": "const"}, ...]}
        let form = &arr[0];
        assert_eq!(form["type"], "list");
        let values = form["values"].as_array().unwrap();
        assert_eq!(values[0]["type"], "atom");
        assert_eq!(values[0]["value"], "const");
        assert_eq!(values[1]["type"], "atom");
        assert_eq!(values[1]["value"], "x");
        assert_eq!(values[2]["type"], "number");
        assert_eq!(values[2]["value"], 42.0);
    }

    #[test]
    fn test_emit_json_strip_assertions() {
        let registry = TypeRegistry::default();
        let forms = vec![SurfaceForm::KernelPassthrough {
            raw: list(vec![atom("console:log"), str_lit("hello")]),
            span: s(),
        }];

        let json_normal = emit_json(&forms, &registry, false);
        let json_stripped = emit_json(&forms, &registry, true);
        // For passthrough, stripping assertions makes no difference
        assert_eq!(json_normal, json_stripped);
    }

    #[test]
    fn test_emit_preserves_form_order() {
        let registry = TypeRegistry::default();
        let forms = vec![
            SurfaceForm::Bind {
                name: atom("a"),
                type_ann: None,
                value: num(1.0),
                span: s(),
            },
            SurfaceForm::Bind {
                name: atom("b"),
                type_ann: None,
                value: num(2.0),
                span: s(),
            },
            SurfaceForm::Bind {
                name: atom("c"),
                type_ann: None,
                value: num(3.0),
                span: s(),
            },
        ];
        let result = emit(&forms, &registry, false);
        assert_eq!(result.len(), 3);

        // Check ordering
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[1].as_atom(), Some("a"));
        }
        if let SExpr::List { values, .. } = &result[1] {
            assert_eq!(values[1].as_atom(), Some("b"));
        }
        if let SExpr::List { values, .. } = &result[2] {
            assert_eq!(values[1].as_atom(), Some("c"));
        }
    }

    #[test]
    fn test_emit_mixed_forms() {
        let registry = TypeRegistry::default();
        let forms = vec![
            SurfaceForm::Bind {
                name: atom("x"),
                type_ann: None,
                value: num(1.0),
                span: s(),
            },
            SurfaceForm::FunctionCall {
                head: atom("console:log"),
                args: vec![atom("x")],
                span: s(),
            },
        ];
        let result = emit(&forms, &registry, false);
        assert_eq!(result.len(), 2);

        // First is a const binding
        if let SExpr::List { values, .. } = &result[0] {
            assert_eq!(values[0].as_atom(), Some("const"));
        }
        // Second is a function call
        if let SExpr::List { values, .. } = &result[1] {
            assert_eq!(values[0].as_atom(), Some("console:log"));
        }
    }
}
