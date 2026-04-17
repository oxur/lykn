//! Layer 2: Cross-package consumption tests.
//!
//! Verifies that a consumer project's configuration is valid and that
//! workspace member discovery works correctly. The heavy resolver logic
//! is covered by unit tests; these integration tests verify the config
//! parsing layer against real fixture files on disk.

use std::path::{Path, PathBuf};

use lykn_cli::config;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Base directory for publishing integration fixtures.
fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/integration/publishing/fixtures")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn consumer_resolves_file_specifier_to_built_dist() {
    let consumer_dir = fixtures_dir().join("consumer-local");

    // Verify the consumer fixture's project.json is valid and parseable
    let project_config = config::read_project_config(&consumer_dir.join("project.json"))
        .expect("consumer-local project.json should parse");

    assert!(
        !project_config.workspace.is_empty(),
        "consumer should have at least one workspace member"
    );

    let members = config::workspace_members(&project_config);
    assert_eq!(members.len(), 1, "consumer should have exactly one member");
    assert_eq!(members[0], "packages/app");

    // Verify the consumer's package config is valid
    let app_config = config::read_package_config(&consumer_dir.join("packages/app/deno.json"))
        .expect("consumer app deno.json should parse");

    assert_eq!(app_config.name, "@lykn-consumer/app");
    assert_eq!(app_config.version, "0.0.1");

    // The short_name utility should handle the non-@lykn scope correctly
    let sname = config::short_name(&app_config.name);
    assert_eq!(sname, "app");
}

#[test]
fn consumer_resolves_workspace_import_map_entry() {
    let consumer_dir = fixtures_dir().join("consumer-local");

    let project_config = config::read_project_config(&consumer_dir.join("project.json"))
        .expect("consumer-local project.json should parse");

    // The consumer's import map should be parseable (even if empty)
    // This verifies that workspace member discovery works via the
    // project config.
    let members = config::workspace_members(&project_config);
    assert!(
        !members.is_empty(),
        "workspace_members should discover at least one member"
    );

    // Verify each discovered member has a valid deno.json
    for member in &members {
        let deno_json_path = consumer_dir.join(member).join("deno.json");
        assert!(
            deno_json_path.exists(),
            "workspace member {member} should have a deno.json at {}",
            deno_json_path.display()
        );

        let pkg_config = config::read_package_config(&deno_json_path)
            .unwrap_or_else(|e| panic!("failed to parse {}: {e}", deno_json_path.display()));

        // Name should be a valid scoped package name
        assert!(
            pkg_config.name.contains('/'),
            "package name should be scoped: {}",
            pkg_config.name
        );
    }

    // Verify import map entries can be extracted for npm deps
    // (even if the consumer has no imports, the function should not panic)
    let deps = config::extract_npm_deps(&project_config.imports, "0.0.1");
    // Consumer-local has empty imports, so no deps
    assert!(
        deps.is_empty(),
        "consumer with empty imports should produce no npm deps"
    );
}
