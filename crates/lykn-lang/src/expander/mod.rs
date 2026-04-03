//! Macro expander — three-pass pipeline for user-defined macros.
//!
//! The expander transforms a list of parsed S-expressions by:
//!
//! 1. **Pass 0** ([`pass0`]): Processing `(import-macros ...)` directives,
//!    loading and compiling macros from external modules.
//! 2. **Pass 1** ([`pass1`]): Compiling local `(macro ...)` definitions to
//!    JavaScript function bodies via a Deno subprocess.
//! 3. **Pass 2** ([`pass2`]): Walking the remaining forms and expanding every
//!    macro invocation, desugaring built-in sugar forms, and recursively
//!    expanding until a fixed point is reached.
//!
//! The primary entry point is [`expand`], which orchestrates all three passes.
//! If the input contains no macro definitions or import directives, the forms
//! are returned unchanged without spawning a Deno subprocess.

pub mod cache;
pub mod deno;
pub mod env;
pub mod pass0;
pub mod pass1;
pub mod pass2;

use std::collections::HashMap;
use std::path::Path;

use crate::ast::sexpr::SExpr;
use crate::error::LyknError;

/// A compiled macro — holds the macro's name and the JavaScript function body
/// that will be evaluated by the Deno subprocess to perform expansion.
#[derive(Debug, Clone)]
pub struct CompiledMacro {
    /// The macro's name as it appears in source code.
    pub name: String,
    /// The compiled JavaScript function body string.
    pub js_body: String,
}

/// Macro environment — maps macro names to their compiled representations.
pub type MacroEnv = HashMap<String, CompiledMacro>;

/// Expand all macros in a list of S-expression forms.
///
/// This is the main entry point for the expander. It runs the three-pass
/// pipeline:
///
/// 1. Process `import-macros` directives (load external macro modules).
/// 2. Compile local `macro` definitions to JavaScript.
/// 3. Expand all macro invocations in the remaining forms.
///
/// If `file_path` is provided, it is used to resolve relative imports and
/// detect circular module dependencies.
///
/// If no macros or import directives are present, the forms are returned
/// unchanged without spawning a subprocess.
pub fn expand(forms: Vec<SExpr>, file_path: Option<&Path>) -> Result<Vec<SExpr>, LyknError> {
    // Quick scan: do any forms contain macro definitions or import directives?
    let has_macros = forms.iter().any(|f| {
        if let SExpr::List { values, .. } = f
            && let Some(SExpr::Atom { value, .. }) = values.first()
        {
            return value == "macro" || value == "import-macros";
        }
        false
    });

    // If there are no macro-related forms at all, skip subprocess creation.
    if !has_macros {
        return Ok(forms);
    }

    let mut env = MacroEnv::new();
    let mut cache = cache::ModuleCache::new();

    // Spawn the Deno subprocess for compilation and evaluation.
    let mut deno = deno::DenoSubprocess::spawn()?;

    // Build the compilation stack for circular dependency detection.
    let compilation_stack = if let Some(p) = file_path {
        vec![p.to_path_buf()]
    } else {
        vec![]
    };

    // Pass 0: Process import-macros directives.
    let forms = pass0::process_import_macros(
        forms,
        file_path,
        &mut deno,
        &mut cache,
        &compilation_stack,
        &mut env,
    )?;

    // Pass 1: Compile local macro definitions.
    let forms = pass1::compile_local_macros(forms, &mut deno, &mut env)?;

    // Pass 2: Expand all macro invocations.
    let forms = pass2::expand_all(forms, &mut deno, &env)?;

    Ok(forms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::source_loc::Span;

    fn s() -> Span {
        Span::default()
    }

    fn atom(name: &str) -> SExpr {
        SExpr::Atom {
            value: name.to_string(),
            span: s(),
        }
    }

    fn num(n: f64) -> SExpr {
        SExpr::Number {
            value: n,
            span: s(),
        }
    }

    fn list(vals: Vec<SExpr>) -> SExpr {
        SExpr::List {
            values: vals,
            span: s(),
        }
    }

    #[test]
    fn test_expand_empty() {
        let result = expand(vec![], None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_expand_no_macros_returns_unchanged() {
        let forms = vec![
            list(vec![atom("define"), atom("x"), num(1.0)]),
            list(vec![atom("+"), atom("x"), num(2.0)]),
        ];
        let result = expand(forms.clone(), None).unwrap();
        assert_eq!(result, forms);
    }

    #[test]
    fn test_expand_single_atom() {
        let forms = vec![atom("hello")];
        let result = expand(forms.clone(), None).unwrap();
        assert_eq!(result, forms);
    }

    #[test]
    fn test_expand_nested_no_macros() {
        let forms = vec![list(vec![
            atom("if"),
            list(vec![atom(">"), atom("x"), num(0.0)]),
            list(vec![atom("console:log"), atom("x")]),
        ])];
        let result = expand(forms.clone(), None).unwrap();
        assert_eq!(result, forms);
    }

    #[test]
    fn test_expand_mixed_types_no_macros() {
        let forms = vec![
            SExpr::String {
                value: "hello".to_string(),
                span: s(),
            },
            SExpr::Bool {
                value: true,
                span: s(),
            },
            SExpr::Null { span: s() },
            SExpr::Keyword {
                value: "key".to_string(),
                span: s(),
            },
            num(42.0),
        ];
        let result = expand(forms.clone(), None).unwrap();
        assert_eq!(result, forms);
    }

    #[test]
    fn test_has_macros_detection_macro() {
        let forms = vec![
            list(vec![
                atom("macro"),
                atom("when"),
                list(vec![atom("test")]),
                atom("body"),
            ]),
            list(vec![atom("when"), atom("x")]),
        ];
        // This should detect macros and attempt to spawn deno.
        // If deno is not available, it returns an error — which is correct
        // behavior.
        let result = expand(forms, None);
        // We just verify it doesn't panic. If deno is available, it might
        // succeed or fail on the JS side; if not, it returns an Io or Read
        // error.
        let _ = result;
    }

    #[test]
    fn test_has_macros_detection_import() {
        let forms = vec![list(vec![
            atom("import-macros"),
            SExpr::String {
                value: "./macros.lykn".to_string(),
                span: s(),
            },
            list(vec![atom("when")]),
        ])];
        let result = expand(forms, None);
        // Same as above — we verify no panic.
        let _ = result;
    }

    #[test]
    fn test_compiled_macro_debug() {
        let m = CompiledMacro {
            name: "when".to_string(),
            js_body: "return args;".to_string(),
        };
        let debug = format!("{m:?}");
        assert!(debug.contains("when"));
        assert!(debug.contains("return args;"));
    }

    #[test]
    fn test_compiled_macro_clone() {
        let m = CompiledMacro {
            name: "when".to_string(),
            js_body: "return args;".to_string(),
        };
        let m2 = m.clone();
        assert_eq!(m.name, m2.name);
        assert_eq!(m.js_body, m2.js_body);
    }
}
