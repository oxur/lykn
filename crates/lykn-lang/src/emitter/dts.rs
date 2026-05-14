//! TypeScript .d.ts declaration emitter.
//!
//! M10: emits .d.ts files alongside compiled .js for lykn packages with
//! :type annotations. The .d.ts is auxiliary type information for TypeScript
//! consumers; the .js remains the canonical compiled output.

use crate::analysis::type_registry::TypeRegistry;
use crate::ast::surface::{
    ArrayParamElement, Constructor, DestructuredField, FuncClause, ParamShape, SurfaceForm,
    TypeAnnotation, TypedParam,
};
use crate::codegen::names::to_js_identifier;
use crate::diagnostics::{Diagnostic, Severity};
use crate::reader::source_loc::Span;

pub fn lykn_type_to_ts(ann: &TypeAnnotation, registry: &TypeRegistry) -> String {
    match ann.name.as_str() {
        "number" => "number".to_string(),
        "string" => "string".to_string(),
        "boolean" => "boolean".to_string(),
        "function" => "Function".to_string(),
        "object" => "object".to_string(),
        "array" => "unknown[]".to_string(),
        "symbol" => "symbol".to_string(),
        "bigint" => "bigint".to_string(),
        "any" => "unknown".to_string(),
        "void" => "void".to_string(),
        "promise" => "Promise<unknown>".to_string(),
        name => {
            if registry.lookup_type(name).is_some() {
                name.to_string()
            } else {
                name.to_string()
            }
        }
    }
}

pub fn param_shape_to_ts(shape: &ParamShape, registry: &TypeRegistry) -> String {
    match shape {
        ParamShape::Simple(tp) => lykn_type_to_ts(&tp.type_ann, registry),
        ParamShape::DestructuredObject { fields, .. } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|f| match f {
                    DestructuredField::Simple(tp) => {
                        let name = to_js_identifier(&tp.name);
                        let ts_type = lykn_type_to_ts(&tp.type_ann, registry);
                        if tp.default_value.is_some() {
                            format!("{name}?: {ts_type}")
                        } else {
                            format!("{name}: {ts_type}")
                        }
                    }
                    DestructuredField::Nested {
                        alias_name,
                        pattern,
                        ..
                    } => {
                        let name = to_js_identifier(alias_name);
                        let inner = param_shape_to_ts(pattern, registry);
                        format!("{name}: {inner}")
                    }
                })
                .collect();
            format!("{{ {} }}", parts.join("; "))
        }
        ParamShape::DestructuredArray { elements, .. } => {
            let mut parts: Vec<String> = Vec::new();
            for elem in elements {
                match elem {
                    ArrayParamElement::Typed(tp) => {
                        parts.push(lykn_type_to_ts(&tp.type_ann, registry));
                    }
                    ArrayParamElement::Rest(tp) => {
                        let inner = lykn_type_to_ts(&tp.type_ann, registry);
                        parts.push(format!("...{inner}[]"));
                    }
                    ArrayParamElement::Skip(_) => {
                        parts.push("unknown".to_string());
                    }
                    ArrayParamElement::Nested { pattern, .. }
                    | ArrayParamElement::NestedWithAlias { pattern, .. } => {
                        parts.push(param_shape_to_ts(pattern, registry));
                    }
                }
            }
            format!("[{}]", parts.join(", "))
        }
    }
}

fn emit_func_args(clauses_args: &[ParamShape], registry: &TypeRegistry) -> String {
    clauses_args
        .iter()
        .enumerate()
        .map(|(i, shape)| {
            let arg_name = match shape {
                ParamShape::Simple(tp) => to_js_identifier(&tp.name),
                _ => format!("arg{i}"),
            };
            let arg_type = param_shape_to_ts(shape, registry);
            format!("{arg_name}: {arg_type}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn emit_func_dts(
    name: &str,
    clauses: &[FuncClause],
    exported: bool,
    registry: &TypeRegistry,
    file_path: &str,
    warnings: &mut Vec<Diagnostic>,
) -> String {
    let modifier = if exported { "export " } else { "declare " };
    let js_name = to_js_identifier(name);
    let mut out = String::new();

    for clause in clauses {
        if let Some(ref pre) = clause.pre {
            out.push_str(&format!(
                "/** @requires {{{}}} */\n",
                format_sexpr_brief(pre)
            ));
        }
        if let Some(ref post) = clause.post {
            out.push_str(&format!(
                "/** @ensures {{{}}} */\n",
                format_sexpr_brief(post)
            ));
        }

        let args = emit_func_args(&clause.args, registry);

        let return_type = match &clause.returns {
            Some(ann) => lykn_type_to_ts(ann, registry),
            None => {
                if exported {
                    warnings.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "exported function `{name}` has no `:returns` annotation; \
                             .d.ts will use `unknown`"
                        ),
                        span: clause.span,
                        suggestion: Some(format!("add `:returns :type` to `{name}`")),
                    });
                }
                "unknown".to_string()
            }
        };

        out.push_str(&format!(
            "{modifier}function {js_name}({args}): {return_type};\n"
        ));
    }
    out
}

