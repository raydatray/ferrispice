use ferrispice_circuit::{Element, Waveform};

use crate::{AssemblyError, MatrixBuilder};

/// current leaves `positive` through the source and enters `negative`
pub(crate) fn current_source(builder: &mut MatrixBuilder, element: &Element, waveform: Waveform) {
    let positive = builder.node_index(element.positive);
    let negative = builder.node_index(element.negative);

    builder.add_source(positive, -1.0, element.id, waveform);
    builder.add_source(negative, 1.0, element.id, waveform);
}

pub(crate) fn voltage_source(
    builder: &mut MatrixBuilder,
    element: &Element,
    waveform: Waveform,
) -> Result<(), AssemblyError> {
    let positive = builder.node_index(element.positive);
    let negative = builder.node_index(element.negative);
    let branch = builder.branch_index(element, element.id)?;

    builder.add_branch_connections(positive, negative, branch);
    builder.add_source(Some(branch), 1.0, element.id, waveform);

    Ok(())
}
