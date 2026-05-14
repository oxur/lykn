//! Core expression and statement emitter.
//!
//! Dispatches on the head atom of an `SExpr::List` to emit the corresponding
//! JavaScript construct. Leaf nodes (atoms, numbers, strings, etc.) are emitted
//! directly.

use crate::ast::sexpr::SExpr;
use crate::error::LyknError;

use super::format::JsWriter;
use super::icu::{try_emit_template_icu, IcuDispatch};
use super::names::{emit_atom, to_js_identifier};
use super::precedence::precedence;

// ── Statement vs expression context ────────────────────────────────────

/// Forms that are emitted as statements (no trailing semicolon added by the
/// caller — the form handler takes care of its own formatting).
const STATEMENT_FORMS: &[&str] = &[
    "if",
    "while",
    "do-while",
    "for",
    "for-of",
    "for-in",
    "for-await-of",
    "try",
    "switch",
    "block",
    "function",
    "function*",
    "class",
    "import",
    "export",
    "const",
    "let",
    "var",
    "return",
    "throw",
    "break",
    "continue",
    "label",
    "debugger",
];

fn is_statement_form(name: &str) -> bool {
    STATEMENT_FORMS.contains(&name)
}

// ── Public entry points ────────────────────────────────────────────────

/// Emit an expression. `parent_prec` is the precedence of the enclosing
/// operator context (0 means top-level / no enclosing operator).
pub fn emit_expr(w: &mut JsWriter, expr: &SExpr, parent_prec: u8) -> Result<(), LyknError> {
    match expr {
        SExpr::Number { value, .. } => emit_number(w, *value),
        SExpr::String { value, .. } => emit_string_literal(w, value),
        SExpr::Bool { value, .. } => {
            w.write(if *value { "true" } else { "false" });
        }
        SExpr::Null { .. } => w.write("null"),
        SExpr::Atom { value, .. } => emit_atom(w, value),
        SExpr::Keyword { value, .. } => {
            // Keywords emit as string literals with camelCase conversion.
            w.write("\"");
            w.write(&to_js_identifier(value));
            w.write("\"");
        }
        SExpr::List { values, .. } => emit_list(w, values, parent_prec)?,
        SExpr::Cons { .. } => {
            // Cons cells are rarely used in kernel output; emit a comment.
            w.write("/* cons */");
        }
    }
    Ok(())
}

/// Emit an expression in statement position: appends a semicolon + newline for
/// expression statements, or just a newline for statement forms.
pub fn emit_statement(w: &mut JsWriter, expr: &SExpr) -> Result<(), LyknError> {
    if let SExpr::List { values, .. } = expr
        && let Some(head) = values.first().and_then(|e| e.as_atom())
        && is_statement_form(head)
    {
        emit_list(w, values, 0)?;
        return Ok(());
    }
    // Expression statement.
    emit_expr(w, expr, 0)?;
    w.semicolon();
    Ok(())
}

// ── Leaf helpers ───────────────────────────────────────────────────────

fn emit_number(w: &mut JsWriter, value: f64) {
    if value.fract() == 0.0 && value.is_finite() {
        // Safe: kernel numbers with zero fractional part are within i64 range.
        #[allow(clippy::cast_possible_truncation)]
        let int_val = value as i64;
        w.write(&int_val.to_string());
    } else {
        w.write(&value.to_string());
    }
}

fn emit_string_literal(w: &mut JsWriter, value: &str) {
    w.write("\"");
    for ch in value.chars() {
        match ch {
            '"' => w.write("\\\""),
            '\\' => w.write("\\\\"),
            '\n' => w.write("\\n"),
            '\r' => w.write("\\r"),
            '\t' => w.write("\\t"),
            c if c.is_control() => w.write(&format!("\\u{:04x}", c as u32)),
            c => w.write_char(c),
        }
    }
    w.write("\"");
}

fn emit_template_text(w: &mut JsWriter, value: &str) {
    // '$' is always escaped to '\$' so that user text never accidentally
    // forms a `${...}` template-literal interpolation in the emitted JS.
    // Concat-mode and ICU-mode agree on this.
    for ch in value.chars() {
        match ch {
            '`' => w.write("\\`"),
            '\\' => w.write("\\\\"),
            '$' => w.write("\\$"),
            c => w.write_char(c),
        }
    }
}

// ── List dispatcher ───────────────────────────────────────────────────

fn emit_list(w: &mut JsWriter, values: &[SExpr], parent_prec: u8) -> Result<(), LyknError> {
    if values.is_empty() {
        return Ok(());
    }

    let head = match values[0].as_atom() {
        Some(h) => h,
        None => {
            // Head is not an atom — could be an IIFE like ((=> () body)) or
            // computed call like ((get-fn) arg). Wrap in parens to ensure the
            // callee is parsed as an expression (e.g. (() => 42)() not () => 42()).
            w.write("(");
            emit_expr(w, &values[0], 0)?;
            w.write(")");
            emit_call_args(w, &values[1..])?;
            return Ok(());
        }
    };

    let args = &values[1..];

    match head {
        // ── Declarations ───────────────────────────────────────────
        "const" | "let" | "var" => emit_declaration(w, head, args)?,

        // ── Functions ──────────────────────────────────────────────
        "=>" => emit_arrow(w, args, false)?,
        "lambda" => emit_lambda(w, args)?,
        "function" => emit_function(w, args)?,
        "function*" => emit_function_star(w, args)?,
        "async" => emit_async(w, args)?,
        "await" => emit_await(w, args)?,
        "yield" => emit_yield(w, args)?,
        "yield*" => emit_yield_star(w, args)?,
        "return" => emit_return(w, args)?,

        // ── Control flow ───────────────────────────────────────────
        "if" => emit_if(w, args)?,
        "block" => emit_block(w, args)?,
        "while" => emit_while(w, args)?,
        "do-while" => emit_do_while(w, args)?,
        "for" => emit_for(w, args)?,
        "for-of" => emit_for_of(w, args)?,
        "for-in" => emit_for_in(w, args)?,
        "for-await-of" => emit_for_await_of(w, args)?,
        "switch" => emit_switch(w, args)?,
        "break" => emit_break(w, args)?,
        "continue" => emit_continue(w, args)?,
        "label" => emit_label(w, args)?,
        "throw" => emit_throw(w, args)?,
        "try" => emit_try(w, args)?,

        // ── Expressions ────────────────────────────────────────────
        "?" => emit_ternary(w, args, parent_prec)?,
        "=" => emit_assignment(w, args)?,
        "new" => emit_new(w, args)?,
        "get" => emit_computed_member(w, args)?,
        "." => emit_method_call(w, args)?,
        "seq" => emit_seq(w, args)?,
        "++" => emit_update(w, "++", args)?,
        "--" => emit_update(w, "--", args)?,

        // ── Unary operators ────────────────────────────────────────
        "!" | "~" => emit_unary_symbol(w, head, args)?,
        "typeof" | "void" | "delete" => emit_unary_word(w, head, args)?,

        // ── Compound assignment ────────────────────────────────────
        "+=" | "-=" | "*=" | "/=" | "%=" | "**=" | "<<=" | ">>=" | ">>>=" | "&=" | "|=" | "^="
        | "&&=" | "||=" | "??=" => emit_compound_assign(w, head, args)?,

        // ── Binary / n-ary operators ───────────────────────────────
        "+" | "-" | "*" | "/" | "%" | "**" | "===" | "!==" | "==" | "!=" | "<" | ">" | "<="
        | ">=" | "&&" | "||" | "??" | "&" | "|" | "^" | "<<" | ">>" | ">>>" | "in"
        | "instanceof" => emit_binary(w, head, args, parent_prec)?,

        // ── Object / Array ─────────────────────────────────────────
        "object" => emit_object(w, args)?,
        "array" => emit_array(w, args)?,
        "spread" => emit_spread(w, args)?,

        // ── Templates & regex ──────────────────────────────────────
        "template" => emit_template(w, args)?,
        "tag" => emit_tagged_template(w, args)?,
        "regex" => emit_regex(w, args),

        // ── Patterns (used in destructuring contexts) ──────────────
        "default" => emit_default_pattern(w, args)?,
        "rest" => emit_rest_pattern(w, args)?,
        "alias" => emit_alias_pattern(w, args)?,

        // ── Classes ────────────────────────────────────────────────
        "class" => emit_class(w, args)?,
        "class-expr" => emit_class_expr(w, args)?,

        // ── Modules ────────────────────────────────────────────────
        "import" => emit_import(w, args)?,
        "export" => emit_export(w, args)?,
        "dynamic-import" => emit_dynamic_import(w, args)?,

        // ── Misc ───────────────────────────────────────────────────
        "debugger" => {
            w.write("debugger");
            w.semicolon();
        }

        // ── Default: function call ─────────────────────────────────
        _ => {
            emit_atom(w, head);
            emit_call_args(w, args)?;
        }
    }
    Ok(())
}

// ── Helper: emit comma-separated call arguments ────────────────────────

