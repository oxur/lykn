//! ICU MessageFormat subset parser and JS emitter for the `template` macro (DD-54 Phase A).
//!
//! Parses an ICU string into a Message Format Tree (MFT) at compile time, then
//! emits JavaScript template literals with IIFE conditionals for plural/select.
//! Zero runtime dependencies in emitted output.

use std::collections::{HashMap, HashSet};
use std::fmt;

use super::emit::emit_expr;
use super::format::JsWriter;
use crate::ast::sexpr::SExpr;

// ── MFT node types ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum MftNode {
    Literal(String),
    Slot(String),
    Plural {
        name: String,
        branches: Vec<PluralBranch>,
    },
    Select {
        name: String,
        branches: Vec<SelectBranch>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluralBranch {
    pub key: PluralKey,
    pub body: Vec<MftNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PluralKey {
    Exact(i64),
    Category(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectBranch {
    pub key: String,
    pub body: Vec<MftNode>,
}

// ── Parser ────────────────────────────────────────────────────────────

const PLURAL_CATEGORIES: &[&str] = &["zero", "one", "two", "few", "many", "other"];

#[derive(Debug)]
pub struct IcuParseError {
    pub message: String,
    pub input: String,
    pub position: usize,
}

impl fmt::Display for IcuParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "template: {}\n  in \"{}\"\n  at position {}",
            self.message, self.input, self.position
        )
    }
}

impl std::error::Error for IcuParseError {}

pub fn parse_icu(input: &str) -> Result<Vec<MftNode>, IcuParseError> {
    let mut parser = IcuParser::new(input);
    let nodes = parser.parse_message(false)?;
    if parser.pos < input.len() {
        return Err(parser.error(&format!(
            "unexpected character '{}'",
            &input[parser.pos..parser.pos + 1]
        )));
    }
    Ok(coalesce_literals(nodes))
}

pub fn collect_slot_names(nodes: &[MftNode]) -> HashSet<String> {
    let mut names = HashSet::new();
    for node in nodes {
        collect_slot_names_inner(node, &mut names);
    }
    names
}

fn collect_slot_names_inner(node: &MftNode, names: &mut HashSet<String>) {
    match node {
        MftNode::Slot(name) => {
            names.insert(name.clone());
        }
        MftNode::Plural { name, branches } => {
            names.insert(name.clone());
            for branch in branches {
                for child in &branch.body {
                    collect_slot_names_inner(child, names);
                }
            }
        }
        MftNode::Select { name, branches } => {
            names.insert(name.clone());
            for branch in branches {
                for child in &branch.body {
                    collect_slot_names_inner(child, names);
                }
            }
        }
        MftNode::Literal(_) => {}
    }
}

struct IcuParser<'a> {
    input: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> IcuParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn error(&self, msg: &str) -> IcuParseError {
        IcuParseError {
            message: msg.to_string(),
            input: self.input.to_string(),
            position: self.pos,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn advance(&mut self) -> u8 {
        let b = self.bytes[self.pos];
        self.pos += 1;
        b
    }

    fn expect(&mut self, ch: u8) -> Result<(), IcuParseError> {
        match self.peek() {
            Some(b) if b == ch => {
                self.advance();
                Ok(())
            }
            Some(b) => Err(self.error(&format!(
                "expected '{}', got '{}'",
                ch as char, b as char
            ))),
            None => Err(self.error(&format!("expected '{}', got end of input", ch as char))),
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn parse_message(&mut self, in_branch: bool) -> Result<Vec<MftNode>, IcuParseError> {
        let mut nodes = Vec::new();
        while self.pos < self.bytes.len() {
            match self.peek() {
                Some(b'}') => break,
                Some(b'{') => nodes.push(self.parse_block()?),
                Some(b'\'') => nodes.push(self.parse_escape()),
                Some(b'#') if in_branch => {
                    nodes.push(MftNode::Literal("#".into()));
                    self.advance();
                }
                _ => nodes.push(self.parse_literal(in_branch)),
            }
        }
        Ok(coalesce_literals(nodes))
    }

    fn parse_branch_body(&mut self, selector_name: &str) -> Result<Vec<MftNode>, IcuParseError> {
        let mut nodes = Vec::new();
        while self.pos < self.bytes.len() {
            match self.peek() {
                Some(b'}') => break,
                Some(b'{') => nodes.push(self.parse_block()?),
                Some(b'\'') => nodes.push(self.parse_escape()),
                Some(b'#') => {
                    nodes.push(MftNode::Slot(selector_name.to_string()));
                    self.advance();
                }
                _ => nodes.push(self.parse_literal(true)),
            }
        }
        Ok(coalesce_literals(nodes))
    }

    fn parse_literal(&mut self, in_branch: bool) -> MftNode {
        let start = self.pos;
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b'{' | b'}' | b'\'' => break,
                b'#' if in_branch => break,
                _ => self.pos += 1,
            }
        }
        MftNode::Literal(self.input[start..self.pos].to_string())
    }

    fn parse_escape(&mut self) -> MftNode {
        self.advance(); // consume opening '
        match self.peek() {
            Some(b'\'') => {
                self.advance();
                MftNode::Literal("'".into())
            }
            Some(b'{') => {
                let ch = self.advance();
                if self.peek() == Some(b'\'') {
                    self.advance();
                }
                MftNode::Literal((ch as char).to_string())
            }
            Some(b'}') => {
                let ch = self.advance();
                if self.peek() == Some(b'\'') {
                    self.advance();
                }
                MftNode::Literal((ch as char).to_string())
            }
            _ => MftNode::Literal("'".into()),
        }
    }

    fn parse_block(&mut self) -> Result<MftNode, IcuParseError> {
        self.expect(b'{')?;
        self.skip_whitespace();

        let name = self.parse_identifier();
        if name.is_empty() {
            return Err(self.error("expected slot name after '{'"));
        }
        self.skip_whitespace();

        if self.peek() == Some(b'}') {
            self.advance();
            return Ok(MftNode::Slot(name));
        }

        if self.peek() == Some(b',') {
            self.advance();
            self.skip_whitespace();
            let kind = self.parse_identifier();
            self.skip_whitespace();

            return match kind.as_str() {
                "plural" => self.parse_plural_body(&name),
                "select" => self.parse_select_body(&name),
                _ => Err(self.error(&format!(
                    "unknown format type '{}'; expected 'plural' or 'select'",
                    kind
                ))),
            };
        }

        Err(self.error(&format!("expected '}}' or ',' after slot name '{}'", name)))
    }

    fn parse_identifier(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.input[start..self.pos].to_string()
    }

    fn parse_plural_body(&mut self, name: &str) -> Result<MftNode, IcuParseError> {
        self.expect(b',')?;
        self.skip_whitespace();

        let mut branches = Vec::new();
        let mut has_other = false;

        while self.pos < self.bytes.len() && self.peek() != Some(b'}') {
            self.skip_whitespace();
            if self.peek() == Some(b'}') {
                break;
            }

            let key = self.parse_plural_key()?;
            self.skip_whitespace();

            if key == PluralKey::Category("other".into()) {
                has_other = true;
            }

            self.expect(b'{')?;
            let raw_body = self.parse_branch_body(name)?;
            self.expect(b'}')?;

            branches.push(PluralBranch { key, body: raw_body });
            self.skip_whitespace();
        }

        if !has_other {
            return Err(self.error(&format!(
                "plural block for {{{}}} missing required 'other' branch",
                name
            )));
        }

        self.expect(b'}')?;
        Ok(MftNode::Plural {
            name: name.to_string(),
            branches,
        })
    }

    fn parse_plural_key(&mut self) -> Result<PluralKey, IcuParseError> {
        if self.peek() == Some(b'=') {
            self.advance();
            let start = self.pos;
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
            if self.pos == start {
                return Err(self.error("expected number after '=' in plural key"));
            }
            let n: i64 = self.input[start..self.pos].parse().unwrap();
            return Ok(PluralKey::Exact(n));
        }
        let cat = self.parse_identifier();
        if cat.is_empty() {
            return Err(self.error("expected plural category or '=N'"));
        }
        if !PLURAL_CATEGORIES.contains(&cat.as_str()) {
            return Err(self.error(&format!(
                "unknown plural category '{}' for plural block; valid categories: {}",
                cat,
                PLURAL_CATEGORIES.join(" ")
            )));
        }
        Ok(PluralKey::Category(cat))
    }

    fn parse_select_body(&mut self, name: &str) -> Result<MftNode, IcuParseError> {
        self.expect(b',')?;
        self.skip_whitespace();

        let mut branches = Vec::new();
        let mut has_other = false;

        while self.pos < self.bytes.len() && self.peek() != Some(b'}') {
            self.skip_whitespace();
            if self.peek() == Some(b'}') {
                break;
            }

            let key = self.parse_identifier();
            if key.is_empty() {
                return Err(self.error("expected select branch key"));
            }
            self.skip_whitespace();

            if key == "other" {
                has_other = true;
            }

            self.expect(b'{')?;
            let body = self.parse_message(true)?;
            self.expect(b'}')?;

            branches.push(SelectBranch { key, body });
            self.skip_whitespace();
        }

        if !has_other {
            return Err(self.error(&format!(
                "select block for {{{}}} missing required 'other' branch",
                name
            )));
        }

        self.expect(b'}')?;
        Ok(MftNode::Select {
            name: name.to_string(),
            branches,
        })
    }
}


fn coalesce_literals(nodes: Vec<MftNode>) -> Vec<MftNode> {
    let mut result: Vec<MftNode> = Vec::new();
    for node in nodes {
        if let MftNode::Literal(ref s) = node
            && let Some(MftNode::Literal(prev)) = result.last_mut()
        {
            prev.push_str(s);
            continue;
        }
        result.push(node);
    }
    result
}

// ── JS emitter ────────────────────────────────────────────────────────

/// Check if a template form should use ICU mode and emit accordingly.
/// Returns true if ICU mode was used; false if the caller should fall through
/// to concat mode.
pub fn try_emit_template_icu(w: &mut JsWriter, args: &[SExpr]) -> bool {
    if args.is_empty() {
        return false;
    }

    let icu_string = match &args[0] {
        SExpr::String { value, .. } => value,
        _ => return false,
    };

    // Single-arg: parse as ICU
    if args.len() == 1 {
        let mft = match parse_icu(icu_string) {
            Ok(mft) => mft,
            Err(_) => return false,
        };
        let slot_names = collect_slot_names(&mft);
        if !slot_names.is_empty() {
            return false;
        }
        emit_mft(w, &mft, &HashMap::new());
        return true;
    }

    // Multi-arg: check if arg[1] is a keyword
    if !matches!(&args[1], SExpr::Keyword { .. }) {
        return false;
    }

    // Ambiguous form: string + lone keyword
    if args.len() == 2 {
        return false;
    }

    let mft = match parse_icu(icu_string) {
        Ok(mft) => mft,
        Err(_) => return false,
    };

    // Parse kwargs
    let mut kwargs: HashMap<String, &SExpr> = HashMap::new();
    let mut i = 1;
    while i < args.len() {
        if let SExpr::Keyword { value, .. } = &args[i] {
            if i + 1 < args.len() {
                kwargs.insert(value.clone(), &args[i + 1]);
                i += 2;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    emit_mft(w, &mft, &kwargs);
    true
}

fn emit_mft(w: &mut JsWriter, nodes: &[MftNode], kwargs: &HashMap<String, &SExpr>) {
    if nodes.iter().all(|n| matches!(n, MftNode::Literal(_))) {
        w.write("`");
        for node in nodes {
            if let MftNode::Literal(s) = node {
                emit_template_text_icu(w, s);
            }
        }
        w.write("`");
        return;
    }

    w.write("`");
    for node in nodes {
        match node {
            MftNode::Literal(s) => emit_template_text_icu(w, s),
            MftNode::Slot(name) => {
                w.write("${");
                if let Some(expr) = kwargs.get(name) {
                    emit_expr(w, expr, 0);
                } else {
                    w.write(name);
                }
                w.write("}");
            }
            MftNode::Plural { name, branches } => {
                w.write("${");
                emit_plural_iife(w, name, branches, kwargs);
                w.write("}");
            }
            MftNode::Select { name, branches } => {
                w.write("${");
                emit_select_iife(w, name, branches, kwargs);
                w.write("}");
            }
        }
    }
    w.write("`");
}

fn emit_template_text_icu(w: &mut JsWriter, value: &str) {
    for ch in value.chars() {
        match ch {
            '`' => w.write("\\`"),
            '\\' => w.write("\\\\"),
            '$' => w.write_char('$'),
            c => w.write_char(c),
        }
    }
}

fn emit_plural_iife(
    w: &mut JsWriter,
    name: &str,
    branches: &[PluralBranch],
    kwargs: &HashMap<String, &SExpr>,
) {
    w.write("(() => {");
    w.write(" const _v = ");
    if let Some(expr) = kwargs.get(name) {
        emit_expr(w, expr, 0);
    } else {
        w.write(name);
    }
    w.write(";");

    // Exact branches first
    for branch in branches {
        if let PluralKey::Exact(n) = &branch.key {
            w.write(&format!(" if (_v === {}) return ", n));
            let mut inner_kwargs = kwargs.clone();
            inner_kwargs.insert(name.to_string(), &IIFE_V_PLACEHOLDER);
            emit_mft(w, &branch.body, &inner_kwargs);
            w.write(";");
        }
    }

    // Category branches (English CLDR)
    for branch in branches {
        if let PluralKey::Category(cat) = &branch.key {
            if cat == "other" {
                continue;
            }
            if let Some(test) = plural_category_test(cat) {
                w.write(&format!(" if ({}) return ", test));
                let mut inner_kwargs = kwargs.clone();
                inner_kwargs.insert(name.to_string(), &IIFE_V_PLACEHOLDER);
                emit_mft(w, &branch.body, &inner_kwargs);
                w.write(";");
            }
        }
    }

    // `other` branch
    if let Some(other) = branches.iter().find(|b| b.key == PluralKey::Category("other".into())) {
        w.write(" return ");
        let mut inner_kwargs = kwargs.clone();
        inner_kwargs.insert(name.to_string(), &IIFE_V_PLACEHOLDER);
        emit_mft(w, &other.body, &inner_kwargs);
        w.write(";");
    }

    w.write(" })()");
}

fn emit_select_iife(
    w: &mut JsWriter,
    name: &str,
    branches: &[SelectBranch],
    kwargs: &HashMap<String, &SExpr>,
) {
    w.write("(() => {");
    w.write(" const _v = ");
    if let Some(expr) = kwargs.get(name) {
        emit_expr(w, expr, 0);
    } else {
        w.write(name);
    }
    w.write(";");

    for branch in branches {
        if branch.key == "other" {
            continue;
        }
        w.write(&format!(" if (_v === \"{}\") return ", branch.key));
        emit_mft(w, &branch.body, kwargs);
        w.write(";");
    }

    if let Some(other) = branches.iter().find(|b| b.key == "other") {
        w.write(" return ");
        emit_mft(w, &other.body, kwargs);
        w.write(";");
    }

    w.write(" })()");
}

fn plural_category_test(category: &str) -> Option<String> {
    match category {
        "one" => Some("_v === 1".into()),
        "zero" => Some("_v === 0".into()),
        _ => None,
    }
}

use std::sync::LazyLock;
use crate::reader::source_loc::Span;

static IIFE_V_PLACEHOLDER: LazyLock<SExpr> = LazyLock::new(|| SExpr::Atom {
    value: "_v".to_string(),
    span: Span::default(),
});

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_literal() {
        let result = parse_icu("hello world").unwrap();
        assert_eq!(result, vec![MftNode::Literal("hello world".into())]);
    }

    #[test]
    fn test_parse_slot() {
        let result = parse_icu("{name}").unwrap();
        assert_eq!(result, vec![MftNode::Slot("name".into())]);
    }

    #[test]
    fn test_parse_slot_surrounded() {
        let result = parse_icu("Hello, {name}!").unwrap();
        assert_eq!(
            result,
            vec![
                MftNode::Literal("Hello, ".into()),
                MftNode::Slot("name".into()),
                MftNode::Literal("!".into()),
            ]
        );
    }

    #[test]
    fn test_parse_escape_brace() {
        let result = parse_icu("a '{' b").unwrap();
        assert_eq!(result, vec![MftNode::Literal("a { b".into())]);
    }

    #[test]
    fn test_parse_escape_apostrophe() {
        let result = parse_icu("it''s").unwrap();
        assert_eq!(result, vec![MftNode::Literal("it's".into())]);
    }

    #[test]
    fn test_parse_lone_apostrophe() {
        let result = parse_icu("it's fine").unwrap();
        assert_eq!(result, vec![MftNode::Literal("it's fine".into())]);
    }

    #[test]
    fn test_parse_plural() {
        let result = parse_icu("{n, plural, one {1 item} other {# items}}").unwrap();
        assert_eq!(result.len(), 1);
        if let MftNode::Plural { name, branches } = &result[0] {
            assert_eq!(name, "n");
            assert_eq!(branches.len(), 2);
            assert_eq!(branches[0].key, PluralKey::Category("one".into()));
            assert_eq!(branches[1].key, PluralKey::Category("other".into()));
            // # is resolved to Slot("n")
            assert!(branches[1].body.contains(&MftNode::Slot("n".into())));
        } else {
            panic!("expected Plural node");
        }
    }

    #[test]
    fn test_parse_plural_exact() {
        let result = parse_icu("{n, plural, =0 {none} one {one} other {many}}").unwrap();
        if let MftNode::Plural { branches, .. } = &result[0] {
            assert_eq!(branches[0].key, PluralKey::Exact(0));
        } else {
            panic!("expected Plural node");
        }
    }

    #[test]
    fn test_parse_plural_missing_other() {
        assert!(parse_icu("{n, plural, one {x}}").is_err());
    }

    #[test]
    fn test_parse_plural_unknown_category() {
        let err = parse_icu("{n, plural, weird {x} other {y}}");
        assert!(err.is_err());
        assert!(err.unwrap_err().message.contains("unknown plural category"));
    }

    #[test]
    fn test_parse_select() {
        let result = parse_icu("{role, select, owner {Own} other {Guest}}").unwrap();
        if let MftNode::Select { name, branches } = &result[0] {
            assert_eq!(name, "role");
            assert_eq!(branches.len(), 2);
        } else {
            panic!("expected Select node");
        }
    }

    #[test]
    fn test_parse_select_missing_other() {
        assert!(parse_icu("{role, select, admin {x}}").is_err());
    }

    #[test]
    fn test_collect_slot_names() {
        let nodes = parse_icu("Hello {name}, you have {count} items").unwrap();
        let names = collect_slot_names(&nodes);
        assert!(names.contains("name"));
        assert!(names.contains("count"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_collect_slot_names_nested() {
        let nodes =
            parse_icu("{role, select, owner {{n, plural, one {1 {unit}} other {# {unit}s}}} other {N/A}}")
                .unwrap();
        let names = collect_slot_names(&nodes);
        assert!(names.contains("role"));
        assert!(names.contains("n"));
        assert!(names.contains("unit"));
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn test_parse_marketing_example() {
        let input = "{role, select, \
            owner {Welcome back, {name}! You have {count, plural, \
            =0 {no pending tasks} one {1 pending task} other {# pending tasks}}.} \
            member {Hi {name}. You have {count, plural, \
            =0 {no items to review} one {1 item to review} other {# items to review}}.} \
            other {Hello, guest.}}";
        let result = parse_icu(input).unwrap();
        assert_eq!(result.len(), 1);
        let names = collect_slot_names(&result);
        assert!(names.contains("role"));
        assert!(names.contains("name"));
        assert!(names.contains("count"));
    }

    #[test]
    fn test_emit_simple_slot() {
        let mft = parse_icu("Hello, {name}!").unwrap();
        let mut kwargs = HashMap::new();
        let expr = SExpr::Atom {
            value: "n".into(),
            span: Span::default(),
        };
        kwargs.insert("name".into(), &expr);
        let mut w = JsWriter::new();
        emit_mft(&mut w, &mft, &kwargs);
        assert_eq!(w.finish(), "`Hello, ${n}!`");
    }

    #[test]
    fn test_emit_no_slots() {
        let mft = parse_icu("hello world").unwrap();
        let mut w = JsWriter::new();
        emit_mft(&mut w, &mft, &HashMap::new());
        assert_eq!(w.finish(), "`hello world`");
    }

    #[test]
    fn test_emit_plural() {
        let mft = parse_icu("{n, plural, one {1 item} other {# items}}").unwrap();
        let mut kwargs = HashMap::new();
        let expr = SExpr::Atom {
            value: "count".into(),
            span: Span::default(),
        };
        kwargs.insert("n".into(), &expr);
        let mut w = JsWriter::new();
        emit_mft(&mut w, &mft, &kwargs);
        let output = w.finish();
        assert!(output.contains("count"));
        assert!(output.contains("=== 1"));
        assert!(output.contains("1 item"));
        assert!(output.contains("items"));
    }

    #[test]
    fn test_emit_select() {
        let mft = parse_icu("{role, select, owner {You own it.} other {Guest.}}").unwrap();
        let mut kwargs = HashMap::new();
        let expr = SExpr::Atom {
            value: "r".into(),
            span: Span::default(),
        };
        kwargs.insert("role".into(), &expr);
        let mut w = JsWriter::new();
        emit_mft(&mut w, &mft, &kwargs);
        let output = w.finish();
        assert!(output.contains("r"));
        assert!(output.contains("\"owner\""));
        assert!(output.contains("You own it."));
        assert!(output.contains("Guest."));
    }
}
