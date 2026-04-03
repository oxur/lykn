use crate::diagnostics::Diagnostic;
use crate::reader::source_loc::SourceLoc;

#[derive(Debug, thiserror::Error)]
pub enum LyknError {
    #[error("{message} at {location}")]
    Read {
        message: String,
        location: SourceLoc,
    },

    #[error("{0}")]
    Classify(Diagnostic),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
