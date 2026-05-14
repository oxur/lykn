//! Pure-Rust JavaScript code generation from kernel `SExpr` forms.
//!
//! This module replaces the previous JSON-serialise-then-Deno pipeline with a
//! direct `SExpr` → JavaScript-text emitter. Zero external dependencies.
//!
//! # Usage
//!
//! ```rust
//! use lykn_lang::codegen::emit_module_js;
//! use lykn_lang::ast::sexpr::SExpr;
//! use lykn_lang::reader::source_loc::Span;
//!
//! let forms = vec![
//!     SExpr::List {
//!         values: vec![
//!             SExpr::Atom { value: "const".into(), span: Span::default() },
//!             SExpr::Atom { value: "x".into(), span: Span::default() },
//!             SExpr::Number { value: 1.0, span: Span::default() },
//!         ],
//!         span: Span::default(),
//!     },
//! ];
//! let js = emit_module_js(&forms);
//! assert_eq!(js, "const x = 1;\n");
//! ```

pub mod emit;
pub mod format;
pub mod icu;
pub mod names;
pub mod precedence;

use crate::ast::sexpr::SExpr;

use emit::emit_statement;
use format::JsWriter;

/// Emit a module (list of kernel forms) as JavaScript source text.
///
/// Each top-level form is emitted as a statement. The resulting string is
/// ready to be written to a `.js` file.
pub fn emit_module_js(forms: &[SExpr]) -> String {
    let mut w = JsWriter::new();
    for form in forms {
        emit_statement(&mut w, form);
    }
    w.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::source_loc::Span;

    fn s() -> Span {
        Span::default()
    }

    fn atom(v: &str) -> SExpr {
        SExpr::Atom {
            value: v.to_string(),
            span: s(),
        }
    }

    fn num(v: f64) -> SExpr {
        SExpr::Number {
            value: v,
            span: s(),
        }
    }

    fn str_lit(v: &str) -> SExpr {
        SExpr::String {
            value: v.to_string(),
            span: s(),
        }
    }

    fn list(items: Vec<SExpr>) -> SExpr {
        SExpr::List {
            values: items,
            span: s(),
        }
    }

    #[test]
    fn test_emit_module_js_empty() {
        assert_eq!(emit_module_js(&[]), "");
    }

    #[test]
    fn test_emit_module_js_single_declaration() {
        let forms = vec![list(vec![atom("const"), atom("x"), num(1.0)])];
        assert_eq!(emit_module_js(&forms), "const x = 1;\n");
    }

    #[test]
    fn test_emit_module_js_multi_form_program() {
        let import = list(vec![
            atom("import"),
            str_lit("./utils.js"),
            list(vec![atom("add"), atom("sub")]),
        ]);
        let constant = list(vec![
            atom("const"),
            atom("result"),
            list(vec![atom("add"), num(1.0), num(2.0)]),
        ]);
        let export = list(vec![atom("export"), atom("default"), atom("result")]);

        let forms = vec![import, constant, export];
        let js = emit_module_js(&forms);

        assert!(js.contains("import {add, sub} from \"./utils.js\";"));
        assert!(js.contains("const result = add(1, 2);"));
        assert!(js.contains("export default result;"));
    }

    #[test]
    fn test_emit_module_js_function_and_call() {
        let func = list(vec![
            atom("function"),
            atom("greet"),
            list(vec![atom("name")]),
            list(vec![
                atom("return"),
                list(vec![
                    atom("template"),
                    str_lit("Hello, "),
                    atom("name"),
                    str_lit("!"),
                ]),
            ]),
        ]);
        let call = list(vec![atom("greet"), str_lit("world")]);

        let forms = vec![func, call];
        let js = emit_module_js(&forms);

        assert!(js.contains("function greet(name)"));
        assert!(js.contains("return `Hello, ${name}!`"));
        assert!(js.contains("greet(\"world\");"));
    }

    #[test]
    fn test_emit_module_js_class() {
        let ctor = list(vec![
            atom("constructor"),
            list(vec![atom("x")]),
            list(vec![atom("="), atom("this:x"), atom("x")]),
        ]);
        let method = list(vec![
            atom("get-x"),
            list(vec![]),
            list(vec![atom("return"), atom("this:x")]),
        ]);
        let cls = list(vec![
            atom("class"),
            atom("Point"),
            list(vec![]),
            ctor,
            method,
        ]);

        let forms = vec![cls];
        let js = emit_module_js(&forms);

        assert!(js.contains("class Point {"));
        assert!(js.contains("constructor(x)"));
        assert!(js.contains("this.x = x"));
        assert!(js.contains("getX()"));
        assert!(js.contains("return this.x"));
    }
}
