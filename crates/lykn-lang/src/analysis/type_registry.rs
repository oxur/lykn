use std::collections::HashMap;

use crate::diagnostics::{Diagnostic, Severity};
use crate::reader::source_loc::Span;

/// A single field within a constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDef {
    pub name: String,
    pub type_keyword: String,
}

/// A constructor belonging to a sum type.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstructorDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub owning_type: String,
    pub span: Span,
}

/// A type definition with its constructors.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDef {
    pub name: String,
    pub module_path: Option<String>,
    pub constructors: Vec<ConstructorDef>,
    /// Blessed types (e.g., `Option`, `Result`) receive enhanced diagnostic
    /// messages.
    pub is_blessed: bool,
    pub span: Span,
}

/// Central registry mapping type names to definitions and constructor names
/// to their owning types.
#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    types: HashMap<String, TypeDef>,
    constructor_to_type: HashMap<String, String>,
}

impl TypeRegistry {
    /// Register a type definition. Returns an error diagnostic if any
    /// constructor name is already registered to another type.
    pub fn register_type(&mut self, typedef: TypeDef) -> Result<(), Diagnostic> {
        for ctor in &typedef.constructors {
            if let Some(existing) = self.constructor_to_type.get(&ctor.name) {
                return Err(Diagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "duplicate constructor '{}' (already defined in type '{}')",
                        ctor.name, existing
                    ),
                    span: ctor.span,
                    suggestion: None,
                });
            }
        }
        for ctor in &typedef.constructors {
            self.constructor_to_type
                .insert(ctor.name.clone(), typedef.name.clone());
        }
        self.types.insert(typedef.name.clone(), typedef);
        Ok(())
    }

    /// Look up a type definition by name.
    pub fn lookup_type(&self, name: &str) -> Option<&TypeDef> {
        self.types.get(name)
    }

    /// Look up a constructor definition by name.
    pub fn lookup_constructor(&self, name: &str) -> Option<&ConstructorDef> {
        let type_name = self.constructor_to_type.get(name)?;
        let typedef = self.types.get(type_name)?;
        typedef.constructors.iter().find(|c| c.name == name)
    }

    /// Get the type definition that owns a given constructor.
    pub fn owning_type_of(&self, constructor: &str) -> Option<&TypeDef> {
        let type_name = self.constructor_to_type.get(constructor)?;
        self.types.get(type_name)
    }

    /// Return all constructors belonging to a named type.
    pub fn all_constructors_of(&self, type_name: &str) -> Vec<&ConstructorDef> {
        self.types
            .get(type_name)
            .map_or(Vec::new(), |td| td.constructors.iter().collect())
    }

    /// Check whether a given name is a registered constructor.
    pub fn is_constructor(&self, name: &str) -> bool {
        self.constructor_to_type.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::source_loc::Span;

    fn span() -> Span {
        Span::default()
    }

    fn option_type() -> TypeDef {
        TypeDef {
            name: "Option".into(),
            module_path: None,
            constructors: vec![
                ConstructorDef {
                    name: "Some".into(),
                    fields: vec![FieldDef {
                        name: "value".into(),
                        type_keyword: "any".into(),
                    }],
                    owning_type: "Option".into(),
                    span: span(),
                },
                ConstructorDef {
                    name: "None".into(),
                    fields: vec![],
                    owning_type: "Option".into(),
                    span: span(),
                },
            ],
            is_blessed: true,
            span: span(),
        }
    }

    #[test]
    fn test_register_and_lookup_type() {
        let mut reg = TypeRegistry::default();
        reg.register_type(option_type()).unwrap();

        let td = reg.lookup_type("Option").unwrap();
        assert_eq!(td.name, "Option");
        assert_eq!(td.constructors.len(), 2);
    }

    #[test]
    fn test_lookup_constructor() {
        let mut reg = TypeRegistry::default();
        reg.register_type(option_type()).unwrap();

        let ctor = reg.lookup_constructor("Some").unwrap();
        assert_eq!(ctor.owning_type, "Option");
        assert_eq!(ctor.fields.len(), 1);

        let ctor = reg.lookup_constructor("None").unwrap();
        assert_eq!(ctor.fields.len(), 0);
    }

    #[test]
    fn test_owning_type_of() {
        let mut reg = TypeRegistry::default();
        reg.register_type(option_type()).unwrap();

        let td = reg.owning_type_of("Some").unwrap();
        assert_eq!(td.name, "Option");

        assert!(reg.owning_type_of("Nonexistent").is_none());
    }

    #[test]
    fn test_all_constructors_of() {
        let mut reg = TypeRegistry::default();
        reg.register_type(option_type()).unwrap();

        let ctors = reg.all_constructors_of("Option");
        assert_eq!(ctors.len(), 2);

        let ctors = reg.all_constructors_of("Nonexistent");
        assert!(ctors.is_empty());
    }

    #[test]
    fn test_duplicate_constructor_detection() {
        let mut reg = TypeRegistry::default();
        reg.register_type(option_type()).unwrap();

        let bad = TypeDef {
            name: "Maybe".into(),
            module_path: None,
            constructors: vec![ConstructorDef {
                name: "Some".into(), // conflicts with Option::Some
                fields: vec![],
                owning_type: "Maybe".into(),
                span: span(),
            }],
            is_blessed: false,
            span: span(),
        };

        let err = reg.register_type(bad).unwrap_err();
        assert_eq!(err.severity, Severity::Error);
        assert!(err.message.contains("duplicate constructor 'Some'"));
        assert!(err.message.contains("Option"));
    }

    #[test]
    fn test_is_constructor() {
        let mut reg = TypeRegistry::default();
        reg.register_type(option_type()).unwrap();

        assert!(reg.is_constructor("Some"));
        assert!(reg.is_constructor("None"));
        assert!(!reg.is_constructor("Option"));
        assert!(!reg.is_constructor("Nonexistent"));
    }

    #[test]
    fn test_blessed_type_flag() {
        let mut reg = TypeRegistry::default();
        reg.register_type(option_type()).unwrap();

        let td = reg.lookup_type("Option").unwrap();
        assert!(td.is_blessed);
    }

    #[test]
    fn test_lookup_nonexistent_type() {
        let reg = TypeRegistry::default();
        assert!(reg.lookup_type("Foo").is_none());
        assert!(reg.lookup_constructor("Foo").is_none());
    }
}
