//! Layer 2: Per-kind build correctness tests.
//!
//! Verifies that `build_dist` produces correct output for each `PackageKind`
//! when run against synthetic fixture packages.

use std::fs;
use std::path::{Path, PathBuf};

use lykn_cli::config::PackageKind;
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
///
/// `std::fs` does not provide a recursive directory copy, so we implement
/// a simple depth-first traversal here.
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
///
/// 1. Creates `<tmpdir>/packages/<pkg_name>/` and copies `fixture_src` into it.
/// 2. Writes a `project.json` with a workspace entry pointing to the package.
/// 3. Writes stub `README.md` and `LICENSE` files (required by `build_dist`).
///
/// The optional `project_imports` parameter lets callers inject import map
/// entries into the generated `project.json`.
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn build_runtime_minimal_produces_correct_dist() {
    let fixture = fixtures_dir().join("pkg-runtime-minimal");
    let tmp = setup_single_package_project(&fixture, "fixture-runtime", None);
    let root = tmp.path();

    let built = build_dist(root).expect("build_dist should succeed");
    assert_eq!(built.len(), 1);
    assert_eq!(built[0].kind, PackageKind::Runtime);
    assert_eq!(built[0].short_name, "fixture-runtime");
    assert_eq!(built[0].name, "@lykn/fixture-runtime");
    assert_eq!(built[0].version, "0.0.1");

    let dist = root.join("dist/fixture-runtime");

    // mod.js exists and contains the original content
    let mod_js = dist.join("mod.js");
    assert!(mod_js.exists(), "dist/fixture-runtime/mod.js should exist");
    let mod_content = fs::read_to_string(&mod_js).unwrap();
    assert!(
        mod_content.contains("VERSION"),
        "mod.js should contain original source"
    );

    // deno.json has correct fields
    let deno_json = fs::read_to_string(dist.join("deno.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&deno_json).unwrap();
    assert_eq!(parsed["name"], "@lykn/fixture-runtime");
    assert_eq!(parsed["version"], "0.0.1");
    assert_eq!(parsed["exports"], "./mod.js");

    // package.json has correct fields
    let pkg_json = fs::read_to_string(dist.join("package.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&pkg_json).unwrap();
    assert_eq!(parsed["name"], "@lykn/fixture-runtime");
    assert_eq!(parsed["version"], "0.0.1");
    assert_eq!(parsed["type"], "module");
    assert_eq!(parsed["main"], "./mod.js");

    // Root files were copied
    assert!(dist.join("README.md").exists());
    assert!(dist.join("LICENSE").exists());

    // Build stamp exists
    assert!(dist.join(".build-stamp").exists());
}

#[test]
fn build_runtime_with_imports_rewrites_correctly() {
    let fixture = fixtures_dir().join("pkg-runtime-with-imports");
    let imports = r#"{ "lang/": "./packages/lang/" }"#;
    let tmp = setup_single_package_project(&fixture, "fixture-runtime-imports", Some(imports));
    let root = tmp.path();

    let built = build_dist(root).expect("build_dist should succeed");
    assert_eq!(built.len(), 1);
    assert_eq!(built[0].kind, PackageKind::Runtime);

    // Verify import rewriting: `from 'lang/reader.js'` -> `from '@lykn/lang/reader.js'`
    let mod_js = fs::read_to_string(root.join("dist/fixture-runtime-imports/mod.js")).unwrap();
    assert!(
        mod_js.contains("@lykn/"),
        "imports should be rewritten to use @lykn/ prefix, got: {mod_js}"
    );
    assert!(
        mod_js.contains("from '@lykn/lang/reader.js'"),
        "expected rewritten import path, got: {mod_js}"
    );
    assert!(
        !mod_js.contains("from 'lang/reader.js'"),
        "original bare import should not remain, got: {mod_js}"
    );
}

#[test]
fn build_macro_module_produces_correct_dist() {
    let fixture = fixtures_dir().join("pkg-macro-module");
    let tmp = setup_single_package_project(&fixture, "fixture-macros", None);
    let root = tmp.path();

    let built = build_dist(root).expect("build_dist should succeed");
    assert_eq!(built.len(), 1);
    assert_eq!(built[0].kind, PackageKind::MacroModule);
    assert_eq!(built[0].short_name, "fixture-macros");

    let dist = root.join("dist/fixture-macros");

    // .lykn source files are copied
    let mod_lykn = dist.join("mod.lykn");
    assert!(
        mod_lykn.exists(),
        "dist/fixture-macros/mod.lykn should exist"
    );
    let lykn_content = fs::read_to_string(&mod_lykn).unwrap();
    assert!(
        lykn_content.contains("fixture-assert"),
        "mod.lykn should contain the macro definition"
    );

    // mod.js is a generated stub with VERSION
    let mod_js = fs::read_to_string(dist.join("mod.js")).unwrap();
    assert!(
        mod_js.contains("export const VERSION"),
        "mod.js should be a stub with VERSION export"
    );
    assert!(
        mod_js.contains("0.0.1"),
        "mod.js stub should contain the package version"
    );

    // deno.json preserves lykn metadata
    let deno_json = fs::read_to_string(dist.join("deno.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&deno_json).unwrap();
    assert_eq!(parsed["name"], "@lykn/fixture-macros");
    assert_eq!(parsed["version"], "0.0.1");
    assert!(
        parsed.get("lykn").is_some(),
        "deno.json should preserve lykn metadata"
    );
    let lykn_meta = &parsed["lykn"];
    assert_eq!(lykn_meta["kind"], "macro-module");

    // package.json includes .lykn in files
    let pkg_json = fs::read_to_string(dist.join("package.json")).unwrap();
    assert!(
        pkg_json.contains("*.lykn"),
        "package.json files array should include *.lykn"
    );
}

#[test]
fn build_tooling_produces_correct_dist() {
    let fixture = fixtures_dir().join("pkg-tooling");
    let tmp = setup_single_package_project(&fixture, "fixture-tools", None);
    let root = tmp.path();

    let built = build_dist(root).expect("build_dist should succeed");
    assert_eq!(built.len(), 1);
    assert_eq!(built[0].kind, PackageKind::Tooling);
    assert_eq!(built[0].short_name, "fixture-tools");

    let dist = root.join("dist/fixture-tools");

    // Same shape as runtime: mod.js, deno.json, package.json
    assert!(dist.join("mod.js").exists());

    let mod_content = fs::read_to_string(dist.join("mod.js")).unwrap();
    assert!(
        mod_content.contains("tool"),
        "mod.js should contain original source"
    );

    let deno_json = fs::read_to_string(dist.join("deno.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&deno_json).unwrap();
    assert_eq!(parsed["name"], "@lykn/fixture-tools");
    assert_eq!(parsed["version"], "0.0.1");
    assert_eq!(parsed["exports"], "./mod.js");

    let pkg_json = fs::read_to_string(dist.join("package.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&pkg_json).unwrap();
    assert_eq!(parsed["name"], "@lykn/fixture-tools");
    assert_eq!(parsed["type"], "module");

    assert!(dist.join("README.md").exists());
    assert!(dist.join("LICENSE").exists());
    assert!(dist.join(".build-stamp").exists());
}

#[test]
fn build_invalid_no_config_errors() {
    let fixture = fixtures_dir().join("pkg-invalid-no-config");
    let tmp = setup_single_package_project(&fixture, "pkg-invalid-no-config", None);
    let root = tmp.path();

    let result = build_dist(root);
    assert!(
        result.is_err(),
        "build_dist should fail for a package with no deno.json"
    );

    let err = result.unwrap_err();
    let err_msg = err.to_string();
    // The error should mention the missing deno.json or a config issue
    assert!(
        err_msg.contains("deno.json") || err_msg.contains("error"),
        "error should reference config problem, got: {err_msg}"
    );
}
