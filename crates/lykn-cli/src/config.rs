//! Typed configuration parsing for lykn project and package JSON files.
//!
//! Provides strongly-typed access to `project.json` (workspace config) and
//! per-package `deno.json` files, replacing the previous ad-hoc string
//! parsing with proper `serde_json` deserialization.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when reading or parsing configuration files.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// An I/O error occurred while reading a config file.
    #[error("error reading {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    /// A JSON parse error occurred.
    #[error("error parsing {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

// ---------------------------------------------------------------------------
// Package-level metadata
// ---------------------------------------------------------------------------

/// The kind of lykn package, used to determine build behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PackageKind {
    /// A normal runtime package containing `.js` files.
    #[default]
    Runtime,
    /// A macro module containing `.lykn` source files.
    MacroModule,
    /// A tooling package (same build behavior as runtime).
    Tooling,
}

/// The `"lykn"` metadata block inside a package's `deno.json`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LyknMetadata {
    /// What kind of package this is.
    #[serde(default)]
    pub kind: PackageKind,

    /// Entry point for macro modules.
    #[serde(skip_serializing_if = "Option::is_none", rename = "macroEntry")]
    pub macro_entry: Option<String>,
}

// ---------------------------------------------------------------------------
// Package config (deno.json)
// ---------------------------------------------------------------------------

fn default_version() -> String {
    "0.0.0".to_string()
}

/// A deserialized per-package `deno.json` configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct PackageConfig {
    /// The package name, e.g. `"@lykn/lang"`.
    pub name: String,

    /// The package version string.
    #[serde(default = "default_version")]
    pub version: String,

    /// The exports field (can be a string or object).
    #[serde(default)]
    #[allow(dead_code)]
    pub exports: serde_json::Value,

    /// Import map entries.
    #[serde(default)]
    pub imports: IndexMap<String, String>,

    /// Lykn-specific metadata.
    #[serde(default)]
    pub lykn: LyknMetadata,
}

// ---------------------------------------------------------------------------
// Project config (project.json)
// ---------------------------------------------------------------------------

/// A deserialized top-level `project.json` workspace configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    /// Workspace member paths (e.g. `["./packages/lang"]`).
    #[serde(default)]
    pub workspace: Vec<String>,

    /// Project-level import map entries.
    #[serde(default)]
    pub imports: IndexMap<String, String>,
}

// ---------------------------------------------------------------------------
// Reading functions
// ---------------------------------------------------------------------------

/// Read and parse a `project.json` file.
pub fn read_project_config(path: &Path) -> Result<ProjectConfig, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    serde_json::from_str(&content).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Read and parse a per-package `deno.json` file.
pub fn read_package_config(path: &Path) -> Result<PackageConfig, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    serde_json::from_str(&content).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        source: e,
    })
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Normalize workspace member paths by stripping leading `./`.
///
/// Returns a list of cleaned paths, e.g. `"./packages/foo"` becomes
/// `"packages/foo"`.
pub fn workspace_members(config: &ProjectConfig) -> Vec<String> {
    config
        .workspace
        .iter()
        .map(|entry| entry.strip_prefix("./").unwrap_or(entry).to_string())
        .collect()
}

/// Extract npm dependency pairs from a package's import map.
///
/// Handles two kinds of entries:
/// - `"npm:pkg@^1.0"` style npm specifiers are parsed into `(pkg, ^1.0)`.
/// - Workspace imports like `"lang/": "./packages/lang/"` become
///   `("@lykn/lang", "^<version>")`.
pub fn extract_npm_deps(
    imports: &IndexMap<String, String>,
    version: &str,
) -> Vec<(String, String)> {
    let mut deps = Vec::new();
    for (key, value) in imports {
        if let Some(npm_spec) = value.strip_prefix("npm:") {
            // npm:astring@^1.9.0 -> ("astring", "^1.9.0")
            if let Some(at) = npm_spec.rfind('@') {
                let npm_name = &npm_spec[..at];
                let npm_ver = &npm_spec[at + 1..];
                deps.push((npm_name.to_string(), npm_ver.to_string()));
            }
        } else if key.ends_with('/') && value.starts_with("./packages/") {
            // Workspace import: "lang/": "./packages/lang/" -> ("@lykn/lang", "^0.5.0")
            let pkg_name = key.trim_end_matches('/');
            deps.push((format!("@lykn/{pkg_name}"), format!("^{version}")));
        }
    }
    deps
}

/// Strip the `@scope/` prefix from a package name.
///
/// For example, `"@lykn/testing"` becomes `"testing"`.
/// If there is no `@scope/` prefix, the name is returned as-is.
pub fn short_name(name: &str) -> &str {
    if let Some(slash_pos) = name.find('/')
        && name.starts_with('@')
    {
        return &name[slash_pos + 1..];
    }
    name
}

