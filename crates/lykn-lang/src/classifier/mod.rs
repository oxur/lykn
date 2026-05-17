pub mod dispatch;
pub mod forms;

use crate::ast::sexpr::SExpr;
use crate::ast::surface::SurfaceForm;
use crate::diagnostics::Diagnostic;

/// Options for the classifier. Extensible for future phases (e.g.,
/// file_kind for extension gating in DD-58 Phase 4).
#[derive(Debug, Clone, Copy, Default)]
pub struct ClassifierOptions {
    /// When true, enforce DD-58's closed-namespace rule: kernel-only
    /// forms without the `kernel:` prefix produce a diagnostic.
    /// Default: false (lax mode — existing behaviour preserved).
    pub strict: bool,
    /// When true, enforce kernel-only classification: surface forms
    /// at the top level produce a diagnostic. Used for `.lyk` files.
    /// Default: false.
    pub kernel_only: bool,
}

pub fn classify(forms: &[SExpr]) -> Result<Vec<SurfaceForm>, Vec<Diagnostic>> {
    classify_with_options(forms, ClassifierOptions::default())
}

pub fn classify_with_options(
    forms: &[SExpr],
    opts: ClassifierOptions,
) -> Result<Vec<SurfaceForm>, Vec<Diagnostic>> {
    let mut results = Vec::new();
    let mut errors = Vec::new();

    for form in forms {
        let result = if opts.kernel_only {
            classify_expr_kernel_only(form)
        } else if opts.strict {
            classify_expr_strict(form)
        } else {
            classify_expr(form)
        };
        match result {
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

pub fn classify_expr_strict(expr: &SExpr) -> Result<SurfaceForm, Diagnostic> {
    forms::classify_form_strict(expr)
}

pub fn classify_expr_kernel_only(expr: &SExpr) -> Result<SurfaceForm, Diagnostic> {
    forms::classify_form_kernel_only(expr)
}
