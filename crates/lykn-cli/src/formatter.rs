//! lykn formatter
//!
//! Pretty-prints s-expressions with consistent indentation.
//! Follows common Lisp/Scheme formatting conventions.

use crate::reader::SExpr;

pub fn format_exprs(exprs: &[SExpr], indent: usize) -> String {
    let mut out = String::new();
    for (i, expr) in exprs.iter().enumerate() {
        out.push_str(&format_expr(expr, indent));
        if i + 1 < exprs.len() {
            out.push('\n');
            if indent == 0 {
                out.push('\n');
            }
        }
    }
    out.push('\n');
    out
}

fn format_expr(expr: &SExpr, indent: usize) -> String {
    match expr {
        SExpr::Atom { value, .. } => value.clone(),
        SExpr::String { value, .. } => format!("\"{}\"", escape_string(value)),
        SExpr::Number { value, .. } => {
            if *value == (*value as i64) as f64 {
                format!("{}", *value as i64)
            } else {
                format!("{value}")
            }
        }
        SExpr::Keyword { value, .. } => format!(":{value}"),
        SExpr::Bool { value, .. } => {
            if *value {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        SExpr::Null { .. } => "null".to_string(),
        SExpr::List { values, .. } => format_list(values, indent),
        SExpr::Cons { car, cdr, .. } => {
            format!("({} . {})", format_expr(car, 0), format_expr(cdr, 0))
        }
    }
}

fn format_list(values: &[SExpr], indent: usize) -> String {
    if values.is_empty() {
        return "()".to_string();
    }

    let single = format_single_line(values);
    if single.len() + indent <= 80 && !single.contains('\n') {
        return format!("({single})");
    }

    let head = format_expr(&values[0], 0);
    let child_indent = indent + 2;
    let child_prefix = " ".repeat(child_indent);

    let mut out = format!("({head}");
    for val in &values[1..] {
        let formatted = format_expr(val, child_indent);
        out.push('\n');
        out.push_str(&child_prefix);
        out.push_str(&formatted);
    }
    out.push(')');
    out
}

fn format_single_line(values: &[SExpr]) -> String {
    values
        .iter()
        .map(|v| format_expr(v, 0))
        .collect::<Vec<_>>()
        .join(" ")
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;
    use lykn_lang::reader::source_loc::Span;

    fn s() -> Span {
        Span::default()
    }

    fn atom(name: &str) -> SExpr {
        SExpr::Atom {
            value: name.into(),
            span: s(),
        }
    }

    fn num(n: f64) -> SExpr {
        SExpr::Number {
            value: n,
            span: s(),
        }
    }

    fn string(v: &str) -> SExpr {
        SExpr::String {
            value: v.into(),
            span: s(),
        }
    }

    fn list(vals: Vec<SExpr>) -> SExpr {
        SExpr::List {
            values: vals,
            span: s(),
        }
    }

    #[test]
    fn format_single_atom() {
        assert_eq!(format_exprs(&[atom("hello")], 0), "hello\n");
    }

    #[test]
    fn format_integer_number() {
        assert_eq!(format_exprs(&[num(42.0)], 0), "42\n");
    }

    #[test]
    fn format_float_number() {
        assert_eq!(format_exprs(&[num(3.14)], 0), "3.14\n");
    }

    #[test]
    fn format_string_simple() {
        assert_eq!(format_exprs(&[string("hello")], 0), "\"hello\"\n");
    }

    #[test]
    fn format_string_with_escapes() {
        assert_eq!(
            format_exprs(&[string("a\nb\t\"c\\")], 0),
            "\"a\\nb\\t\\\"c\\\\\"\n"
        );
    }

    #[test]
    fn format_empty_list() {
        assert_eq!(format_exprs(&[list(vec![])], 0), "()\n");
    }

    #[test]
    fn format_short_list() {
        assert_eq!(
            format_exprs(&[list(vec![atom("+"), num(1.0), num(2.0)])], 0),
            "(+ 1 2)\n"
        );
    }

    #[test]
    fn format_long_list_wraps() {
        let mut vals = vec![atom("function-with-a-very-long-name")];
        for _ in 0..5 {
            vals.push(string("some-really-long-argument-value"));
        }
        let result = format_exprs(&[list(vals)], 0);
        assert!(result.contains('\n'));
        assert!(result.starts_with("(function-with-a-very-long-name"));
    }

    #[test]
    fn format_multiple_top_level_exprs() {
        let result = format_exprs(&[atom("a"), atom("b")], 0);
        assert_eq!(result, "a\n\nb\n");
    }

    #[test]
    fn format_nested_list() {
        let inner = list(vec![atom("+"), num(1.0), num(2.0)]);
        let outer = list(vec![atom("define"), atom("x"), inner]);
        assert_eq!(format_exprs(&[outer], 0), "(define x (+ 1 2))\n");
    }

    #[test]
    fn format_indented_children() {
        let exprs = [list(vec![atom("define"), atom("x")])];
        assert_eq!(format_exprs(&exprs, 4), "(define x)\n");
    }

    #[test]
    fn escape_string_empty() {
        assert_eq!(escape_string(""), "");
    }

    #[test]
    fn escape_string_no_special() {
        assert_eq!(escape_string("hello"), "hello");
    }

    #[test]
    fn escape_string_all_special() {
        assert_eq!(escape_string("\\\"\n\t"), "\\\\\\\"\\n\\t");
    }

    // --- New variant tests ---

    #[test]
    fn format_keyword() {
        let kw = SExpr::Keyword {
            value: "name".into(),
            span: s(),
        };
        assert_eq!(format_exprs(&[kw], 0), ":name\n");
    }

    #[test]
    fn format_bool_true() {
        let b = SExpr::Bool {
            value: true,
            span: s(),
        };
        assert_eq!(format_exprs(&[b], 0), "true\n");
    }

    #[test]
    fn format_bool_false() {
        let b = SExpr::Bool {
            value: false,
            span: s(),
        };
        assert_eq!(format_exprs(&[b], 0), "false\n");
    }

    #[test]
    fn format_null() {
        let n = SExpr::Null { span: s() };
        assert_eq!(format_exprs(&[n], 0), "null\n");
    }

    #[test]
    fn format_cons() {
        let c = SExpr::Cons {
            car: Box::new(num(1.0)),
            cdr: Box::new(num(2.0)),
            span: s(),
        };
        assert_eq!(format_exprs(&[c], 0), "(1 . 2)\n");
    }

    #[test]
    fn format_list_with_keywords() {
        let exprs = [list(vec![
            atom("obj"),
            SExpr::Keyword {
                value: "name".into(),
                span: s(),
            },
            string("lykn"),
        ])];
        assert_eq!(format_exprs(&exprs, 0), "(obj :name \"lykn\")\n");
    }
}
