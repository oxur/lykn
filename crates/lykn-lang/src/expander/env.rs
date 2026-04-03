//! Macro environment API and JSON protocol for Deno interop.
//!
//! This module provides:
//! - The JavaScript source that defines the macro helper API (`$array`,
//!   `$sym`, `$gensym`, etc.) available inside macro bodies.
//! - The Deno evaluator script that runs as a long-lived subprocess,
//!   accepting JSON-line requests on stdin and returning results on stdout.
//! - Bidirectional conversion between [`SExpr`] and the JSON wire format
//!   used by the Deno protocol.

use crate::ast::sexpr::SExpr;
use crate::error::LyknError;
use crate::reader::source_loc::{SourceLoc, Span};
use serde_json::Value;

/// JavaScript source defining the macro helper API.
///
/// These functions are injected into the Deno evaluator's global scope before
/// any macro body is executed. They operate on the JSON wire representation
/// of S-expressions (objects with `type` and `value`/`values` fields).
pub const MACRO_ENV_JS: &str = r#"
let __gensymCounter = 0;
const $array = (...items) => ({ type: "list", values: items });
const $sym = (name) => ({ type: "atom", value: String(name) });
const $gensym = (prefix = "g") => ({ type: "atom", value: `${prefix}__gensym${__gensymCounter++}` });
const $isArray = (x) => x !== null && x !== undefined && x.type === "list";
const $isSymbol = (x) => x !== null && x !== undefined && x.type === "atom";
const $isNumber = (x) => x !== null && x !== undefined && x.type === "number";
const $isString = (x) => x !== null && x !== undefined && x.type === "string";
const $isKeyword = (x) => x !== null && x !== undefined && x.type === "keyword";
const $first = (arr) => arr.values[0];
const $rest = (arr) => ({ type: "list", values: arr.values.slice(1) });
const $concat = (...arrays) => {
    const values = [];
    for (const arr of arrays) {
        if (Array.isArray(arr)) values.push(...arr);
        else if (arr && arr.type === "list") values.push(...arr.values);
    }
    return { type: "list", values };
};
const $append = $concat;
const $nth = (arr, n) => arr.values[n];
const $length = (arr) => arr.values.length;
"#;

/// JavaScript source for the long-lived Deno evaluator subprocess.
///
/// The evaluator reads JSON-line requests from stdin and writes JSON-line
/// responses to stdout. Supported actions:
///
/// - `"compile"` — compile a macro definition (lykn source) to a JS function
///   body, using the JS-side compiler.
/// - `"eval"` — evaluate a previously compiled macro with the given arguments.
/// - `"ping"` — health check; returns `"pong"`.
pub const DENO_EVALUATOR_JS: &str = r#"
const decoder = new TextDecoder();
const encoder = new TextEncoder();

async function readLine() {
    const buf = new Uint8Array(1);
    let line = "";
    while (true) {
        const n = await Deno.stdin.read(buf);
        if (n === null) return null;
        const ch = decoder.decode(buf.subarray(0, n));
        if (ch === "\n") return line;
        line += ch;
    }
}

function writeLine(s) {
    Deno.stdout.writeSync(encoder.encode(s + "\n"));
}

