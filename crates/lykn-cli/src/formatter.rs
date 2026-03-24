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
            // Add blank line between top-level forms
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
        SExpr::Atom(s) => s.clone(),
        SExpr::Str(s) => format!("\"{}\"", escape_string(s)),
        SExpr::Num(n) => {
            if *n == (*n as i64) as f64 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        SExpr::List(values) => format_list(values, indent),
    }
}

fn format_list(values: &[SExpr], indent: usize) -> String {
    if values.is_empty() {
        return "()".to_string();
    }

    // Try single-line first
    let single = format_single_line(values);
    if single.len() + indent <= 80 && !single.contains('\n') {
        return format!("({})", single);
    }

    // Multi-line: head on first line, rest indented
    let head = format_expr(&values[0], 0);
    let child_indent = indent + 2;
    let child_prefix = " ".repeat(child_indent);

    let mut out = format!("({}", head);
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
