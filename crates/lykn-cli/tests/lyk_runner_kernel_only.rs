//! M20-3: Integration tests for .lyk kernel-only classification.
//! M20-9: Negative-direction test for .lykn strict-mode enforcement.

use std::fs;
use std::process::Command;

fn lykn_bin() -> String {
    env!("CARGO_BIN_EXE_lykn").to_string()
}

fn project_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("project root")
}

// M20-3 negative-direction: .lyk file with surface form at top level
// should be REJECTED with "surface form" diagnostic.
#[test]
fn lyk_file_rejects_surface_form_at_top_level() {
    let root = project_root();
    let fixture = root.join("test/kernel/fixtures/surface-form-in-lyk.lyk");
    fs::create_dir_all(fixture.parent().unwrap()).unwrap();
    fs::write(&fixture, "(bind x 42)\n").unwrap();

    let output = Command::new(lykn_bin())
        .args(["test", fixture.to_str().unwrap()])
        .current_dir(&root)
        .output()
        .expect("failed to run lykn test");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let _ = fs::remove_file(&fixture);

    assert!(
        !output.status.success(),
        "lykn test on .lyk with surface form should fail"
    );
    assert!(
        stderr.contains("surface form") || stderr.contains("kernel-only"),
        "diagnostic should mention surface form or kernel-only, got:\n{stderr}"
    );
}

// M20-9 negative-direction: .lykn file with bare kernel-only form
// should be REJECTED by strict-mode validation.
#[test]
fn lykn_file_rejects_bare_kernel_only_form() {
    let root = project_root();
    let fixture = root.join("test/kernel/fixtures/kernel-only-in-lykn.lykn");
    fs::create_dir_all(fixture.parent().unwrap()).unwrap();
    fs::write(&fixture, "(const x 42)\n").unwrap();

    let output = Command::new(lykn_bin())
        .args(["test", fixture.to_str().unwrap()])
        .current_dir(&root)
        .output()
        .expect("failed to run lykn test");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let _ = fs::remove_file(&fixture);

    assert!(
        !output.status.success(),
        "lykn test on .lykn with bare const should fail under strict"
    );
    assert!(
        stderr.contains("kernel-only") || stderr.contains("const"),
        "diagnostic should mention kernel-only or const, got:\n{stderr}"
    );
}
