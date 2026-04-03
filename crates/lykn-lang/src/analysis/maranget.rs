//! Core Maranget 2007 usefulness algorithm for exhaustiveness and
//! reachability checking of pattern matches.

use super::pattern::{DeconPattern, LiteralKind};
use super::type_registry::TypeRegistry;

/// A pattern matrix: each row is a vector of `DeconPattern` columns.
pub type PatternMatrix = Vec<Vec<DeconPattern>>;

/// Check if pattern vector `q` is useful with respect to matrix `p`.
///
/// A vector is useful if there exists a value matched by `q` but not by any
/// row in `p`. This is the core of Maranget's algorithm.
pub fn is_useful(matrix: &PatternMatrix, q: &[DeconPattern], registry: &TypeRegistry) -> bool {
    // Base cases
    if q.is_empty() {
        return matrix.is_empty();
    }
    if matrix.is_empty() {
        return true;
    }

    let head = &q[0];
    let rest = &q[1..];

    match head {
        DeconPattern::Constructor {
            ctor_name, fields, ..
        } => {
            let specialized = specialize(matrix, ctor_name, fields.len());
            let mut new_q = fields.clone();
            new_q.extend_from_slice(rest);
            is_useful(&specialized, &new_q, registry)
        }
        DeconPattern::Literal(lit) => {
            let specialized = specialize_literal(matrix, lit);
            is_useful(&specialized, rest, registry)
        }
        DeconPattern::TypeKeyword(kw) => {
            let specialized = specialize_type_keyword(matrix, kw);
            is_useful(&specialized, rest, registry)
        }
        DeconPattern::Wildcard => {
            let head_ctors = collect_head_constructors(matrix);

            if is_complete_signature(&head_ctors, registry) {
                // All constructors of the type are present -- specialize each
                for (ctor_name, _type_name, arity) in &head_ctors {
                    let specialized = specialize(matrix, ctor_name, *arity);
                    let mut new_q: Vec<DeconPattern> = vec![DeconPattern::Wildcard; *arity];
                    new_q.extend_from_slice(rest);
                    if is_useful(&specialized, &new_q, registry) {
                        return true;
                    }
                }
                false
            } else {
                let default = default_matrix(matrix);
                is_useful(&default, rest, registry)
            }
        }
        DeconPattern::Structural { .. } => {
            // Structural patterns are open, treat like wildcard for
            // exhaustiveness.
            let default = default_matrix(matrix);
            is_useful(&default, rest, registry)
        }
    }
}

/// Check if the pattern matrix is exhaustive for the given number of columns.
pub fn is_exhaustive(matrix: &PatternMatrix, col_count: usize, registry: &TypeRegistry) -> bool {
    let witness = vec![DeconPattern::Wildcard; col_count];
    !is_useful(matrix, &witness, registry)
}

/// Find uncovered pattern witnesses -- constructor-level patterns that are
/// not matched by the matrix.
pub fn find_uncovered(
    matrix: &PatternMatrix,
    col_count: usize,
    registry: &TypeRegistry,
) -> Vec<Vec<DeconPattern>> {
    let mut uncovered = Vec::new();
    let head_ctors = collect_head_constructors(matrix);

    if head_ctors.is_empty() {
        if !is_exhaustive(matrix, col_count, registry) {
            uncovered.push(vec![DeconPattern::Wildcard; col_count]);
        }
        return uncovered;
    }

    // Get the type and enumerate all its constructors
    if let Some((_, type_name, _)) = head_ctors.first()
        && let Some(typedef) = registry.lookup_type(type_name)
    {
        for ctor in &typedef.constructors {
            let arity = ctor.fields.len();
            let specialized = specialize(matrix, &ctor.name, arity);
            let witness_cols = arity + col_count.saturating_sub(1);
            let witness = vec![DeconPattern::Wildcard; witness_cols];
            if is_useful(&specialized, &witness, registry) {
                uncovered.push(vec![DeconPattern::Constructor {
                    type_name: type_name.clone(),
                    ctor_name: ctor.name.clone(),
                    fields: vec![DeconPattern::Wildcard; arity],
                }]);
            }
        }
    }
    uncovered
}