/// Attempt to locate and read a `project.json` by walking upward from the
/// current working directory. Returns `None` if no file is found or if
/// parsing fails (a missing or malformed config should not be fatal for
/// macro expansion — it simply means no import map is available).
pub fn read_project_config_optional() -> Option<ProjectConfig> {
    let cwd = std::env::current_dir().ok()?;
    let mut dir = cwd.as_path();
    loop {
        let path = dir.join("project.json");
        if path.exists() {
            return read_project_config(&path).ok();
        }
        dir = dir.parent()?;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_name_scoped() {
        assert_eq!(short_name("@lykn/testing"), "testing");
    }

    #[test]
    fn test_short_name_scoped_lang() {
        assert_eq!(short_name("@lykn/lang"), "lang");
    }

    #[test]
    fn test_short_name_unscoped() {
        assert_eq!(short_name("some-package"), "some-package");
    }

    #[test]
    fn test_short_name_empty() {
        assert_eq!(short_name(""), "");
    }

    #[test]
    fn test_workspace_members_strips_dot_slash() {
        let config = ProjectConfig {
            workspace: vec![
                "./packages/lang".to_string(),
                "./packages/browser".to_string(),
            ],
            imports: IndexMap::new(),
        };
        let members = workspace_members(&config);
        assert_eq!(members, vec!["packages/lang", "packages/browser"]);
    }

    #[test]
    fn test_workspace_members_no_prefix() {
        let config = ProjectConfig {
            workspace: vec!["packages/lang".to_string()],
            imports: IndexMap::new(),
        };
        let members = workspace_members(&config);
        assert_eq!(members, vec!["packages/lang"]);
    }

    #[test]
    fn test_workspace_members_empty() {
        let config = ProjectConfig {
            workspace: vec![],
            imports: IndexMap::new(),
        };
        assert!(workspace_members(&config).is_empty());
    }

    #[test]
    fn test_extract_npm_deps_npm_specifier() {
        let mut imports = IndexMap::new();
        imports.insert("astring".to_string(), "npm:astring@^1.9.0".to_string());
        let deps = extract_npm_deps(&imports, "0.5.0");
        assert_eq!(deps, vec![("astring".to_string(), "^1.9.0".to_string())]);
    }

    #[test]
    fn test_extract_npm_deps_scoped_npm() {
        let mut imports = IndexMap::new();
        imports.insert(
            "@scope/pkg".to_string(),
            "npm:@scope/pkg@^2.0.0".to_string(),
        );
        let deps = extract_npm_deps(&imports, "1.0.0");
        assert_eq!(deps, vec![("@scope/pkg".to_string(), "^2.0.0".to_string())]);
    }

    #[test]
    fn test_extract_npm_deps_workspace_import() {
        let mut imports = IndexMap::new();
        imports.insert("lang/".to_string(), "./packages/lang/".to_string());
        let deps = extract_npm_deps(&imports, "0.5.0");
        assert_eq!(deps, vec![("@lykn/lang".to_string(), "^0.5.0".to_string())]);
    }

    #[test]
    fn test_extract_npm_deps_mixed() {
        let mut imports = IndexMap::new();
        imports.insert("lang/".to_string(), "./packages/lang/".to_string());
        imports.insert("astring".to_string(), "npm:astring@^1.9.0".to_string());
        let deps = extract_npm_deps(&imports, "0.5.0");
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&("@lykn/lang".to_string(), "^0.5.0".to_string())));
        assert!(deps.contains(&("astring".to_string(), "^1.9.0".to_string())));
    }

    #[test]
    fn test_extract_npm_deps_empty() {
        let imports = IndexMap::new();
        let deps = extract_npm_deps(&imports, "0.5.0");
        assert!(deps.is_empty());
    }

    #[test]
    fn test_extract_npm_deps_ignores_non_npm_non_workspace() {
        let mut imports = IndexMap::new();
        imports.insert(
            "some-alias".to_string(),
            "https://example.com/mod.ts".to_string(),
        );
        let deps = extract_npm_deps(&imports, "0.5.0");
        assert!(deps.is_empty());
    }

    #[test]
    fn test_parse_project_config() {
        let json = r#"{
            "workspace": ["./packages/lang", "./packages/browser"],
            "imports": {
                "lang/": "./packages/lang/",
                "astring": "npm:astring@^1.9.0"
            }
        }"#;
        let config: ProjectConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.workspace.len(), 2);
        assert_eq!(config.imports.len(), 2);
        assert_eq!(config.imports.get("astring").unwrap(), "npm:astring@^1.9.0");
    }

    #[test]
    fn test_parse_project_config_no_imports() {
        let json = r#"{ "workspace": ["./packages/lang"] }"#;
        let config: ProjectConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.workspace.len(), 1);
        assert!(config.imports.is_empty());
    }

    #[test]
    fn test_parse_project_config_empty() {
        let json = r#"{}"#;
        let config: ProjectConfig = serde_json::from_str(json).unwrap();
        assert!(config.workspace.is_empty());
        assert!(config.imports.is_empty());
    }

    #[test]
    fn test_parse_package_config_full() {
        let json = r#"{
            "name": "@lykn/lang",
            "version": "0.5.0",
            "exports": "./mod.js",
            "imports": {
                "astring": "npm:astring@^1.9.0"
            }
        }"#;
        let config: PackageConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.name, "@lykn/lang");
        assert_eq!(config.version, "0.5.0");
        assert_eq!(config.exports, serde_json::json!("./mod.js"));
        assert_eq!(config.imports.len(), 1);
        assert_eq!(config.lykn.kind, PackageKind::Runtime);
        assert!(config.lykn.macro_entry.is_none());
    }

    #[test]
    fn test_parse_package_config_default_version() {
        let json = r#"{ "name": "@lykn/test" }"#;
        let config: PackageConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.version, "0.0.0");
    }

    #[test]
    fn test_parse_package_config_with_lykn_metadata() {
        let json = r#"{
            "name": "@lykn/testing",
            "version": "0.5.0",
            "exports": "./mod.lykn",
            "lykn": {
                "kind": "macro-module",
                "macroEntry": "mod.lykn"
            }
        }"#;
        let config: PackageConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.lykn.kind, PackageKind::MacroModule);
        assert_eq!(config.lykn.macro_entry.as_deref(), Some("mod.lykn"));
    }

    #[test]
    fn test_parse_package_config_tooling_kind() {
        let json = r#"{
            "name": "@lykn/tools",
            "lykn": { "kind": "tooling" }
        }"#;
        let config: PackageConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.lykn.kind, PackageKind::Tooling);
    }

    #[test]
    fn test_parse_package_config_object_exports() {
        let json = r#"{
            "name": "@lykn/multi",
            "exports": {
                ".": "./mod.js",
                "./reader": "./reader.js"
            }
        }"#;
        let config: PackageConfig = serde_json::from_str(json).unwrap();
        assert!(config.exports.is_object());
    }

    #[test]
    fn test_read_project_config_nonexistent() {
        let result = read_project_config(Path::new("/nonexistent/project.json"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::Io { .. }));
        assert!(err.to_string().contains("/nonexistent/project.json"));
    }

    #[test]
    fn test_read_package_config_nonexistent() {
        let result = read_package_config(Path::new("/nonexistent/deno.json"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::Io { .. }));
    }

    #[test]
    fn test_read_project_config_invalid_json() {
        let tmp = std::env::temp_dir().join("lykn_test_invalid_project.json");
        std::fs::write(&tmp, "not json at all").unwrap();
        let result = read_project_config(&tmp);
        let _ = std::fs::remove_file(&tmp);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn test_read_package_config_invalid_json() {
        let tmp = std::env::temp_dir().join("lykn_test_invalid_pkg.json");
        std::fs::write(&tmp, "{ broken }").unwrap();
        let result = read_package_config(&tmp);
        let _ = std::fs::remove_file(&tmp);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::Parse { .. }));
    }

    #[test]
    fn test_read_project_config_from_tempfile() {
        let tmp = std::env::temp_dir().join("lykn_test_project.json");
        std::fs::write(
            &tmp,
            r#"{ "workspace": ["./packages/lang"], "imports": {} }"#,
        )
        .unwrap();
        let config = read_project_config(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        assert_eq!(config.workspace.len(), 1);
    }

    #[test]
    fn test_read_package_config_from_tempfile() {
        let tmp = std::env::temp_dir().join("lykn_test_pkg.json");
        std::fs::write(&tmp, r#"{ "name": "@lykn/foo", "version": "1.2.3" }"#).unwrap();
        let config = read_package_config(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        assert_eq!(config.name, "@lykn/foo");
        assert_eq!(config.version, "1.2.3");
    }

    #[test]
    fn test_package_kind_default() {
        assert_eq!(PackageKind::default(), PackageKind::Runtime);
    }

    #[test]
    fn test_lykn_metadata_default() {
        let meta = LyknMetadata::default();
        assert_eq!(meta.kind, PackageKind::Runtime);
        assert!(meta.macro_entry.is_none());
    }

    #[test]
    fn test_lykn_metadata_serialization_skips_none() {
        let meta = LyknMetadata {
            kind: PackageKind::Runtime,
            macro_entry: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains("macroEntry"));
    }

    #[test]
    fn test_lykn_metadata_serialization_includes_macro_entry() {
        let meta = LyknMetadata {
            kind: PackageKind::MacroModule,
            macro_entry: Some("mod.lykn".to_string()),
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("macroEntry"));
        assert!(json.contains("mod.lykn"));
    }

    #[test]
    fn test_package_kind_kebab_case_serialization() {
        let json = serde_json::to_string(&PackageKind::MacroModule).unwrap();
        assert_eq!(json, "\"macro-module\"");
    }

    #[test]
    fn test_read_project_config_optional_from_tempdir() {
        // Create a temporary project with project.json
        let tmp = std::env::temp_dir().join("lykn_test_optional_config");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("project.json"),
            r#"{ "workspace": [], "imports": { "testing/": "./packages/testing/" } }"#,
        )
        .unwrap();

        // If we happen to be in a directory with project.json, this will
        // return something. We just verify the function does not panic.
        let _result = read_project_config_optional();

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_read_project_config_optional_returns_none_for_nonexistent() {
        // When run from a directory without any project.json ancestors,
        // the function should return None (not error).
        // We cannot easily control cwd in a test, but we verify no panic.
        let _result = read_project_config_optional();
    }
}
