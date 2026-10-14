mod builder;
mod error;
mod layout;

use ferrispice_circuit::Circuit;
use nalgebra::{DMatrix, DVector};

pub use builder::MatrixBuilder;
pub use error::{AssemblyError, AssemblyErrorKind};
pub use layout::{Unknown, UnknownKind, UnknownLayout};

#[derive(Clone, Debug, PartialEq)]
pub struct MnaSystem {
    pub g: DMatrix<f64>,
    pub c: DMatrix<f64>,
    pub b: DVector<f64>,
    pub layout: UnknownLayout,
}

/// allocate every unknown, then assemble without solving
pub fn assemble(circuit: &Circuit) -> Result<MnaSystem, AssemblyError> {
    let _span = tracing::debug_span!("assemble_mna", elements = circuit.elements().len()).entered();

    let builder = MatrixBuilder::new(UnknownLayout::new(circuit));

    if let Some(element) = circuit.elements().first() {
        return Err(AssemblyError::new(
            element,
            AssemblyErrorKind::UnsupportedElement {
                kind: element.kind.name(),
            },
        ));
    }

    let system = builder.finish();

    tracing::debug!(unknowns = system.layout.len(), "assembled MNA system");

    Ok(system)
}
