use crate::ast::sexpr::SExpr;

pub fn serialize_sexpr(expr: &SExpr) -> String {
    match expr {
        SExpr::Atom { value, .. } => value.clone(),
        SExpr::Keyword { value, .. } => format!(":{value}"),
        SExpr::String { value, .. } => format!("\"{value}\""),
        SExpr::Number { value, .. } => {
            if *value == value.floor() && value.is_finite() {
                format!("{}", *value as i64)
            } else {
                format!("{value}")
            }
        }
        SExpr::Bool { value: true, .. } => "true".to_string(),
        SExpr::Bool { value: false, .. } => "false".to_string(),
        SExpr::Null { .. } => "null".to_string(),
        SExpr::List { values, .. } => {
            let inner: Vec<String> = values.iter().map(serialize_sexpr).collect();
            format!("({})", inner.join(" "))
        }
        SExpr::Cons { car, cdr, .. } => {
            format!("({} . {})", serialize_sexpr(car), serialize_sexpr(cdr))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::source_loc::Span;

    fn s() -> Span {
        Span::default()
    }

    #[test]
    fn serialize_atom() {
        let expr = SExpr::Atom {
            value: "foo".to_string(),
            span: s(),
        };
        assert_eq!(serialize_sexpr(&expr), "foo");
    }

    #[test]
    fn serialize_keyword() {
        let expr = SExpr::Keyword {
            value: "name".to_string(),
            span: s(),
        };
        assert_eq!(serialize_sexpr(&expr), ":name");
    }

    #[test]
    fn serialize_list() {
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
        assert_eq!(serialize_sexpr(&expr), "(+ 1 2)");
    }
}
