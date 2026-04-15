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
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayParamElement {
    Typed(TypedParam),
    Rest(TypedParam),
    Skip(Span),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamShape {
    Simple(TypedParam),
    DestructuredObject {
        fields: Vec<TypedParam>,
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
    /// All typed params — flattened.
    pub fn typed_params(&self) -> Vec<&TypedParam> {
        match self {
            Self::Simple(tp) => vec![tp],
            Self::DestructuredObject { fields, .. } => fields.iter().collect(),
            Self::DestructuredArray { elements, .. } => elements
                .iter()
                .filter_map(|e| match e {
                    ArrayParamElement::Typed(tp) | ArrayParamElement::Rest(tp) => Some(tp),
                    ArrayParamElement::Skip(_) => None,
                })
                .collect(),
        }
    }

    /// All bound names — for scope tracking.
    pub fn bound_names(&self) -> Vec<&str> {
        self.typed_params()
            .iter()
            .map(|tp| tp.name.as_str())
            .collect()
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
