pub mod lexer;
pub mod parser;
pub mod source_loc;

use crate::ast::sexpr::SExpr;
use crate::error::LyknError;

pub fn read(source: &str) -> Result<Vec<SExpr>, LyknError> {
    let tokens = lexer::tokenize(source)?;
    parser::parse(&tokens)
}
