/// lykn s-expression reader
///
/// Parses lykn source text into a tree of SExpr nodes.

#[derive(Debug, Clone)]
pub enum SExpr {
    Atom(String),
    Str(String),
    Num(f64),
    List(Vec<SExpr>),
}

pub fn read(source: &str) -> Vec<SExpr> {
    let chars: Vec<char> = source.chars().collect();
    let mut pos = 0;
    let mut exprs = Vec::new();

    skip_ws(&chars, &mut pos);
    while pos < chars.len() {
        if let Some(expr) = read_expr(&chars, &mut pos) {
            exprs.push(expr);
        }
        skip_ws(&chars, &mut pos);
    }
    exprs
}

fn skip_ws(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() {
        match chars[*pos] {
            ' ' | '\t' | '\n' | '\r' => *pos += 1,
            ';' => {
                while *pos < chars.len() && chars[*pos] != '\n' {
                    *pos += 1;
                }
            }
            _ => break,
        }
    }
}

fn read_expr(chars: &[char], pos: &mut usize) -> Option<SExpr> {
    skip_ws(chars, pos);
    if *pos >= chars.len() {
        return None;
    }

    match chars[*pos] {
        '(' => Some(read_list(chars, pos)),
        '"' => Some(read_string(chars, pos)),
        _ => Some(read_atom_or_num(chars, pos)),
    }
}

fn read_list(chars: &[char], pos: &mut usize) -> SExpr {
    *pos += 1; // skip (
    let mut values = Vec::new();
    skip_ws(chars, pos);
    while *pos < chars.len() && chars[*pos] != ')' {
        if let Some(expr) = read_expr(chars, pos) {
            values.push(expr);
        }
        skip_ws(chars, pos);
    }
    if *pos < chars.len() {
        *pos += 1; // skip )
    }
    SExpr::List(values)
}

fn read_string(chars: &[char], pos: &mut usize) -> SExpr {
    *pos += 1; // skip opening "
    let mut value = String::new();
    while *pos < chars.len() && chars[*pos] != '"' {
        if chars[*pos] == '\\' && *pos + 1 < chars.len() {
            *pos += 1;
            match chars[*pos] {
                'n' => value.push('\n'),
                't' => value.push('\t'),
                '\\' => value.push('\\'),
                '"' => value.push('"'),
                c => value.push(c),
            }
        } else {
            value.push(chars[*pos]);
        }
        *pos += 1;
    }
    if *pos < chars.len() {
        *pos += 1; // skip closing "
    }
    SExpr::Str(value)
}

fn read_atom_or_num(chars: &[char], pos: &mut usize) -> SExpr {
    let mut value = String::new();
    while *pos < chars.len() {
        match chars[*pos] {
            ' ' | '\t' | '\n' | '\r' | '(' | ')' | ';' => break,
            c => {
                value.push(c);
                *pos += 1;
            }
        }
    }

    // Try parsing as number
    if let Ok(n) = value.parse::<f64>() {
        SExpr::Num(n)
    } else {
        SExpr::Atom(value)
    }
}
