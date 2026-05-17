//! Module cache for compiled macro environments.
//!
//! When `import-macros` loads and compiles macros from an external `.lykn`
//! file, the resulting [`MacroEnv`] and runtime imports are cached by
//! canonical path so that repeated imports of the same module skip
//! recompilation.

use std::collections::HashMap;
use std::path::PathBuf;

use super::MacroEnv;
use crate::ast::sexpr::SExpr;

/// A cached macro module: its compiled macros plus any runtime imports
/// that must be emitted into consuming files.
#[derive(Debug, Clone)]
pub struct CachedModule {
    pub macros: MacroEnv,
    pub runtime_imports: Vec<SExpr>,
}

/// Cache of already-compiled macro modules, keyed by file path.
#[derive(Debug, Default)]
pub struct ModuleCache {
    entries: HashMap<PathBuf, CachedModule>,
}

impl ModuleCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a previously compiled module.
    pub fn get(&self, path: &PathBuf) -> Option<&CachedModule> {
        self.entries.get(path)
    }

    /// Store a compiled module.
    pub fn insert(&mut self, path: PathBuf, module: CachedModule) {
        self.entries.insert(path, module);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expander::CompiledMacro;

    #[test]
    fn test_cache_new_is_empty() {
        let cache = ModuleCache::new();
        let path = PathBuf::from("/tmp/test.lykn");
        assert!(cache.get(&path).is_none());
    }

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = ModuleCache::new();
        let path = PathBuf::from("/tmp/macros.lykn");

        let mut env = MacroEnv::new();
        env.insert(
            "when".to_string(),
            CompiledMacro {
                name: "when".to_string(),
                js_body: "return 42;".to_string(),
            },
        );

        cache.insert(
            path.clone(),
            CachedModule {
                macros: env,
                runtime_imports: vec![],
            },
        );

        let retrieved = cache.get(&path).expect("should find cached module");
        assert!(retrieved.macros.contains_key("when"));
        assert_eq!(retrieved.macros["when"].name, "when");
    }

    #[test]
    fn test_cache_overwrite() {
        let mut cache = ModuleCache::new();
        let path = PathBuf::from("/tmp/macros.lykn");

        let mut env1 = MacroEnv::new();
        env1.insert(
            "a".to_string(),
            CompiledMacro {
                name: "a".to_string(),
                js_body: "v1".to_string(),
            },
        );
        cache.insert(
            path.clone(),
            CachedModule {
                macros: env1,
                runtime_imports: vec![],
            },
        );

        let mut env2 = MacroEnv::new();
        env2.insert(
            "b".to_string(),
            CompiledMacro {
                name: "b".to_string(),
                js_body: "v2".to_string(),
            },
        );
        cache.insert(
            path.clone(),
            CachedModule {
                macros: env2,
                runtime_imports: vec![],
            },
        );

        let retrieved = cache.get(&path).unwrap();
        assert!(!retrieved.macros.contains_key("a"));
        assert!(retrieved.macros.contains_key("b"));
    }
}