fn emit_call_args(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    w.write("(");
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            w.write(", ");
        }
        emit_expr(w, arg, 0)?;
    }
    w.write(")");
    Ok(())
}

// ── Helper: emit a block body `{ stmt; stmt; }` ───────────────────────

fn emit_block_body(w: &mut JsWriter, stmts: &[SExpr]) -> Result<(), LyknError> {
    w.write("{");
    w.newline();
    w.indent();
    for stmt in stmts {
        emit_statement(w, stmt)?;
    }
    w.dedent();
    w.write("}");
    Ok(())
}

// ── Helper: emit a pattern (destructuring left-hand side) ──────────────

fn emit_pattern(w: &mut JsWriter, expr: &SExpr) -> Result<(), LyknError> {
    match expr {
        SExpr::Atom { value, .. } => {
            if value == "_" {
                // empty slot marker — but in object patterns _ is just an ident
                w.write(&to_js_identifier(value));
            } else {
                emit_atom(w, value);
            }
        }
        SExpr::List { values, .. } if !values.is_empty() => match values[0].as_atom() {
            Some("object") => emit_object_pattern(w, &values[1..])?,
            Some("array") => emit_array_pattern(w, &values[1..])?,
            Some("default") => emit_default_pattern(w, &values[1..])?,
            Some("rest") => emit_rest_pattern(w, &values[1..])?,
            Some("alias") => emit_alias_pattern(w, &values[1..])?,
            _ => emit_expr(w, expr, 0)?,
        },
        _ => emit_expr(w, expr, 0)?,
    }
    Ok(())
}

fn emit_object_pattern(w: &mut JsWriter, members: &[SExpr]) -> Result<(), LyknError> {
    w.write("{");
    for (i, member) in members.iter().enumerate() {
        if i > 0 {
            w.write(", ");
        }
        match member {
            SExpr::Atom { value, .. } => w.write(&to_js_identifier(value)),
            SExpr::List { values, .. } if !values.is_empty() => {
                match values[0].as_atom() {
                    Some("default") => {
                        // (default name val)
                        if values.len() >= 3 {
                            emit_pattern(w, &values[1])?;
                            w.write(" = ");
                            emit_expr(w, &values[2], 0)?;
                        }
                    }
                    Some("alias") => {
                        emit_alias_pattern(w, &values[1..])?;
                    }
                    Some("rest") => {
                        emit_rest_pattern(w, &values[1..])?;
                    }
                    Some("spread") => {
                        w.write("...");
                        if values.len() >= 2 {
                            emit_expr(w, &values[1], 0)?;
                        }
                    }
                    _ => emit_pattern(w, member)?,
                }
            }
            _ => emit_pattern(w, member)?,
        }
    }
    w.write("}");
    Ok(())
}

fn emit_array_pattern(w: &mut JsWriter, elements: &[SExpr]) -> Result<(), LyknError> {
    w.write("[");
    for (i, elem) in elements.iter().enumerate() {
        if i > 0 {
            w.write(", ");
        }
        match elem {
            SExpr::Atom { value, .. } if value == "_" => {
                // empty slot — just leave blank
            }
            SExpr::List { values, .. }
                if !values.is_empty() && values[0].as_atom() == Some("rest") =>
            {
                emit_rest_pattern(w, &values[1..])?;
            }
            _ => emit_pattern(w, elem)?,
        }
    }
    w.write("]");
    Ok(())
}

// ── Declarations ───────────────────────────────────────────────────────

fn emit_declaration(w: &mut JsWriter, kind: &str, args: &[SExpr]) -> Result<(), LyknError> {
    // (const binding) or (const binding init)
    w.write(kind);
    w.write(" ");
    if args.is_empty() {
        w.semicolon();
        return Ok(());
    }
    // Left-hand side: could be a pattern (object/array) or simple name.
    emit_pattern(w, &args[0])?;
    if args.len() >= 2 {
        w.write(" = ");
        emit_expr(w, &args[1], 0)?;
    }
    w.semicolon();
    Ok(())
}

// ── Functions ──────────────────────────────────────────────────────────

fn emit_arrow(w: &mut JsWriter, args: &[SExpr], is_async: bool) -> Result<(), LyknError> {
    // (=> (params...) body...) or (=> () body...)
    if is_async {
        w.write("async ");
    }
    if args.is_empty() {
        w.write("() => {}");
        return Ok(());
    }

    // Params.
    emit_params(w, &args[0])?;
    w.write(" => ");

    let body = &args[1..];
    if body.len() == 1 {
        // Single expression body — check if it is a block.
        if let SExpr::List { values, .. } = &body[0]
            && values.first().and_then(|e| e.as_atom()) == Some("block")
        {
            emit_block_body(w, &values[1..])?;
            return Ok(());
        }
        emit_expr(w, &body[0], 0)?;
    } else {
        // Multiple statements → block body.
        emit_block_body(w, body)?;
    }
    Ok(())
}

fn emit_lambda(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (lambda (params...) body...)
    w.write("function");
    if args.is_empty() {
        w.write("() {}");
        return Ok(());
    }
    emit_params(w, &args[0])?;
    w.write(" ");
    emit_block_body(w, &args[1..])?;
    Ok(())
}

fn emit_function(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (function name (params...) body...)
    if args.is_empty() {
        w.write("function() {}");
        w.newline();
        return Ok(());
    }
    w.write("function ");
    emit_expr(w, &args[0], 0)?;
    if args.len() >= 2 {
        emit_params(w, &args[1])?;
    } else {
        w.write("()");
    }
    w.write(" ");
    if args.len() >= 3 {
        emit_block_body(w, &args[2..])?;
    } else {
        w.write("{}");
    }
    w.newline();
    Ok(())
}

fn emit_function_star(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (function* name (params...) body...) — named
    // (function* (params...) body...)      — anonymous
    if args.is_empty() {
        w.write("function*() {}");
        w.newline();
        return Ok(());
    }
    // Check if first arg is a param list (anonymous) or a name (named)
    if args[0].is_list() {
        // Anonymous: (function* (params) body...)
        w.write("function*");
        emit_params(w, &args[0])?;
        w.write(" ");
        if args.len() >= 2 {
            emit_block_body(w, &args[1..])?;
        } else {
            w.write("{}");
        }
    } else {
        // Named: (function* name (params) body...)
        w.write("function* ");
        emit_expr(w, &args[0], 0)?;
        if args.len() >= 2 {
            emit_params(w, &args[1])?;
        } else {
            w.write("()");
        }
        w.write(" ");
        if args.len() >= 3 {
            emit_block_body(w, &args[2..])?;
        } else {
            w.write("{}");
        }
    }
    w.newline();
    Ok(())
}

fn emit_async(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (async (=> (params) body)) — unwrap child function, prepend async
    if args.is_empty() {
        return Ok(());
    }
    let inner = &args[0];
    if let SExpr::List { values, .. } = inner
        && let Some(head) = values.first().and_then(|e| e.as_atom())
    {
        match head {
            "=>" => {
                emit_arrow(w, &values[1..], true)?;
                return Ok(());
            }
            "function" => {
                w.write("async ");
                emit_function(w, &values[1..])?;
                return Ok(());
            }
            "function*" => {
                w.write("async ");
                emit_function_star(w, &values[1..])?;
                return Ok(());
            }
            "lambda" => {
                w.write("async ");
                emit_lambda(w, &values[1..])?;
                return Ok(());
            }
            _ => {}
        }
    }
    w.write("async ");
    emit_expr(w, inner, 0)?;
    Ok(())
}

fn emit_await(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    w.write("await ");
    if let Some(expr) = args.first() {
        emit_expr(w, expr, 0)?;
    }
    Ok(())
}

fn emit_yield(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    w.write("yield");
    if let Some(expr) = args.first() {
        w.write(" ");
        emit_expr(w, expr, 0)?;
    }
    Ok(())
}

fn emit_yield_star(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    w.write("yield* ");
    if let Some(expr) = args.first() {
        emit_expr(w, expr, 0)?;
    }
    Ok(())
}

fn emit_return(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    w.write("return");
    if let Some(expr) = args.first() {
        w.write(" ");
        emit_expr(w, expr, 0)?;
    }
    w.semicolon();
    Ok(())
}

fn emit_params(w: &mut JsWriter, params_expr: &SExpr) -> Result<(), LyknError> {
    w.write("(");
    if let SExpr::List { values, .. } = params_expr {
        for (i, param) in values.iter().enumerate() {
            if i > 0 {
                w.write(", ");
            }
            emit_pattern(w, param)?;
        }
    }
    w.write(")");
    Ok(())
}

// ── Control flow ───────────────────────────────────────────────────────

fn emit_if(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (if cond then) or (if cond then else)
    if args.is_empty() {
        return Ok(());
    }
    w.write("if (");
    emit_expr(w, &args[0], 0)?;
    w.write(") ");

    if args.len() >= 2 {
        emit_stmt_or_block(w, &args[1])?;
    }

    if args.len() >= 3 {
        // Check if we just emitted a block — if so, the cursor is after `}`.
        w.write(" else ");
        emit_stmt_or_block(w, &args[2])?;
    }

    w.newline();
    Ok(())
}

