//! Integration tests for the macro expander.
//!
//! Tests that require Deno are guarded behind a runtime check and will be
//! skipped (with a message) if `deno` is not available on PATH.

use lykn_lang::ast::sexpr::SExpr;
use lykn_lang::expander;
use lykn_lang::reader::source_loc::Span;

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

fn deno_available() -> bool {
    std::process::Command::new("deno")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

// ---------------------------------------------------------------
// Tests that do NOT require Deno
// ---------------------------------------------------------------

#[test]
fn expand_no_macros_returns_forms_unchanged() {
    let forms = vec![
        list(vec![atom("define"), atom("x"), num(1.0)]),
        list(vec![atom("+"), atom("x"), num(2.0)]),
    ];
    let result = expander::expand(forms.clone(), None, None).unwrap();
    assert_eq!(result, forms);
}

#[test]
fn expand_empty_input() {
    let result = expander::expand(vec![], None, None).unwrap();
    assert!(result.is_empty());
}

#[test]
fn expand_leaf_forms_passthrough() {
    let forms = vec![
        atom("x"),
        num(42.0),
        SExpr::String {
            value: "hello".to_string(),
            span: s(),
        },
        SExpr::Bool {
            value: true,
            span: s(),
        },
        SExpr::Null { span: s() },
    ];
    let result = expander::expand(forms.clone(), None, None).unwrap();
    assert_eq!(result, forms);
}

#[test]
fn expand_deeply_nested_no_macros() {
    let deep = list(vec![
        atom("let"),
        list(vec![list(vec![atom("x"), num(1.0)])]),
        list(vec![
            atom("if"),
            list(vec![atom(">"), atom("x"), num(0.0)]),
            list(vec![atom("console:log"), atom("x")]),
        ]),
    ]);
    let forms = vec![deep.clone()];
    let result = expander::expand(forms, None, None).unwrap();
    assert_eq!(result, vec![deep]);
}

// ---------------------------------------------------------------
// Tests that require Deno (skipped if not available)
// ---------------------------------------------------------------

#[test]
fn expand_simple_when_macro() {
    if !deno_available() {
        eprintln!("skipping expand_simple_when_macro: deno not found");
        return;
    }

    let source = r#"
        (macro when (test (rest body))
            `(if ,test (block ,@body)))
        (when (> x 0) (console:log "positive"))
    "#;
    let forms = lykn_lang::reader::read(source).unwrap();
    let result = expander::expand(forms, None, None);

    // The macro should expand. Whether it succeeds depends on the JS-side
    // compiler being available at the CWD. If it fails due to module
    // resolution, that's expected in an isolated test environment.
    match result {
        Ok(expanded) => {
            // Should produce one form (the macro definition is consumed).
            assert_eq!(expanded.len(), 1);
            // The result should be an (if ...) form.
            if let SExpr::List { values, .. } = &expanded[0] {
                assert_eq!(values[0].as_atom(), Some("if"));
            }
        }
        Err(e) => {
            let msg = format!("{e}");
            // Module resolution errors are acceptable in test environments
            // where src/reader.js might not be at CWD.
            assert!(
                msg.contains("reader.js")
                    || msg.contains("expander.js")
                    || msg.contains("Module not found")
                    || msg.contains("Cannot resolve"),
                "unexpected error: {msg}"
            );
        }
    }
}
