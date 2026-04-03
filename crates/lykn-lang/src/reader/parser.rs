use crate::ast::sexpr::SExpr;
use crate::error::LyknError;
use crate::reader::lexer::{SpannedToken, Token};
use crate::reader::source_loc::{SourceLoc, Span};

pub fn parse(tokens: &[SpannedToken]) -> Result<Vec<SExpr>, LyknError> {
    let mut parser = Parser::new(tokens);
    let mut forms = Vec::new();
    while !parser.is_eof() {
        forms.push(parser.parse_expr()?);
    }
    Ok(forms)
}

struct Parser<'a> {
    tokens: &'a [SpannedToken],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [SpannedToken]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&SpannedToken> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&SpannedToken> {
        let t = self.tokens.get(self.pos)?;
        self.pos += 1;
        Some(t)
    }

    fn parse_expr(&mut self) -> Result<SExpr, LyknError> {
        let st = self.advance().ok_or_else(|| LyknError::Read {
            message: "unexpected end of input".to_string(),
            location: SourceLoc::default(),
        })?;
        let token = st.token.clone();
        let span = st.span;

        match token {
            Token::LParen => self.parse_list(span.start),
            Token::Atom(s) => Ok(SExpr::Atom { value: s, span }),
            Token::Keyword(s) => Ok(SExpr::Keyword { value: s, span }),
            Token::String(s) => Ok(SExpr::String { value: s, span }),
            Token::Number(n) => Ok(SExpr::Number { value: n, span }),
            Token::Bool(b) => Ok(SExpr::Bool { value: b, span }),
            Token::Null => Ok(SExpr::Null { span }),
            Token::Quote => {
                let inner = self.parse_expr()?;
                let end_span = inner.span();
                Ok(SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "quote".to_string(),
                            span,
                        },
                        inner,
                    ],
                    span: Span::new(span.start, end_span.end),
                })
            }
            Token::Quasiquote => {
                let inner = self.parse_expr()?;
                let end_span = inner.span();
                Ok(SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "quasiquote".to_string(),
                            span,
                        },
                        inner,
                    ],
                    span: Span::new(span.start, end_span.end),
                })
            }
            Token::Unquote => {
                let inner = self.parse_expr()?;
                let end_span = inner.span();
                Ok(SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "unquote".to_string(),
                            span,
                        },
                        inner,
                    ],
                    span: Span::new(span.start, end_span.end),
                })
            }
            Token::UnquoteSplice => {
                let inner = self.parse_expr()?;
                let end_span = inner.span();
                Ok(SExpr::List {
                    values: vec![
                        SExpr::Atom {
                            value: "unquote-splicing".to_string(),
                            span,
                        },
                        inner,
                    ],
                    span: Span::new(span.start, end_span.end),
                })
            }
            Token::Hash => self.parse_dispatch(span.start),
            Token::RParen => Err(LyknError::Read {
                message: "unexpected ')'".to_string(),
                location: span.start,
            }),
            Token::Dot => Err(LyknError::Read {
                message: "unexpected '.'".to_string(),
                location: span.start,
            }),
        }
    }

    fn parse_list(&mut self, start: SourceLoc) -> Result<SExpr, LyknError> {
        let mut values = Vec::new();

        loop {
            match self.peek() {
                None => {
                    return Err(LyknError::Read {
                        message: "unterminated list".to_string(),
                        location: start,
                    });
                }
                Some(st) if st.token == Token::RParen => {
                    let end = self.advance().unwrap().span.end;
                    return Ok(SExpr::List {
                        values,
                        span: Span::new(start, end),
                    });
                }
                Some(st) if st.token == Token::Dot => {
                    // Dotted pair: (a . b)
                    self.advance(); // consume dot
                    let cdr = self.parse_expr()?;
                    // Expect RParen
                    match self.advance() {
                        Some(st) if st.token == Token::RParen => {
                            if values.len() != 1 {
                                return Err(LyknError::Read {
                                    message:
                                        "dotted pair must have exactly one element before the dot"
                                            .to_string(),
                                    location: start,
                                });
                            }
                            let car = values.remove(0);
                            let end = st.span.end;
                            return Ok(SExpr::Cons {
                                car: Box::new(car),
                                cdr: Box::new(cdr),
                                span: Span::new(start, end),
                            });
                        }
                        _ => {
                            return Err(LyknError::Read {
                                message: "expected ')' after dotted pair".to_string(),
                                location: start,
                            });
                        }
                    }
                }
                _ => {
                    values.push(self.parse_expr()?);
                }
            }
        }
    }

    fn parse_dispatch(&mut self, start: SourceLoc) -> Result<SExpr, LyknError> {
        let next = self.advance().ok_or_else(|| LyknError::Read {
            message: "unexpected end of input after #".to_string(),
            location: start,
        })?;
        let next_token = next.token.clone();
        let next_span = next.span;

        match next_token {
            Token::Atom(s) if s == ";" => {
                // #; — datum comment: skip next form
                self.parse_expr()?;
                // Return next form after the skipped one
                self.parse_expr()
            }
            Token::Atom(s) if s.starts_with(';') => {
                // #;expr — skip and continue
                self.parse_expr()
            }
            Token::Atom(s) if s == "a" => {
                let inner = self.parse_expr()?;
                match inner {
                    SExpr::List {
                        mut values,
                        span: inner_span,
                    } => {
                        values.insert(
                            0,
                            SExpr::Atom {
                                value: "array".to_string(),
                                span: Span::new(start, next_span.end),
                            },
                        );
                        Ok(SExpr::List {
                            values,
                            span: Span::new(start, inner_span.end),
                        })
                    }
                    _ => Err(LyknError::Read {
                        message: "#a must be followed by a list".to_string(),
                        location: start,
                    }),
                }
            }
            Token::Atom(s) if s == "o" => {
                let inner = self.parse_expr()?;
                match inner {
                    SExpr::List {
                        mut values,
                        span: inner_span,
                    } => {
                        values.insert(
                            0,
                            SExpr::Atom {
                                value: "object".to_string(),
                                span: Span::new(start, next_span.end),
                            },
                        );
                        Ok(SExpr::List {
                            values,
                            span: Span::new(start, inner_span.end),
                        })
                    }
                    _ => Err(LyknError::Read {
                        message: "#o must be followed by a list".to_string(),
                        location: start,
                    }),
                }
            }
            Token::Atom(ref s) if s.contains('r') => {
                // Radix literal: #NNrDIGITS
                self.parse_radix(s, start)
            }
            Token::Atom(ref s) if s.starts_with('|') => {
                // Block comment: #|...|#
                self.skip_block_comment(start)?;
                if self.is_eof() {
                    return Err(LyknError::Read {
                        message: "unexpected end of input after block comment".to_string(),
                        location: start,
                    });
                }
                self.parse_expr()
            }
            Token::Atom(s) => Err(LyknError::Read {
                message: format!("unknown dispatch: #{s}"),
                location: start,
            }),
            _ => Err(LyknError::Read {
                message: "unknown dispatch character after #".to_string(),
                location: start,
            }),
        }
    }

    fn parse_radix(&mut self, s: &str, start: SourceLoc) -> Result<SExpr, LyknError> {
        // s is like "16rff" or "2r1010"
        let parts: Vec<&str> = s.splitn(2, 'r').collect();
        if parts.len() != 2 {
            return Err(LyknError::Read {
                message: format!("invalid radix literal: #{s}"),
                location: start,
            });
        }
        let base: u32 = parts[0].parse().map_err(|_| LyknError::Read {
            message: format!("invalid radix base: {}", parts[0]),
            location: start,
        })?;
        if !(2..=36).contains(&base) {
            return Err(LyknError::Read {
                message: format!("radix base must be 2-36, got {base}"),
                location: start,
            });
        }
        let value = i64::from_str_radix(parts[1], base).map_err(|_| LyknError::Read {
            message: format!("invalid digits for base {base}: {}", parts[1]),
            location: start,
        })?;
        Ok(SExpr::Number {
            value: value as f64,
            span: Span::new(
                start,
                self.tokens
                    .get(self.pos.saturating_sub(1))
                    .map_or(SourceLoc::default(), |t| t.span.end),
            ),
        })
    }

    fn skip_block_comment(&mut self, _start: SourceLoc) -> Result<(), LyknError> {
        // The lexer already tokenized #| as Hash + Atom("|...")
        // For now, we handle this at lexer level in a future enhancement
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::lexer::tokenize;

    fn parse_str(s: &str) -> Vec<SExpr> {
        let tokens = tokenize(s).unwrap();
        parse(&tokens).unwrap()
    }

    #[test]
    fn parse_atom() {
        let forms = parse_str("foo");
        assert_eq!(forms.len(), 1);
        assert!(matches!(&forms[0], SExpr::Atom { value, .. } if value == "foo"));
    }

    #[test]
    fn parse_list() {
        let forms = parse_str("(+ 1 2)");
        assert_eq!(forms.len(), 1);
        match &forms[0] {
            SExpr::List { values, .. } => {
                assert_eq!(values.len(), 3);
                assert!(matches!(&values[0], SExpr::Atom { value, .. } if value == "+"));
                assert!(matches!(&values[1], SExpr::Number { value, .. } if *value == 1.0));
                assert!(matches!(&values[2], SExpr::Number { value, .. } if *value == 2.0));
            }
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn parse_nested_lists() {
        let forms = parse_str("(bind x (+ 1 2))");
        assert_eq!(forms.len(), 1);
        match &forms[0] {
            SExpr::List { values, .. } => {
                assert_eq!(values.len(), 3);
                assert!(matches!(&values[2], SExpr::List { .. }));
            }
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn parse_keyword() {
        let forms = parse_str(":name");
        assert_eq!(forms.len(), 1);
        assert!(matches!(&forms[0], SExpr::Keyword { value, .. } if value == "name"));
    }

    #[test]
    fn parse_quote() {
        let forms = parse_str("'foo");
        assert_eq!(forms.len(), 1);
        match &forms[0] {
            SExpr::List { values, .. } => {
                assert_eq!(values.len(), 2);
                assert!(matches!(&values[0], SExpr::Atom { value, .. } if value == "quote"));
            }
            _ => panic!("expected quoted form"),
        }
    }

    #[test]
    fn parse_string() {
        let forms = parse_str("\"hello world\"");
        assert_eq!(forms.len(), 1);
        assert!(matches!(&forms[0], SExpr::String { value, .. } if value == "hello world"));
    }

    #[test]
    fn parse_multiple_forms() {
        let forms = parse_str("(bind x 1) (bind y 2)");
        assert_eq!(forms.len(), 2);
    }

    #[test]
    fn parse_obj_with_keywords() {
        let forms = parse_str("(obj :name \"Duncan\" :age 42)");
        assert_eq!(forms.len(), 1);
        match &forms[0] {
            SExpr::List { values, .. } => {
                assert_eq!(values.len(), 5);
                assert!(matches!(&values[1], SExpr::Keyword { value, .. } if value == "name"));
                assert!(matches!(&values[3], SExpr::Keyword { value, .. } if value == "age"));
            }
            _ => panic!("expected list"),
        }
    }
}
