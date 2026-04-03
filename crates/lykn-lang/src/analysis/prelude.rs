use super::type_registry::{ConstructorDef, FieldDef, TypeDef, TypeRegistry};
use crate::reader::source_loc::Span;

/// Register the built-in (blessed) types that every lykn program has
/// access to: `Option` and `Result`.
pub fn register_prelude_types(registry: &mut TypeRegistry) {
    let _ = registry.register_type(TypeDef {
        name: "Option".into(),
        module_path: Some("lykn/core/option".into()),
        constructors: vec![
            ConstructorDef {
                name: "Some".into(),
                fields: vec![FieldDef {
                    name: "value".into(),
                    type_keyword: "any".into(),
                }],
                owning_type: "Option".into(),
                span: Span::default(),
            },
            ConstructorDef {
                name: "None".into(),
                fields: vec![],
                owning_type: "Option".into(),
                span: Span::default(),
            },
        ],
        is_blessed: true,
        span: Span::default(),
    });

    let _ = registry.register_type(TypeDef {
        name: "Result".into(),
        module_path: Some("lykn/core/result".into()),
        constructors: vec![
            ConstructorDef {
                name: "Ok".into(),
                fields: vec![FieldDef {
                    name: "value".into(),
                    type_keyword: "any".into(),
                }],
                owning_type: "Result".into(),
                span: Span::default(),
            },
            ConstructorDef {
                name: "Err".into(),
                fields: vec![FieldDef {
                    name: "error".into(),
                    type_keyword: "any".into(),
                }],
                owning_type: "Result".into(),
                span: Span::default(),
            },
        ],
        is_blessed: true,
        span: Span::default(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prelude_registers_option() {
        let mut reg = TypeRegistry::default();
        register_prelude_types(&mut reg);

        let opt = reg.lookup_type("Option").unwrap();
        assert!(opt.is_blessed);
        assert_eq!(opt.constructors.len(), 2);
        assert_eq!(opt.module_path.as_deref(), Some("lykn/core/option"));
    }

    #[test]
    fn test_prelude_registers_result() {
        let mut reg = TypeRegistry::default();
        register_prelude_types(&mut reg);

        let res = reg.lookup_type("Result").unwrap();
        assert!(res.is_blessed);
        assert_eq!(res.constructors.len(), 2);
        assert_eq!(res.module_path.as_deref(), Some("lykn/core/result"));
    }

    #[test]
    fn test_prelude_constructors_lookup() {
        let mut reg = TypeRegistry::default();
        register_prelude_types(&mut reg);

        assert!(reg.lookup_constructor("Some").is_some());
        assert!(reg.lookup_constructor("None").is_some());
        assert!(reg.lookup_constructor("Ok").is_some());
        assert!(reg.lookup_constructor("Err").is_some());
    }

    #[test]
    fn test_prelude_constructor_fields() {
        let mut reg = TypeRegistry::default();
        register_prelude_types(&mut reg);

        let some = reg.lookup_constructor("Some").unwrap();
        assert_eq!(some.fields.len(), 1);
        assert_eq!(some.fields[0].name, "value");

        let none = reg.lookup_constructor("None").unwrap();
        assert!(none.fields.is_empty());

        let ok = reg.lookup_constructor("Ok").unwrap();
        assert_eq!(ok.fields.len(), 1);

        let err = reg.lookup_constructor("Err").unwrap();
        assert_eq!(err.fields.len(), 1);
        assert_eq!(err.fields[0].name, "error");
    }
}
