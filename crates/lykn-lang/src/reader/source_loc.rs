#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceLoc {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: SourceLoc,
    pub end: SourceLoc,
}

impl Span {
    pub fn new(start: SourceLoc, end: SourceLoc) -> Self {
        Self { start, end }
    }
}

impl std::fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}
