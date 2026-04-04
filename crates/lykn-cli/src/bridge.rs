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

    // The inline script reads kernel JSON, reconstructs reader-shaped AST nodes,
    // and feeds them to the JS compiler.
    let script = format!(
        r#"
import {{ compile }} from "./src/compiler.js";
const kernelJson = Deno.readTextFileSync("{tmp_path}");
const kernel = JSON.parse(kernelJson);

function fromJson(val) {{
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
    );

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

/// Walk up from `start` looking for a directory that contains `deno.json`
/// or `src/compiler.js`.
fn find_project_root(start: &Path) -> Option<PathBuf> {
    let start = if start.is_file() {
        start.parent()?
    } else {
        start
    };

    let mut current = start.canonicalize().ok()?;
    loop {
        if current.join("deno.json").exists() || current.join("src/compiler.js").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}
