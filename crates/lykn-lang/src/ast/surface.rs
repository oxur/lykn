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
