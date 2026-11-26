use ferrispice_netlist::{ByteSpan, NetlistPosition};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CircuitError {
    #[error(transparent)]
    Element(#[from] ElementError),

    #[error("no element terminal connects to ground node 0")]
    MissingGround,

    #[error(transparent)]
    Transient(#[from] TransientRequestError),
}

/// a `.tran` request whose times cannot describe a simulation
#[derive(Clone, Debug, Error, PartialEq)]
#[error("{location}: .tran: {kind}")]
pub struct TransientRequestError {
    pub location: NetlistPosition,
    pub span: ByteSpan,
    pub kind: TransientRequestErrorKind,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum TransientRequestErrorKind {
    #[error("stop time must be positive, found {stop}")]
    NonPositiveStop { stop: f64 },

    #[error("time step must be positive, found {step}")]
    NonPositiveStep { step: f64 },

    #[error("time step {step} is longer than the stop time {stop}")]
    StepExceedsStop { stop: f64, step: f64 },
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

    #[error("{parameter} must be positive, found {value}")]
    NonPositiveParameter { parameter: &'static str, value: f64 },

    #[error("resistance must be nonzero")]
    ZeroResistance,

    #[error("{parameter} must be nonnegative, found {value}")]
    NegativeParameter { parameter: &'static str, value: f64 },

    #[error("unknown controlling element {name:?}")]
    UnknownController { name: String },

    #[error("controlling element {name:?} is a {kind} without an explicit branch-current unknown")]
    UnsupportedController { name: String, kind: &'static str },
}