pub fn emit_type_dts(
    name: &str,
    constructors: &[Constructor],
    exported: bool,
    registry: &TypeRegistry,
) -> String {
    let modifier = if exported { "export " } else { "" };
    let variants: Vec<String> = constructors
        .iter()
        .map(|c| {
            let mut parts = vec![format!("tag: \"{}\"", c.name)];
            for field in &c.fields {
                let ts_type = lykn_type_to_ts(&field.type_ann, registry);
                let field_name = to_js_identifier(&field.name);
                parts.push(format!("{field_name}: {ts_type}"));
            }
            format!("{{ {} }}", parts.join("; "))
        })
        .collect();

    if variants.len() == 1 {
        format!("{modifier}type {name} = {};\n", variants[0])
    } else {
        let joined = variants
            .iter()
            .map(|v| format!("  | {v}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{modifier}type {name} =\n{joined};\n")
    }
}

pub fn emit_bind_dts(
    name: &str,
    type_ann: &TypeAnnotation,
    exported: bool,
    registry: &TypeRegistry,
) -> String {
    let modifier = if exported { "export " } else { "declare " };
    let js_name = to_js_identifier(name);
    let ts_type = lykn_type_to_ts(type_ann, registry);
    format!("{modifier}const {js_name}: {ts_type};\n")
}

pub fn emit_dts_module(
    forms: &[SurfaceForm],
    registry: &TypeRegistry,
    file_path: &str,
) -> (String, Vec<Diagnostic>) {
    let mut out = String::new();
    let mut warnings = Vec::new();

    for form in forms {
        match form {
            SurfaceForm::Export { inner, .. } => match inner.as_ref() {
                SurfaceForm::Func {
                    name, clauses, ..
                } => {
                    out.push_str(&emit_func_dts(
                        name, clauses, true, registry, file_path, &mut warnings,
                    ));
                }
                SurfaceForm::Bind {
                    name, type_ann, value, ..
                } => {
                    if let Some(ann) = type_ann {
                        if let Some(n) = name.as_atom() {
                            out.push_str(&emit_bind_dts(n, ann, true, registry));
                        }
                    } else if let Some(n) = name.as_atom() {
                        out.push_str(&emit_bind_inferred_dts(n, value, true));
                    }
                }
                SurfaceForm::Type {
                    name, constructors, ..
                } => {
                    out.push_str(&emit_type_dts(name, constructors, true, registry));
                    for c in constructors {
                        out.push_str(&emit_constructor_fn_dts(name, c, true, registry));
                    }
                }
                _ => {}
            },
            SurfaceForm::Type {
                name, constructors, ..
            } => {
                out.push_str(&emit_type_dts(name, constructors, false, registry));
            }
            _ => {}
        }
    }

    (out, warnings)
}

fn infer_literal_ts_type(expr: &crate::ast::sexpr::SExpr) -> &'static str {
    use crate::ast::sexpr::SExpr;
    match expr {
        SExpr::String { .. } => "string",
        SExpr::Number { .. } => "number",
        SExpr::Bool { .. } => "boolean",
        _ => "unknown",
    }
}

fn emit_bind_inferred_dts(name: &str, value: &crate::ast::sexpr::SExpr, exported: bool) -> String {
    let modifier = if exported { "export " } else { "declare " };
    let js_name = to_js_identifier(name);
    let ts_type = infer_literal_ts_type(value);
    format!("{modifier}const {js_name}: {ts_type};\n")
}

fn emit_constructor_fn_dts(
    type_name: &str,
    constructor: &Constructor,
    exported: bool,
    registry: &TypeRegistry,
) -> String {
    let modifier = if exported { "export " } else { "declare " };
    if constructor.fields.is_empty() {
        format!("{modifier}const {}: {type_name};\n", constructor.name)
    } else {
        let args: Vec<String> = constructor
            .fields
            .iter()
            .map(|f| {
                let name = to_js_identifier(&f.name);
                let ts_type = lykn_type_to_ts(&f.type_ann, registry);
                format!("{name}: {ts_type}")
            })
            .collect();
        format!(
            "{modifier}function {}({}): {type_name};\n",
            constructor.name,
            args.join(", ")
        )
    }
}