/// Specialize a matrix by a constructor: keep rows whose first column matches
/// the constructor (expanding fields) or is a wildcard (expanding to arity
/// wildcards).
fn specialize(matrix: &PatternMatrix, ctor_name: &str, arity: usize) -> PatternMatrix {
    let mut result = Vec::new();
    for row in matrix {
        if row.is_empty() {
            continue;
        }
        match &row[0] {
            DeconPattern::Constructor {
                ctor_name: cn,
                fields,
                ..
            } if cn == ctor_name => {
                let mut new_row = fields.clone();
                new_row.extend_from_slice(&row[1..]);
                result.push(new_row);
            }
            DeconPattern::Wildcard => {
                let mut new_row = vec![DeconPattern::Wildcard; arity];
                new_row.extend_from_slice(&row[1..]);
                result.push(new_row);
            }
            _ => {
                // Different constructor or non-constructor -- skip
            }
        }
    }
    result
}

/// Specialize by a literal value: keep rows whose first column matches the
/// literal or is a wildcard.
fn specialize_literal(matrix: &PatternMatrix, lit: &LiteralKind) -> PatternMatrix {
    let mut result = Vec::new();
    for row in matrix {
        if row.is_empty() {
            continue;
        }
        match &row[0] {
            DeconPattern::Literal(l) if l == lit => {
                result.push(row[1..].to_vec());
            }
            DeconPattern::Wildcard => {
                result.push(row[1..].to_vec());
            }
            _ => {}
        }
    }
    result
}

/// Specialize by a type keyword.
fn specialize_type_keyword(matrix: &PatternMatrix, kw: &str) -> PatternMatrix {
    let mut result = Vec::new();
    for row in matrix {
        if row.is_empty() {
            continue;
        }
        match &row[0] {
            DeconPattern::TypeKeyword(k) if k == kw => {
                result.push(row[1..].to_vec());
            }
            DeconPattern::Wildcard => {
                result.push(row[1..].to_vec());
            }
            _ => {}
        }
    }
    result
}

/// Build the default matrix: rows whose first column is a wildcard, with the
/// first column removed.
fn default_matrix(matrix: &PatternMatrix) -> PatternMatrix {
    let mut result = Vec::new();
    for row in matrix {
        if row.is_empty() {
            continue;
        }
        if matches!(
            row[0],
            DeconPattern::Wildcard | DeconPattern::Structural { .. }
        ) {
            result.push(row[1..].to_vec());
        }
    }
    result
}

/// Collect all distinct constructors appearing in the first column of the
/// matrix. Returns `(ctor_name, type_name, arity)` triples.
fn collect_head_constructors(matrix: &PatternMatrix) -> Vec<(String, String, usize)> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for row in matrix {
        if row.is_empty() {
            continue;
        }
        if let DeconPattern::Constructor {
            type_name,
            ctor_name,
            fields,
        } = &row[0]
            && seen.insert(ctor_name.clone())
        {
            result.push((ctor_name.clone(), type_name.clone(), fields.len()));
        }
    }
    result
}

