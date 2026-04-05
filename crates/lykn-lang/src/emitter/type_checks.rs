use crate::ast::sexpr::SExpr;
use crate::reader::source_loc::Span;

use super::forms::{atom, list, str_lit};

/// Emit a runtime type-check assertion for a parameter.
///
/// Returns `None` for `:any` (no check needed). Otherwise returns a kernel
/// `(if <check> (throw (new TypeError <message>)))` form.
///
/// The `label` parameter describes the role of the value being checked
/// (e.g., `"arg"`, `"field"`, `"return"`).
pub fn emit_type_check(
    param_name: &str,
    type_keyword: &str,
    func_name: &str,
    label: &str,
    _span: Span,
) -> Option<SExpr> {
    let check = build_check(param_name, type_keyword)?;
    let message = build_error_message(param_name, type_keyword, func_name, label);
    Some(list(vec![
        atom("if"),
        check,
        list(vec![
            atom("throw"),
            list(vec![atom("new"), atom("TypeError"), message]),
        ]),
    ]))
}

/// Build the negated type-check condition for the given type keyword.
///
/// Returns `None` for `:any`.
fn build_check(param_name: &str, type_keyword: &str) -> Option<SExpr> {
    let p = atom(param_name);
    match type_keyword {
        "any" => None,
        "number" => Some(list(vec![
            atom("||"),
            list(vec![
                atom("!=="),
                list(vec![atom("typeof"), p.clone()]),
                str_lit("number"),
            ]),
            list(vec![atom("Number:isNaN"), p]),
        ])),
        "string" => Some(list(vec![
            atom("!=="),
            list(vec![atom("typeof"), p]),
            str_lit("string"),
        ])),
        "boolean" => Some(list(vec![
            atom("!=="),
            list(vec![atom("typeof"), p]),
            str_lit("boolean"),
        ])),
        "function" => Some(list(vec![
            atom("!=="),
            list(vec![atom("typeof"), p]),
            str_lit("function"),
        ])),
        "object" => Some(list(vec![
            atom("||"),
            list(vec![
                atom("!=="),
                list(vec![atom("typeof"), p.clone()]),
                str_lit("object"),
            ]),
            list(vec![atom("==="), p, atom("null")]),
        ])),
        "array" => Some(list(vec![atom("!"), list(vec![atom("Array:isArray"), p])])),
        "promise" => Some(list(vec![
            atom("||"),
            list(vec![
                atom("!=="),
                list(vec![atom("typeof"), p.clone()]),
                str_lit("object"),
            ]),
            list(vec![
                atom("||"),
                list(vec![atom("==="), p.clone(), atom("null")]),
                list(vec![
                    atom("!"),
                    list(vec![atom("instanceof"), p, atom("Promise")]),
                ]),
            ]),
        ])),
        // User-defined type: check it's a tagged object
        _ => Some(list(vec![
            atom("||"),
            list(vec![
                atom("!=="),
                list(vec![atom("typeof"), p.clone()]),
                str_lit("object"),
            ]),
            list(vec![
                atom("||"),
                list(vec![atom("==="), p.clone(), atom("null")]),
                list(vec![atom("!"), list(vec![atom("in"), str_lit("tag"), p])]),
            ]),
        ])),
    }
}

/// Emit a return-type check that uses a user-friendly label in the error
/// message instead of the gensym variable name.
///
/// The `result_var` is the gensym variable holding the return value (used
/// for the `typeof` check), but the error message says `"return value"`
/// instead of `"result__gensym0"`.
pub fn emit_return_type_check(
    result_var: &str,
    type_keyword: &str,
    func_name: &str,
    span: Span,
) -> Option<SExpr> {
    let check = build_check(result_var, type_keyword)?;
    let message = list(vec![
        atom("+"),
        str_lit(&format!(
            "{func_name}: return value expected {type_keyword}, got "
        )),
        list(vec![atom("typeof"), atom(result_var)]),
    ]);
    let _ = span;
    Some(list(vec![
        atom("if"),
        check,
        list(vec![
            atom("throw"),
            list(vec![atom("new"), atom("TypeError"), message]),
        ]),
    ]))
}

