use crate::diagnostics::Diagnostic;
use crate::reader::source_loc::SourceLoc;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LyknError {
    #[error("{message} at {location}")]
    Read {
        message: String,
        location: SourceLoc,
    },

    #[error("{0}")]
    Classify(Diagnostic),

    #[error("{0}")]
    Codegen(Diagnostic),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
