use crate::reader::source_loc::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum SExpr {
    Atom {
        value: String,
        span: Span,
    },
    Keyword {
        value: String,
        span: Span,
    },
    String {
        value: String,
        span: Span,
    },
    Number {
        value: f64,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    Null {
        span: Span,
    },
    List {
        values: Vec<SExpr>,
        span: Span,
    },
    Cons {
        car: Box<SExpr>,
        cdr: Box<SExpr>,
        span: Span,
    },
}

impl SExpr {
    pub fn span(&self) -> Span {
        match self {
            SExpr::Atom { span, .. }
            | SExpr::Keyword { span, .. }
            | SExpr::String { span, .. }
            | SExpr::Number { span, .. }
            | SExpr::Bool { span, .. }
            | SExpr::Null { span }
            | SExpr::List { span, .. }
            | SExpr::Cons { span, .. } => *span,
        }
    }

    pub fn is_atom(&self) -> bool {
        matches!(self, SExpr::Atom { .. })
    }

    pub fn is_keyword(&self) -> bool {
        matches!(self, SExpr::Keyword { .. })
    }

    pub fn is_list(&self) -> bool {
        matches!(self, SExpr::List { .. })
    }

    pub fn as_atom(&self) -> Option<&str> {
        match self {
            SExpr::Atom { value, .. } => Some(value),
            _ => None,
        }
    }

    pub fn as_keyword(&self) -> Option<&str> {
        match self {
            SExpr::Keyword { value, .. } => Some(value),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[SExpr]> {
        match self {
            SExpr::List { values, .. } => Some(values),
            _ => None,
        }
    }
}