while (true) {
    const line = await readLine();
    if (line === null) break;

    try {
        const request = JSON.parse(line);

        if (request.action === "compile") {
            const { read } = await import("./src/reader.js");
            const { compileMacroBody, extractParamNames } = await import("./src/expander.js");

            const forms = read(request.source);
            const macroForm = forms[0];
            const paramsNode = macroForm.values[2];
            const bodyForms = macroForm.values.slice(3);
            const paramNames = extractParamNames(paramsNode);
            const jsBody = compileMacroBody(paramNames, paramsNode, bodyForms);

            writeLine(JSON.stringify({ ok: true, result: jsBody }));
        } else if (request.action === "eval") {
            const MACRO_API_PARAMS = [
                "$array", "$sym", "$gensym",
                "$isArray", "$isSymbol", "$isNumber", "$isString", "$isKeyword",
                "$first", "$rest", "$concat", "$nth", "$length",
                "$append",
            ];
            const macroFn = new Function(...MACRO_API_PARAMS, request.jsBody);
            const boundFn = macroFn(
                $array, $sym, $gensym,
                $isArray, $isSymbol, $isNumber, $isString, $isKeyword,
                $first, $rest, $concat, $nth, $length,
                $append,
            );
            const result = boundFn(...request.args);
            writeLine(JSON.stringify({ ok: true, result }));
        } else if (request.action === "ping") {
            writeLine(JSON.stringify({ ok: true, result: "pong" }));
        } else {
            writeLine(JSON.stringify({ ok: false, error: "unknown action: " + request.action }));
        }
    } catch (e) {
        writeLine(JSON.stringify({ ok: false, error: e.message }));
    }
}
"#;

/// Convert an [`SExpr`] to the JSON wire format used by the Deno protocol.
///
/// Each variant maps to a JSON object with a `"type"` discriminator and the
/// appropriate value field(s). Lists become `{ type: "list", values: [...] }`.
pub fn sexpr_to_protocol_json(expr: &SExpr) -> Value {
    match expr {
        SExpr::Atom { value, .. } => serde_json::json!({ "type": "atom", "value": value }),
        SExpr::Keyword { value, .. } => {
            serde_json::json!({ "type": "keyword", "value": value })
        }
        SExpr::String { value, .. } => {
            serde_json::json!({ "type": "string", "value": value })
        }
        SExpr::Number { value, .. } => {
            serde_json::json!({ "type": "number", "value": value })
        }
        SExpr::Bool { value, .. } => {
            serde_json::json!({ "type": "boolean", "value": value })
        }
        SExpr::Null { .. } => serde_json::json!({ "type": "atom", "value": "null" }),
        SExpr::List { values, .. } => {
            let arr: Vec<Value> = values.iter().map(sexpr_to_protocol_json).collect();
            serde_json::json!({ "type": "list", "values": arr })
        }
        SExpr::Cons { car, cdr, .. } => {
            serde_json::json!({
                "type": "cons",
                "car": sexpr_to_protocol_json(car),
                "cdr": sexpr_to_protocol_json(cdr),
            })
        }
    }
}

