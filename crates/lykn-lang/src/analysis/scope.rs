//! Scope tracker with binding introduction, usage tracking, and detection
//! of unused bindings and shadowing.

use std::collections::HashMap;

use crate::diagnostics::{Diagnostic, Severity};
use crate::reader::source_loc::Span;

/// A single binding within a scope.
#[derive(Debug, Clone)]
pub struct Binding {
    pub name: String,
    pub span: Span,
    pub used: bool,
    pub exported: bool,
    pub is_constructor: bool,
}

#[derive(Debug)]
struct ScopeLevel {
    bindings: HashMap<String, Binding>,
}

/// Tracks lexical scopes, detecting unused bindings and shadowing.
#[derive(Debug)]
pub struct ScopeTracker {
    stack: Vec<ScopeLevel>,
    diagnostics: Vec<Diagnostic>,
}

impl Default for ScopeTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ScopeTracker {
    /// Create a new scope tracker with a single top-level scope.
    pub fn new() -> Self {
        Self {
            stack: vec![ScopeLevel {
                bindings: HashMap::new(),
            }],
            diagnostics: Vec::new(),
        }
    }

    /// Push a new lexical scope onto the stack.
    pub fn enter_scope(&mut self) {
        self.stack.push(ScopeLevel {
            bindings: HashMap::new(),
        });
    }

