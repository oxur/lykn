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
pub struct FuncClause {
    pub args: Vec<TypedParam>,
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
        params: Vec<TypedParam>,
        body: Vec<SExpr>,
        span: Span,
    },
    Lambda {
        params: Vec<TypedParam>,
        body: Vec<SExpr>,
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