/// Convert a JSON value from the Deno protocol back into an [`SExpr`].
///
/// Objects with a `"type"` field are interpreted as typed S-expressions.
/// Raw JSON scalars (null, string, number, bool) are converted to the
/// corresponding `SExpr` variant with a default span.
pub fn protocol_json_to_sexpr(val: &Value) -> Result<SExpr, LyknError> {
    let span = Span::default();

    match val.get("type").and_then(|t| t.as_str()) {
        Some("atom") => {
            let value = val["value"].as_str().unwrap_or("").to_string();
            Ok(SExpr::Atom { value, span })
        }
        Some("keyword") => {
            let value = val["value"].as_str().unwrap_or("").to_string();
            Ok(SExpr::Keyword { value, span })
        }
        Some("string") => {
            let value = val["value"].as_str().unwrap_or("").to_string();
            Ok(SExpr::String { value, span })
        }
        Some("number") => {
            let value = val["value"].as_f64().unwrap_or(0.0);
            Ok(SExpr::Number { value, span })
        }
        Some("boolean") => {
            let value = val["value"].as_bool().unwrap_or(false);
            Ok(SExpr::Bool { value, span })
        }
        Some("list") => {
            let empty = vec![];
            let values = val["values"]
                .as_array()
                .unwrap_or(&empty)
                .iter()
                .map(protocol_json_to_sexpr)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(SExpr::List { values, span })
        }
        Some("cons") => {
            let car = protocol_json_to_sexpr(&val["car"])?;
            let cdr = protocol_json_to_sexpr(&val["cdr"])?;
            Ok(SExpr::Cons {
                car: Box::new(car),
                cdr: Box::new(cdr),
                span,
            })
        }
        Some(unknown) => Err(LyknError::Read {
            message: format!("unknown type in macro expansion result: {unknown:?}"),
            location: SourceLoc::default(),
        }),
        None => {
            // Handle raw JSON scalars that lack a "type" field.
            if val.is_null() {
                Ok(SExpr::Null { span })
            } else if let Some(s) = val.as_str() {
                Ok(SExpr::Atom {
                    value: s.to_string(),
                    span,
                })
            } else if let Some(n) = val.as_f64() {
                Ok(SExpr::Number { value: n, span })
            } else if let Some(b) = val.as_bool() {
                Ok(SExpr::Bool { value: b, span })
            } else {
                Err(LyknError::Read {
                    message: format!("invalid macro expansion result: {val}"),
                    location: SourceLoc::default(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s() -> Span {
        Span::default()
    }

    // ---------------------------------------------------------------
    // sexpr_to_protocol_json
    // ---------------------------------------------------------------

    #[test]
    fn test_atom_to_json() {
        let expr = SExpr::Atom {
            value: "foo".to_string(),
            span: s(),
        };
        let json = sexpr_to_protocol_json(&expr);
        assert_eq!(json["type"], "atom");
        assert_eq!(json["value"], "foo");
    }

    #[test]
    fn test_keyword_to_json() {
        let expr = SExpr::Keyword {
            value: "name".to_string(),
            span: s(),
        };
        let json = sexpr_to_protocol_json(&expr);
        assert_eq!(json["type"], "keyword");
        assert_eq!(json["value"], "name");
    }

    #[test]
    fn test_string_to_json() {
        let expr = SExpr::String {
            value: "hello".to_string(),
            span: s(),
        };
        let json = sexpr_to_protocol_json(&expr);
        assert_eq!(json["type"], "string");
        assert_eq!(json["value"], "hello");
    }

    #[test]
    fn test_number_to_json() {
        let expr = SExpr::Number {
            value: 42.0,
            span: s(),
        };
        let json = sexpr_to_protocol_json(&expr);
        assert_eq!(json["type"], "number");
        assert_eq!(json["value"], 42.0);
    }

    #[test]
    fn test_bool_true_to_json() {
        let expr = SExpr::Bool {
            value: true,
            span: s(),
        };
        let json = sexpr_to_protocol_json(&expr);
        assert_eq!(json["type"], "boolean");
        assert_eq!(json["value"], true);
    }

    #[test]
    fn test_bool_false_to_json() {
        let expr = SExpr::Bool {
            value: false,
            span: s(),
        };
        let json = sexpr_to_protocol_json(&expr);
        assert_eq!(json["type"], "boolean");
        assert_eq!(json["value"], false);
    }

    #[test]
    fn test_null_to_json() {
        let expr = SExpr::Null { span: s() };
        let json = sexpr_to_protocol_json(&expr);
        assert_eq!(json["type"], "atom");
        assert_eq!(json["value"], "null");
    }

    #[test]
    fn test_list_to_json() {
        let expr = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "+".to_string(),
                    span: s(),
                },
                SExpr::Number {
                    value: 1.0,
                    span: s(),
                },
                SExpr::Number {
                    value: 2.0,
                    span: s(),
                },
            ],
            span: s(),
        };
        let json = sexpr_to_protocol_json(&expr);
        assert_eq!(json["type"], "list");
        let values = json["values"].as_array().unwrap();
        assert_eq!(values.len(), 3);
        assert_eq!(values[0]["value"], "+");
        assert_eq!(values[1]["value"], 1.0);
    }

    #[test]
    fn test_cons_to_json() {
        let expr = SExpr::Cons {
            car: Box::new(SExpr::Atom {
                value: "a".to_string(),
                span: s(),
            }),
            cdr: Box::new(SExpr::Number {
                value: 1.0,
                span: s(),
            }),
            span: s(),
        };
        let json = sexpr_to_protocol_json(&expr);
        assert_eq!(json["type"], "cons");
        assert_eq!(json["car"]["value"], "a");
        assert_eq!(json["cdr"]["value"], 1.0);
    }

    #[test]
    fn test_nested_list_to_json() {
        let inner = SExpr::List {
            values: vec![SExpr::Atom {
                value: "x".to_string(),
                span: s(),
            }],
            span: s(),
        };
        let expr = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "fn".to_string(),
                    span: s(),
                },
                inner,
            ],
            span: s(),
        };
        let json = sexpr_to_protocol_json(&expr);
        let inner_json = &json["values"][1];
        assert_eq!(inner_json["type"], "list");
        assert_eq!(inner_json["values"][0]["value"], "x");
    }

    // ---------------------------------------------------------------
    // protocol_json_to_sexpr
    // ---------------------------------------------------------------

    #[test]
    fn test_json_atom_to_sexpr() {
        let json = serde_json::json!({ "type": "atom", "value": "foo" });
        let expr = protocol_json_to_sexpr(&json).unwrap();
        assert_eq!(
            expr,
            SExpr::Atom {
                value: "foo".to_string(),
                span: s()
            }
        );
    }

    #[test]
    fn test_json_keyword_to_sexpr() {
        let json = serde_json::json!({ "type": "keyword", "value": "name" });
        let expr = protocol_json_to_sexpr(&json).unwrap();
        assert_eq!(
            expr,
            SExpr::Keyword {
                value: "name".to_string(),
                span: s()
            }
        );
    }

    #[test]
    fn test_json_string_to_sexpr() {
        let json = serde_json::json!({ "type": "string", "value": "hello" });
        let expr = protocol_json_to_sexpr(&json).unwrap();
        assert_eq!(
            expr,
            SExpr::String {
                value: "hello".to_string(),
                span: s()
            }
        );
    }

    #[test]
    fn test_json_number_to_sexpr() {
        let json = serde_json::json!({ "type": "number", "value": 42.0 });
        let expr = protocol_json_to_sexpr(&json).unwrap();
        assert_eq!(
            expr,
            SExpr::Number {
                value: 42.0,
                span: s()
            }
        );
    }

    #[test]
    fn test_json_boolean_to_sexpr() {
        let json = serde_json::json!({ "type": "boolean", "value": true });
        let expr = protocol_json_to_sexpr(&json).unwrap();
        assert_eq!(
            expr,
            SExpr::Bool {
                value: true,
                span: s()
            }
        );
    }

    #[test]
    fn test_json_list_to_sexpr() {
        let json = serde_json::json!({
            "type": "list",
            "values": [
                { "type": "atom", "value": "+" },
                { "type": "number", "value": 1 },
            ]
        });
        let expr = protocol_json_to_sexpr(&json).unwrap();
        if let SExpr::List { values, .. } = &expr {
            assert_eq!(values.len(), 2);
            assert_eq!(values[0].as_atom(), Some("+"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_json_cons_to_sexpr() {
        let json = serde_json::json!({
            "type": "cons",
            "car": { "type": "atom", "value": "a" },
            "cdr": { "type": "number", "value": 1 },
        });
        let expr = protocol_json_to_sexpr(&json).unwrap();
        if let SExpr::Cons { car, cdr, .. } = &expr {
            assert_eq!(car.as_atom(), Some("a"));
        } else {
            panic!("expected cons");
        }
    }

    #[test]
    fn test_raw_null_to_sexpr() {
        let json = Value::Null;
        let expr = protocol_json_to_sexpr(&json).unwrap();
        assert_eq!(expr, SExpr::Null { span: s() });
    }

    #[test]
    fn test_raw_string_to_sexpr() {
        let json = serde_json::json!("hello");
        let expr = protocol_json_to_sexpr(&json).unwrap();
        assert_eq!(
            expr,
            SExpr::Atom {
                value: "hello".to_string(),
                span: s()
            }
        );
    }

    #[test]
    fn test_raw_number_to_sexpr() {
        let json = serde_json::json!(3.14);
        let expr = protocol_json_to_sexpr(&json).unwrap();
        assert_eq!(
            expr,
            SExpr::Number {
                value: 3.14,
                span: s()
            }
        );
    }

    #[test]
    fn test_raw_bool_to_sexpr() {
        let json = serde_json::json!(false);
        let expr = protocol_json_to_sexpr(&json).unwrap();
        assert_eq!(
            expr,
            SExpr::Bool {
                value: false,
                span: s()
            }
        );
    }

    #[test]
    fn test_unknown_type_returns_error() {
        let json = serde_json::json!({ "type": "foobar", "value": 1 });
        let result = protocol_json_to_sexpr(&json);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_object_returns_error() {
        let json = serde_json::json!([1, 2, 3]);
        let result = protocol_json_to_sexpr(&json);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // Round-trip tests
    // ---------------------------------------------------------------

    fn assert_round_trip(expr: SExpr) {
        let json = sexpr_to_protocol_json(&expr);
        let back = protocol_json_to_sexpr(&json).unwrap();
        // Spans are lost (default), but structural equality holds for
        // everything except Null which maps to Atom { value: "null" }.
        match &expr {
            SExpr::Null { .. } => {
                // Null serializes as atom "null", so round-trip produces Atom.
                assert_eq!(
                    back,
                    SExpr::Atom {
                        value: "null".to_string(),
                        span: s()
                    }
                );
            }
            _ => assert_eq!(back, expr),
        }
    }

    #[test]
    fn test_round_trip_atom() {
        assert_round_trip(SExpr::Atom {
            value: "define".to_string(),
            span: s(),
        });
    }

    #[test]
    fn test_round_trip_keyword() {
        assert_round_trip(SExpr::Keyword {
            value: "key".to_string(),
            span: s(),
        });
    }

    #[test]
    fn test_round_trip_string() {
        assert_round_trip(SExpr::String {
            value: "hello world".to_string(),
            span: s(),
        });
    }

    #[test]
    fn test_round_trip_number() {
        assert_round_trip(SExpr::Number {
            value: -3.14,
            span: s(),
        });
    }

    #[test]
    fn test_round_trip_bool() {
        assert_round_trip(SExpr::Bool {
            value: true,
            span: s(),
        });
        assert_round_trip(SExpr::Bool {
            value: false,
            span: s(),
        });
    }

    #[test]
    fn test_round_trip_null() {
        assert_round_trip(SExpr::Null { span: s() });
    }

    #[test]
    fn test_round_trip_list() {
        assert_round_trip(SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "if".to_string(),
                    span: s(),
                },
                SExpr::Bool {
                    value: true,
                    span: s(),
                },
                SExpr::Number {
                    value: 1.0,
                    span: s(),
                },
                SExpr::Number {
                    value: 0.0,
                    span: s(),
                },
            ],
            span: s(),
        });
    }

    #[test]
    fn test_round_trip_cons() {
        assert_round_trip(SExpr::Cons {
            car: Box::new(SExpr::Atom {
                value: "a".to_string(),
                span: s(),
            }),
            cdr: Box::new(SExpr::Atom {
                value: "b".to_string(),
                span: s(),
            }),
            span: s(),
        });
    }

    #[test]
    fn test_round_trip_empty_list() {
        assert_round_trip(SExpr::List {
            values: vec![],
            span: s(),
        });
    }

    #[test]
    fn test_round_trip_deeply_nested() {
        let inner = SExpr::List {
            values: vec![SExpr::Atom {
                value: "x".to_string(),
                span: s(),
            }],
            span: s(),
        };
        let outer = SExpr::List {
            values: vec![
                SExpr::Atom {
                    value: "lambda".to_string(),
                    span: s(),
                },
                inner,
                SExpr::List {
                    values: vec![SExpr::Atom {
                        value: "x".to_string(),
                        span: s(),
                    }],
                    span: s(),
                },
            ],
            span: s(),
        };
        assert_round_trip(outer);
    }
}