fn format_sexpr_brief(expr: &crate::ast::sexpr::SExpr) -> String {
    use crate::ast::sexpr::SExpr;
    match expr {
        SExpr::Atom { value, .. } => value.clone(),
        SExpr::String { value, .. } => format!("\"{value}\""),
        SExpr::Number { value, .. } => value.to_string(),
        SExpr::Bool { value, .. } => value.to_string(),
        SExpr::Null { .. } => "null".to_string(),
        SExpr::Keyword { value, .. } => format!(":{value}"),
        SExpr::List { values, .. } => {
            let inner: Vec<String> = values.iter().map(format_sexpr_brief).collect();
            format!("({})", inner.join(" "))
        }
        SExpr::Cons { car, cdr, .. } => {
            format!("({} . {})", format_sexpr_brief(car), format_sexpr_brief(cdr))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::type_registry::TypeRegistry;
    use crate::reader::source_loc::Span;

    fn s() -> Span {
        Span::default()
    }

    fn ann(name: &str) -> TypeAnnotation {
        TypeAnnotation {
            name: name.to_string(),
            span: s(),
        }
    }

    fn tp(type_name: &str, param_name: &str) -> TypedParam {
        TypedParam {
            type_ann: ann(type_name),
            name: param_name.to_string(),
            name_span: s(),
            default_value: None,
            is_rest: false,
        }
    }

    fn reg() -> TypeRegistry {
        TypeRegistry::default()
    }

    #[test]
    fn test_type_number() {
        assert_eq!(lykn_type_to_ts(&ann("number"), &reg()), "number");
    }
    #[test]
    fn test_type_string() {
        assert_eq!(lykn_type_to_ts(&ann("string"), &reg()), "string");
    }
    #[test]
    fn test_type_boolean() {
        assert_eq!(lykn_type_to_ts(&ann("boolean"), &reg()), "boolean");
    }
    #[test]
    fn test_type_function() {
        assert_eq!(lykn_type_to_ts(&ann("function"), &reg()), "Function");
    }
    #[test]
    fn test_type_object() {
        assert_eq!(lykn_type_to_ts(&ann("object"), &reg()), "object");
    }
    #[test]
    fn test_type_array() {
        assert_eq!(lykn_type_to_ts(&ann("array"), &reg()), "unknown[]");
    }
    #[test]
    fn test_type_symbol() {
        assert_eq!(lykn_type_to_ts(&ann("symbol"), &reg()), "symbol");
    }
    #[test]
    fn test_type_bigint() {
        assert_eq!(lykn_type_to_ts(&ann("bigint"), &reg()), "bigint");
    }
    #[test]
    fn test_type_any() {
        assert_eq!(lykn_type_to_ts(&ann("any"), &reg()), "unknown");
    }
    #[test]
    fn test_type_void() {
        assert_eq!(lykn_type_to_ts(&ann("void"), &reg()), "void");
    }
    #[test]
    fn test_type_promise() {
        assert_eq!(lykn_type_to_ts(&ann("promise"), &reg()), "Promise<unknown>");
    }
    #[test]
    fn test_type_user() {
        assert_eq!(lykn_type_to_ts(&ann("MyType"), &reg()), "MyType");
    }

    #[test]
    fn test_param_simple() {
        let shape = ParamShape::Simple(tp("number", "x"));
        assert_eq!(param_shape_to_ts(&shape, &reg()), "number");
    }

    #[test]
    fn test_param_destructured_object() {
        let shape = ParamShape::DestructuredObject {
            fields: vec![
                DestructuredField::Simple(tp("string", "host")),
                DestructuredField::Simple(TypedParam {
                    type_ann: ann("boolean"),
                    name: "ssl".to_string(),
                    name_span: s(),
                    default_value: Some(crate::ast::sexpr::SExpr::Bool {
                        value: true,
                        span: s(),
                    }),
                    is_rest: false,
                }),
            ],
            span: s(),
        };
        assert_eq!(
            param_shape_to_ts(&shape, &reg()),
            "{ host: string; ssl?: boolean }"
        );
    }

    #[test]
    fn test_param_destructured_array_with_rest() {
        let shape = ParamShape::DestructuredArray {
            elements: vec![
                ArrayParamElement::Typed(tp("string", "head")),
                ArrayParamElement::Rest(tp("number", "tail")),
            ],
            span: s(),
        };
        assert_eq!(param_shape_to_ts(&shape, &reg()), "[string, ...number[]]");
    }

    #[test]
    fn test_emit_func_single_clause() {
        let clause = FuncClause {
            args: vec![ParamShape::Simple(tp("string", "name"))],
            returns: Some(ann("string")),
            pre: None,
            post: None,
            body: vec![],
            span: s(),
        };
        let mut warnings = vec![];
        let result = emit_func_dts("greet", &[clause], true, &reg(), "<test>", &mut warnings);
        assert_eq!(result, "export function greet(name: string): string;\n");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_emit_func_multi_clause_overloads() {
        let c1 = FuncClause {
            args: vec![ParamShape::Simple(tp("string", "name"))],
            returns: Some(ann("string")),
            pre: None,
            post: None,
            body: vec![],
            span: s(),
        };
        let c2 = FuncClause {
            args: vec![
                ParamShape::Simple(tp("string", "greeting")),
                ParamShape::Simple(tp("string", "name")),
            ],
            returns: Some(ann("string")),
            pre: None,
            post: None,
            body: vec![],
            span: s(),
        };
        let mut warnings = vec![];
        let result = emit_func_dts("greet", &[c1, c2], true, &reg(), "<test>", &mut warnings);
        assert!(result.contains("export function greet(name: string): string;"));
        assert!(result.contains("export function greet(greeting: string, name: string): string;"));
    }

    #[test]
    fn test_emit_func_no_returns_warning() {
        let clause = FuncClause {
            args: vec![ParamShape::Simple(tp("any", "x"))],
            returns: None,
            pre: None,
            post: None,
            body: vec![],
            span: s(),
        };
        let mut warnings = vec![];
        let result = emit_func_dts("untyped", &[clause], true, &reg(), "<test>", &mut warnings);
        assert!(result.contains(": unknown;"));
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].severity, Severity::Warning);
    }

    #[test]
    fn test_emit_type_adt() {
        let constructors = vec![
            Constructor {
                name: "Some".to_string(),
                name_span: s(),
                fields: vec![tp("any", "value")],
                span: s(),
            },
            Constructor {
                name: "None".to_string(),
                name_span: s(),
                fields: vec![],
                span: s(),
            },
        ];
        let result = emit_type_dts("Option", &constructors, true, &reg());
        assert!(result.contains("tag: \"Some\"; value: unknown"));
        assert!(result.contains("tag: \"None\""));
        assert!(result.starts_with("export type Option ="));
    }

    #[test]
    fn test_emit_bind() {
        let result = emit_bind_dts("VERSION", &ann("string"), true, &reg());
        assert_eq!(result, "export const VERSION: string;\n");
    }

    #[test]
    fn test_emit_bind_lisp_case() {
        let result = emit_bind_dts("max-retries", &ann("number"), true, &reg());
        assert_eq!(result, "export const maxRetries: number;\n");
    }

    #[test]
    fn test_emit_bind_inferred_string() {
        let val = crate::ast::sexpr::SExpr::String { value: "hello".to_string(), span: s() };
        assert_eq!(emit_bind_inferred_dts("NAME", &val, true), "export const NAME: string;\n");
    }

    #[test]
    fn test_emit_bind_inferred_number() {
        let val = crate::ast::sexpr::SExpr::Number { value: 42.0, span: s() };
        assert_eq!(emit_bind_inferred_dts("COUNT", &val, true), "export const COUNT: number;\n");
    }

    #[test]
    fn test_emit_bind_inferred_boolean() {
        let val = crate::ast::sexpr::SExpr::Bool { value: true, span: s() };
        assert_eq!(emit_bind_inferred_dts("ENABLED", &val, true), "export const ENABLED: boolean;\n");
    }

    #[test]
    fn test_emit_bind_inferred_null_to_unknown() {
        let val = crate::ast::sexpr::SExpr::Null { span: s() };
        assert_eq!(emit_bind_inferred_dts("NOTHING", &val, true), "export const NOTHING: unknown;\n");
    }

    #[test]
    fn test_emit_bind_inferred_computed_to_unknown() {
        let val = crate::ast::sexpr::SExpr::List { values: vec![], span: s() };
        assert_eq!(emit_bind_inferred_dts("COMPUTED", &val, true), "export const COMPUTED: unknown;\n");
    }
}
