use ferrispice_netlist::{ByteSpan, NetlistPosition};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CircuitError {
    #[error(transparent)]
    Element(#[from] ElementError),

    #[error("no element terminal connects to ground node 0")]
    MissingGround,
}

#[derive(Clone, Debug, Error, PartialEq)]
#[error("{location}: element {element:?}: {kind}")]
pub struct ElementError {
    pub location: NetlistPosition,
    pub span: ByteSpan,
    pub element: String,
    pub kind: ElementErrorKind,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ElementErrorKind {
    #[error("duplicate name; first declared at {first}")]
    DuplicateElement {
        first: NetlistPosition,
        first_span: ByteSpan,
    },

    #[error("invalid {field} name {name:?}; expected a nonempty whitespace-free name")]
    InvalidName { field: &'static str, name: String },

    #[error("{parameter} must be finite, found {value}")]
    NonFiniteParameter { parameter: &'static str, value: f64 },

    #[error("resistance must be nonzero")]
    ZeroResistance,

    #[error("{parameter} must be nonnegative, found {value}")]
    NegativeParameter { parameter: &'static str, value: f64 },

    #[error("unknown controlling element {name:?}")]
    UnknownController { name: String },

    #[error("controlling element {name:?} is a {kind} without an explicit branch-current unknown")]
    UnsupportedController { name: String, kind: &'static str },
}
