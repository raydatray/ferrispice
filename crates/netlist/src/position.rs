use std::fmt;

/// one-based physical line and character column in the netlist text for
/// diagnostics
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetlistPosition {
    pub line: usize,
    pub column: usize,
}

impl NetlistPosition {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    pub(crate) fn end_of_input(text: &str) -> Self {
        let line = text.bytes().filter(|&byte| byte == b'\n').count() + 1;
        let column = text.rsplit('\n').next().unwrap_or_default().chars().count() + 1;

        Self { line, column }
    }
}

impl fmt::Display for NetlistPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}", self.line, self.column)
    }
}
