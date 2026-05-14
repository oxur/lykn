//! Cross-compiler equivalence tests for DD-55 ICU MessageFormat support.
//!
//! For each test case:
//!   1. Compile lykn source through the Rust pipeline → JS source A.
//!   2. Compile the same source through the JS pipeline (via Deno) → JS source B.
//!   3. Evaluate both with sample inputs in a Deno subprocess.
//!   4. Assert the two runtime outputs are equal AND match the expected value.
//!
//! Requires `deno` to be installed; tests are skipped if it isn't.

use std::process::Command;

use lykn_lang::codegen::emit_module_js;
use lykn_lang::reader;

fn deno_available() -> bool {
    Command::new("deno").arg("--version").output().is_ok()
}

fn project_root() -> String {
    format!("{}/../..", env!("CARGO_MANIFEST_DIR"))
}

fn rust_compile(source: &str) -> Result<String, String> {
    let sexprs = reader::read(source).map_err(|e| e.to_string())?;
    emit_module_js(&sexprs).map_err(|e| e.to_string())
}

fn js_compile(source: &str) -> String {
    let script = format!(
        r#"
import {{ read }} from "./packages/lang/reader.js";
import {{ compile }} from "./packages/lang/compiler.js";
const source = `{escaped}`;
console.log(compile(read(source)));
"#,
        escaped = source.replace('`', "\\`").replace('$', "\\$"),
    );
    let output = Command::new("deno")
        .arg("eval")
        .arg("--config")
        .arg("project.json")
        .arg("--ext=js")
        .arg(&script)
        .current_dir(project_root())
        .output()
        .expect("failed to run deno");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("JS pipeline failed:\n{stderr}");
    }
    String::from_utf8(output.stdout)
        .expect("invalid utf8")
        .trim()
        .to_string()
}

fn eval_js(js: &str, bindings: &[(&str, &str)]) -> String {
    let params = bindings
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ");
    let args = bindings
        .iter()
        .map(|(_, v)| *v)
        .collect::<Vec<_>>()
        .join(", ");
    let stripped = js.trim().trim_end_matches(';');
    let script = format!(
        r#"console.log((function({params}) {{ return {stripped}; }})({args}));"#,
    );
    let output = Command::new("deno")
        .arg("eval")
        .arg("--ext=js")
        .arg(&script)
        .output()
        .expect("failed to run deno");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("JS eval failed:\n{stderr}\nscript was:\n{script}");
    }
    String::from_utf8(output.stdout)
        .expect("invalid utf8")
        .trim()
        .to_string()
}

fn assert_cross_equiv(source: &str, bindings: &[(&str, &str)], expected: &str) {
    if !deno_available() {
        eprintln!("skipping cross-compiler test: deno not available");
        return;
    }
    let rust_js = rust_compile(source).expect("Rust compile failed");
    let js_js = js_compile(source);
    let rust_out = eval_js(&rust_js, bindings);
    let js_out = eval_js(&js_js, bindings);
    assert_eq!(
        rust_out, js_out,
        "pipelines disagree!\nsource: {source}\nrust JS:\n{rust_js}\nJS JS:\n{js_js}",
    );
    assert_eq!(
        rust_out, expected,
        "wrong output!\nsource: {source}\nexpected: {expected:?}\ngot: {rust_out:?}",
    );
}

fn assert_cross_error(source: &str, expected_substring: &str) {
    if !deno_available() {
        eprintln!("skipping cross-compiler test: deno not available");
        return;
    }

    let rust_result = rust_compile(source);
    let rust_err = rust_result.expect_err(&format!(
        "Rust pipeline accepted source it should have rejected: {source}"
    ));
    assert!(
        rust_err.contains(expected_substring),
        "Rust error doesn't contain expected substring\n  expected: {expected_substring:?}\n  got: {rust_err}",
    );

    let script = format!(
        r#"
import {{ read }} from "./packages/lang/reader.js";
import {{ compile }} from "./packages/lang/compiler.js";
try {{
    compile(read(`{escaped}`));
    console.log("ACCEPTED");
}} catch (e) {{
    console.log("REJECTED:" + e.message);
}}
"#,
        escaped = source.replace('`', "\\`").replace('$', "\\$"),
    );
    let output = Command::new("deno")
        .arg("eval")
        .arg("--config")
        .arg("project.json")
        .arg("--ext=js")
        .arg(&script)
        .current_dir(project_root())
        .output()
        .expect("failed to run deno");
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(
        out.contains("REJECTED:"),
        "JS pipeline didn't reject source: {source}\noutput: {out}",
    );
}

