use crate::error::LyknError;
use crate::reader::source_loc::{SourceLoc, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    LParen,
    RParen,
    Dot,
    Atom(String),
    Keyword(String),
    String(String),
    Number(f64),
    Bool(bool),
    Null,
    Hash,
    Quote,
    Quasiquote,
    Unquote,
    UnquoteSplice,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

pub fn tokenize(source: &str) -> Result<Vec<SpannedToken>, LyknError> {
    let mut lexer = Lexer::new(source);
    lexer.tokenize_all()
}

struct Lexer<'a> {
    source: &'a [u8],
    pos: usize,
    line: u32,
    column: u32,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn tokenize_all(&mut self) -> Result<Vec<SpannedToken>, LyknError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.source.len() {
                break;
            }
            tokens.push(self.next_token()?);
        }
        Ok(tokens)
    }

    fn loc(&self) -> SourceLoc {
        SourceLoc {
            line: self.line,
            column: self.column,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let ch = self.source.get(self.pos).copied()?;
        self.pos += 1;
        if ch == b'\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.pos < self.source.len() {
                let ch = self.source[self.pos];
                if ch == b' ' || ch == b'\t' || ch == b'\n' || ch == b'\r' {
                    self.advance();
                } else {
                    break;
                }
            }
            // Skip line comments
            if self.pos < self.source.len() && self.source[self.pos] == b';' {
                while self.pos < self.source.len() && self.source[self.pos] != b'\n' {
                    self.advance();
                }
                continue;
            }
            break;
        }
    }

    fn next_token(&mut self) -> Result<SpannedToken, LyknError> {
        let start = self.loc();
        let ch = self.advance().unwrap();

        match ch {
            b'(' => Ok(SpannedToken {
                token: Token::LParen,
                span: Span::new(start, self.loc()),
            }),
            b')' => Ok(SpannedToken {
                token: Token::RParen,
                span: Span::new(start, self.loc()),
            }),
            b'\'' => Ok(SpannedToken {
                token: Token::Quote,
                span: Span::new(start, self.loc()),
            }),
            b'`' => Ok(SpannedToken {
                token: Token::Quasiquote,
                span: Span::new(start, self.loc()),
            }),
            b',' => {
                if self.peek() == Some(b'@') {
                    self.advance();
                    Ok(SpannedToken {
                        token: Token::UnquoteSplice,
                        span: Span::new(start, self.loc()),
                    })
                } else {
                    Ok(SpannedToken {
                        token: Token::Unquote,
                        span: Span::new(start, self.loc()),
                    })
                }
            }
            b'#' => Ok(SpannedToken {
                token: Token::Hash,
                span: Span::new(start, self.loc()),
            }),
            b'"' => self.read_string(start),
            b':' => {
                // Keyword: read the atom part after :
                if self.peek().is_some_and(|c| !is_delimiter(c)) {
                    let value = self.read_atom_chars();
                    Ok(SpannedToken {
                        token: Token::Keyword(value),
                        span: Span::new(start, self.loc()),
                    })
                } else {
                    // Bare colon — treat as atom
                    Ok(SpannedToken {
                        token: Token::Atom(":".to_string()),
                        span: Span::new(start, self.loc()),
                    })
                }
            }
            _ => {
                // Atom or number
                let mut value = String::new();
                value.push(ch as char);
                while self.peek().is_some_and(|c| !is_delimiter(c)) {
                    value.push(self.advance().unwrap() as char);
                }

                // Check for special atoms
                let token = match value.as_str() {
                    "true" => Token::Bool(true),
                    "false" => Token::Bool(false),
                    "null" | "undefined" => Token::Null,
                    "." => Token::Dot,
                    _ => {
                        // Try number parse
                        if let Ok(n) = value.parse::<f64>() {
                            if value.starts_with('-')
                                || value.starts_with('+')
                                || value.starts_with(|c: char| c.is_ascii_digit())
                            {
                                Token::Number(n)
                            } else {
                                Token::Atom(value)
                            }
                        } else {
                            Token::Atom(value)
                        }
                    }
                };
                Ok(SpannedToken {
                    token,
                    span: Span::new(start, self.loc()),
                })
            }
        }
    }

    fn read_string(&mut self, start: SourceLoc) -> Result<SpannedToken, LyknError> {
        let mut value = String::new();
        loop {
            match self.advance() {
                None => {
                    return Err(LyknError::Read {
                        message: "unterminated string".to_string(),
                        location: start,
                    });
                }
                Some(b'"') => break,
                Some(b'\\') => match self.advance() {
                    Some(b'n') => value.push('\n'),
                    Some(b't') => value.push('\t'),
                    Some(b'\\') => value.push('\\'),
                    Some(b'"') => value.push('"'),
                    Some(c) => value.push(c as char),
                    None => {
                        return Err(LyknError::Read {
                            message: "unterminated escape in string".to_string(),
                            location: self.loc(),
                        });
                    }
                },
                Some(c) => value.push(c as char),
            }
        }
        Ok(SpannedToken {
            token: Token::String(value),
            span: Span::new(start, self.loc()),
        })
    }

    fn read_atom_chars(&mut self) -> String {
        let mut value = String::new();
        while self.peek().is_some_and(|c| !is_delimiter(c)) {
            value.push(self.advance().unwrap() as char);
        }
        value
    }
}

fn is_delimiter(ch: u8) -> bool {
    matches!(
        ch,
        b' ' | b'\t' | b'\n' | b'\r' | b'(' | b')' | b';' | b'`' | b'\'' | b','
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_atom() {
        let tokens = tokenize("foo").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Atom("foo".to_string()));
    }

    #[test]
    fn tokenize_keyword() {
        let tokens = tokenize(":name").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Keyword("name".to_string()));
    }

    #[test]
    fn tokenize_number() {
        let tokens = tokenize("42").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Number(42.0));
    }

    #[test]
    fn tokenize_string() {
        let tokens = tokenize("\"hello\"").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::String("hello".to_string()));
    }

    #[test]
    fn tokenize_list() {
        let tokens = tokenize("(+ 1 2)").unwrap();
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].token, Token::LParen);
        assert_eq!(tokens[1].token, Token::Atom("+".to_string()));
        assert_eq!(tokens[2].token, Token::Number(1.0));
        assert_eq!(tokens[3].token, Token::Number(2.0));
        assert_eq!(tokens[4].token, Token::RParen);
    }

    #[test]
    fn tokenize_bool() {
        let tokens = tokenize("true false").unwrap();
        assert_eq!(tokens[0].token, Token::Bool(true));
        assert_eq!(tokens[1].token, Token::Bool(false));
    }

    #[test]
    fn tokenize_null() {
        let tokens = tokenize("null undefined").unwrap();
        assert_eq!(tokens[0].token, Token::Null);
        assert_eq!(tokens[1].token, Token::Null);
    }

    #[test]
    fn tokenize_line_comment() {
        let tokens = tokenize("; comment\nfoo").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Atom("foo".to_string()));
    }

    #[test]
    fn source_location_tracking() {
        let tokens = tokenize("foo\nbar").unwrap();
        assert_eq!(tokens[0].span.start.line, 1);
        assert_eq!(tokens[0].span.start.column, 1);
        assert_eq!(tokens[1].span.start.line, 2);
        assert_eq!(tokens[1].span.start.column, 1);
    }
}
