//! Integration tests for `--source-context-path` routing in `lykn compile`.
//!
//! A-1: Exercises both branches of cmd_compile's `match source_context_path`
//! (Some → synthetic path, None → actual file path).
//! A-2: Verifies the flag appears in `lykn compile --help`.

use std::fs;
use std::path::Path;
use std::process::Command;

fn lykn_bin() -> String {
    env!("CARGO_BIN_EXE_lykn").to_string()
}

fn project_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("project root")
}

// A-2: --source-context-path appears in lykn compile --help
#[test]
fn help_output_contains_source_context_path_flag() {
    let output = Command::new(lykn_bin())
        .args(["compile", "--help"])
        .output()
        .expect("failed to run lykn compile --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--source-context-path"),
        "lykn compile --help should mention --source-context-path, got:\n{stdout}"
    );
}

// A-1: With --source-context-path, relative imports resolve from context path
#[test]
fn with_context_path_resolves_from_context_directory() {
    let root = project_root();

    // Write a source that uses a relative import-macros to a temp file
    // far from the project root
    let tmp = std::env::temp_dir().join("lykn_ctx_test_with.lykn");
    fs::write(
        &tmp,
        "(import-macros \"./packages/testing\" (test is-equal))\n(test \"x\" (is-equal 1 1))",
    )
    .expect("write temp file");

    let output = Command::new(lykn_bin())
        .args([
            "compile",
            "--source-context-path",
            root.to_str().unwrap(),
            tmp.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run lykn compile");

    let _ = fs::remove_file(&tmp);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "should succeed with --source-context-path pointing at project root.\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("Deno.test"),
        "output should contain expanded macro output.\ngot: {stdout}"
    );
}

// A-1: Without --source-context-path, relative imports resolve from the file's
// own directory (which is /tmp, so the import will fail)
#[test]
fn without_context_path_resolves_from_file_directory() {
    let tmp = std::env::temp_dir().join("lykn_ctx_test_without.lykn");
    fs::write(
        &tmp,
        "(import-macros \"./packages/testing\" (test is-equal))\n(test \"x\" (is-equal 1 1))",
    )
    .expect("write temp file");

    let output = Command::new(lykn_bin())
        .args(["compile", tmp.to_str().unwrap()])
        .output()
        .expect("failed to run lykn compile");

    let _ = fs::remove_file(&tmp);

    // Without context path, the import resolves from /tmp — should fail
    // because ./packages/testing doesn't exist there
    assert!(
        !output.status.success(),
        "should fail without --source-context-path when file is in /tmp"
    );
}
