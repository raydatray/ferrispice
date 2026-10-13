use ferrispice_netlist::{ByteSpan, NetlistPosition};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq)]
#[error("{location}: element {element:?}: {kind}")]
pub struct CircuitError {
    pub location: NetlistPosition,
    pub span: ByteSpan,
    pub element: String,
    pub kind: CircuitErrorKind,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CircuitErrorKind {
    #[error("duplicate name; first declared at {first}")]
    DuplicateElement {
        first: NetlistPosition,
        first_span: ByteSpan,
    },

    #[error("invalid {field} name {name:?}; expected a nonempty whitespace-free name")]
    InvalidName { field: &'static str, name: String },

    #[error("unknown controlling element {name:?}")]
    UnknownController { name: String },
}