/// Emit a single statement as either a block `{ ... }` or inline `stmt;`.
fn emit_stmt_or_block(w: &mut JsWriter, expr: &SExpr) -> Result<(), LyknError> {
    if let SExpr::List { values, .. } = expr
        && values.first().and_then(|e| e.as_atom()) == Some("block")
    {
        emit_block_body(w, &values[1..])?;
        return Ok(());
    }
    // Inline statement.
    emit_statement(w, expr)?;
    Ok(())
}

fn emit_block(w: &mut JsWriter, stmts: &[SExpr]) -> Result<(), LyknError> {
    emit_block_body(w, stmts)?;
    w.newline();
    Ok(())
}

fn emit_while(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    if args.is_empty() {
        return Ok(());
    }
    w.write("while (");
    emit_expr(w, &args[0], 0)?;
    w.write(") ");
    emit_block_body(w, &args[1..])?;
    w.newline();
    Ok(())
}

fn emit_do_while(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (do-while cond body...)
    if args.is_empty() {
        return Ok(());
    }
    w.write("do ");
    emit_block_body(w, &args[1..])?;
    w.write(" while (");
    emit_expr(w, &args[0], 0)?;
    w.write(")");
    w.semicolon();
    Ok(())
}

fn emit_for(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (for init test update body...)
    if args.len() < 3 {
        return Ok(());
    }
    w.write("for (");
    emit_for_clause(w, &args[0])?;
    w.write("; ");
    emit_for_clause(w, &args[1])?;
    w.write("; ");
    emit_for_clause(w, &args[2])?;
    w.write(") ");
    emit_block_body(w, &args[3..])?;
    w.newline();
    Ok(())
}

fn emit_for_clause(w: &mut JsWriter, expr: &SExpr) -> Result<(), LyknError> {
    // An empty list means "omit".
    if let SExpr::List { values, .. } = expr {
        if values.is_empty() {
            return Ok(());
        }
        // Special handling for declarations in for-init: emit without
        // trailing semicolon.
        if let Some(head) = values.first().and_then(|e| e.as_atom())
            && matches!(head, "const" | "let" | "var")
        {
            emit_declaration_bare(w, head, &values[1..])?;
            return Ok(());
        }
    }
    emit_expr(w, expr, 0)?;
    Ok(())
}

/// Emit a declaration without a trailing semicolon (for use in for-init).
fn emit_declaration_bare(w: &mut JsWriter, kind: &str, args: &[SExpr]) -> Result<(), LyknError> {
    w.write(kind);
    w.write(" ");
    if args.is_empty() {
        return Ok(());
    }
    emit_pattern(w, &args[0])?;
    if args.len() >= 2 {
        w.write(" = ");
        emit_expr(w, &args[1], 0)?;
    }
    Ok(())
}

fn emit_for_of(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (for-of binding iterable body...)
    if args.len() < 2 {
        return Ok(());
    }
    w.write("for (const ");
    emit_pattern(w, &args[0])?;
    w.write(" of ");
    emit_expr(w, &args[1], 0)?;
    w.write(") ");
    emit_block_body(w, &args[2..])?;
    w.newline();
    Ok(())
}

fn emit_for_in(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (for-in binding obj body...)
    if args.len() < 2 {
        return Ok(());
    }
    w.write("for (const ");
    emit_pattern(w, &args[0])?;
    w.write(" in ");
    emit_expr(w, &args[1], 0)?;
    w.write(") ");
    emit_block_body(w, &args[2..])?;
    w.newline();
    Ok(())
}

fn emit_for_await_of(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (for-await-of binding iterable body...)
    if args.len() < 2 {
        return Ok(());
    }
    w.write("for await (const ");
    emit_pattern(w, &args[0])?;
    w.write(" of ");
    emit_expr(w, &args[1], 0)?;
    w.write(") ");
    emit_block_body(w, &args[2..])?;
    w.newline();
    Ok(())
}

fn emit_switch(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (switch disc case1 case2 ...)
    if args.is_empty() {
        return Ok(());
    }
    w.write("switch (");
    emit_expr(w, &args[0], 0)?;
    w.write(") {");
    w.newline();
    w.indent();

    for case in &args[1..] {
        if let SExpr::List { values, .. } = case {
            if values.is_empty() {
                continue;
            }
            let is_default = values[0].as_atom() == Some("default");
            if is_default {
                w.write("default:");
            } else {
                w.write("case ");
                emit_expr(w, &values[0], 0)?;
                w.write(":");
            }
            w.newline();
            w.indent();
            for stmt in &values[1..] {
                emit_statement(w, stmt)?;
            }
            w.dedent();
        }
    }

    w.dedent();
    w.write("}");
    w.newline();
    Ok(())
}

fn emit_break(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    w.write("break");
    if let Some(label) = args.first() {
        w.write(" ");
        emit_expr(w, label, 0)?;
    }
    w.semicolon();
    Ok(())
}

fn emit_continue(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    w.write("continue");
    if let Some(label) = args.first() {
        w.write(" ");
        emit_expr(w, label, 0)?;
    }
    w.semicolon();
    Ok(())
}

fn emit_label(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (label name body)
    if args.len() < 2 {
        return Ok(());
    }
    emit_expr(w, &args[0], 0)?;
    w.write(": ");
    emit_statement(w, &args[1])?;
    Ok(())
}

fn emit_throw(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    w.write("throw ");
    if let Some(expr) = args.first() {
        emit_expr(w, expr, 0)?;
    }
    w.semicolon();
    Ok(())
}

fn emit_try(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (try body... (catch e handler...) (finally cleanup...))
    // Separate body forms from catch/finally.
    let mut body: Vec<&SExpr> = Vec::new();
    let mut catch_clause: Option<&[SExpr]> = None;
    let mut finally_clause: Option<&[SExpr]> = None;

    for arg in args {
        if let SExpr::List { values, .. } = arg
            && let Some(head) = values.first().and_then(|e| e.as_atom())
        {
            if head == "catch" {
                catch_clause = Some(&values[1..]);
                continue;
            }
            if head == "finally" {
                finally_clause = Some(&values[1..]);
                continue;
            }
        }
        body.push(arg);
    }

    w.write("try ");
    w.write("{");
    w.newline();
    w.indent();
    for stmt in &body {
        emit_statement(w, stmt)?;
    }
    w.dedent();
    w.write("}");

    if let Some(catch_args) = catch_clause {
        w.write(" catch");
        if let Some(binding) = catch_args.first() {
            w.write(" (");
            emit_expr(w, binding, 0)?;
            w.write(")");
        }
        w.write(" ");
        w.write("{");
        w.newline();
        w.indent();
        for stmt in &catch_args[1..] {
            emit_statement(w, stmt)?;
        }
        w.dedent();
        w.write("}");
    }

    if let Some(finally_args) = finally_clause {
        w.write(" finally ");
        w.write("{");
        w.newline();
        w.indent();
        for stmt in finally_args {
            emit_statement(w, stmt)?;
        }
        w.dedent();
        w.write("}");
    }

    w.newline();
    Ok(())
}

// ── Expressions ────────────────────────────────────────────────────────

fn emit_ternary(w: &mut JsWriter, args: &[SExpr], parent_prec: u8) -> Result<(), LyknError> {
    // (? test then else) — precedence 3 (lower than ??)
    let my_prec: u8 = 3;
    let needs_parens = my_prec < parent_prec;
    if needs_parens {
        w.write("(");
    }
    if args.len() >= 3 {
        emit_expr(w, &args[0], my_prec)?;
        w.write(" ? ");
        emit_expr(w, &args[1], 0)?;
        w.write(" : ");
        emit_expr(w, &args[2], my_prec)?;
    }
    if needs_parens {
        w.write(")");
    }
    Ok(())
}

fn emit_assignment(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (= left right)
    if args.len() >= 2 {
        emit_pattern(w, &args[0])?;
        w.write(" = ");
        emit_expr(w, &args[1], 0)?;
    }
    Ok(())
}

fn emit_new(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (new Thing a b)
    w.write("new ");
    if let Some(constructor) = args.first() {
        emit_expr(w, constructor, 0)?;
        emit_call_args(w, &args[1..])?;
    }
    Ok(())
}

fn emit_computed_member(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (get obj key)
    if args.len() >= 2 {
        emit_expr(w, &args[0], 20)?;
        w.write("[");
        emit_expr(w, &args[1], 0)?;
        w.write("]");
    }
    Ok(())
}

fn emit_method_call(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (. obj method arg...)
    if args.len() < 2 {
        return Ok(());
    }
    emit_expr(w, &args[0], 20)?;
    w.write(".");
    if let SExpr::Atom { value, .. } = &args[1] {
        w.write(&to_js_identifier(value));
    } else {
        emit_expr(w, &args[1], 20)?;
    }
    emit_call_args(w, &args[2..])?;
    Ok(())
}

