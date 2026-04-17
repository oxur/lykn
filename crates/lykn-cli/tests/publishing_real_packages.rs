//! Layer 2: Real packages round-trip tests.
//!
//! Builds the actual `@lykn/lang`, `@lykn/browser`, and `@lykn/testing`
//! workspace members and verifies dry-run acceptance for both JSR and npm.

use std::path::{Path, PathBuf};
use std::process::Command;

use lykn_cli::config::PackageKind;
use lykn_cli::dist::build_dist;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The real project root (two levels up from `crates/lykn-cli/`).
fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
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
fn real_packages_build_dist_succeeds() {
    let root = project_root();

    let built = build_dist(&root).expect("build_dist should succeed on the real workspace");

    // The real workspace has 3 packages: lang, browser, testing
    assert!(
        built.len() >= 3,
        "expected at least 3 built packages, got {}",
        built.len()
    );

    // Verify lang
    let lang = built
        .iter()
        .find(|p| p.short_name == "lang")
        .expect("lang package should be built");
    assert_eq!(lang.name, "@lykn/lang");
    assert_eq!(lang.kind, PackageKind::Runtime);

    // Verify browser
    let browser = built
        .iter()
        .find(|p| p.short_name == "browser")
        .expect("browser package should be built");
    assert_eq!(browser.name, "@lykn/browser");
    assert_eq!(browser.kind, PackageKind::Tooling);

    // Verify testing
    let testing = built
        .iter()
        .find(|p| p.short_name == "testing")
        .expect("testing package should be built");
    assert_eq!(testing.name, "@lykn/testing");
    assert_eq!(testing.kind, PackageKind::MacroModule);

    // Verify dist directories were created
    let dist = root.join("dist");
    assert!(dist.join("lang").is_dir(), "dist/lang/ should exist");
    assert!(dist.join("browser").is_dir(), "dist/browser/ should exist");
    assert!(dist.join("testing").is_dir(), "dist/testing/ should exist");
    assert!(
        dist.join("project.json").exists(),
        "dist/project.json should exist"
    );
}

#[test]
fn real_packages_dry_run_jsr() {
    if !deno_available() {
        eprintln!("SKIP: deno not found on PATH");
        return;
    }

    let root = project_root();

    // Ensure dist is built
    build_dist(&root).expect("build_dist should succeed");

    let output = Command::new("deno")
        .arg("publish")
        .arg("--dry-run")
        .arg("--allow-dirty")
        .arg("--config")
        .arg(root.join("dist/project.json"))
        .current_dir(root.join("dist"))
        .output()
        .expect("failed to run deno publish --dry-run");

    assert!(
        output.status.success(),
        "deno publish --dry-run should succeed for real packages.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn real_packages_dry_run_npm() {
    if !npm_available() {
        eprintln!("SKIP: npm not found on PATH");
        return;
    }

    let root = project_root();

    // Ensure dist is built
    let built = build_dist(&root).expect("build_dist should succeed");

    for pkg in &built {
        let dist_pkg = root.join("dist").join(&pkg.short_name);
        assert!(
            dist_pkg.join("package.json").exists(),
            "dist/{}/package.json should exist",
            pkg.short_name
        );

        let output = Command::new("npm")
            .arg("pack")
            .arg("--dry-run")
            .current_dir(&dist_pkg)
            .output()
            .unwrap_or_else(|e| {
                panic!(
                    "failed to run npm pack --dry-run in {}: {e}",
                    pkg.short_name
                )
            });

        assert!(
            output.status.success(),
            "npm pack --dry-run should succeed for {}.\nstdout: {}\nstderr: {}",
            pkg.short_name,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