/// Check whether the set of constructors covers all variants of their type.
fn is_complete_signature(ctors: &[(String, String, usize)], registry: &TypeRegistry) -> bool {
    if ctors.is_empty() {
        return false;
    }
    let type_name = &ctors[0].1;
    let Some(typedef) = registry.lookup_type(type_name) else {
        return false;
    };
    let ctor_names: std::collections::HashSet<&str> =
        ctors.iter().map(|(name, _, _)| name.as_str()).collect();
    typedef
        .constructors
        .iter()
        .all(|c| ctor_names.contains(c.name.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::prelude::register_prelude_types;
    use crate::analysis::type_registry::{ConstructorDef, FieldDef, TypeDef, TypeRegistry};
    use crate::reader::source_loc::Span;

    fn span() -> Span {
        Span::default()
    }

    fn option_registry() -> TypeRegistry {
        let mut r = TypeRegistry::default();
        register_prelude_types(&mut r);
        r
    }

    fn color_registry() -> TypeRegistry {
        let mut r = option_registry();
        r.register_type(TypeDef {
            name: "Color".into(),
            module_path: None,
            constructors: vec![
                ConstructorDef {
                    name: "Red".into(),
                    fields: vec![],
                    owning_type: "Color".into(),
                    span: span(),
                },
                ConstructorDef {
                    name: "Green".into(),
                    fields: vec![],
                    owning_type: "Color".into(),
                    span: span(),
                },
                ConstructorDef {
                    name: "Blue".into(),
                    fields: vec![],
                    owning_type: "Color".into(),
                    span: span(),
                },
            ],
            is_blessed: false,
            span: span(),
        })
        .unwrap();
        r
    }

    fn ctor(type_name: &str, name: &str, fields: Vec<DeconPattern>) -> DeconPattern {
        DeconPattern::Constructor {
            type_name: type_name.into(),
            ctor_name: name.into(),
            fields,
        }
    }

    // --- Exhaustiveness tests ---

    #[test]
    fn test_exhaustive_option_both_variants() {
        let reg = option_registry();
        let matrix = vec![
            vec![ctor("Option", "Some", vec![DeconPattern::Wildcard])],
            vec![ctor("Option", "None", vec![])],
        ];
        assert!(is_exhaustive(&matrix, 1, &reg));
    }

    #[test]
    fn test_non_exhaustive_option_missing_none() {
        let reg = option_registry();
        let matrix = vec![vec![ctor("Option", "Some", vec![DeconPattern::Wildcard])]];
        assert!(!is_exhaustive(&matrix, 1, &reg));
    }

    #[test]
    fn test_non_exhaustive_option_missing_some() {
        let reg = option_registry();
        let matrix = vec![vec![ctor("Option", "None", vec![])]];
        assert!(!is_exhaustive(&matrix, 1, &reg));
    }

    #[test]
    fn test_exhaustive_wildcard_covers_all() {
        let reg = option_registry();
        let matrix = vec![
            vec![ctor("Option", "Some", vec![DeconPattern::Wildcard])],
            vec![DeconPattern::Wildcard],
        ];
        assert!(is_exhaustive(&matrix, 1, &reg));
    }

    #[test]
    fn test_exhaustive_single_wildcard() {
        let reg = option_registry();
        let matrix = vec![vec![DeconPattern::Wildcard]];
        assert!(is_exhaustive(&matrix, 1, &reg));
    }

    #[test]
    fn test_exhaustive_color_all_three() {
        let reg = color_registry();
        let matrix = vec![
            vec![ctor("Color", "Red", vec![])],
            vec![ctor("Color", "Green", vec![])],
            vec![ctor("Color", "Blue", vec![])],
        ];
        assert!(is_exhaustive(&matrix, 1, &reg));
    }

    #[test]
    fn test_non_exhaustive_color_missing_blue() {
        let reg = color_registry();
        let matrix = vec![
            vec![ctor("Color", "Red", vec![])],
            vec![ctor("Color", "Green", vec![])],
        ];
        assert!(!is_exhaustive(&matrix, 1, &reg));
    }

    // --- Nested patterns ---

    #[test]
    fn test_exhaustive_nested_option() {
        let reg = option_registry();
        // match on Option<Option<_>>:
        // Some(Some(_)), Some(None), None
        let matrix = vec![
            vec![ctor(
                "Option",
                "Some",
                vec![ctor("Option", "Some", vec![DeconPattern::Wildcard])],
            )],
            vec![ctor("Option", "Some", vec![ctor("Option", "None", vec![])])],
            vec![ctor("Option", "None", vec![])],
        ];
        assert!(is_exhaustive(&matrix, 1, &reg));
    }

    #[test]
    fn test_non_exhaustive_nested_option_missing_inner_none() {
        let reg = option_registry();
        // Missing Some(None)
        let matrix = vec![
            vec![ctor(
                "Option",
                "Some",
                vec![ctor("Option", "Some", vec![DeconPattern::Wildcard])],
            )],
            vec![ctor("Option", "None", vec![])],
        ];
        assert!(!is_exhaustive(&matrix, 1, &reg));
    }

    // --- Literal patterns ---

    #[test]
    fn test_literals_not_exhaustive_without_wildcard() {
        let reg = option_registry();
        let matrix = vec![
            vec![DeconPattern::Literal(LiteralKind::Number(
                1.0_f64.to_bits(),
            ))],
            vec![DeconPattern::Literal(LiteralKind::Number(
                2.0_f64.to_bits(),
            ))],
        ];
        // Literals are open-type, so not exhaustive without a wildcard
        assert!(!is_exhaustive(&matrix, 1, &reg));
    }

    #[test]
    fn test_literals_exhaustive_with_wildcard() {
        let reg = option_registry();
        let matrix = vec![
            vec![DeconPattern::Literal(LiteralKind::Number(
                1.0_f64.to_bits(),
            ))],
            vec![DeconPattern::Wildcard],
        ];
        assert!(is_exhaustive(&matrix, 1, &reg));
    }

    // --- Usefulness (reachability) tests ---

    #[test]
    fn test_first_clause_always_useful() {
        let reg = option_registry();
        let matrix: PatternMatrix = vec![];
        let q = vec![ctor("Option", "Some", vec![DeconPattern::Wildcard])];
        assert!(is_useful(&matrix, &q, &reg));
    }

    #[test]
    fn test_clause_after_wildcard_not_useful() {
        let reg = option_registry();
        let matrix = vec![vec![DeconPattern::Wildcard]];
        let q = vec![ctor("Option", "Some", vec![DeconPattern::Wildcard])];
        assert!(!is_useful(&matrix, &q, &reg));
    }

    #[test]
    fn test_none_useful_after_some() {
        let reg = option_registry();
        let matrix = vec![vec![ctor("Option", "Some", vec![DeconPattern::Wildcard])]];
        let q = vec![ctor("Option", "None", vec![])];
        assert!(is_useful(&matrix, &q, &reg));
    }

    #[test]
    fn test_duplicate_pattern_not_useful() {
        let reg = option_registry();
        let matrix = vec![vec![ctor("Option", "None", vec![])]];
        let q = vec![ctor("Option", "None", vec![])];
        assert!(!is_useful(&matrix, &q, &reg));
    }

    #[test]
    fn test_distinct_literals_useful() {
        let reg = option_registry();
        let matrix = vec![vec![DeconPattern::Literal(LiteralKind::Number(
            1.0_f64.to_bits(),
        ))]];
        let q = vec![DeconPattern::Literal(LiteralKind::Number(
            2.0_f64.to_bits(),
        ))];
        assert!(is_useful(&matrix, &q, &reg));
    }

    #[test]
    fn test_same_literal_not_useful() {
        let reg = option_registry();
        let matrix = vec![vec![DeconPattern::Literal(LiteralKind::Number(
            1.0_f64.to_bits(),
        ))]];
        let q = vec![DeconPattern::Literal(LiteralKind::Number(
            1.0_f64.to_bits(),
        ))];
        assert!(!is_useful(&matrix, &q, &reg));
    }

    // --- find_uncovered ---

    #[test]
    fn test_find_uncovered_missing_none() {
        let reg = option_registry();
        let matrix = vec![vec![ctor("Option", "Some", vec![DeconPattern::Wildcard])]];
        let uncov = find_uncovered(&matrix, 1, &reg);
        assert_eq!(uncov.len(), 1);
        assert!(matches!(
            &uncov[0][0],
            DeconPattern::Constructor { ctor_name, .. } if ctor_name == "None"
        ));
    }

    #[test]
    fn test_find_uncovered_all_covered() {
        let reg = option_registry();
        let matrix = vec![
            vec![ctor("Option", "Some", vec![DeconPattern::Wildcard])],
            vec![ctor("Option", "None", vec![])],
        ];
        let uncov = find_uncovered(&matrix, 1, &reg);
        assert!(uncov.is_empty());
    }

    #[test]
    fn test_find_uncovered_color_missing_two() {
        let reg = color_registry();
        let matrix = vec![vec![ctor("Color", "Red", vec![])]];
        let uncov = find_uncovered(&matrix, 1, &reg);
        assert_eq!(uncov.len(), 2);
        let names: Vec<&str> = uncov
            .iter()
            .filter_map(|row| match &row[0] {
                DeconPattern::Constructor { ctor_name, .. } => Some(ctor_name.as_str()),
                _ => None,
            })
            .collect();
        assert!(names.contains(&"Green"));
        assert!(names.contains(&"Blue"));
    }

    // --- Multi-column ---

    #[test]
    fn test_multi_column_exhaustive() {
        let reg = option_registry();
        // Two columns: (Option, Option) -- need all 4 combos or wildcards
        let matrix = vec![
            vec![
                ctor("Option", "Some", vec![DeconPattern::Wildcard]),
                DeconPattern::Wildcard,
            ],
            vec![ctor("Option", "None", vec![]), DeconPattern::Wildcard],
        ];
        assert!(is_exhaustive(&matrix, 2, &reg));
    }

    #[test]
    fn test_multi_column_not_exhaustive() {
        let reg = option_registry();
        let matrix = vec![vec![
            ctor("Option", "Some", vec![DeconPattern::Wildcard]),
            ctor("Option", "Some", vec![DeconPattern::Wildcard]),
        ]];
        assert!(!is_exhaustive(&matrix, 2, &reg));
    }

    // --- Empty matrix ---

    #[test]
    fn test_empty_matrix_not_exhaustive() {
        let reg = option_registry();
        assert!(!is_exhaustive(&vec![], 1, &reg));
    }

    #[test]
    fn test_empty_columns_exhaustive_with_rows() {
        let reg = option_registry();
        // 0 columns, 1 row: exhaustive (trivially)
        assert!(is_exhaustive(&vec![vec![]], 0, &reg));
    }

    #[test]
    fn test_empty_columns_not_exhaustive_without_rows() {
        let reg = option_registry();
        // 0 columns, 0 rows: not exhaustive
        assert!(!is_exhaustive(&vec![], 0, &reg));
    }
}