// ── Happy-path equivalence tests ──────────────────────────────────────

#[test]
fn cross_simple_slot() {
    assert_cross_equiv(
        r#"(template "Hello, {name}!" :name name)"#,
        &[("name", "\"Duncan\"")],
        "Hello, Duncan!",
    );
}

#[test]
fn cross_multi_use_identifier() {
    assert_cross_equiv(
        r#"(template "{name} is {name}" :name name)"#,
        &[("name", "\"Bob\"")],
        "Bob is Bob",
    );
}

#[test]
fn cross_multi_use_triple() {
    assert_cross_equiv(
        r#"(template "{x}-{x}-{x}" :x x)"#,
        &[("x", "\"a\"")],
        "a-a-a",
    );
}

#[test]
fn cross_plural_one() {
    assert_cross_equiv(
        r#"(template "You have {n, plural, one {1 item} other {# items}}." :n n)"#,
        &[("n", "1")],
        "You have 1 item.",
    );
}

#[test]
fn cross_plural_other() {
    assert_cross_equiv(
        r#"(template "You have {n, plural, one {1 item} other {# items}}." :n n)"#,
        &[("n", "5")],
        "You have 5 items.",
    );
}

#[test]
fn cross_plural_explicit_zero() {
    assert_cross_equiv(
        r#"(template "{n, plural, =0 {none} one {1 item} other {# items}}" :n n)"#,
        &[("n", "0")],
        "none",
    );
}

#[test]
fn cross_select_three_branches() {
    assert_cross_equiv(
        r#"(template "{role, select, owner {Owner} member {Member} other {Guest}}" :role r)"#,
        &[("r", "\"member\"")],
        "Member",
    );
    assert_cross_equiv(
        r#"(template "{role, select, owner {Owner} member {Member} other {Guest}}" :role r)"#,
        &[("r", "\"visitor\"")],
        "Guest",
    );
}

#[test]
fn cross_select_with_slot_in_branch() {
    assert_cross_equiv(
        r#"(template "{role, select, admin {Hi {name}} other {Hello}}" :role r :name n)"#,
        &[("r", "\"admin\""), ("n", "\"Alice\"")],
        "Hi Alice",
    );
    assert_cross_equiv(
        r#"(template "{role, select, admin {Hi {name}} other {Hello}}" :role r :name n)"#,
        &[("r", "\"guest\""), ("n", "\"Alice\"")],
        "Hello",
    );
}

#[test]
fn cross_nested_plural_same_selector() {
    assert_cross_equiv(
        r#"(template "{n, plural, one {{n, plural, one {a} other {b}}} other {c}}" :n n)"#,
        &[("n", "1")],
        "a",
    );
    assert_cross_equiv(
        r#"(template "{n, plural, one {{n, plural, one {a} other {b}}} other {c}}" :n n)"#,
        &[("n", "2")],
        "c",
    );
}

#[test]
fn cross_plural_inside_select() {
    assert_cross_equiv(
        r#"(template "{role, select, owner {{n, plural, one {1 item} other {# items}}} other {N/A}}" :role r :n n)"#,
        &[("r", "\"owner\""), ("n", "3")],
        "3 items",
    );
    assert_cross_equiv(
        r#"(template "{role, select, owner {{n, plural, one {1 item} other {# items}}} other {N/A}}" :role r :n n)"#,
        &[("r", "\"viewer\""), ("n", "3")],
        "N/A",
    );
}

#[test]
fn cross_marketing_screenshot() {
    let source = r#"(template "{role, select, owner {Welcome back, {name}! You have {count, plural, =0 {no pending tasks} one {1 pending task} other {# pending tasks}}.} member {Hi {name}. You have {count, plural, =0 {no items to review} one {1 item to review} other {# items to review}}.} other {Hello, guest.}}" :role role :name name :count count)"#;
    assert_cross_equiv(
        source,
        &[("role", "\"member\""), ("name", "\"Bob\""), ("count", "3")],
        "Hi Bob. You have 3 items to review.",
    );
    assert_cross_equiv(
        source,
        &[("role", "\"owner\""), ("name", "\"Alice\""), ("count", "0")],
        "Welcome back, Alice! You have no pending tasks.",
    );
    assert_cross_equiv(
        source,
        &[("role", "\"owner\""), ("name", "\"Alice\""), ("count", "1")],
        "Welcome back, Alice! You have 1 pending task.",
    );
    assert_cross_equiv(
        source,
        &[("role", "\"viewer\""), ("name", "\"X\""), ("count", "99")],
        "Hello, guest.",
    );
}

