use super::gensym::EmitterGensym;

/// Describes how the result of an emitted expression will be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExprContext {
    /// Value is unused (top-level form, statement body).
    Statement,
    /// Value is used (bind RHS, function argument).
    Value,
    /// Last expression in a function body with `:returns` — needs `return`.
    Tail,
}

/// Mutable state threaded through the emitter.
#[derive(Debug)]
pub struct EmitterContext {
    /// How the current expression's result will be consumed.
    pub expr_context: ExprContext,
    /// Whether to omit type-check assertions and contract checks.
    pub strip_assertions: bool,
    /// Deterministic name generator for intermediate bindings.
    pub gensym: EmitterGensym,
    /// Whether we're currently inside a class body (enables `assign`).
    pub in_class_body: bool,
}

impl EmitterContext {
    pub fn new(strip_assertions: bool) -> Self {
        Self {
            expr_context: ExprContext::Statement,
            gensym: EmitterGensym::new(),
            strip_assertions,
            in_class_body: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_context_defaults() {
        let ctx = EmitterContext::new(false);
        assert_eq!(ctx.expr_context, ExprContext::Statement);
        assert!(!ctx.strip_assertions);
    }

    #[test]
    fn test_strip_assertions_flag() {
        let ctx = EmitterContext::new(true);
        assert!(ctx.strip_assertions);
    }

    #[test]
    fn test_expr_context_equality() {
        assert_eq!(ExprContext::Statement, ExprContext::Statement);
        assert_ne!(ExprContext::Statement, ExprContext::Value);
        assert_ne!(ExprContext::Value, ExprContext::Tail);
    }
}
