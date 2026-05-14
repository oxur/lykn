//! ICU MessageFormat subset parser and JS emitter for the `template` macro (DD-54 Phase A).
//!
//! Parses an ICU string into a Message Format Tree (MFT) at compile time, then
//! emits JavaScript template literals with IIFE conditionals for plural/select.
//! Zero runtime dependencies in emitted output.

use std::collections::{HashMap, HashSet};
use std::fmt;

use super::emit::emit_expr;
use super::format::JsWriter;
use super::names::to_js_identifier;
use crate::ast::sexpr::SExpr;
use crate::diagnostics::{Diagnostic, Severity};
use crate::error::LyknError;
use crate::reader::source_loc::Span;

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

const ALL_CLDR_CATEGORIES: &[&str] = &["zero", "one", "two", "few", "many", "other"];
const ENGLISH_CLDR_CATEGORIES: &[&str] = &["one", "other"];

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
            "{}\n  in \"{}\"\n  at position {}",
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

pub fn count_slot_references(nodes: &[MftNode]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for node in nodes {
        count_slot_references_inner(node, &mut counts);
    }
    counts
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

fn count_slot_references_inner(node: &MftNode, counts: &mut HashMap<String, usize>) {
    match node {
        MftNode::Slot(name) => {
            *counts.entry(name.clone()).or_insert(0) += 1;
        }
        MftNode::Plural { name, branches } => {
            *counts.entry(name.clone()).or_insert(0) += 1;
            for branch in branches {
                for child in &branch.body {
                    count_slot_references_inner(child, counts);
                }
            }
        }
        MftNode::Select { name, branches } => {
            *counts.entry(name.clone()).or_insert(0) += 1;
            for branch in branches {
                for child in &branch.body {
                    count_slot_references_inner(child, counts);
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

    // R-10: '#' at top level is handled by parse_literal which guarantees
    // forward progress. Inside plural branches, parse_branch_body routes
    // '#' → Slot(selector).
    fn parse_message(&mut self, in_branch: bool) -> Result<Vec<MftNode>, IcuParseError> {
        let mut nodes = Vec::new();
        while self.pos < self.bytes.len() {
            match self.peek() {
                Some(b'}') => break,
                Some(b'{') => nodes.push(self.parse_block()?),
                Some(b'\'') => nodes.push(self.parse_escape()),
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

    // R-10: guarantees forward progress — '#' with in_branch only breaks
    // after at least one character has been consumed (self.pos > start).
    fn parse_literal(&mut self, in_branch: bool) -> MftNode {
        let start = self.pos;
        loop {
            if self.pos >= self.bytes.len() {
                break;
            }
            match self.bytes[self.pos] {
                b'{' | b'}' | b'\'' => break,
                b'#' if in_branch && self.pos > start => break,
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

        // English CLDR Phase A: =1 collides with `one`
        let exact_values: HashSet<i64> = branches
            .iter()
            .filter_map(|b| match &b.key {
                PluralKey::Exact(n) => Some(*n),
                _ => None,
            })
            .collect();
        let category_keys: HashSet<&str> = branches
            .iter()
            .filter_map(|b| match &b.key {
                PluralKey::Category(c) => Some(c.as_str()),
                _ => None,
            })
            .collect();
        if exact_values.contains(&1) && category_keys.contains("one") {
            return Err(self.error(&format!(
                "plural block for {{{}}} has overlapping branches: \
                 '=1' and 'one' both match count == 1 under English plural rules. \
                 Remove one — they handle the same case.",
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
        if !ALL_CLDR_CATEGORIES.contains(&cat.as_str()) {
            return Err(self.error(&format!(
                "unknown plural category '{}'; valid CLDR categories: {}",
                cat,
                ALL_CLDR_CATEGORIES.join(" ")
            )));
        }
        if !ENGLISH_CLDR_CATEGORIES.contains(&cat.as_str()) {
            return Err(self.error(&format!(
                "plural category '{}' is not valid under English plural rules. \
                 English CLDR uses only 'one' and 'other'. \
                 Hint: use '=N {{...}}' for specific numeric values, \
                 e.g. '=0 {{none}}' for n=0 or '=2 {{pair}}' for n=2.",
                cat
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

fn fresh_icu_var(counter: &mut usize) -> String {
    let n = *counter;
    *counter += 1;
    format!("_v{}", n)
}

/// Outcome of attempting ICU-mode dispatch for a `template` form.
pub enum IcuDispatch {
    /// Form is unambiguously not ICU mode. Caller should fall through to concat.
    NotIcu,
    /// Form was ICU mode and was emitted successfully.
    Handled,
}

/// Try ICU mode for a template form. Returns `Ok(IcuDispatch::Handled)` if
/// ICU mode emitted, `Ok(IcuDispatch::NotIcu)` if the caller should use
/// concat mode, or `Err(LyknError)` for malformed ICU forms.
pub fn try_emit_template_icu(
    w: &mut JsWriter,
    args: &[SExpr],
) -> Result<IcuDispatch, LyknError> {
    if args.is_empty() {
        return Ok(IcuDispatch::NotIcu);
    }

    let icu_string = match &args[0] {
        SExpr::String { value, .. } => value.clone(),
        _ => return Ok(IcuDispatch::NotIcu),
    };
    let anchor_span = args[0].span();

    // Rule 1: single literal string — parse as ICU
    if args.len() == 1 {
        let mft = parse_icu(&icu_string).map_err(|e| icu_err(&e, anchor_span))?;
        let slot_names = collect_slot_names(&mft);
        if !slot_names.is_empty() {
            return Err(missing_kwarg_err(&slot_names, &icu_string, anchor_span));
        }
        let mut counter = 0;
        emit_mft(w, &mft, &HashMap::new(), &mut counter)?;
        return Ok(IcuDispatch::Handled);
    }

    // Not ICU if arg[1] isn't a keyword
    if !matches!(&args[1], SExpr::Keyword { .. }) {
        return Ok(IcuDispatch::NotIcu);
    }

    // Rule 2 ambiguous form: string + lone keyword, no value
    if args.len() == 2 {
        let kw = args[1].as_keyword().unwrap();
        return Err(codegen_err(
            format!(
                "template: ambiguous form\n  \
                 arg 0 is a literal string and arg 1 is a keyword (:{kw}) with no\n  \
                 following value, which matches both ICU mode (missing kwarg value)\n  \
                 and concat mode (keyword as positional arg)."
            ),
            args[1].span(),
            Some(format!(
                "for ICU mode, add a value: (template \"{icu_string}\" :{kw} <expr>)\n\
                 for concat mode, use string concatenation instead"
            )),
        ));
    }

    // Rule 2: ICU mode — parse and validate
    let mft = parse_icu(&icu_string).map_err(|e| icu_err(&e, anchor_span))?;
    let slot_names = collect_slot_names(&mft);
    let kwargs = parse_and_validate_kwargs(args, &slot_names, &icu_string)?;

    // R-4: hoist non-trivial kwargs referenced more than once
    let multiplicity = count_slot_references(&mft);
    let mut hoisted: Vec<(String, SExpr)> = Vec::new();
    let mut final_kwargs: HashMap<String, SExpr> = HashMap::new();

    for (key, expr) in kwargs {
        let refs = multiplicity.get(&key).copied().unwrap_or(0);
        let trivial = matches!(
            &expr,
            SExpr::Atom { .. } | SExpr::Number { .. } | SExpr::String { .. } | SExpr::Bool { .. }
        );
        if refs > 1 && !trivial {
            let local_name = format!("_{}", to_js_identifier(&key));
            let placeholder = SExpr::Atom {
                value: local_name.clone(),
                span: Span::default(),
            };
            hoisted.push((local_name, expr));
            final_kwargs.insert(key, placeholder);
        } else {
            final_kwargs.insert(key, expr);
        }
    }

    let mut counter = 0;
    if hoisted.is_empty() {
        emit_mft(w, &mft, &final_kwargs, &mut counter)?;
    } else {
        w.write("(() => { ");
        for (name, expr) in &hoisted {
            w.write(&format!("const {name} = "));
            emit_expr(w, expr, 0)?;
            w.write("; ");
        }
        w.write("return ");
        emit_mft(w, &mft, &final_kwargs, &mut counter)?;
        w.write("; })()");
    }
    Ok(IcuDispatch::Handled)
}

fn parse_and_validate_kwargs(
    args: &[SExpr],
    slot_names: &HashSet<String>,
    icu_string: &str,
) -> Result<HashMap<String, SExpr>, LyknError> {
    let mut kwargs: HashMap<String, SExpr> = HashMap::new();
    let mut i = 1;
    while i < args.len() {
        match &args[i] {
            SExpr::Keyword { value, .. } => {
                if kwargs.contains_key(value) {
                    return Err(codegen_err(
                        format!("template: duplicate keyword argument :{value}"),
                        args[i].span(),
                        None,
                    ));
                }
                if i + 1 >= args.len() {
                    return Err(codegen_err(
                        format!("template: keyword :{value} has no value"),
                        args[i].span(),
                        None,
                    ));
                }
                kwargs.insert(value.clone(), args[i + 1].clone());
                i += 2;
            }
            other => {
                return Err(codegen_err(
                    format!("template: expected keyword argument at position {i}, got {other:?}"),
                    other.span(),
                    None,
                ));
            }
        }
    }

    // Every slot must have a kwarg
    for name in sorted_set(slot_names) {
        if !kwargs.contains_key(&name) {
            return Err(codegen_err(
                format!(
                    "template: no binding for slot {{{name}}}\n  \
                     in (template \"{icu_string}\" ...)\n  \
                     expected slots: {}\n  \
                     provided kwargs: {}",
                    sorted_set(slot_names).join(", "),
                    if kwargs.is_empty() {
                        "(none)".into()
                    } else {
                        sorted_map_keys(&kwargs).join(", ")
                    },
                ),
                args[0].span(),
                Some(format!("add :{name} <value> to the template call")),
            ));
        }
    }

    // Every kwarg must be used by a slot
    for key in sorted_map_keys(&kwargs) {
        if !slot_names.contains(&key) {
            return Err(codegen_err(
                format!(
                    "template: unused keyword argument :{key}\n  \
                     in (template \"{icu_string}\" ...)\n  \
                     expected slots: {}\n  \
                     provided kwargs: {}",
                    sorted_set(slot_names).join(", "),
                    sorted_map_keys(&kwargs).join(", "),
                ),
                args[0].span(),
                Some(format!("remove :{key}, or add a {{{key}}} slot to the template")),
            ));
        }
    }

    Ok(kwargs)
}

// ── Error helpers ─────────────────────────────────────────────────────

fn codegen_err(message: String, span: Span, suggestion: Option<String>) -> LyknError {
    LyknError::Codegen(Diagnostic {
        severity: Severity::Error,
        message,
        span,
        suggestion,
    })
}

fn icu_err(e: &IcuParseError, span: Span) -> LyknError {
    // TODO: translate e.position into a Span offset for finer attribution
    codegen_err(format!("template: {}", e.message), span, None)
}

fn missing_kwarg_err(slot_names: &HashSet<String>, icu_string: &str, span: Span) -> LyknError {
    let names = sorted_set(slot_names);
    let first = names.first().cloned().unwrap_or_default();
    codegen_err(
        format!(
            "template: no binding for slot {{{first}}}\n  \
             in (template \"{icu_string}\")\n  \
             expected slots: {}\n  \
             provided kwargs: (none)",
            names.join(", "),
        ),
        span,
        Some(format!("add :{first} <value> to the template call")),
    )
}

fn sorted_set(names: &HashSet<String>) -> Vec<String> {
    let mut v: Vec<String> = names.iter().cloned().collect();
    v.sort();
    v
}

fn sorted_map_keys<V>(map: &HashMap<String, V>) -> Vec<String> {
    let mut v: Vec<String> = map.keys().cloned().collect();
    v.sort();
    v
}

// ── MFT emission ──────────────────────────────────────────────────────

fn emit_mft(
    w: &mut JsWriter,
    nodes: &[MftNode],
    kwargs: &HashMap<String, SExpr>,
    counter: &mut usize,
) -> Result<(), LyknError> {
    if nodes.iter().all(|n| matches!(n, MftNode::Literal(_))) {
        w.write("`");
        for node in nodes {
            if let MftNode::Literal(s) = node {
                emit_template_text_icu(w, s);
            }
        }
        w.write("`");
        return Ok(());
    }

    w.write("`");
    for node in nodes {
        match node {
            MftNode::Literal(s) => emit_template_text_icu(w, s),
            MftNode::Slot(name) => {
                w.write("${");
                if let Some(expr) = kwargs.get(name) {
                    emit_expr(w, expr, 0)?;
                } else {
                    unreachable!("slot {{{}}} has no kwarg after validation", name);
                }
                w.write("}");
            }
            MftNode::Plural { name, branches } => {
                w.write("${");
                emit_plural_iife(w, name, branches, kwargs, counter)?;
                w.write("}");
            }
            MftNode::Select { name, branches } => {
                w.write("${");
                emit_select_iife(w, name, branches, kwargs, counter)?;
                w.write("}");
            }
        }
    }
    w.write("`");
    Ok(())
}

fn emit_template_text_icu(w: &mut JsWriter, value: &str) {
    for ch in value.chars() {
        match ch {
            '`' => w.write("\\`"),
            '\\' => w.write("\\\\"),
            // '$' is always escaped to '\$' so that user text never accidentally
            // forms a `${...}` template-literal interpolation in the emitted JS.
            '$' => w.write("\\$"),
            c => w.write_char(c),
        }
    }
}

fn make_slot_override(
    kwargs: &HashMap<String, SExpr>,
    name: &str,
    replacement: SExpr,
) -> HashMap<String, SExpr> {
    let mut copy = kwargs.clone();
    copy.insert(name.to_string(), replacement);
    copy
}

fn emit_plural_iife(
    w: &mut JsWriter,
    name: &str,
    branches: &[PluralBranch],
    kwargs: &HashMap<String, SExpr>,
    counter: &mut usize,
) -> Result<(), LyknError> {
    let var_name = fresh_icu_var(counter);
    let var_expr = SExpr::Atom {
        value: var_name.clone(),
        span: Span::default(),
    };

    w.write("(() => {");
    w.write(&format!(" const {} = ", var_name));
    if let Some(expr) = kwargs.get(name) {
        emit_expr(w, expr, 0)?;
    } else {
        w.write(name);
    }
    w.write(";");

    for branch in branches {
        if let PluralKey::Exact(n) = &branch.key {
            w.write(&format!(" if ({} === {}) return ", var_name, n));
            let inner_kwargs = make_slot_override(kwargs, name, var_expr.clone());
            emit_mft(w, &branch.body, &inner_kwargs, counter)?;
            w.write(";");
        }
    }

    for branch in branches {
        if let PluralKey::Category(cat) = &branch.key {
            if cat == "other" {
                continue;
            }
            if let Some(test) = plural_category_test(cat, &var_name) {
                w.write(&format!(" if ({}) return ", test));
                let inner_kwargs = make_slot_override(kwargs, name, var_expr.clone());
                emit_mft(w, &branch.body, &inner_kwargs, counter)?;
                w.write(";");
            }
        }
    }

    if let Some(other) = branches
        .iter()
        .find(|b| b.key == PluralKey::Category("other".into()))
    {
        w.write(" return ");
        let inner_kwargs = make_slot_override(kwargs, name, var_expr);
        emit_mft(w, &other.body, &inner_kwargs, counter)?;
        w.write(";");
    }

    w.write(" })()");
    Ok(())
}

fn emit_select_iife(
    w: &mut JsWriter,
    name: &str,
    branches: &[SelectBranch],
    kwargs: &HashMap<String, SExpr>,
    counter: &mut usize,
) -> Result<(), LyknError> {
    let var_name = fresh_icu_var(counter);
    let var_expr = SExpr::Atom {
        value: var_name.clone(),
        span: Span::default(),
    };

    w.write("(() => {");
    w.write(&format!(" const {} = ", var_name));
    if let Some(expr) = kwargs.get(name) {
        emit_expr(w, expr, 0)?;
    } else {
        w.write(name);
    }
    w.write(";");

    for branch in branches {
        if branch.key == "other" {
            continue;
        }
        w.write(&format!(" if ({} === \"{}\") return ", var_name, branch.key));
        let inner_kwargs = make_slot_override(kwargs, name, var_expr.clone());
        emit_mft(w, &branch.body, &inner_kwargs, counter)?;
        w.write(";");
    }

    if let Some(other) = branches.iter().find(|b| b.key == "other") {
        w.write(" return ");
        let inner_kwargs = make_slot_override(kwargs, name, var_expr);
        emit_mft(w, &other.body, &inner_kwargs, counter)?;
        w.write(";");
    }

    w.write(" })()");
    Ok(())
}

fn plural_category_test(category: &str, var_name: &str) -> Option<String> {
    // English CLDR Phase A: only 'one' has a test
    if category == "one" {
        Some(format!("{} === 1", var_name))
    } else {
        None
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Parser tests ──────────────────────────────────────────────

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
        assert_eq!(result, vec![
            MftNode::Literal("Hello, ".into()),
            MftNode::Slot("name".into()),
            MftNode::Literal("!".into()),
        ]);
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
        let err = parse_icu("{n, plural, weird {x} other {y}}").unwrap_err();
        assert!(err.message.contains("unknown plural category"));
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

    // ── Review regression: categories ─────────────────────────────

    #[test]
    fn test_reject_zero_category() {
        let err = parse_icu("{n, plural, zero {none} other {many}}").unwrap_err();
        assert!(err.message.contains("not valid under English plural rules"));
    }

    #[test]
    fn test_reject_two_few_many() {
        for cat in &["two", "few", "many"] {
            let input = format!("{{n, plural, {} {{x}} other {{y}}}}", cat);
            let err = parse_icu(&input).unwrap_err();
            assert!(err.message.contains("not valid under English plural rules"), "{}", cat);
        }
    }

    #[test]
    fn test_exact_one_overlap() {
        let err = parse_icu("{n, plural, =1 {x} one {y} other {z}}").unwrap_err();
        assert!(err.message.contains("overlapping branches"));
    }

    #[test]
    fn test_exact_one_overlap_reverse() {
        let err = parse_icu("{n, plural, one {y} =1 {x} other {z}}").unwrap_err();
        assert!(err.message.contains("overlapping branches"));
    }

    #[test]
    fn test_error_no_template_prefix() {
        let err = parse_icu("{n, plural, weird {x} other {y}}").unwrap_err();
        assert!(!err.message.starts_with("template:"), "{}", err.message);
    }

    // ── Review regression: R-10 (# handling) ──────────────────────

    #[test]
    fn parse_hash_at_top_level_is_literal() {
        let result = parse_icu("a # b").unwrap();
        assert_eq!(result, vec![MftNode::Literal("a # b".into())]);
    }

    #[test]
    fn parse_hash_only_top_level() {
        let result = parse_icu("#").unwrap();
        assert_eq!(result, vec![MftNode::Literal("#".into())]);
    }

    #[test]
    fn parse_message_does_not_spin_on_hash() {
        let result = parse_icu("####").unwrap();
        assert_eq!(result, vec![MftNode::Literal("####".into())]);
    }

    // ── Emitter tests ─────────────────────────────────────────────

    fn mk(name: &str) -> SExpr {
        SExpr::Atom { value: name.into(), span: Span::default() }
    }

    fn emit_mft_str(mft: &[MftNode], kwargs: &HashMap<String, SExpr>) -> String {
        let mut w = JsWriter::new();
        let mut counter = 0;
        emit_mft(&mut w, mft, kwargs, &mut counter).unwrap();
        w.finish()
    }

    #[test]
    fn test_emit_simple_slot() {
        let mft = parse_icu("Hello, {name}!").unwrap();
        let mut kwargs = HashMap::new();
        kwargs.insert("name".into(), mk("n"));
        assert_eq!(emit_mft_str(&mft, &kwargs), "`Hello, ${n}!`");
    }

    #[test]
    fn test_emit_no_slots() {
        let mft = parse_icu("hello world").unwrap();
        assert_eq!(emit_mft_str(&mft, &HashMap::new()), "`hello world`");
    }

    #[test]
    fn test_emit_plural() {
        let mft = parse_icu("{n, plural, one {1 item} other {# items}}").unwrap();
        let mut kwargs = HashMap::new();
        kwargs.insert("n".into(), mk("count"));
        let output = emit_mft_str(&mft, &kwargs);
        assert!(output.contains("count"));
        assert!(output.contains("=== 1"));
        assert!(output.contains("1 item"));
    }

    #[test]
    fn test_emit_select() {
        let mft = parse_icu("{role, select, owner {You own it.} other {Guest.}}").unwrap();
        let mut kwargs = HashMap::new();
        kwargs.insert("role".into(), mk("r"));
        let output = emit_mft_str(&mft, &kwargs);
        assert!(output.contains("\"owner\""));
        assert!(output.contains("You own it."));
    }

    #[test]
    fn test_emit_nested_no_tdz() {
        let mft = parse_icu("{n, plural, one {{n, plural, one {a} other {b}}} other {c}}").unwrap();
        let mut kwargs = HashMap::new();
        kwargs.insert("n".into(), mk("n"));
        let output = emit_mft_str(&mft, &kwargs);
        assert!(output.contains("_v0"), "expected _v0: {output}");
        assert!(output.contains("_v1"), "expected _v1: {output}");
        assert!(!output.contains("const _v0 = _v0"), "TDZ: {output}");
    }

    #[test]
    fn test_emit_select_uses_override() {
        let mft = parse_icu("{role, select, owner {Owner: {role}} other {Guest: {role}}}").unwrap();
        let mut kwargs = HashMap::new();
        kwargs.insert("role".into(), mk("r"));
        let output = emit_mft_str(&mft, &kwargs);
        assert!(output.contains("_v0"), "expected _v0 in branches: {output}");
    }

    // ── Review regression: R-7 ($ escaping) ───────────────────────

    #[test]
    fn emit_dollar_in_literal_is_escaped() {
        let mft = parse_icu("$5").unwrap();
        let output = emit_mft_str(&mft, &HashMap::new());
        assert!(output.contains("\\$"), "expected escaped \\$, got: {output}");
    }

    #[test]
    fn emit_dollar_before_brace_is_escaped() {
        let mft = parse_icu("$'{'name'}'").unwrap();
        let output = emit_mft_str(&mft, &HashMap::new());
        assert!(output.contains("\\$"), "expected escaped \\$, got: {output}");
    }
}