fn emit_seq(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            w.write(", ");
        }
        emit_expr(w, arg, 0)?;
    }
    Ok(())
}

fn emit_update(w: &mut JsWriter, op: &str, args: &[SExpr]) -> Result<(), LyknError> {
    // (++ x) → ++x
    if let Some(operand) = args.first() {
        w.write(op);
        emit_expr(w, operand, 20)?;
    }
    Ok(())
}

// ── Unary ──────────────────────────────────────────────────────────────

fn emit_unary_symbol(w: &mut JsWriter, op: &str, args: &[SExpr]) -> Result<(), LyknError> {
    if let Some(operand) = args.first() {
        w.write(op);
        emit_expr(w, operand, 20)?;
    }
    Ok(())
}

fn emit_unary_word(w: &mut JsWriter, op: &str, args: &[SExpr]) -> Result<(), LyknError> {
    if let Some(operand) = args.first() {
        w.write(op);
        w.write(" ");
        emit_expr(w, operand, 20)?;
    }
    Ok(())
}

// ── Compound assignment ────────────────────────────────────────────────

fn emit_compound_assign(w: &mut JsWriter, op: &str, args: &[SExpr]) -> Result<(), LyknError> {
    if args.len() >= 2 {
        emit_expr(w, &args[0], 0)?;
        w.write(" ");
        w.write(op);
        w.write(" ");
        emit_expr(w, &args[1], 0)?;
    }
    Ok(())
}

// ── Binary / n-ary ─────────────────────────────────────────────────────

fn emit_binary(w: &mut JsWriter, op: &str, args: &[SExpr], parent_prec: u8) -> Result<(), LyknError> {
    let my_prec = precedence(op);
    let needs_parens = my_prec < parent_prec;

    if needs_parens {
        w.write("(");
    }

    if args.len() == 1 {
        // Unary minus: (- x) → -x
        if op == "-" {
            w.write("-");
            emit_expr(w, &args[0], 20)?;
        } else {
            emit_expr(w, &args[0], my_prec)?;
        }
    } else {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                // Word operators need spaces; symbol operators too for readability.
                w.write(" ");
                w.write(op);
                w.write(" ");
            }
            emit_expr(w, arg, my_prec + 1)?;
        }
    }

    if needs_parens {
        w.write(")");
    }
    Ok(())
}

// ── Object / Array ─────────────────────────────────────────────────────

fn emit_object(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    w.write("{");
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            w.write(", ");
        }
        match arg {
            SExpr::Atom { value, .. } => {
                // Shorthand property.
                w.write(&to_js_identifier(value));
            }
            SExpr::List { values, .. } if !values.is_empty() => {
                match values[0].as_atom() {
                    Some("spread") => {
                        w.write("...");
                        if values.len() >= 2 {
                            emit_expr(w, &values[1], 0)?;
                        }
                    }
                    _ => {
                        // Check if the key is a list (computed property).
                        if values[0].is_list() {
                            w.write("[");
                            emit_expr(w, &values[0], 0)?;
                            w.write("]");
                        } else {
                            emit_expr(w, &values[0], 0)?;
                        }
                        if values.len() >= 2 {
                            w.write(": ");
                            emit_expr(w, &values[1], 0)?;
                        }
                    }
                }
            }
            _ => emit_expr(w, arg, 0)?,
        }
    }
    w.write("}");
    Ok(())
}

fn emit_array(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    w.write("[");
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            w.write(", ");
        }
        if let SExpr::List { values, .. } = arg
            && values.first().and_then(|e| e.as_atom()) == Some("spread")
        {
            w.write("...");
            if values.len() >= 2 {
                emit_expr(w, &values[1], 0)?;
            }
            continue;
        }
        emit_expr(w, arg, 0)?;
    }
    w.write("]");
    Ok(())
}

fn emit_spread(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    w.write("...");
    if let Some(expr) = args.first() {
        emit_expr(w, expr, 0)?;
    }
    Ok(())
}

// ── Templates & regex ──────────────────────────────────────────────────

fn emit_template(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // DD-54: try ICU mode first; fall through to concat if not applicable
    match try_emit_template_icu(w, args)? {
        IcuDispatch::Handled => return Ok(()),
        IcuDispatch::NotIcu => {}
    }
    w.write("`");
    for arg in args {
        match arg {
            SExpr::String { value, .. } => {
                emit_template_text(w, value);
            }
            _ => {
                w.write("${");
                emit_expr(w, arg, 0)?;
                w.write("}");
            }
        }
    }
    w.write("`");
    Ok(())
}

fn emit_tagged_template(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (tag fn (template ...))
    if args.len() >= 2 {
        emit_expr(w, &args[0], 20)?;
        // The second arg should be a template form — emit it directly.
        if let SExpr::List { values, .. } = &args[1]
            && values.first().and_then(|e| e.as_atom()) == Some("template")
        {
            emit_template(w, &values[1..])?;
            return Ok(());
        }
        emit_expr(w, &args[1], 0)?;
    }
    Ok(())
}

fn emit_regex(w: &mut JsWriter, args: &[SExpr]) {
    // (regex "pattern" "flags"?)
    w.write("/");
    if let Some(SExpr::String { value, .. }) = args.first() {
        w.write(value);
    }
    w.write("/");
    if let Some(SExpr::String { value, .. }) = args.get(1) {
        w.write(value);
    }
}

// ── Patterns ───────────────────────────────────────────────────────────

fn emit_default_pattern(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (default x 0) → x = 0
    if args.len() >= 2 {
        emit_pattern(w, &args[0])?;
        w.write(" = ");
        emit_expr(w, &args[1], 0)?;
    }
    Ok(())
}

fn emit_rest_pattern(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (rest x) → ...x
    w.write("...");
    if let Some(expr) = args.first() {
        emit_pattern(w, expr)?;
    }
    Ok(())
}

fn emit_alias_pattern(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (alias original local) → original: local
    // (alias original local default) → original: local = default
    if args.len() >= 2 {
        emit_expr(w, &args[0], 0)?;
        w.write(": ");
        emit_pattern(w, &args[1])?;
        if args.len() >= 3 {
            w.write(" = ");
            emit_expr(w, &args[2], 0)?;
        }
    }
    Ok(())
}

// ── Classes ────────────────────────────────────────────────────────────

fn emit_class(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (class Name (Super) members...) or (class Name () members...)
    if args.len() < 2 {
        return Ok(());
    }
    w.write("class ");
    emit_expr(w, &args[0], 0)?;

    // Superclass list.
    if let SExpr::List { values, .. } = &args[1]
        && !values.is_empty()
    {
        w.write(" extends ");
        emit_expr(w, &values[0], 0)?;
    }

    w.write(" ");
    emit_class_body(w, &args[2..])?;
    w.newline();
    Ok(())
}

fn emit_class_expr(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (class-expr (Super) members...) — anonymous
    if args.is_empty() {
        w.write("class {}");
        return Ok(());
    }
    w.write("class");

    if let SExpr::List { values, .. } = &args[0]
        && !values.is_empty()
    {
        w.write(" extends ");
        emit_expr(w, &values[0], 0)?;
    }

    w.write(" ");
    emit_class_body(w, &args[1..])?;
    Ok(())
}

fn emit_class_body(w: &mut JsWriter, members: &[SExpr]) -> Result<(), LyknError> {
    w.write("{");
    w.newline();
    w.indent();

    for member in members {
        emit_class_member(w, member, "")?;
    }

    w.dedent();
    w.write("}");
    Ok(())
}

fn emit_class_member(w: &mut JsWriter, member: &SExpr, prefix: &str) -> Result<(), LyknError> {
    let values = match member {
        SExpr::List { values, .. } if !values.is_empty() => values,
        _ => return Ok(()),
    };

    let head = match values[0].as_atom() {
        Some(h) => h,
        None => return Ok(()),
    };

    match head {
        "static" => {
            // (static (...member...))
            if values.len() >= 2 {
                let new_prefix = if prefix.is_empty() {
                    "static ".to_string()
                } else {
                    format!("{prefix}static ")
                };
                emit_class_member(w, &values[1], &new_prefix)?;
            }
        }
        "async" => {
            if values.len() >= 2 {
                let new_prefix = if prefix.is_empty() {
                    "async ".to_string()
                } else {
                    format!("{prefix}async ")
                };
                emit_class_member(w, &values[1], &new_prefix)?;
            }
        }
        "get" => {
            // (get name () body...)
            if values.len() >= 3 {
                w.write(prefix);
                w.write("get ");
                emit_expr(w, &values[1], 0)?;
                emit_params(w, &values[2])?;
                w.write(" ");
                emit_block_body(w, &values[3..])?;
                w.newline();
            }
        }
        "set" => {
            // (set name (param) body...)
            if values.len() >= 3 {
                w.write(prefix);
                w.write("set ");
                emit_expr(w, &values[1], 0)?;
                emit_params(w, &values[2])?;
                w.write(" ");
                emit_block_body(w, &values[3..])?;
                w.newline();
            }
        }
        "field" => {
            // (field name) or (field name value)
            w.write(prefix);
            if values.len() >= 2 {
                emit_field_name(w, &values[1])?;
                if values.len() >= 3 {
                    w.write(" = ");
                    emit_expr(w, &values[2], 0)?;
                }
            }
            w.semicolon();
        }
        "constructor" => {
            // (constructor (params) body...)
            w.write(prefix);
            w.write("constructor");
            if values.len() >= 2 {
                emit_params(w, &values[1])?;
            } else {
                w.write("()");
            }
            w.write(" ");
            emit_block_body(w, &values[2..])?;
            w.newline();
        }
        _ => {
            // Regular method: (method-name (params) body...)
            w.write(prefix);
            emit_field_name(w, &values[0])?;
            if values.len() >= 2 {
                emit_params(w, &values[1])?;
            } else {
                w.write("()");
            }
            w.write(" ");
            if values.len() >= 3 {
                emit_block_body(w, &values[2..])?;
            } else {
                w.write("{}");
            }
            w.newline();
        }
    }
    Ok(())
}

