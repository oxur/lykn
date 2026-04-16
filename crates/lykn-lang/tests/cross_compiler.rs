//! Cross-compiler integration tests.
//!
//! Compares Rust emitter JSON output with JS surface compiler output.
//! Requires `deno` to be installed. Tests are skipped if deno is not available.

use std::process::Command;

use lykn_lang::analysis::type_registry::TypeRegistry;
use lykn_lang::ast::surface::SurfaceForm;
use lykn_lang::classifier;
use lykn_lang::emitter;
use lykn_lang::reader;

fn deno_available() -> bool {
    Command::new("deno").arg("--version").output().is_ok()
}

/// Run lykn source through the Rust pipeline: read → classify → emit → JSON
fn rust_compile(source: &str) -> String {
    let sexprs = reader::read(source).expect("Rust reader failed");
    let forms = classifier::classify(&sexprs).expect("Rust classifier failed");

    // Register types from Type forms
    let mut registry = TypeRegistry::default();
    lykn_lang::analysis::prelude::register_prelude_types(&mut registry);
    for form in &forms {
        if let SurfaceForm::Type {
            name,
            name_span: _,
            constructors,
            span,
        } = form
        {
            use lykn_lang::analysis::type_registry::{ConstructorDef, FieldDef, TypeDef};
            let ctors = constructors
                .iter()
                .map(|c| ConstructorDef {
                    name: c.name.clone(),
                    fields: c
                        .fields
                        .iter()
                        .map(|f| FieldDef {
                            name: f.name.clone(),
                            type_keyword: f.type_ann.name.clone(),
                        })
                        .collect(),
                    owning_type: name.clone(),
                    span: c.span,
                })
                .collect();
            let _ = registry.register_type(TypeDef {
                name: name.clone(),
                module_path: None,
                constructors: ctors,
                is_blessed: false,
                span: *span,
            });
        }
    }

    let kernel = emitter::emit(&forms, &registry, false);
    emitter::json::emit_module_json(&kernel)
}

/// Run lykn source through the JS pipeline via deno
fn js_compile(source: &str) -> String {
    let script = format!(
        r#"
import {{ read }} from "./packages/lang/reader.js";
import {{ expand, resetMacros, resetGensym }} from "./packages/lang/expander.js";
resetMacros();
resetGensym();
const source = `{source}`;
const expanded = expand(read(source));
// Serialize expanded AST to JSON
function toJson(node) {{
    if (!node) return null;
    if (node.type === 'atom') return {{ type: "atom", value: node.value }};
    if (node.type === 'keyword') return {{ type: "string", value: node.value }};
    if (node.type === 'string') return {{ type: "string", value: node.value }};
    if (node.type === 'number') return {{ type: "number", value: node.value }};
    if (node.type === 'list') return {{ type: "list", values: node.values.map(toJson) }};
    if (typeof node === 'boolean') return {{ type: "atom", value: String(node) }};
    return null;
}}
const json = expanded.filter(x => x !== null).map(toJson);
console.log(JSON.stringify(json, null, 2));
"#,
        source = source.replace('`', "\\`").replace('$', "\\$")
    );

    let output = Command::new("deno")
        .arg("eval")
        .arg("--config")
        .arg("project.json")
        .arg("--ext=js")
        .arg(&script)
        .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/../..")
        .output()
        .expect("failed to run deno");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("JS compilation failed:\n{stderr}");
    }

    String::from_utf8(output.stdout)
        .expect("invalid utf8")
        .trim()
        .to_string()
}

/// Normalize JSON for comparison (strip whitespace differences)
fn normalize_json(json: &str) -> serde_json::Value {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("Invalid JSON: {e}\n{json}"))
}

macro_rules! cross_test {
    ($name:ident, $source:expr) => {
        #[test]
        fn $name() {
            if !deno_available() {
                eprintln!("skipping cross-compiler test: deno not available");
                return;
            }
            let rust_json = rust_compile($source);
            let js_json = js_compile($source);
            let rust_val = normalize_json(&rust_json);
            let js_val = normalize_json(&js_json);
            assert_eq!(
                rust_val, js_val,
                "Rust and JS compilers produced different output!\nRust:\n{rust_json}\n\nJS:\n{js_json}"
            );
        }
    };
}

// Simple forms where both pipelines produce identical kernel output
cross_test!(cross_bind_simple, "(bind x 42)");
cross_test!(cross_bind_string, r#"(bind name "Duncan")"#);
cross_test!(cross_bind_typed, "(bind :number age 42)");
cross_test!(cross_obj_simple, r#"(obj :name "Duncan" :age 42)"#);
cross_test!(cross_cell, "(cell 0)");
cross_test!(cross_thread_first, "(-> x f g)");
cross_test!(cross_thread_last, "(->> x (f a) (g b))");
// js: namespace interop forms
cross_test!(cross_js_eq, "(js:eq a b)");
cross_test!(cross_js_typeof, "(js:typeof x)");
cross_test!(cross_js_eval, r#"(js:eval "1 + 2")"#);
cross_test!(cross_js_call, r#"(js:call console:log "hello")"#);
cross_test!(cross_js_bind, "(js:bind obj:method obj)");
// DD-22: Surface equality and logical operators
cross_test!(cross_eq_binary, "(= a b)");
cross_test!(cross_eq_variadic, "(= a b c)");
cross_test!(cross_neq_binary, "(!= a b)");
cross_test!(cross_and_binary, "(and a b)");
cross_test!(cross_or_binary, "(or a b)");
cross_test!(cross_not_unary, "(not x)");
cross_test!(cross_and_variadic, "(and a b c d)");
cross_test!(cross_or_variadic, "(or a b c d)");
cross_test!(cross_not_nested, "(not (not x))");
// DD: fn typed return
cross_test!(cross_fn_typed_return, "(bind f (fn (:number x) (* x 2)))");
