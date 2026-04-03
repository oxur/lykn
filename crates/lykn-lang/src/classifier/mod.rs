pub mod dispatch;
pub mod forms;

use crate::ast::sexpr::SExpr;
use crate::ast::surface::SurfaceForm;
use crate::diagnostics::Diagnostic;

pub fn classify(forms: &[SExpr]) -> Result<Vec<SurfaceForm>, Vec<Diagnostic>> {
    let mut results = Vec::new();
    let mut errors = Vec::new();

    for form in forms {
        match classify_expr(form) {
            Ok(sf) => results.push(sf),
            Err(diag) => errors.push(diag),
        }
    }

    if errors.is_empty() {
        Ok(results)
    } else {
        Err(errors)
    }
}

pub fn classify_expr(expr: &SExpr) -> Result<SurfaceForm, Diagnostic> {
    forms::classify_form(expr)
}