/// Emit a field/method name, handling private fields (leading hyphen → `#_`).
fn emit_field_name(w: &mut JsWriter, expr: &SExpr) -> Result<(), LyknError> {
    if let SExpr::Atom { value, .. } = expr {
        if let Some(rest) = value.strip_prefix('-') {
            w.write("#_");
            w.write(&to_js_identifier(rest));
            return Ok(());
        }
        w.write(&to_js_identifier(value));
    } else {
        emit_expr(w, expr, 0)?;
    }
    Ok(())
}

// ── Modules ────────────────────────────────────────────────────────────

fn emit_import(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    // (import "mod")
    // (import "mod" name)
    // (import "mod" (a b))
    // (import "mod" name (a b))
    if args.is_empty() {
        return Ok(());
    }

    w.write("import ");

    let module_path = &args[0];

    match args.len() {
        1 => {
            // Side-effect import.
            emit_expr(w, module_path, 0)?;
        }
        2 => {
            // Default import or named imports.
            match &args[1] {
                SExpr::List { values, .. } => {
                    // Named imports.
                    emit_import_specifiers(w, values)?;
                    w.write(" from ");
                    emit_expr(w, module_path, 0)?;
                }
                _ => {
                    // Default import.
                    emit_expr(w, &args[1], 0)?;
                    w.write(" from ");
                    emit_expr(w, module_path, 0)?;
                }
            }
        }
        _ => {
            // Default + named: (import "mod" name (a b))
            emit_expr(w, &args[1], 0)?;
            if let Some(SExpr::List { values, .. }) = args.get(2) {
                w.write(", ");
                emit_import_specifiers(w, values)?;
            }
            w.write(" from ");
            emit_expr(w, module_path, 0)?;
        }
    }

    w.semicolon();
    Ok(())
}

fn emit_import_specifiers(w: &mut JsWriter, specs: &[SExpr]) -> Result<(), LyknError> {
    w.write("{");
    for (i, spec) in specs.iter().enumerate() {
        if i > 0 {
            w.write(", ");
        }
        match spec {
            SExpr::List { values, .. }
                if values.first().and_then(|e| e.as_atom()) == Some("alias") =>
            {
                // (alias original local) → original as local
                if values.len() >= 3 {
                    emit_expr(w, &values[1], 0)?;
                    w.write(" as ");
                    emit_expr(w, &values[2], 0)?;
                }
            }
            _ => emit_expr(w, spec, 0)?,
        }
    }
    w.write("}");
    Ok(())
}

fn emit_export(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    if args.is_empty() {
        return Ok(());
    }

    w.write("export ");

    // (export default expr)
    if args[0].as_atom() == Some("default") {
        w.write("default ");
        if args.len() >= 2 {
            emit_expr(w, &args[1], 0)?;
        }
        w.semicolon();
        return Ok(());
    }

    // (export (const x 1)) — export a declaration
    if let SExpr::List { values, .. } = &args[0]
        && let Some(head) = values.first().and_then(|e| e.as_atom())
    {
        if head == "names" {
            // (export (names a b))
            emit_export_names(w, &values[1..])?;
            w.semicolon();
            return Ok(());
        }
        // Export a declaration.
        emit_list(w, values, 0)?;
        return Ok(());
    }

    // (export "mod" (names a b))
    if let SExpr::String { .. } = &args[0]
        && args.len() >= 2
        && let SExpr::List { values, .. } = &args[1]
        && values.first().and_then(|e| e.as_atom()) == Some("names")
    {
        emit_export_names(w, &values[1..])?;
        w.write(" from ");
        emit_expr(w, &args[0], 0)?;
        w.semicolon();
        return Ok(());
    }

    // (export name) → export { name };
    // Bare atom export — wrap in named export braces
    if let SExpr::Atom { .. } = &args[0] {
        w.write("{ ");
        emit_expr(w, &args[0], 0)?;
        w.write(" }");
        w.semicolon();
        return Ok(());
    }

    emit_expr(w, &args[0], 0)?;
    w.semicolon();
    Ok(())
}

fn emit_export_names(w: &mut JsWriter, specs: &[SExpr]) -> Result<(), LyknError> {
    w.write("{");
    for (i, spec) in specs.iter().enumerate() {
        if i > 0 {
            w.write(", ");
        }
        match spec {
            SExpr::List { values, .. }
                if values.first().and_then(|e| e.as_atom()) == Some("alias") =>
            {
                // (alias local exported) → local as exported
                if values.len() >= 3 {
                    emit_expr(w, &values[1], 0)?;
                    w.write(" as ");
                    emit_expr(w, &values[2], 0)?;
                }
            }
            _ => emit_expr(w, spec, 0)?,
        }
    }
    w.write("}");
    Ok(())
}

