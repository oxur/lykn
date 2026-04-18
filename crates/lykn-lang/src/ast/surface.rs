use crate::ast::sexpr::SExpr;
use crate::reader::source_loc::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnnotation {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedParam {
    pub type_ann: TypeAnnotation,
    pub name: String,
    pub name_span: Span,
    pub default_value: Option<SExpr>,
    pub is_rest: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DestructuredField {
    Simple(TypedParam),
    Nested {
        alias_name: String,
        alias_name_span: Span,
        type_ann: TypeAnnotation,
        pattern: Box<ParamShape>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayParamElement {
    Typed(TypedParam),
    Rest(TypedParam),
    Skip(Span),
    Nested {
        pattern: Box<ParamShape>,
        span: Span,
    },
    NestedWithAlias {
        alias_name: String,
        alias_name_span: Span,
        type_ann: TypeAnnotation,
        pattern: Box<ParamShape>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamShape {
    Simple(TypedParam),
    DestructuredObject {
        fields: Vec<DestructuredField>,
        span: Span,
    },
    DestructuredArray {
        elements: Vec<ArrayParamElement>,
        span: Span,
    },
}

impl From<TypedParam> for ParamShape {
    fn from(tp: TypedParam) -> Self {
        ParamShape::Simple(tp)
    }
}

impl ParamShape {
    /// All typed params — flattened, recursing into nested patterns.
    pub fn typed_params(&self) -> Vec<&TypedParam> {
        match self {
            Self::Simple(tp) => vec![tp],
            Self::DestructuredObject { fields, .. } => fields
                .iter()
                .flat_map(|f| match f {
                    DestructuredField::Simple(tp) => vec![tp],
                    DestructuredField::Nested { pattern, .. } => pattern.typed_params(),
                })
                .collect(),
            Self::DestructuredArray { elements, .. } => elements
                .iter()
                .flat_map(|e| match e {
                    ArrayParamElement::Typed(tp) | ArrayParamElement::Rest(tp) => vec![tp],
                    ArrayParamElement::Skip(_) => vec![],
                    ArrayParamElement::Nested { pattern, .. }
                    | ArrayParamElement::NestedWithAlias { pattern, .. } => pattern.typed_params(),
                })
                .collect(),
        }
    }

    /// All bound names — for scope tracking.
    /// Includes alias names AND leaf names from nested patterns.
    pub fn bound_names(&self) -> Vec<&str> {
        match self {
            Self::Simple(tp) => vec![tp.name.as_str()],
            Self::DestructuredObject { fields, .. } => fields
                .iter()
                .flat_map(|f| match f {
                    DestructuredField::Simple(tp) => vec![tp.name.as_str()],
                    DestructuredField::Nested {
                        alias_name,
                        pattern,
                        ..
                    } => {
                        let mut names = vec![alias_name.as_str()];
                        names.extend(pattern.bound_names());
                        names
                    }
                })
                .collect(),
            Self::DestructuredArray { elements, .. } => elements
                .iter()
                .flat_map(|e| match e {
                    ArrayParamElement::Typed(tp) | ArrayParamElement::Rest(tp) => {
                        vec![tp.name.as_str()]
                    }
                    ArrayParamElement::Skip(_) => vec![],
                    ArrayParamElement::Nested { pattern, .. } => pattern.bound_names(),
                    ArrayParamElement::NestedWithAlias {
                        alias_name,
                        pattern,
                        ..
                    } => {
                        let mut names = vec![alias_name.as_str()];
                        names.extend(pattern.bound_names());
                        names
                    }
                })
                .collect(),
        }
    }

    /// The type keyword for dispatch purposes.
    pub fn dispatch_type(&self) -> &str {
        match self {
            Self::Simple(tp) => &tp.type_ann.name,
            Self::DestructuredObject { .. } => "object",
            Self::DestructuredArray { .. } => "array",
        }
    }

    /// The span of this param shape.
    pub fn span(&self) -> Span {
        match self {
            Self::Simple(tp) => tp.name_span,
            Self::DestructuredObject { span, .. } | Self::DestructuredArray { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FuncClause {
    pub args: Vec<ParamShape>,
    pub returns: Option<TypeAnnotation>,
    pub pre: Option<SExpr>,
    pub post: Option<SExpr>,
    pub body: Vec<SExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenfuncClause {
    pub args: Vec<ParamShape>,
    pub yields: Option<TypeAnnotation>,
    pub returns: Option<TypeAnnotation>,
    pub pre: Option<SExpr>,
    pub post: Option<SExpr>,
    pub body: Vec<SExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Constructor {
    pub name: String,
    pub name_span: Span,
    pub fields: Vec<TypedParam>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard(Span),
    Literal(SExpr),
    Binding {
        name: String,
        span: Span,
    },
    Constructor {
        name: String,
        name_span: Span,
        bindings: Vec<Pattern>,
        span: Span,
    },
    Obj {
        pairs: Vec<(String, Pattern)>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchClause {
    pub pattern: Pattern,
    pub guard: Option<SExpr>,
    pub body: Vec<SExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ThreadingStep {
    Bare(SExpr),
    Call(Vec<SExpr>),
}

/// A classified class member whose body expressions have been run through the
/// surface pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassMemberForm {
    /// Constructor or method with body as raw S-expressions.
    /// The emitter processes body through `emit_expr` which recursively
    /// expands surface forms at any nesting depth.
    Method {
        /// Everything before the body: head atom, name, params.
        prefix: Vec<SExpr>,
        /// Body expressions — raw S-expressions, expanded by the emitter.
        body: Vec<SExpr>,
        is_static: bool,
        is_async: bool,
    },
    /// Field declaration with optional initializer.
    Field {
        prefix: Vec<SExpr>,
        /// Optional initializer — raw S-expression, expanded by the emitter.
        init: Option<SExpr>,
        is_static: bool,
    },
    /// Unrecognized member — pass through as-is.
    Raw(SExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceForm {
    Bind {
        name: SExpr,
        type_ann: Option<TypeAnnotation>,
        value: SExpr,
        span: Span,
    },
    Func {
        name: String,
        name_span: Span,
        clauses: Vec<FuncClause>,
        span: Span,
    },
    Match {
        target: SExpr,
        clauses: Vec<MatchClause>,
        span: Span,
    },
    Type {
        name: String,
        name_span: Span,
        constructors: Vec<Constructor>,
        span: Span,
    },
    Obj {
        pairs: Vec<(String, SExpr)>,
        span: Span,
    },
    Cell {
        value: SExpr,
        span: Span,
    },
    Express {
        target: SExpr,
        span: Span,
    },
    Swap {
        target: SExpr,
        func: SExpr,
        extra_args: Vec<SExpr>,
        span: Span,
    },
    Reset {
        target: SExpr,
        value: SExpr,
        span: Span,
    },
    Set {
        target: SExpr,
        value: SExpr,
        span: Span,
    },
    // TODO: deprecate when surface/kernel syntaxes are separated; remove the release after that.
    SetSymbol {
        obj: SExpr,
        key: SExpr,
        value: SExpr,
        span: Span,
    },
    ThreadFirst {
        initial: SExpr,
        steps: Vec<ThreadingStep>,
        span: Span,
    },
    ThreadLast {
        initial: SExpr,
        steps: Vec<ThreadingStep>,
        span: Span,
    },
    SomeThreadFirst {
        initial: SExpr,
        steps: Vec<ThreadingStep>,
        span: Span,
    },
    SomeThreadLast {
        initial: SExpr,
        steps: Vec<ThreadingStep>,
        span: Span,
    },
    IfLet {
        pattern: Pattern,
        expr: SExpr,
        then_body: SExpr,
        else_body: Option<SExpr>,
        span: Span,
    },
    WhenLet {
        pattern: Pattern,
        expr: SExpr,
        body: Vec<SExpr>,
        span: Span,
    },
    Genfunc {
        name: String,
        name_span: Span,
        clauses: Vec<GenfuncClause>,
        span: Span,
    },
    Genfn {
        params: Vec<ParamShape>,
        yields: Option<TypeAnnotation>,
        body: Vec<SExpr>,
        span: Span,
    },
    Fn {
        params: Vec<ParamShape>,
        body: Vec<SExpr>,
        span: Span,
    },
    Lambda {
        params: Vec<ParamShape>,
        body: Vec<SExpr>,
        span: Span,
    },
    Conj {
        arr: SExpr,
        value: SExpr,
        span: Span,
    },
    Assoc {
        obj: SExpr,
        pairs: Vec<(String, SExpr)>,
        span: Span,
    },
    Dissoc {
        obj: SExpr,
        keys: Vec<String>,
        span: Span,
    },
    MacroDef {
        name: String,
        raw: SExpr,
        span: Span,
    },
    ImportMacros {
        raw: SExpr,
        span: Span,
    },
    Eq {
        args: Vec<SExpr>,
        span: Span,
    },
    NotEq {
        left: SExpr,
        right: SExpr,
        span: Span,
    },
    And {
        args: Vec<SExpr>,
        span: Span,
    },
    Or {
        args: Vec<SExpr>,
        span: Span,
    },
    Not {
        operand: SExpr,
        span: Span,
    },
    Async {
        inner: Box<SurfaceForm>,
        span: Span,
    },
    Export {
        inner: Box<SurfaceForm>,
        /// Remaining raw args after the inner form (e.g., nothing for
        /// `(export (func ...))`, but could be present for other export patterns)
        extra_args: Vec<SExpr>,
        span: Span,
    },
    Class {
        /// The class name atom.
        name: SExpr,
        /// The superclass list (possibly empty `()`).
        superclass: SExpr,
        /// Members with body expressions classified through the surface
        /// pipeline.
        members: Vec<ClassMemberForm>,
        span: Span,
    },
    ClassExpr {
        /// The superclass list (possibly empty `()`).
        superclass: SExpr,
        /// Members with body expressions classified through the surface
        /// pipeline.
        members: Vec<ClassMemberForm>,
        span: Span,
    },
    KernelPassthrough {
        raw: SExpr,
        span: Span,
    },
    FunctionCall {
        head: SExpr,
        args: Vec<SExpr>,
        span: Span,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s() -> Span {
        Span::default()
    }

    fn tp(name: &str, ty: &str) -> TypedParam {
        TypedParam {
            type_ann: TypeAnnotation {
                name: ty.to_string(),
                span: s(),
            },
            name: name.to_string(),
            name_span: s(),
            default_value: None,
            is_rest: false,
        }
    }

    // -- typed_params --

    #[test]
    fn typed_params_simple() {
        let shape = ParamShape::Simple(tp("x", "number"));
        let params = shape.typed_params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "x");
    }

    #[test]
    fn typed_params_destructured_object() {
        let shape = ParamShape::DestructuredObject {
            fields: vec![
                DestructuredField::Simple(tp("host", "string")),
                DestructuredField::Simple(tp("port", "number")),
            ],
            span: s(),
        };
        let params = shape.typed_params();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, "host");
        assert_eq!(params[1].name, "port");
    }

    #[test]
    fn typed_params_nested_object() {
        let inner = ParamShape::DestructuredObject {
            fields: vec![DestructuredField::Simple(tp("street", "string"))],
            span: s(),
        };
        let shape = ParamShape::DestructuredObject {
            fields: vec![
                DestructuredField::Simple(tp("name", "string")),
                DestructuredField::Nested {
                    alias_name: "addr".to_string(),
                    alias_name_span: s(),
                    type_ann: TypeAnnotation {
                        name: "object".to_string(),
                        span: s(),
                    },
                    pattern: Box::new(inner),
                    span: s(),
                },
            ],
            span: s(),
        };
        let params = shape.typed_params();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, "name");
        assert_eq!(params[1].name, "street");
    }

    #[test]
    fn typed_params_destructured_array() {
        let shape = ParamShape::DestructuredArray {
            elements: vec![
                ArrayParamElement::Typed(tp("first", "string")),
                ArrayParamElement::Skip(s()),
                ArrayParamElement::Rest(tp("tail", "any")),
            ],
            span: s(),
        };
        let params = shape.typed_params();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, "first");
        assert_eq!(params[1].name, "tail");
        assert!(params[1].is_rest == false);
    }

    // -- bound_names --

    #[test]
    fn bound_names_simple() {
        let shape = ParamShape::Simple(tp("x", "number"));
        assert_eq!(shape.bound_names(), vec!["x"]);
    }

    #[test]
    fn bound_names_object_with_nested() {
        let inner = ParamShape::DestructuredObject {
            fields: vec![DestructuredField::Simple(tp("street", "string"))],
            span: s(),
        };
        let shape = ParamShape::DestructuredObject {
            fields: vec![
                DestructuredField::Simple(tp("name", "string")),
                DestructuredField::Nested {
                    alias_name: "addr".to_string(),
                    alias_name_span: s(),
                    type_ann: TypeAnnotation {
                        name: "object".to_string(),
                        span: s(),
                    },
                    pattern: Box::new(inner),
                    span: s(),
                },
            ],
            span: s(),
        };
        let names = shape.bound_names();
        assert_eq!(names, vec!["name", "addr", "street"]);
    }

    #[test]
    fn bound_names_array_with_skip() {
        let shape = ParamShape::DestructuredArray {
            elements: vec![
                ArrayParamElement::Typed(tp("a", "number")),
                ArrayParamElement::Skip(s()),
                ArrayParamElement::Typed(tp("c", "number")),
            ],
            span: s(),
        };
        let names = shape.bound_names();
        assert_eq!(names, vec!["a", "c"]);
    }

    // -- dispatch_type --

    #[test]
    fn dispatch_type_simple() {
        assert_eq!(
            ParamShape::Simple(tp("x", "number")).dispatch_type(),
            "number"
        );
        assert_eq!(
            ParamShape::Simple(tp("s", "string")).dispatch_type(),
            "string"
        );
    }

    #[test]
    fn dispatch_type_destructured() {
        let obj = ParamShape::DestructuredObject {
            fields: vec![],
            span: s(),
        };
        assert_eq!(obj.dispatch_type(), "object");

        let arr = ParamShape::DestructuredArray {
            elements: vec![],
            span: s(),
        };
        assert_eq!(arr.dispatch_type(), "array");
    }

    // -- span --

    #[test]
    fn span_returns_correct_span() {
        let custom_span = Span {
            start: crate::reader::source_loc::SourceLoc { line: 5, column: 3 },
            end: crate::reader::source_loc::SourceLoc {
                line: 5,
                column: 13,
            },
        };
        let shape = ParamShape::Simple(TypedParam {
            type_ann: TypeAnnotation {
                name: "number".to_string(),
                span: s(),
            },
            name: "x".to_string(),
            name_span: custom_span,
            default_value: None,
            is_rest: false,
        });
        assert_eq!(shape.span(), custom_span);
    }

    // -- From<TypedParam> --

    #[test]
    fn from_typed_param() {
        let param = tp("x", "number");
        let shape: ParamShape = param.clone().into();
        assert_eq!(shape, ParamShape::Simple(param));
    }
}
