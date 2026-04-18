//! Re-exports the canonical S-expression reader from `lykn-lang`.
//!
//! The formatter and CLI commands use these types directly.

pub use lykn_lang::ast::sexpr::SExpr;
pub use lykn_lang::reader::read;