    /// Pop the innermost scope, emitting warnings for unused bindings.
    pub fn exit_scope(&mut self) {
        if let Some(level) = self.stack.pop() {
            for binding in level.bindings.values() {
                if !binding.used
                    && !binding.exported
                    && !binding.is_constructor
                    && !binding.name.starts_with('_')
                {
                    self.diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!("unused binding '{}'", binding.name),
                        span: binding.span,
                        suggestion: Some(format!("prefix with underscore: _{}", binding.name)),
                    });
                }
            }
        }
    }

    /// Introduce a new binding in the current scope. Emits a shadowing
    /// warning if the name already exists in an outer scope.
    pub fn introduce(&mut self, name: &str, span: Span, exported: bool, is_constructor: bool) {
        if self.lookup(name).is_some() {
            self.diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                message: format!("binding '{name}' shadows an outer binding"),
                span,
                suggestion: None,
            });
        }
        if let Some(level) = self.stack.last_mut() {
            level.bindings.insert(
                name.to_string(),
                Binding {
                    name: name.to_string(),
                    span,
                    used: false,
                    exported,
                    is_constructor,
                },
            );
        }
    }

    /// Record a reference to a binding. Walks the scope stack from innermost
    /// to outermost, marking the first match as used.
    pub fn reference(&mut self, name: &str, _span: Span) {
        for level in self.stack.iter_mut().rev() {
            if let Some(binding) = level.bindings.get_mut(name) {
                binding.used = true;
                return;
            }
        }
        // Not found -- could be global, built-in, or undefined.
        // We do not emit an error here; undefined detection is form-specific.
    }

    /// Look up a binding by name, walking from innermost to outermost scope.
    fn lookup(&self, name: &str) -> Option<&Binding> {
        for level in self.stack.iter().rev() {
            if let Some(binding) = level.bindings.get(name) {
                return Some(binding);
            }
        }
        None
    }

    /// Flush all remaining scopes and return accumulated diagnostics.
    pub fn collect_diagnostics(&mut self) -> Vec<Diagnostic> {
        // Flush remaining inner scopes
        while self.stack.len() > 1 {
            self.exit_scope();
        }
        // Check top-level scope
        if let Some(level) = self.stack.last() {
            for binding in level.bindings.values() {
                if !binding.used
                    && !binding.exported
                    && !binding.is_constructor
                    && !binding.name.starts_with('_')
                {
                    self.diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!("unused binding '{}'", binding.name),
                        span: binding.span,
                        suggestion: Some(format!("prefix with underscore: _{}", binding.name)),
                    });
                }
            }
        }
        std::mem::take(&mut self.diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::source_loc::Span;

    fn span() -> Span {
        Span::default()
    }

    #[test]
    fn test_introduce_and_reference_no_warning() {
        let mut tracker = ScopeTracker::new();
        tracker.introduce("x", span(), false, false);
        tracker.reference("x", span());
        let diags = tracker.collect_diagnostics();
        let unused: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("unused"))
            .collect();
        assert!(unused.is_empty());
    }

    #[test]
    fn test_introduce_without_reference_warns_unused() {
        let mut tracker = ScopeTracker::new();
        tracker.introduce("x", span(), false, false);
        let diags = tracker.collect_diagnostics();
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unused binding 'x'"))
        );
    }

    #[test]
    fn test_underscore_prefix_suppresses_unused() {
        let mut tracker = ScopeTracker::new();
        tracker.introduce("_x", span(), false, false);
        let diags = tracker.collect_diagnostics();
        let unused: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("unused"))
            .collect();
        assert!(unused.is_empty());
    }

    #[test]
    fn test_exported_binding_suppresses_unused() {
        let mut tracker = ScopeTracker::new();
        tracker.introduce("x", span(), true, false);
        let diags = tracker.collect_diagnostics();
        let unused: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("unused"))
            .collect();
        assert!(unused.is_empty());
    }

    #[test]
    fn test_constructor_binding_suppresses_unused() {
        let mut tracker = ScopeTracker::new();
        tracker.introduce("Some", span(), false, true);
        let diags = tracker.collect_diagnostics();
        let unused: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("unused"))
            .collect();
        assert!(unused.is_empty());
    }

    #[test]
    fn test_shadowing_warning() {
        let mut tracker = ScopeTracker::new();
        tracker.introduce("x", span(), false, false);
        tracker.enter_scope();
        tracker.introduce("x", span(), false, false);
        let diags = tracker.collect_diagnostics();
        assert!(diags.iter().any(|d| d.message.contains("shadows")));
    }

    #[test]
    fn test_inner_scope_reference_marks_outer_used() {
        let mut tracker = ScopeTracker::new();
        tracker.introduce("x", span(), false, false);
        tracker.enter_scope();
        tracker.reference("x", span());
        tracker.exit_scope();
        let diags = tracker.collect_diagnostics();
        let unused: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("unused binding 'x'"))
            .collect();
        assert!(unused.is_empty());
    }

    #[test]
    fn test_exit_scope_emits_unused_for_inner() {
        let mut tracker = ScopeTracker::new();
        tracker.enter_scope();
        tracker.introduce("y", span(), false, false);
        tracker.exit_scope();
        let diags = tracker.collect_diagnostics();
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unused binding 'y'"))
        );
    }

    #[test]
    fn test_reference_nonexistent_no_crash() {
        let mut tracker = ScopeTracker::new();
        // Should not panic or emit diagnostics
        tracker.reference("nonexistent", span());
        let diags = tracker.collect_diagnostics();
        assert!(diags.is_empty());
    }

    #[test]
    fn test_multiple_scopes_independent() {
        let mut tracker = ScopeTracker::new();
        tracker.enter_scope();
        tracker.introduce("a", span(), false, false);
        tracker.reference("a", span());
        tracker.exit_scope();

        tracker.enter_scope();
        tracker.introduce("b", span(), false, false);
        // b is not referenced
        tracker.exit_scope();

        let diags = tracker.collect_diagnostics();
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unused binding 'b'"))
        );
        assert!(
            !diags
                .iter()
                .any(|d| d.message.contains("unused binding 'a'"))
        );
    }

    #[test]
    fn test_collect_flushes_remaining_scopes() {
        let mut tracker = ScopeTracker::new();
        tracker.enter_scope();
        tracker.introduce("leaked", span(), false, false);
        // Don't call exit_scope -- collect_diagnostics should handle it
        let diags = tracker.collect_diagnostics();
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unused binding 'leaked'"))
        );
    }
}
