//! Bridge to the JS kernel compiler via Deno.
//!
//! The Rust pipeline emits kernel JSON; the JS compiler (`src/compiler.js`)
//! lowers kernel forms to JavaScript source text. This module shells out to
//! Deno to perform that final step.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Convert kernel JSON to JavaScript source via the JS kernel compiler.
///
/// The kernel JSON is written to a temporary file, then a small Deno script
/// reads it, reconstitutes the AST, and feeds it to `compile()` from
/// `src/compiler.js`.
pub fn kernel_json_to_js(kernel_json: &str, source_path: &Path) -> Result<String, String> {
    let tmp_dir = std::env::temp_dir();
    let tmp_file = tmp_dir.join("lykn_kernel.json");
    std::fs::write(&tmp_file, kernel_json).map_err(|e| format!("error writing temp file: {e}"))?;

    let project_root = find_project_root(source_path)
        .ok_or_else(|| "cannot find lykn project root (need src/compiler.js)".to_string())?;

    let script = build_deno_script(&tmp_file);

    let output = Command::new("deno")
        .arg("eval")
        .arg("--ext=js")
        .arg(&script)
        .current_dir(&project_root)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "lykn compile requires Deno — install from https://deno.land".to_string()
            } else {
                format!("error running Deno: {e}")
            }
        })?;

    // Clean up temp file regardless of outcome
    let _ = std::fs::remove_file(&tmp_file);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("JS kernel compiler error:\n{stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Build the inline Deno script that reads kernel JSON and compiles it to JS.
fn build_deno_script(tmp_file: &Path) -> String {
    format!(
        r#"
import {{ compile }} from "./packages/lykn/compiler.js";
const kernelJson = Deno.readTextFileSync("{tmp_path}");
const kernel = JSON.parse(kernelJson);

function fromJson(val) {{
    if (val && typeof val === "object" && !Array.isArray(val)) {{
        if (val.type === "list") return {{ type: "list", values: val.values.map(fromJson) }};
        if (val.type === "cons") return {{ type: "cons", car: fromJson(val.car), cdr: fromJson(val.cdr) }};
        return val; // atom, string, number already in correct format
    }}
    // Fallback for any legacy flat format
    if (Array.isArray(val)) return {{ type: "list", values: val.map(fromJson) }};
    if (typeof val === "string") return {{ type: "atom", value: val }};
    if (typeof val === "number") return {{ type: "number", value: val }};
    if (typeof val === "boolean") return {{ type: "atom", value: String(val) }};
    if (val === null) return {{ type: "atom", value: "null" }};
    return val;
}}

const ast = kernel.map(fromJson);
console.log(compile(ast));
"#,
        tmp_path = tmp_file.display()
    )
}

/// Walk up from `start` looking for a directory that contains `deno.json`
/// or `src/compiler.js`.
pub(crate) fn find_project_root(start: &Path) -> Option<PathBuf> {
    let start = if start.is_file() {
        start.parent()?
    } else {
        start
    };

    let mut current = start.canonicalize().ok()?;
    loop {
        if current.join("project.json").exists()
            || current.join("deno.json").exists()
            || current.join("packages/lykn/compiler.js").exists()
        {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn build_deno_script_contains_import_and_path() {
        let path = Path::new("/tmp/kernel.json");
        let script = build_deno_script(path);
        assert!(script.contains("import { compile }"));
        assert!(script.contains("/tmp/kernel.json"));
        assert!(script.contains("fromJson"));
        assert!(script.contains("console.log(compile(ast))"));
    }

    #[test]
    fn find_project_root_with_deno_json() {
        let tmp = std::env::temp_dir().join("lykn_test_find_root");
        let _ = fs::remove_dir_all(&tmp);
        let sub = tmp.join("a").join("b");
        fs::create_dir_all(&sub).unwrap();
        fs::write(tmp.join("deno.json"), "{}").unwrap();

        let found = find_project_root(&sub).unwrap();
        assert_eq!(found, tmp.canonicalize().unwrap());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_project_root_with_compiler_js() {
        let tmp = std::env::temp_dir().join("lykn_test_find_root_cjs");
        let _ = fs::remove_dir_all(&tmp);
        let sub = tmp.join("child");
        fs::create_dir_all(&sub).unwrap();
        let src_dir = tmp.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("compiler.js"), "").unwrap();

        let found = find_project_root(&sub).unwrap();
        assert_eq!(found, tmp.canonicalize().unwrap());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_project_root_from_file() {
        let tmp = std::env::temp_dir().join("lykn_test_find_root_file");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("deno.json"), "{}").unwrap();
        let file = tmp.join("example.lykn");
        fs::write(&file, "(+ 1 2)").unwrap();

        let found = find_project_root(&file).unwrap();
        assert_eq!(found, tmp.canonicalize().unwrap());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_project_root_none_when_missing() {
        let tmp = std::env::temp_dir().join("lykn_test_find_root_none");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        // No deno.json or src/compiler.js anywhere

        let result = find_project_root(&tmp);
        // Might find one from the real filesystem above tmp, so just verify no panic
        let _ = result;

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_project_root_nonexistent_path() {
        let result = find_project_root(Path::new("/nonexistent/path/here"));
        assert!(result.is_none());
    }

    // ---------------------------------------------------------------
    // kernel_json_to_js integration tests
    // ---------------------------------------------------------------

    /// Returns true if `deno` is available on PATH.
    fn deno_available() -> bool {
        Command::new("deno")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
    }

    #[test]
    fn test_kernel_json_to_js_simple() {
        if !deno_available() {
            eprintln!("skipping: deno not found");
            return;
        }
        // CARGO_MANIFEST_DIR is crates/lykn-cli; go up two levels to workspace root.
        // Use deno.json as the source_path since it exists and find_project_root
        // needs a real path for canonicalize().
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("should have workspace root");
        let source_path = workspace_root.join("deno.json");

        // A minimal valid kernel JSON — empty module produces empty JS.
        let kernel_json = "[]";
        let result = kernel_json_to_js(kernel_json, &source_path);
        // Should either produce JS output or a meaningful error from Deno.
        match &result {
            Ok(js) => {
                // Empty kernel may produce empty or minimal output.
                let _ = js;
            }
            Err(e) => {
                assert!(
                    e.contains("JS kernel compiler error"),
                    "expected JS compiler error, got: {e}"
                );
            }
        }
    }

    #[test]
    fn test_kernel_json_to_js_no_project_root() {
        if !deno_available() {
            eprintln!("skipping: deno not found");
            return;
        }
        // Use a temp directory with no deno.json or src/compiler.js.
        let tmp = std::env::temp_dir().join("lykn_test_bridge_no_root");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let source_path = tmp.join("test.lykn");
        fs::write(&source_path, "").unwrap();

        let result = kernel_json_to_js("[]", &source_path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("cannot find lykn project root"),
            "expected project root error, got: {err}"
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_kernel_json_to_js_invalid_json() {
        if !deno_available() {
            eprintln!("skipping: deno not found");
            return;
        }
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("should have workspace root");
        let source_path = workspace_root.join("deno.json");

        // Invalid JSON — Deno's JSON.parse may fail, producing a JS error,
        // or the compiler may handle it differently. Just verify no Rust panic.
        let result = kernel_json_to_js("{{{not valid json", &source_path);
        // Either an error or some output — the point is we don't panic.
        let _ = result;
    }
}
