use ferrispice_circuit::{Element, ElementId};
use ferrispice_netlist::{ByteSpan, NetlistPosition};
use thiserror::Error;

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

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AssemblyErrorKind {
    #[error("element {branch:?} has no allocated branch-current unknown")]
    MissingBranchCurrent { branch: ElementId },
}
