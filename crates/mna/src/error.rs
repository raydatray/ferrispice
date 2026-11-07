use ferrispice_circuit::{Element, ElementId};
use ferrispice_netlist::{ByteSpan, NetlistPosition};
use thiserror::Error;

use crate::NonFiniteDiode;

#[derive(Clone, Debug, Error, PartialEq)]
#[error("{location}: element {element:?}: {kind}")]
pub struct AssemblyError {
    pub location: NetlistPosition,
    pub span: ByteSpan,
    pub element: String,
    pub kind: AssemblyErrorKind,
}

impl AssemblyError {
    pub(crate) fn new(element: &Element, kind: AssemblyErrorKind) -> Self {
        Self {
            location: element.location,
            span: element.span,
            element: element.name.clone(),
            kind,
        }
    }
}

/// a nonlinear device could not be evaluated at the requested unknowns
#[derive(Clone, Debug, Error, PartialEq)]
#[error("element {element:?}: {cause}")]
pub struct DeviceError {
    /// the device in the circuit, which owns its source location
    pub id: ElementId,
    pub element: String,
    pub cause: NonFiniteDiode,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AssemblyErrorKind {
    #[error("element {branch:?} has no allocated branch-current unknown")]
    MissingBranchCurrent { branch: ElementId },
}
