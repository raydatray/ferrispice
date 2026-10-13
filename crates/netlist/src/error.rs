use thiserror::Error;

use crate::{ByteSpan, NetlistPosition};

/// a netlist syntax error with its position and exact byte span in the netlist
/// text
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{location}: {kind}")]
pub struct ParseError {
    pub location: NetlistPosition,
    pub span: ByteSpan,
    pub kind: ParseErrorKind,
}

impl ParseError {
    pub(crate) fn new(location: NetlistPosition, span: ByteSpan, kind: ParseErrorKind) -> Self {
        Self {
            location,
            span,
            kind,
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ParseErrorKind {
    #[error("missing the first-line circuit title")]
    MissingTitle,

    #[error("missing the final .end directive")]
    MissingEnd,

    #[error("a record appears after .end")]
    RecordAfterEnd,

    #[error("unsupported directive {directive:?}")]
    UnsupportedDirective { directive: String },

    #[error("unsupported element {name:?}")]
    UnsupportedElement { name: String },

    #[error("expected {expected}; found {found} fields")]
    WrongFieldCount {
        expected: &'static str,
        found: usize,
    },

    #[error("expected the DC keyword, found {found:?}")]
    ExpectedDc { found: String },

    #[error("expected a decimal or scientific-notation number, found {value:?}")]
    InvalidNumber { value: String },

    #[error("expected a finite number, found {value:?}")]
    NonFiniteNumber { value: String },
}
