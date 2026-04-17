//! Dist staging module for `lykn build --dist`.
//!
//! Reads `project.json`, iterates workspace members, and stages each
//! package into `dist/<short_name>/` with generated `deno.json` and
//! `package.json` files ready for publishing to JSR and npm.

use indexmap::IndexMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{self, ConfigError, PackageKind};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during dist staging.
#[derive(Debug, thiserror::Error)]
pub enum DistError {
    /// A configuration file could not be read or parsed.
    #[error("{0}")]
    Config(#[from] ConfigError),

    /// An I/O error occurred during file operations.
    #[error("error at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    /// No workspace members were found in project.json.
    #[error("no workspace members found in project.json")]
    EmptyWorkspace,
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// Information about a successfully built package.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BuiltPackage {
    /// The full package name (e.g. `"@lykn/lang"`).
    pub name: String,
    /// The short name without scope (e.g. `"lang"`).
    pub short_name: String,
    /// The package version.
    pub version: String,
    /// The kind of package that was built.
    pub kind: PackageKind,
}

// ---------------------------------------------------------------------------
// Import rewriting (Phase 3)
// ---------------------------------------------------------------------------

/// Rewrite workspace import paths in JavaScript source for npm compatibility.
///
/// For each workspace import entry in the import map (key ending in `/`,
/// value starting with `./packages/`), rewrites bare imports to use the
/// scoped `@lykn/` prefix.
///
/// For example, with an import map entry `"lang/": "./packages/lang/"`,
/// this rewrites `from 'lang/reader.js'` to `from '@lykn/lang/reader.js'`.
fn rewrite_imports(source: &str, import_map: &IndexMap<String, String>) -> String {
    let mut result = source.to_string();
    for (key, value) in import_map {
        if key.ends_with('/') && value.starts_with("./packages/") {
            let pkg_name = key.trim_end_matches('/');
            let scoped = format!("@lykn/{pkg_name}/");

            // Rewrite single-quoted imports
            let from_single = format!("from '{key}");
            let to_single = format!("from '{scoped}");
            result = result.replace(&from_single, &to_single);

            // Rewrite double-quoted imports
            let from_double = format!("from \"{key}");
            let to_double = format!("from \"{scoped}");
            result = result.replace(&from_double, &to_double);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Per-kind builders
// ---------------------------------------------------------------------------

/// Build a runtime package: copy `.js` files and generate configs.
fn build_runtime(
    project_root: &Path,
    pkg_dir: &str,
    pkg_config: &config::PackageConfig,
    project_imports: &IndexMap<String, String>,
) -> Result<(), DistError> {
    let sname = config::short_name(&pkg_config.name);
    let dist_dir = project_root.join("dist").join(sname);
    let pkg_path = project_root.join(pkg_dir);

    prepare_dist_dir(&dist_dir)?;
    copy_js_files(&pkg_path, &dist_dir, project_imports)?;
    copy_root_files(project_root, &dist_dir);
    write_deno_json(&dist_dir, pkg_config, None)?;
    write_package_json(&dist_dir, pkg_config)?;
    write_build_stamp(&dist_dir)?;

    Ok(())
}

/// Build a macro module package: copy ALL files (`.lykn` + `.js`) and
/// generate a `mod.js` stub.
fn build_macro_module(
    project_root: &Path,
    pkg_dir: &str,
    pkg_config: &config::PackageConfig,
) -> Result<(), DistError> {
    let sname = config::short_name(&pkg_config.name);
    let dist_dir = project_root.join("dist").join(sname);
    let pkg_path = project_root.join(pkg_dir);

    prepare_dist_dir(&dist_dir)?;
    copy_all_files(&pkg_path, &dist_dir)?;

    // Generate mod.js stub
    let stub = format!("export const VERSION = \"{}\";\n", pkg_config.version);
    write_text(&dist_dir.join("mod.js"), &stub)?;

    copy_root_files(project_root, &dist_dir);
    write_deno_json(&dist_dir, pkg_config, Some(&pkg_config.lykn))?;
    write_package_json(&dist_dir, pkg_config)?;
    write_build_stamp(&dist_dir)?;

    Ok(())
}

/// Build a tooling package (same behavior as runtime).
fn build_tooling(
    project_root: &Path,
    pkg_dir: &str,
    pkg_config: &config::PackageConfig,
    project_imports: &IndexMap<String, String>,
) -> Result<(), DistError> {
    build_runtime(project_root, pkg_dir, pkg_config, project_imports)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Remove and recreate the dist directory for a package.
fn prepare_dist_dir(dist_dir: &Path) -> Result<(), DistError> {
    let _ = fs::remove_dir_all(dist_dir);
    fs::create_dir_all(dist_dir).map_err(|e| DistError::Io {
        path: dist_dir.to_path_buf(),
        source: e,
    })
}

/// Copy all `.js` files from the source directory to the dist directory,
/// rewriting workspace imports.
fn copy_js_files(
    src: &Path,
    dst: &Path,
    project_imports: &IndexMap<String, String>,
) -> Result<(), DistError> {
    let entries = fs::read_dir(src).map_err(|e| DistError::Io {
        path: src.to_path_buf(),
        source: e,
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "js")
            && let Some(filename) = path.file_name()
        {
            let content = fs::read_to_string(&path).map_err(|e| DistError::Io {
                path: path.clone(),
                source: e,
            })?;
            let rewritten = rewrite_imports(&content, project_imports);
            write_text(&dst.join(filename), &rewritten)?;
        }
    }
    Ok(())
}

/// Copy all files (`.js`, `.lykn`, etc.) from the source directory to dist.
fn copy_all_files(src: &Path, dst: &Path) -> Result<(), DistError> {
    let entries = fs::read_dir(src).map_err(|e| DistError::Io {
        path: src.to_path_buf(),
        source: e,
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && let Some(filename) = path.file_name()
        {
            // Skip deno.json — we generate our own
            if filename == "deno.json" {
                continue;
            }
            fs::copy(&path, dst.join(filename)).map_err(|e| DistError::Io {
                path: path.clone(),
                source: e,
            })?;
        }
    }
    Ok(())
}

/// Copy README.md and LICENSE from the project root into the dist package.
fn copy_root_files(project_root: &Path, dist_dir: &Path) {
    let _ = fs::copy(project_root.join("README.md"), dist_dir.join("README.md"));
    let _ = fs::copy(project_root.join("LICENSE"), dist_dir.join("LICENSE"));
}

/// Write a text file to disk.
fn write_text(path: &Path, content: &str) -> Result<(), DistError> {
    fs::write(path, content).map_err(|e| DistError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Write a `.build-stamp` file with the current timestamp.
fn write_build_stamp(dist_dir: &Path) -> Result<(), DistError> {
    let stamp = format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );
    write_text(&dist_dir.join(".build-stamp"), &stamp)
}

/// Generate a `deno.json` config for a dist package.
///
/// For runtime and tooling packages, the source `exports` field is passed
/// through (it may be a string or an object with subpath exports). For
/// macro modules, exports is always `"./mod.js"` because JSR cannot parse
/// `.lykn` files — the generated `mod.js` stub serves as the JS entry point.
fn write_deno_json(
    dist_dir: &Path,
    pkg_config: &config::PackageConfig,
    lykn_meta: Option<&config::LyknMetadata>,
) -> Result<(), DistError> {
    let mut map = serde_json::Map::new();
    map.insert(
        "name".to_string(),
        serde_json::Value::String(pkg_config.name.clone()),
    );
    map.insert(
        "version".to_string(),
        serde_json::Value::String(pkg_config.version.clone()),
    );

    let is_macro_module =
        lykn_meta.is_some_and(|m| matches!(m.kind, config::PackageKind::MacroModule));
    let exports = if is_macro_module {
        serde_json::Value::String("./mod.js".to_string())
    } else {
        pkg_config.exports.clone()
    };
    map.insert("exports".to_string(), exports);

    if let Some(meta) = lykn_meta {
        map.insert(
            "lykn".to_string(),
            serde_json::to_value(meta).unwrap_or_default(),
        );
    }

    let json = serde_json::to_string_pretty(&map).unwrap_or_default();
    write_text(&dist_dir.join("deno.json"), &format!("{json}\n"))
}

/// Generate a `package.json` for npm publishing.
fn write_package_json(
    dist_dir: &Path,
    pkg_config: &config::PackageConfig,
) -> Result<(), DistError> {
    let deps = config::extract_npm_deps(&pkg_config.imports, &pkg_config.version);

    let mut deps_map = serde_json::Map::new();
    for (name, version) in &deps {
        deps_map.insert(name.clone(), serde_json::Value::String(version.clone()));
    }

    let mut exports_map = serde_json::Map::new();
    exports_map.insert(
        ".".to_string(),
        serde_json::Value::String("./mod.js".to_string()),
    );

    let package = serde_json::json!({
        "name": pkg_config.name,
        "version": pkg_config.version,
        "type": "module",
        "main": "./mod.js",
        "exports": exports_map,
        "files": ["*.js", "*.lykn", "README.md", "LICENSE"],
        "keywords": ["lisp", "s-expression", "lykn"],
        "author": "Duncan McGreggor",
        "license": "Apache-2.0",
        "repository": {
            "type": "git",
            "url": "https://github.com/oxur/lykn"
        },
        "dependencies": deps_map,
    });

    let json = serde_json::to_string_pretty(&package).unwrap_or_default();
    write_text(&dist_dir.join("package.json"), &format!("{json}\n"))
}

/// Generate the top-level `dist/project.json` workspace config.
fn write_dist_project_json(project_root: &Path, short_names: &[String]) -> Result<(), DistError> {
    let members: Vec<serde_json::Value> = short_names
        .iter()
        .map(|n| serde_json::Value::String(format!("./{n}")))
        .collect();

    let config = serde_json::json!({
        "workspace": members,
    });

    let json = serde_json::to_string_pretty(&config).unwrap_or_default();
    write_text(
        &project_root.join("dist").join("project.json"),
        &format!("{json}\n"),
    )
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Stage all workspace packages into `dist/` for publishing.
///
/// Reads `project.json` from `project_root`, iterates each workspace member,
/// and dispatches to the appropriate builder based on `lykn.kind` metadata.
///
/// Returns a list of successfully built packages.
pub fn build_dist(project_root: &Path) -> Result<Vec<BuiltPackage>, DistError> {
    let project_config = config::read_project_config(&project_root.join("project.json"))?;
    let members = config::workspace_members(&project_config);

    if members.is_empty() {
        return Err(DistError::EmptyWorkspace);
    }

    // Ensure dist/ exists
    let dist_root = project_root.join("dist");
    fs::create_dir_all(&dist_root).map_err(|e| DistError::Io {
        path: dist_root.clone(),
        source: e,
    })?;

    let mut built = Vec::new();
    let mut short_names = Vec::new();

    for member in &members {
        let deno_json_path = project_root.join(member).join("deno.json");
        let pkg_config = config::read_package_config(&deno_json_path)?;
        let sname = config::short_name(&pkg_config.name).to_string();

        match pkg_config.lykn.kind {
            PackageKind::Runtime => {
                build_runtime(project_root, member, &pkg_config, &project_config.imports)?;
            }
            PackageKind::MacroModule => {
                build_macro_module(project_root, member, &pkg_config)?;
            }
            PackageKind::Tooling => {
                build_tooling(project_root, member, &pkg_config, &project_config.imports)?;
            }
        }

        short_names.push(sname.clone());
        built.push(BuiltPackage {
            name: pkg_config.name.clone(),
            short_name: sname,
            version: pkg_config.version.clone(),
            kind: pkg_config.lykn.kind,
        });
    }

    write_dist_project_json(project_root, &short_names)?;

    Ok(built)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Import rewriting tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_rewrite_imports_single_quote() {
        let mut imports = IndexMap::new();
        imports.insert("lang/".to_string(), "./packages/lang/".to_string());

        let source = "import { read } from 'lang/reader.js';";
        let result = rewrite_imports(source, &imports);
        assert_eq!(result, "import { read } from '@lykn/lang/reader.js';");
    }

    #[test]
    fn test_rewrite_imports_double_quote() {
        let mut imports = IndexMap::new();
        imports.insert("lang/".to_string(), "./packages/lang/".to_string());

        let source = "import { read } from \"lang/reader.js\";";
        let result = rewrite_imports(source, &imports);
        assert_eq!(result, "import { read } from \"@lykn/lang/reader.js\";");
    }

    #[test]
    fn test_rewrite_imports_multiple_entries() {
        let mut imports = IndexMap::new();
        imports.insert("lang/".to_string(), "./packages/lang/".to_string());
        imports.insert("testing/".to_string(), "./packages/testing/".to_string());

        let source =
            "import { read } from 'lang/reader.js';\nimport { assert } from 'testing/assert.js';";
        let result = rewrite_imports(source, &imports);
        assert!(result.contains("from '@lykn/lang/reader.js'"));
        assert!(result.contains("from '@lykn/testing/assert.js'"));
    }

    #[test]
    fn test_rewrite_imports_ignores_npm() {
        let mut imports = IndexMap::new();
        imports.insert("astring".to_string(), "npm:astring@^1.9.0".to_string());

        let source = "import { generate } from 'astring';";
        let result = rewrite_imports(source, &imports);
        // Should not be modified (key doesn't end with '/')
        assert_eq!(result, source);
    }

    #[test]
    fn test_rewrite_imports_no_match() {
        let mut imports = IndexMap::new();
        imports.insert("lang/".to_string(), "./packages/lang/".to_string());

        let source = "import { something } from 'other/module.js';";
        let result = rewrite_imports(source, &imports);
        assert_eq!(result, source);
    }

    #[test]
    fn test_rewrite_imports_empty_map() {
        let imports = IndexMap::new();
        let source = "import { read } from 'lang/reader.js';";
        let result = rewrite_imports(source, &imports);
        assert_eq!(result, source);
    }

    #[test]
    fn test_rewrite_imports_empty_source() {
        let mut imports = IndexMap::new();
        imports.insert("lang/".to_string(), "./packages/lang/".to_string());

        let result = rewrite_imports("", &imports);
        assert_eq!(result, "");
    }

    #[test]
    fn test_rewrite_imports_mixed_quotes_same_line() {
        let mut imports = IndexMap::new();
        imports.insert("lang/".to_string(), "./packages/lang/".to_string());

        // Only the from clause should be rewritten
        let source = "const x = 'lang/'; import { y } from \"lang/mod.js\";";
        let result = rewrite_imports(source, &imports);
        // Both occurrences with from prefix get rewritten
        assert!(result.contains("from \"@lykn/lang/mod.js\""));
    }

    // -----------------------------------------------------------------------
    // Dist build tests (filesystem-based)
    // -----------------------------------------------------------------------

    /// Create a minimal project layout in a temp directory for testing.
    ///
    /// Returns a `tempfile::TempDir` whose `path()` is the project root.
    /// The directory (and all contents) is automatically removed when the
    /// returned value is dropped.
    fn setup_test_project() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let root = tmp.path();

        fs::create_dir_all(root.join("packages/lang")).unwrap();
        fs::create_dir_all(root.join("packages/browser")).unwrap();

        // project.json
        fs::write(
            root.join("project.json"),
            r#"{
                "workspace": ["./packages/lang", "./packages/browser"],
                "imports": {
                    "lang/": "./packages/lang/",
                    "astring": "npm:astring@^1.9.0"
                }
            }"#,
        )
        .unwrap();

        // packages/lang/deno.json
        fs::write(
            root.join("packages/lang/deno.json"),
            r#"{
                "name": "@lykn/lang",
                "version": "0.5.0",
                "exports": "./mod.js",
                "imports": {
                    "astring": "npm:astring@^1.9.0"
                }
            }"#,
        )
        .unwrap();

        // packages/lang/mod.js
        fs::write(
            root.join("packages/lang/mod.js"),
            "export function lykn() { return 42; }\n",
        )
        .unwrap();

        // packages/browser/deno.json
        fs::write(
            root.join("packages/browser/deno.json"),
            r#"{
                "name": "@lykn/browser",
                "version": "0.5.0",
                "exports": "./mod.js",
                "imports": {
                    "lang/": "./packages/lang/"
                }
            }"#,
        )
        .unwrap();

        // packages/browser/mod.js — uses workspace import
        fs::write(
            root.join("packages/browser/mod.js"),
            "import { lykn } from 'lang/mod.js';\nexport { lykn };\n",
        )
        .unwrap();

        // Root files
        fs::write(root.join("README.md"), "# Test Project\n").unwrap();
        fs::write(root.join("LICENSE"), "Apache-2.0\n").unwrap();

        tmp
    }

    /// Create a minimal project with a single macro-module package for testing.
    ///
    /// Returns a `tempfile::TempDir` whose `path()` is the project root.
    fn setup_macro_module_project() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let root = tmp.path();

        fs::create_dir_all(root.join("packages/testing")).unwrap();

        fs::write(
            root.join("project.json"),
            r#"{ "workspace": ["./packages/testing"], "imports": {} }"#,
        )
        .unwrap();

        fs::write(
            root.join("packages/testing/deno.json"),
            r#"{
                "name": "@lykn/testing",
                "version": "0.5.0",
                "exports": "./mod.lykn",
                "lykn": {
                    "kind": "macro-module",
                    "macroEntry": "mod.lykn"
                }
            }"#,
        )
        .unwrap();

        fs::write(
            root.join("packages/testing/mod.lykn"),
            "(defmacro assert-eq (a b) `(if (!= ,a ,b) (throw \"assertion failed\")))\n",
        )
        .unwrap();

        fs::write(root.join("README.md"), "# Test\n").unwrap();
        fs::write(root.join("LICENSE"), "Apache-2.0\n").unwrap();

        tmp
    }

    #[test]
    fn test_build_dist_creates_packages() {
        let tmp = setup_test_project();
        let root = tmp.path();
        let result = build_dist(root);
        assert!(result.is_ok());
        let built = result.unwrap();
        assert_eq!(built.len(), 2);

        // Check that dist directories exist
        assert!(root.join("dist/lang").is_dir());
        assert!(root.join("dist/browser").is_dir());
    }

    #[test]
    fn test_build_dist_generates_project_json() {
        let tmp = setup_test_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        let project_json_path = root.join("dist/project.json");
        assert!(project_json_path.exists());

        let content = fs::read_to_string(&project_json_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let workspace = parsed["workspace"].as_array().unwrap();
        assert_eq!(workspace.len(), 2);
    }

    #[test]
    fn test_build_dist_copies_js_files() {
        let tmp = setup_test_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        assert!(root.join("dist/lang/mod.js").exists());
        assert!(root.join("dist/browser/mod.js").exists());
    }

    #[test]
    fn test_build_dist_rewrites_imports_in_js() {
        let tmp = setup_test_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        let content = fs::read_to_string(root.join("dist/browser/mod.js")).unwrap();
        assert!(
            content.contains("from '@lykn/lang/mod.js'"),
            "expected rewritten import, got: {content}"
        );
    }

    #[test]
    fn test_build_dist_generates_deno_json() {
        let tmp = setup_test_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        let deno_json = fs::read_to_string(root.join("dist/lang/deno.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&deno_json).unwrap();
        assert_eq!(parsed["name"], "@lykn/lang");
        assert_eq!(parsed["version"], "0.5.0");
        assert_eq!(parsed["exports"], "./mod.js");
    }

    #[test]
    fn test_build_dist_generates_package_json() {
        let tmp = setup_test_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        let pkg_json = fs::read_to_string(root.join("dist/lang/package.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&pkg_json).unwrap();
        assert_eq!(parsed["name"], "@lykn/lang");
        assert_eq!(parsed["version"], "0.5.0");
        assert_eq!(parsed["type"], "module");
        assert_eq!(parsed["main"], "./mod.js");
        assert_eq!(parsed["license"], "Apache-2.0");

        // Check npm deps
        let deps = parsed["dependencies"].as_object().unwrap();
        assert_eq!(deps.get("astring").unwrap(), "^1.9.0");
    }

    #[test]
    fn test_build_dist_copies_readme_and_license() {
        let tmp = setup_test_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        assert!(root.join("dist/lang/README.md").exists());
        assert!(root.join("dist/lang/LICENSE").exists());
        assert!(root.join("dist/browser/README.md").exists());
        assert!(root.join("dist/browser/LICENSE").exists());
    }

    #[test]
    fn test_build_dist_creates_build_stamp() {
        let tmp = setup_test_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        assert!(root.join("dist/lang/.build-stamp").exists());
        assert!(root.join("dist/browser/.build-stamp").exists());

        // Verify it is a numeric timestamp
        let stamp = fs::read_to_string(root.join("dist/lang/.build-stamp")).unwrap();
        assert!(stamp.parse::<u64>().is_ok());
    }

    #[test]
    fn test_build_dist_empty_workspace_errors() {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let root = tmp.path();
        fs::write(root.join("project.json"), r#"{ "workspace": [] }"#).unwrap();

        let result = build_dist(root);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("no workspace members"),
            "expected empty workspace error, got: {err}"
        );
    }

    #[test]
    fn test_build_dist_missing_project_json_errors() {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let root = tmp.path();

        let result = build_dist(root);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_dist_macro_module() {
        let tmp = setup_macro_module_project();
        let root = tmp.path();

        let result = build_dist(root).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].kind, PackageKind::MacroModule);
        assert_eq!(result[0].short_name, "testing");

        // Check that .lykn file was copied
        assert!(root.join("dist/testing/mod.lykn").exists());

        // Check that mod.js stub was generated
        let stub = fs::read_to_string(root.join("dist/testing/mod.js")).unwrap();
        assert!(stub.contains("export const VERSION = \"0.5.0\""));

        // Check deno.json preserves lykn metadata
        let deno_json = fs::read_to_string(root.join("dist/testing/deno.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&deno_json).unwrap();
        assert!(parsed.get("lykn").is_some());

        // Check package.json includes .lykn in files
        let pkg_json = fs::read_to_string(root.join("dist/testing/package.json")).unwrap();
        assert!(pkg_json.contains("*.lykn"));
    }

    #[test]
    fn test_build_dist_built_package_fields() {
        let tmp = setup_test_project();
        let root = tmp.path();
        let built = build_dist(root).unwrap();

        let lang = built.iter().find(|p| p.short_name == "lang").unwrap();
        assert_eq!(lang.name, "@lykn/lang");
        assert_eq!(lang.version, "0.5.0");
        assert_eq!(lang.kind, PackageKind::Runtime);

        let browser = built.iter().find(|p| p.short_name == "browser").unwrap();
        assert_eq!(browser.name, "@lykn/browser");
        assert_eq!(browser.version, "0.5.0");
    }

    #[test]
    fn test_build_dist_browser_deps_include_workspace() {
        let tmp = setup_test_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        let pkg_json = fs::read_to_string(root.join("dist/browser/package.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&pkg_json).unwrap();
        let deps = parsed["dependencies"].as_object().unwrap();
        assert!(
            deps.contains_key("@lykn/lang"),
            "browser should depend on @lykn/lang"
        );
    }

    #[test]
    fn test_build_dist_idempotent() {
        let tmp = setup_test_project();
        let root = tmp.path();

        // Build twice — should not error
        build_dist(root).unwrap();
        let result = build_dist(root);
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Snapshot tests
    // -----------------------------------------------------------------------

    #[test]
    fn snapshot_runtime_pkg_deno_json() {
        let tmp = setup_test_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        let content = fs::read_to_string(root.join("dist/lang/deno.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        insta::assert_json_snapshot!("runtime_pkg_deno_json", value);
    }

    #[test]
    fn snapshot_runtime_pkg_package_json() {
        let tmp = setup_test_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        let content = fs::read_to_string(root.join("dist/lang/package.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        insta::assert_json_snapshot!("runtime_pkg_package_json", value);
    }

    #[test]
    fn snapshot_macro_module_deno_json() {
        let tmp = setup_macro_module_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        let content = fs::read_to_string(root.join("dist/testing/deno.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        insta::assert_json_snapshot!("macro_module_deno_json", value);
    }

    #[test]
    fn snapshot_macro_module_package_json() {
        let tmp = setup_macro_module_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        let content = fs::read_to_string(root.join("dist/testing/package.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        insta::assert_json_snapshot!("macro_module_package_json", value);
    }

    #[test]
    fn snapshot_macro_module_mod_js_stub() {
        let tmp = setup_macro_module_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        let content = fs::read_to_string(root.join("dist/testing/mod.js")).unwrap();
        insta::assert_snapshot!("macro_module_mod_js_stub", content);
    }

    #[test]
    fn snapshot_workspace_project_json() {
        let tmp = setup_test_project();
        let root = tmp.path();
        build_dist(root).unwrap();

        let content = fs::read_to_string(root.join("dist/project.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        insta::assert_json_snapshot!("workspace_project_json", value);
    }

    #[test]
    fn snapshot_import_rewriter_output() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../test/fixtures/publishing/import-rewriter-input.js");
        let source = fs::read_to_string(&fixture_path)
            .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", fixture_path.display()));

        let mut imports = IndexMap::new();
        imports.insert("lang/".to_string(), "./packages/lang/".to_string());

        let result = rewrite_imports(&source, &imports);
        insta::assert_snapshot!("import_rewriter_output", result);
    }
}
