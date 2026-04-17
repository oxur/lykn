//! Layer 2: Dry-run publish acceptance tests.
//!
//! Verifies that built dist packages pass `deno publish --dry-run` and
//! `npm pack --dry-run`. Requires Deno and npm on PATH.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use lykn_cli::dist::build_dist;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Base directory for synthetic fixture packages.
fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/integration/publishing/fixtures/synthetic")
}

/// Recursively copy the contents of `src` into `dst`.
fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("failed to create destination directory");
    for entry in fs::read_dir(src).expect("failed to read source directory") {
        let entry = entry.expect("failed to read directory entry");
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            fs::copy(&src_path, &dst_path).unwrap_or_else(|e| {
                panic!(
                    "failed to copy {} -> {}: {e}",
                    src_path.display(),
                    dst_path.display()
                )
            });
        }
    }
}

/// Scaffold a single-package project inside a temp directory.
fn setup_single_package_project(
    fixture_src: &Path,
    pkg_name: &str,
    project_imports: Option<&str>,
) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let root = tmp.path();

    let pkg_dir = root.join("packages").join(pkg_name);
    copy_dir_recursive(fixture_src, &pkg_dir);

    let imports_json = project_imports.unwrap_or("{}");
    let project_json = format!(
        r#"{{
    "workspace": ["./packages/{pkg_name}"],
    "imports": {imports_json}
}}"#
    );
    fs::write(root.join("project.json"), project_json).unwrap();
    fs::write(root.join("README.md"), "# Fixture Project\n").unwrap();
    fs::write(root.join("LICENSE"), "Apache-2.0\n").unwrap();

    tmp
}

/// Check whether `deno` is available on PATH.
fn deno_available() -> bool {
    Command::new("deno")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Check whether `npm` is available on PATH.
fn npm_available() -> bool {
    Command::new("npm")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn jsr_dry_run_accepts_runtime_fixture() {
    if !deno_available() {
        eprintln!("SKIP: deno not found on PATH");
        return;
    }

    let fixture = fixtures_dir().join("pkg-runtime-minimal");
    let tmp = setup_single_package_project(&fixture, "fixture-runtime", None);
    let root = tmp.path();

    build_dist(root).expect("build_dist should succeed");

    let output = Command::new("deno")
        .arg("publish")
        .arg("--dry-run")
        .arg("--config")
        .arg(root.join("dist/project.json"))
        .current_dir(root.join("dist"))
        .output()
        .expect("failed to run deno publish --dry-run");

    assert!(
        output.status.success(),
        "deno publish --dry-run should succeed for runtime fixture.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn jsr_dry_run_accepts_macro_module_fixture() {
    if !deno_available() {
        eprintln!("SKIP: deno not found on PATH");
        return;
    }

    let fixture = fixtures_dir().join("pkg-macro-module");
    let tmp = setup_single_package_project(&fixture, "fixture-macros", None);
    let root = tmp.path();

    build_dist(root).expect("build_dist should succeed");

    let output = Command::new("deno")
        .arg("publish")
        .arg("--dry-run")
        .arg("--config")
        .arg(root.join("dist/project.json"))
        .current_dir(root.join("dist"))
        .output()
        .expect("failed to run deno publish --dry-run");

    assert!(
        output.status.success(),
        "deno publish --dry-run should succeed for macro-module fixture.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn jsr_dry_run_accepts_tooling_fixture() {
    if !deno_available() {
        eprintln!("SKIP: deno not found on PATH");
        return;
    }

    let fixture = fixtures_dir().join("pkg-tooling");
    let tmp = setup_single_package_project(&fixture, "fixture-tools", None);
    let root = tmp.path();

    build_dist(root).expect("build_dist should succeed");

    let output = Command::new("deno")
        .arg("publish")
        .arg("--dry-run")
        .arg("--config")
        .arg(root.join("dist/project.json"))
        .current_dir(root.join("dist"))
        .output()
        .expect("failed to run deno publish --dry-run");

    assert!(
        output.status.success(),
        "deno publish --dry-run should succeed for tooling fixture.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn npm_dry_run_produces_expected_file_list() {
    if !npm_available() {
        eprintln!("SKIP: npm not found on PATH");
        return;
    }

    let fixture = fixtures_dir().join("pkg-runtime-minimal");
    let tmp = setup_single_package_project(&fixture, "fixture-runtime", None);
    let root = tmp.path();

    build_dist(root).expect("build_dist should succeed");

    let dist_pkg = root.join("dist/fixture-runtime");
    let output = Command::new("npm")
        .arg("pack")
        .arg("--dry-run")
        .current_dir(&dist_pkg)
        .output()
        .expect("failed to run npm pack --dry-run");

    assert!(
        output.status.success(),
        "npm pack --dry-run should succeed.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // npm pack --dry-run outputs the file list to either stdout or stderr
    // depending on version. Check both.
    let combined = format!("{stdout}\n{stderr}");

    assert!(
        combined.contains("mod.js"),
        "npm pack output should list mod.js, got:\n{combined}"
    );
    assert!(
        combined.contains("package.json"),
        "npm pack output should list package.json, got:\n{combined}"
    );
}

#[test]
fn jsr_dry_run_rejects_invalid_fixture() {
    // The invalid fixture has no deno.json, so build_dist itself should
    // fail before we ever get to `deno publish`.
    let fixture = fixtures_dir().join("pkg-invalid-no-config");
    let tmp = setup_single_package_project(&fixture, "pkg-invalid-no-config", None);
    let root = tmp.path();

    let result = build_dist(root);
    assert!(
        result.is_err(),
        "build_dist should fail for a package with no deno.json"
    );
}