/// Build the error message expression for a type mismatch.
///
/// Produces: `"funcName: label 'paramName' expected type, got " + (typeof param)`
fn build_error_message(
    param_name: &str,
    type_keyword: &str,
    func_name: &str,
    label: &str,
) -> SExpr {
    list(vec![
        atom("+"),
        str_lit(&format!(
            "{func_name}: {label} '{param_name}' expected {type_keyword}, got "
        )),
        list(vec![atom("typeof"), atom(param_name)]),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_any_returns_none() {
        let result = emit_type_check("x", "any", "foo", "arg", Span::default());
        assert!(result.is_none());
    }

    #[test]
    fn test_number_check() {
        let result = emit_type_check("x", "number", "foo", "arg", Span::default()).unwrap();
        // Should be: (if (|| (!== (typeof x) "number") (Number:isNaN x)) (throw ...))
        if let SExpr::List { values, .. } = &result {
            assert_eq!(values[0].as_atom(), Some("if"));
            if let SExpr::List {
                values: or_vals, ..
            } = &values[1]
            {
                assert_eq!(or_vals[0].as_atom(), Some("||"));
            } else {
                panic!("expected list for check");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_string_check() {
        let result = emit_type_check("s", "string", "greet", "arg", Span::default()).unwrap();
        if let SExpr::List { values, .. } = &result {
            assert_eq!(values[0].as_atom(), Some("if"));
            // check is (!== (typeof s) "string")
            if let SExpr::List { values: check, .. } = &values[1] {
                assert_eq!(check[0].as_atom(), Some("!=="));
            } else {
                panic!("expected !== check");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_boolean_check() {
        let result = emit_type_check("b", "boolean", "f", "arg", Span::default()).unwrap();
        assert!(result.is_list());
    }

    #[test]
    fn test_function_check() {
        let result = emit_type_check("cb", "function", "f", "arg", Span::default()).unwrap();
        assert!(result.is_list());
    }

    #[test]
    fn test_object_check() {
        let result = emit_type_check("o", "object", "f", "arg", Span::default()).unwrap();
        if let SExpr::List { values, .. } = &result {
            if let SExpr::List { values: check, .. } = &values[1] {
                assert_eq!(check[0].as_atom(), Some("||"));
            } else {
                panic!("expected || check");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_array_check() {
        let result = emit_type_check("a", "array", "f", "arg", Span::default()).unwrap();
        if let SExpr::List { values, .. } = &result {
            if let SExpr::List { values: check, .. } = &values[1] {
                assert_eq!(check[0].as_atom(), Some("!"));
            } else {
                panic!("expected ! check");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_promise_check() {
        let result = emit_type_check("p", "promise", "f", "arg", Span::default()).unwrap();
        if let SExpr::List { values, .. } = &result {
            if let SExpr::List { values: check, .. } = &values[1] {
                // (|| (!== (typeof p) "object") (|| (=== p null) (! (instanceof p Promise))))
                assert_eq!(check[0].as_atom(), Some("||"));
            } else {
                panic!("expected || check");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_user_defined_type_check() {
        let result = emit_type_check("p", "Person", "f", "arg", Span::default()).unwrap();
        if let SExpr::List { values, .. } = &result {
            if let SExpr::List { values: check, .. } = &values[1] {
                // (|| (!== (typeof p) "object") (|| (=== p null) (! (in "tag" p))))
                assert_eq!(check[0].as_atom(), Some("||"));
            } else {
                panic!("expected || check");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_error_message_format() {
        let result = emit_type_check("x", "number", "add", "arg", Span::default()).unwrap();
        // Dig into (throw (new TypeError <msg>))
        if let SExpr::List { values, .. } = &result {
            if let SExpr::List {
                values: throw_vals, ..
            } = &values[2]
            {
                if let SExpr::List {
                    values: new_vals, ..
                } = &throw_vals[1]
                {
                    // new_vals = [new, TypeError, <msg>]
                    if let SExpr::List { values: msg, .. } = &new_vals[2] {
                        // msg = [+, "add: arg 'x' expected number, got ", (typeof x)]
                        assert_eq!(msg[0].as_atom(), Some("+"));
                        if let SExpr::String { value, .. } = &msg[1] {
                            assert!(value.contains("add"));
                            assert!(value.contains("arg"));
                            assert!(value.contains("'x'"));
                            assert!(value.contains("number"));
                        } else {
                            panic!("expected string literal in error message");
                        }
                    } else {
                        panic!("expected + expression for message");
                    }
                } else {
                    panic!("expected new expression");
                }
            } else {
                panic!("expected throw expression");
            }
        } else {
            panic!("expected if expression");
        }
    }
}
