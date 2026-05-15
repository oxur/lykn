//! Embedded packages/lang/ source for the Deno subprocess.
//!
//! DD-54: the subprocess uses filesystem imports (./packages/lang/reader.js etc.)
//! relative to its CWD. To make the lykn binary self-contained, packages/lang/
//! is embedded into the binary at compile time and materialized to a known
//! XDG-cache location at runtime.

use include_dir::{Dir, include_dir};
use std::path::PathBuf;

static PACKAGES_LANG: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../packages/lang");

pub const EMBEDDED_VERSION: &str = env!("CARGO_PKG_VERSION");

fn embedded_cache_dir() -> PathBuf {
    let cache_root = if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache")
    } else {
        std::env::temp_dir()
    };
    cache_root
        .join("lykn")
        .join("embedded")
        .join(EMBEDDED_VERSION)
}

pub fn materialize_packages() -> std::io::Result<PathBuf> {
    let root = embedded_cache_dir();
    let sentinel = root.join("_embedded").join(".lykn-version");

    if let Ok(existing) = std::fs::read_to_string(&sentinel)
        && existing.trim() == EMBEDDED_VERSION
        && root.join("packages").join("lang").exists()
    {
        return Ok(root);
    }

    std::fs::create_dir_all(&root)?;
    let pkg_dir = root.join("packages").join("lang");
    let _ = std::fs::remove_dir_all(&pkg_dir);
    std::fs::create_dir_all(&pkg_dir)?;
    PACKAGES_LANG.extract(&pkg_dir)?;
    std::fs::create_dir_all(sentinel.parent().unwrap())?;
    std::fs::write(&sentinel, EMBEDDED_VERSION)?;

    Ok(root)
}