fn emit_dynamic_import(w: &mut JsWriter, args: &[SExpr]) -> Result<(), LyknError> {
    w.write("import(");
    if let Some(expr) = args.first() {
        emit_expr(w, expr, 0)?;
    }
    w.write(")");
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::source_loc::Span;

    fn s() -> Span {
        Span::default()
    }

    fn atom(v: &str) -> SExpr {
        SExpr::Atom {
            value: v.to_string(),
            span: s(),
        }
    }

    fn num(v: f64) -> SExpr {
        SExpr::Number {
            value: v,
            span: s(),
        }
    }

    fn str_lit(v: &str) -> SExpr {
        SExpr::String {
            value: v.to_string(),
            span: s(),
        }
    }

    fn bool_lit(v: bool) -> SExpr {
        SExpr::Bool {
            value: v,
            span: s(),
        }
    }

    fn null_lit() -> SExpr {
        SExpr::Null { span: s() }
    }

    fn list(items: Vec<SExpr>) -> SExpr {
        SExpr::List {
            values: items,
            span: s(),
        }
    }

    fn keyword(v: &str) -> SExpr {
        SExpr::Keyword {
            value: v.to_string(),
            span: s(),
        }
    }

    fn emit_to_string(expr: &SExpr) -> String {
        let mut w = JsWriter::new();
        emit_expr(&mut w, expr, 0).unwrap();
        w.finish()
    }

    fn stmt_to_string(expr: &SExpr) -> String {
        let mut w = JsWriter::new();
        emit_statement(&mut w, expr).unwrap();
        w.finish()
    }

    // ── Leaf nodes ─────────────────────────────────────────────────

    #[test]
    fn test_emit_number_integer() {
        assert_eq!(emit_to_string(&num(42.0)), "42");
    }

    #[test]
    fn test_emit_number_float() {
        assert_eq!(emit_to_string(&num(3.14)), "3.14");
    }

    #[test]
    fn test_emit_string() {
        assert_eq!(emit_to_string(&str_lit("hello")), "\"hello\"");
    }

    #[test]
    fn test_emit_string_with_escapes() {
        assert_eq!(
            emit_to_string(&str_lit("a\"b\\c\nd")),
            "\"a\\\"b\\\\c\\nd\""
        );
    }

    #[test]
    fn test_emit_bool_true() {
        assert_eq!(emit_to_string(&bool_lit(true)), "true");
    }

    #[test]
    fn test_emit_bool_false() {
        assert_eq!(emit_to_string(&bool_lit(false)), "false");
    }

    #[test]
    fn test_emit_null() {
        assert_eq!(emit_to_string(&null_lit()), "null");
    }

    #[test]
    fn test_emit_atom_ident() {
        assert_eq!(emit_to_string(&atom("my-var")), "myVar");
    }

    #[test]
    fn test_emit_keyword() {
        assert_eq!(emit_to_string(&keyword("my-key")), "\"myKey\"");
    }

    // ── Declarations ───────────────────────────────────────────────

    #[test]
    fn test_const_simple() {
        let expr = list(vec![atom("const"), atom("x"), num(1.0)]);
        assert_eq!(stmt_to_string(&expr), "const x = 1;\n");
    }

    #[test]
    fn test_let_no_init() {
        let expr = list(vec![atom("let"), atom("x")]);
        assert_eq!(stmt_to_string(&expr), "let x;\n");
    }

    #[test]
    fn test_const_destructure() {
        // (const (object name age) (get-user))
        let pattern = list(vec![atom("object"), atom("name"), atom("age")]);
        let init = list(vec![atom("get-user")]);
        let expr = list(vec![atom("const"), pattern, init]);
        assert_eq!(stmt_to_string(&expr), "const {name, age} = getUser();\n");
    }

    // ── Functions ──────────────────────────────────────────────────

    #[test]
    fn test_arrow_single_expr() {
        // (=> (a b) (+ a b))
        let params = list(vec![atom("a"), atom("b")]);
        let body = list(vec![atom("+"), atom("a"), atom("b")]);
        let expr = list(vec![atom("=>"), params, body]);
        assert_eq!(emit_to_string(&expr), "(a, b) => a + b");
    }

    #[test]
    fn test_arrow_multi_body() {
        // (=> (a b) (console:log a) (return (+ a b)))
        let params = list(vec![atom("a"), atom("b")]);
        let log = list(vec![atom("console:log"), atom("a")]);
        let ret = list(vec![
            atom("return"),
            list(vec![atom("+"), atom("a"), atom("b")]),
        ]);
        let expr = list(vec![atom("=>"), params, log, ret]);
        assert_eq!(
            emit_to_string(&expr),
            "(a, b) => {\n  console.log(a);\n  return a + b;\n}"
        );
    }

    #[test]
    fn test_arrow_empty_params() {
        let expr = list(vec![atom("=>"), list(vec![]), num(42.0)]);
        assert_eq!(emit_to_string(&expr), "() => 42");
    }

    #[test]
    fn test_function_declaration() {
        // (function add (a b) (return (+ a b)))
        let ret = list(vec![
            atom("return"),
            list(vec![atom("+"), atom("a"), atom("b")]),
        ]);
        let expr = list(vec![
            atom("function"),
            atom("add"),
            list(vec![atom("a"), atom("b")]),
            ret,
        ]);
        assert_eq!(
            stmt_to_string(&expr),
            "function add(a, b) {\n  return a + b;\n}\n"
        );
    }

    #[test]
    fn test_lambda() {
        let expr = list(vec![
            atom("lambda"),
            list(vec![atom("a")]),
            list(vec![atom("return"), atom("a")]),
        ]);
        assert_eq!(emit_to_string(&expr), "function(a) {\n  return a;\n}");
    }

    #[test]
    fn test_async_arrow() {
        // (async (=> () (await (fetch url))))
        let fetch_call = list(vec![atom("fetch"), atom("url")]);
        let aw = list(vec![atom("await"), fetch_call]);
        let arrow = list(vec![atom("=>"), list(vec![]), aw]);
        let expr = list(vec![atom("async"), arrow]);
        assert_eq!(emit_to_string(&expr), "async () => await fetch(url)");
    }

    #[test]
    fn test_return_empty() {
        let expr = list(vec![atom("return")]);
        assert_eq!(stmt_to_string(&expr), "return;\n");
    }

    #[test]
    fn test_return_value() {
        let expr = list(vec![atom("return"), num(5.0)]);
        assert_eq!(stmt_to_string(&expr), "return 5;\n");
    }

    // ── Control flow ───────────────────────────────────────────────

    #[test]
    fn test_if_simple() {
        // (if x (f))
        let expr = list(vec![atom("if"), atom("x"), list(vec![atom("f")])]);
        assert_eq!(stmt_to_string(&expr), "if (x) f();\n\n");
    }

    #[test]
    fn test_if_else() {
        // (if x (f) (g))
        let expr = list(vec![
            atom("if"),
            atom("x"),
            list(vec![atom("f")]),
            list(vec![atom("g")]),
        ]);
        assert_eq!(stmt_to_string(&expr), "if (x) f();\n else g();\n\n");
    }

    #[test]
    fn test_if_block() {
        // (if x (block (f) (g)))
        let blk = list(vec![
            atom("block"),
            list(vec![atom("f")]),
            list(vec![atom("g")]),
        ]);
        let expr = list(vec![atom("if"), atom("x"), blk]);
        assert_eq!(stmt_to_string(&expr), "if (x) {\n  f();\n  g();\n}\n");
    }

    #[test]
    fn test_while_loop() {
        let expr = list(vec![atom("while"), atom("x"), list(vec![atom("f")])]);
        assert_eq!(stmt_to_string(&expr), "while (x) {\n  f();\n}\n");
    }

    #[test]
    fn test_do_while() {
        let expr = list(vec![atom("do-while"), atom("cond"), list(vec![atom("f")])]);
        assert_eq!(stmt_to_string(&expr), "do {\n  f();\n} while (cond);\n");
    }

    #[test]
    fn test_for_loop() {
        // (for (let i 0) (< i 10) (++ i) (f i))
        let init = list(vec![atom("let"), atom("i"), num(0.0)]);
        let test = list(vec![atom("<"), atom("i"), num(10.0)]);
        let update = list(vec![atom("++"), atom("i")]);
        let body = list(vec![atom("f"), atom("i")]);
        let expr = list(vec![atom("for"), init, test, update, body]);
        assert_eq!(
            stmt_to_string(&expr),
            "for (let i = 0; i < 10; ++i) {\n  f(i);\n}\n"
        );
    }

    #[test]
    fn test_for_of() {
        let expr = list(vec![
            atom("for-of"),
            atom("x"),
            atom("items"),
            list(vec![atom("f"), atom("x")]),
        ]);
        assert_eq!(
            stmt_to_string(&expr),
            "for (const x of items) {\n  f(x);\n}\n"
        );
    }

    #[test]
    fn test_for_in() {
        let expr = list(vec![
            atom("for-in"),
            atom("k"),
            atom("obj"),
            list(vec![atom("f"), atom("k")]),
        ]);
        assert_eq!(
            stmt_to_string(&expr),
            "for (const k in obj) {\n  f(k);\n}\n"
        );
    }

    #[test]
    fn test_break_simple() {
        let expr = list(vec![atom("break")]);
        assert_eq!(stmt_to_string(&expr), "break;\n");
    }

    #[test]
    fn test_break_label() {
        let expr = list(vec![atom("break"), atom("outer")]);
        assert_eq!(stmt_to_string(&expr), "break outer;\n");
    }

    #[test]
    fn test_continue_simple() {
        let expr = list(vec![atom("continue")]);
        assert_eq!(stmt_to_string(&expr), "continue;\n");
    }

    #[test]
    fn test_throw() {
        let expr = list(vec![
            atom("throw"),
            list(vec![atom("new"), atom("Error"), str_lit("oops")]),
        ]);
        assert_eq!(stmt_to_string(&expr), "throw new Error(\"oops\");\n");
    }

    #[test]
    fn test_try_catch_finally() {
        // (try (f) (catch e (g e)) (finally (cleanup)))
        let catch_clause = list(vec![
            atom("catch"),
            atom("e"),
            list(vec![atom("g"), atom("e")]),
        ]);
        let finally_clause = list(vec![atom("finally"), list(vec![atom("cleanup")])]);
        let expr = list(vec![
            atom("try"),
            list(vec![atom("f")]),
            catch_clause,
            finally_clause,
        ]);
        assert_eq!(
            stmt_to_string(&expr),
            "try {\n  f();\n} catch (e) {\n  g(e);\n} finally {\n  cleanup();\n}\n"
        );
    }

    // ── Expressions ────────────────────────────────────────────────

    #[test]
    fn test_ternary() {
        let expr = list(vec![atom("?"), atom("x"), num(1.0), num(2.0)]);
        assert_eq!(emit_to_string(&expr), "x ? 1 : 2");
    }

    #[test]
    fn test_assignment() {
        let expr = list(vec![atom("="), atom("x"), num(5.0)]);
        assert_eq!(emit_to_string(&expr), "x = 5");
    }

    #[test]
    fn test_new() {
        let expr = list(vec![atom("new"), atom("Thing"), atom("a"), atom("b")]);
        assert_eq!(emit_to_string(&expr), "new Thing(a, b)");
    }

    #[test]
    fn test_computed_member() {
        let expr = list(vec![atom("get"), atom("obj"), str_lit("key")]);
        assert_eq!(emit_to_string(&expr), "obj[\"key\"]");
    }

    #[test]
    fn test_method_call() {
        let expr = list(vec![
            atom("."),
            atom("obj"),
            atom("my-method"),
            atom("a"),
            atom("b"),
        ]);
        assert_eq!(emit_to_string(&expr), "obj.myMethod(a, b)");
    }

    #[test]
    fn test_seq() {
        let expr = list(vec![atom("seq"), atom("a"), atom("b"), atom("c")]);
        assert_eq!(emit_to_string(&expr), "a, b, c");
    }

    #[test]
    fn test_increment() {
        let expr = list(vec![atom("++"), atom("x")]);
        assert_eq!(emit_to_string(&expr), "++x");
    }

    // ── Binary operators ───────────────────────────────────────────

    #[test]
    fn test_binary_add() {
        let expr = list(vec![atom("+"), atom("a"), atom("b")]);
        assert_eq!(emit_to_string(&expr), "a + b");
    }

    #[test]
    fn test_binary_nary() {
        let expr = list(vec![atom("+"), atom("a"), atom("b"), atom("c")]);
        assert_eq!(emit_to_string(&expr), "a + b + c");
    }

    #[test]
    fn test_binary_precedence_parens() {
        // (+ a (* b c)) — multiplication inside addition, no parens needed
        let mul = list(vec![atom("*"), atom("b"), atom("c")]);
        let expr = list(vec![atom("+"), atom("a"), mul]);
        assert_eq!(emit_to_string(&expr), "a + b * c");
    }

    #[test]
    fn test_binary_precedence_needs_parens() {
        // (* a (+ b c)) — addition inside multiplication needs parens
        let add = list(vec![atom("+"), atom("b"), atom("c")]);
        let expr = list(vec![atom("*"), atom("a"), add]);
        assert_eq!(emit_to_string(&expr), "a * (b + c)");
    }

    #[test]
    fn test_logical_and() {
        let expr = list(vec![atom("&&"), atom("a"), atom("b")]);
        assert_eq!(emit_to_string(&expr), "a && b");
    }

    #[test]
    fn test_instanceof() {
        let expr = list(vec![atom("instanceof"), atom("x"), atom("Array")]);
        assert_eq!(emit_to_string(&expr), "x instanceof Array");
    }

    // ── Unary operators ────────────────────────────────────────────

    #[test]
    fn test_not() {
        let expr = list(vec![atom("!"), atom("x")]);
        assert_eq!(emit_to_string(&expr), "!x");
    }

    #[test]
    fn test_typeof() {
        let expr = list(vec![atom("typeof"), atom("x")]);
        assert_eq!(emit_to_string(&expr), "typeof x");
    }

    #[test]
    fn test_void() {
        let expr = list(vec![atom("void"), num(0.0)]);
        assert_eq!(emit_to_string(&expr), "void 0");
    }

    #[test]
    fn test_bitwise_not() {
        let expr = list(vec![atom("~"), atom("x")]);
        assert_eq!(emit_to_string(&expr), "~x");
    }

    // ── Compound assignment ────────────────────────────────────────

    #[test]
    fn test_plus_assign() {
        let expr = list(vec![atom("+="), atom("x"), num(1.0)]);
        assert_eq!(emit_to_string(&expr), "x += 1");
    }

    // ── Object / Array ─────────────────────────────────────────────

    #[test]
    fn test_object_mixed() {
        // (object (name "x") age (spread rest))
        let name_pair = list(vec![atom("name"), str_lit("x")]);
        let spread = list(vec![atom("spread"), atom("rest")]);
        let expr = list(vec![atom("object"), name_pair, atom("age"), spread]);
        assert_eq!(emit_to_string(&expr), "{name: \"x\", age, ...rest}");
    }

    #[test]
    fn test_array() {
        let spread = list(vec![atom("spread"), atom("rest")]);
        let expr = list(vec![atom("array"), num(1.0), num(2.0), spread]);
        assert_eq!(emit_to_string(&expr), "[1, 2, ...rest]");
    }

    // ── Templates ──────────────────────────────────────────────────

    #[test]
    fn test_template() {
        let expr = list(vec![
            atom("template"),
            str_lit("Hello, "),
            atom("name"),
            str_lit("!"),
        ]);
        assert_eq!(emit_to_string(&expr), "`Hello, ${name}!`");
    }

    #[test]
    fn test_regex_with_flags() {
        let expr = list(vec![atom("regex"), str_lit("^hello"), str_lit("gi")]);
        assert_eq!(emit_to_string(&expr), "/^hello/gi");
    }

    #[test]
    fn test_regex_no_flags() {
        let expr = list(vec![atom("regex"), str_lit("^hello")]);
        assert_eq!(emit_to_string(&expr), "/^hello/");
    }

    // ── Classes ────────────────────────────────────────────────────

    #[test]
    fn test_class_with_extends() {
        // (class Dog (Animal) (constructor (name) (= this:name name)))
        let ctor = list(vec![
            atom("constructor"),
            list(vec![atom("name")]),
            list(vec![atom("="), atom("this:name"), atom("name")]),
        ]);
        let expr = list(vec![
            atom("class"),
            atom("Dog"),
            list(vec![atom("Animal")]),
            ctor,
        ]);
        assert_eq!(
            stmt_to_string(&expr),
            "class Dog extends Animal {\n  constructor(name) {\n    this.name = name;\n  }\n}\n"
        );
    }

    #[test]
    fn test_class_no_extends() {
        let field = list(vec![atom("field"), atom("x"), num(0.0)]);
        let expr = list(vec![atom("class"), atom("Foo"), list(vec![]), field]);
        assert_eq!(stmt_to_string(&expr), "class Foo {\n  x = 0;\n}\n");
    }

    // ── Modules ────────────────────────────────────────────────────

    #[test]
    fn test_import_side_effect() {
        let expr = list(vec![atom("import"), str_lit("mod")]);
        assert_eq!(stmt_to_string(&expr), "import \"mod\";\n");
    }

    #[test]
    fn test_import_default() {
        let expr = list(vec![atom("import"), str_lit("mod"), atom("name")]);
        assert_eq!(stmt_to_string(&expr), "import name from \"mod\";\n");
    }

    #[test]
    fn test_import_named() {
        let expr = list(vec![
            atom("import"),
            str_lit("mod"),
            list(vec![atom("a"), atom("b")]),
        ]);
        assert_eq!(stmt_to_string(&expr), "import {a, b} from \"mod\";\n");
    }

    #[test]
    fn test_import_default_and_named() {
        let expr = list(vec![
            atom("import"),
            str_lit("mod"),
            atom("name"),
            list(vec![atom("a"), atom("b")]),
        ]);
        assert_eq!(stmt_to_string(&expr), "import name, {a, b} from \"mod\";\n");
    }

    #[test]
    fn test_export_default() {
        let expr = list(vec![atom("export"), atom("default"), atom("x")]);
        assert_eq!(stmt_to_string(&expr), "export default x;\n");
    }

    #[test]
    fn test_export_bare_name() {
        let expr = list(vec![atom("export"), atom("add")]);
        assert_eq!(stmt_to_string(&expr), "export { add };\n");
    }

    #[test]
    fn test_export_declaration() {
        let decl = list(vec![atom("const"), atom("x"), num(1.0)]);
        let expr = list(vec![atom("export"), decl]);
        assert_eq!(stmt_to_string(&expr), "export const x = 1;\n");
    }

    #[test]
    fn test_export_names() {
        let names = list(vec![atom("names"), atom("a"), atom("b")]);
        let expr = list(vec![atom("export"), names]);
        assert_eq!(stmt_to_string(&expr), "export {a, b};\n");
    }

    #[test]
    fn test_export_re_export() {
        let names = list(vec![atom("names"), atom("a"), atom("b")]);
        let expr = list(vec![atom("export"), str_lit("mod"), names]);
        assert_eq!(stmt_to_string(&expr), "export {a, b} from \"mod\";\n");
    }

    #[test]
    fn test_dynamic_import() {
        let expr = list(vec![atom("dynamic-import"), str_lit("./mod.js")]);
        assert_eq!(emit_to_string(&expr), "import(\"./mod.js\")");
    }

    // ── Misc ───────────────────────────────────────────────────────

    #[test]
    fn test_debugger() {
        let expr = list(vec![atom("debugger")]);
        assert_eq!(stmt_to_string(&expr), "debugger;\n");
    }

    // ── Default: function call ─────────────────────────────────────

    #[test]
    fn test_function_call() {
        let expr = list(vec![atom("foo"), atom("a"), atom("b")]);
        assert_eq!(emit_to_string(&expr), "foo(a, b)");
    }

    #[test]
    fn test_console_log() {
        let expr = list(vec![atom("console:log"), str_lit("hi")]);
        assert_eq!(emit_to_string(&expr), "console.log(\"hi\")");
    }

    // ── Switch ─────────────────────────────────────────────────────

    #[test]
    fn test_switch() {
        // (switch disc ("a" (f) (break)) (default (g)))
        let case_a = list(vec![
            str_lit("a"),
            list(vec![atom("f")]),
            list(vec![atom("break")]),
        ]);
        let default_case = list(vec![atom("default"), list(vec![atom("g")])]);
        let expr = list(vec![atom("switch"), atom("disc"), case_a, default_case]);
        let result = stmt_to_string(&expr);
        assert!(result.contains("switch (disc)"));
        assert!(result.contains("case \"a\":"));
        assert!(result.contains("default:"));
    }

    // ── Patterns ───────────────────────────────────────────────────

    #[test]
    fn test_array_pattern_with_holes() {
        // (const (array _ _ third) arr)
        let pattern = list(vec![atom("array"), atom("_"), atom("_"), atom("third")]);
        let expr = list(vec![atom("const"), pattern, atom("arr")]);
        assert_eq!(stmt_to_string(&expr), "const [, , third] = arr;\n");
    }

    #[test]
    fn test_object_pattern_with_default() {
        // (const (object name (default age 0)) obj)
        let default_age = list(vec![atom("default"), atom("age"), num(0.0)]);
        let pattern = list(vec![atom("object"), atom("name"), default_age]);
        let expr = list(vec![atom("const"), pattern, atom("obj")]);
        assert_eq!(stmt_to_string(&expr), "const {name, age = 0} = obj;\n");
    }

    #[test]
    fn test_object_pattern_with_alias() {
        // (const (object (alias data items)) obj)
        let alias = list(vec![atom("alias"), atom("data"), atom("items")]);
        let pattern = list(vec![atom("object"), alias]);
        let expr = list(vec![atom("const"), pattern, atom("obj")]);
        assert_eq!(stmt_to_string(&expr), "const {data: items} = obj;\n");
    }

    #[test]
    fn test_rest_pattern_in_array() {
        // (const (array first (rest tail)) arr)
        let rest = list(vec![atom("rest"), atom("tail")]);
        let pattern = list(vec![atom("array"), atom("first"), rest]);
        let expr = list(vec![atom("const"), pattern, atom("arr")]);
        assert_eq!(stmt_to_string(&expr), "const [first, ...tail] = arr;\n");
    }

    // ── Label ──────────────────────────────────────────────────────

    #[test]
    fn test_label() {
        let expr = list(vec![
            atom("label"),
            atom("outer"),
            list(vec![
                atom("while"),
                atom("true"),
                list(vec![atom("break"), atom("outer")]),
            ]),
        ]);
        let result = stmt_to_string(&expr);
        assert!(result.starts_with("outer: while"));
    }

    // ── Unary minus ────────────────────────────────────────────────

    #[test]
    fn test_unary_minus() {
        let expr = list(vec![atom("-"), atom("x")]);
        assert_eq!(emit_to_string(&expr), "-x");
    }

    // ── Delete ─────────────────────────────────────────────────────

    #[test]
    fn test_delete() {
        let expr = list(vec![atom("delete"), atom("x")]);
        assert_eq!(emit_to_string(&expr), "delete x");
    }

    // ── Tagged template ────────────────────────────────────────────

    #[test]
    fn test_tagged_template() {
        let tmpl = list(vec![atom("template"), str_lit("hello "), atom("x")]);
        let expr = list(vec![atom("tag"), atom("html"), tmpl]);
        assert_eq!(emit_to_string(&expr), "html`hello ${x}`");
    }

    // ── Spread (standalone) ────────────────────────────────────────

    #[test]
    fn test_spread() {
        let expr = list(vec![atom("spread"), atom("args")]);
        assert_eq!(emit_to_string(&expr), "...args");
    }

    // ── Computed property in object ────────────────────────────────

    #[test]
    fn test_object_computed_key() {
        // (object ((sym) "value"))
        let computed = list(vec![list(vec![atom("sym")]), str_lit("value")]);
        let expr = list(vec![atom("object"), computed]);
        assert_eq!(emit_to_string(&expr), "{[sym()]: \"value\"}");
    }

    // ── Class expressions ──────────────────────────────────────────

    #[test]
    fn test_class_expr_anonymous() {
        let method = list(vec![
            atom("foo"),
            list(vec![]),
            list(vec![atom("return"), num(1.0)]),
        ]);
        let expr = list(vec![atom("class-expr"), list(vec![]), method]);
        assert_eq!(
            emit_to_string(&expr),
            "class {\n  foo() {\n    return 1;\n  }\n}"
        );
    }

    // ── Private field ──────────────────────────────────────────────

    #[test]
    fn test_private_field() {
        let field = list(vec![atom("field"), atom("-name"), str_lit("default")]);
        let expr = list(vec![atom("class"), atom("Foo"), list(vec![]), field]);
        assert_eq!(
            stmt_to_string(&expr),
            "class Foo {\n  #_name = \"default\";\n}\n"
        );
    }

    // ── Static member ──────────────────────────────────────────────

    #[test]
    fn test_static_field() {
        let inner = list(vec![atom("field"), atom("count"), num(0.0)]);
        let member = list(vec![atom("static"), inner]);
        let expr = list(vec![atom("class"), atom("Foo"), list(vec![]), member]);
        assert_eq!(
            stmt_to_string(&expr),
            "class Foo {\n  static count = 0;\n}\n"
        );
    }

    // ── Getter / setter ────────────────────────────────────────────

    #[test]
    fn test_getter() {
        let getter = list(vec![
            atom("get"),
            atom("name"),
            list(vec![]),
            list(vec![atom("return"), atom("this:-name")]),
        ]);
        let expr = list(vec![atom("class"), atom("Foo"), list(vec![]), getter]);
        assert_eq!(
            stmt_to_string(&expr),
            "class Foo {\n  get name() {\n    return this.#_name;\n  }\n}\n"
        );
    }

    #[test]
    fn test_setter() {
        let setter = list(vec![
            atom("set"),
            atom("name"),
            list(vec![atom("v")]),
            list(vec![atom("="), atom("this:-name"), atom("v")]),
        ]);
        let expr = list(vec![atom("class"), atom("Foo"), list(vec![]), setter]);
        assert_eq!(
            stmt_to_string(&expr),
            "class Foo {\n  set name(v) {\n    this.#_name = v;\n  }\n}\n"
        );
    }

    // ── Import alias ───────────────────────────────────────────────

    #[test]
    fn test_import_alias() {
        let alias = list(vec![atom("alias"), atom("original"), atom("local")]);
        let named = list(vec![alias]);
        let expr = list(vec![atom("import"), str_lit("mod"), named]);
        assert_eq!(
            stmt_to_string(&expr),
            "import {original as local} from \"mod\";\n"
        );
    }

    // ── Export alias ───────────────────────────────────────────────

    #[test]
    fn test_export_names_with_alias() {
        let alias = list(vec![atom("alias"), atom("local"), atom("exported")]);
        let names = list(vec![atom("names"), atom("a"), alias]);
        let expr = list(vec![atom("export"), names]);
        assert_eq!(stmt_to_string(&expr), "export {a, local as exported};\n");
    }

    // ── Object pattern with rest ───────────────────────────────────

    #[test]
    fn test_object_pattern_with_rest() {
        let rest = list(vec![atom("rest"), atom("other")]);
        let pattern = list(vec![atom("object"), atom("name"), rest]);
        let expr = list(vec![atom("const"), pattern, atom("obj")]);
        assert_eq!(stmt_to_string(&expr), "const {name, ...other} = obj;\n");
    }

    // ── Alias with default in object pattern ───────────────────────

    #[test]
    fn test_alias_with_default() {
        // (const (object (alias data items 0)) obj)
        let alias = list(vec![atom("alias"), atom("data"), atom("items"), num(0.0)]);
        let pattern = list(vec![atom("object"), alias]);
        let expr = list(vec![atom("const"), pattern, atom("obj")]);
        assert_eq!(stmt_to_string(&expr), "const {data: items = 0} = obj;\n");
    }

    // ── Generator forms ───────────────────────────────────────────────

    #[test]
    fn test_function_star() {
        let expr = list(vec![
            atom("function*"),
            atom("gen"),
            list(vec![]),
            list(vec![atom("yield"), num(1.0)]),
        ]);
        assert_eq!(stmt_to_string(&expr), "function* gen() {\n  yield 1;\n}\n");
    }

    #[test]
    fn test_function_star_no_args() {
        let expr = list(vec![atom("function*")]);
        assert_eq!(stmt_to_string(&expr), "function*() {}\n");
    }

    #[test]
    fn test_yield() {
        let expr = list(vec![atom("yield"), num(42.0)]);
        assert_eq!(emit_to_string(&expr), "yield 42");
    }

    #[test]
    fn test_yield_no_arg() {
        let expr = list(vec![atom("yield")]);
        assert_eq!(emit_to_string(&expr), "yield");
    }

    #[test]
    fn test_yield_star() {
        let expr = list(vec![atom("yield*"), atom("other")]);
        assert_eq!(emit_to_string(&expr), "yield* other");
    }

    #[test]
    fn test_for_await_of() {
        let body = list(vec![atom("console:log"), atom("item")]);
        let expr = list(vec![
            atom("for-await-of"),
            atom("item"),
            atom("stream"),
            body,
        ]);
        let result = stmt_to_string(&expr);
        assert!(result.contains("for await (const item of stream)"));
    }

    #[test]
    fn test_async_generator() {
        let inner = list(vec![
            atom("function*"),
            atom("gen"),
            list(vec![]),
            list(vec![atom("yield"), num(1.0)]),
        ]);
        let expr = list(vec![atom("async"), inner]);
        let result = stmt_to_string(&expr);
        assert!(result.contains("async function* gen()"));
    }
}