#[test]
fn cross_dollar_escape() {
    assert_cross_equiv(r#"(template "price is $5")"#, &[], "price is $5");
}

#[test]
fn cross_icu_dollar_before_escaped_brace() {
    assert_cross_equiv(r#"(template "$'{'name'}'")"#, &[], "${name}");
}

#[test]
fn cross_escape_sequences() {
    assert_cross_equiv(r#"(template "'{' literal '}'")"#, &[], "{ literal }");
    assert_cross_equiv(r#"(template "it''s fine")"#, &[], "it's fine");
}

#[test]
fn cross_concat_backward_compat() {
    assert_cross_equiv(
        r#"(template "Hello, " name "!")"#,
        &[("name", "\"world\"")],
        "Hello, world!",
    );
}

// ── Side-effect tests (expression evaluated exactly once) ─────────────

#[test]
fn cross_multi_use_side_effect() {
    if !deno_available() {
        return;
    }
    let source = r#"(template "{x}-{x}-{x}" :x (next-id))"#;
    let rust_js = rust_compile(source).expect("Rust compile failed");
    let js_js = js_compile(source);
    for (label, js) in &[("rust", &rust_js), ("js", &js_js)] {
        let stripped = js.trim().trim_end_matches(';');
        let script = format!(
            r#"
let counter = 0;
const nextId = () => {{ counter += 1; return counter; }};
const result = {stripped};
if (counter !== 1) {{
    console.log("FAIL:" + counter);
}} else {{
    console.log("OK:" + result);
}}
"#
        );
        let output = Command::new("deno")
            .arg("eval")
            .arg("--ext=js")
            .arg(&script)
            .output()
            .expect("failed to run deno");
        let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert!(
            out.starts_with("OK:"),
            "{label} pipeline evaluated next-id more than once: {out}\nJS:\n{js}",
        );
    }
}

#[test]
fn cross_select_branch_single_eval() {
    if !deno_available() {
        return;
    }
    let source = r#"(template "{role, select, owner {Owner: {role}} other {Guest: {role}}}" :role (lookup-role))"#;
    let rust_js = rust_compile(source).expect("Rust compile failed");
    let js_js = js_compile(source);
    for (label, js) in &[("rust", &rust_js), ("js", &js_js)] {
        let stripped = js.trim().trim_end_matches(';');
        let script = format!(
            r#"
let calls = 0;
const lookupRole = () => {{ calls += 1; return "owner"; }};
const result = {stripped};
console.log(`${{calls}}:${{result}}`);
"#
        );
        let output = Command::new("deno")
            .arg("eval")
            .arg("--ext=js")
            .arg(&script)
            .output()
            .expect("failed to run deno");
        let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let (calls, result) = out.split_once(':').unwrap_or(("?", "?"));
        assert_eq!(
            calls, "1",
            "{label} pipeline called lookupRole {calls} times (want 1)\nJS:\n{js}"
        );
        assert_eq!(result, "Owner: owner", "{label} pipeline wrong output");
    }
}

// ── Error-case parity ─────────────────────────────────────────────────

#[test]
fn cross_error_unknown_plural_category() {
    assert_cross_error(
        r#"(template "{n, plural, weird {x} other {y}}" :n n)"#,
        "unknown plural category",
    );
}

#[test]
fn cross_error_missing_other_branch() {
    assert_cross_error(
        r#"(template "{n, plural, one {x}}" :n n)"#,
        "missing required",
    );
}

#[test]
fn cross_error_missing_kwarg() {
    assert_cross_error(r#"(template "Hello, {name}!")"#, "no binding for slot");
}

#[test]
fn cross_error_unused_kwarg() {
    assert_cross_error(
        r#"(template "Hello, {name}!" :name n :extra v)"#,
        "unused keyword argument",
    );
}

#[test]
fn cross_error_duplicate_kwarg() {
    assert_cross_error(r#"(template "{a}" :a 1 :a 2)"#, "duplicate keyword argument");
}

#[test]
fn cross_error_zero_category() {
    assert_cross_error(
        r#"(template "{n, plural, zero {none} other {many}}" :n n)"#,
        "not valid under English plural rules",
    );
}

#[test]
fn cross_error_ambiguous_form() {
    assert_cross_error(r#"(template "Hi, " :name)"#, "ambiguous form");
}

#[test]
fn cross_error_overlap() {
    assert_cross_error(
        r#"(template "{n, plural, =1 {a} one {b} other {c}}" :n n)"#,
        "overlapping branches",
    );
}
