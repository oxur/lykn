pub mod serializer;

use crate::reader::source_loc::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub suggestion: Option<String>,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        write!(
            f,
            "{}:{}: {}: {}",
            self.span.start.line, self.span.start.column, sev, self.message
        )?;
        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\n  suggestion: {suggestion}")?;
        }
        Ok(())
    }
}
