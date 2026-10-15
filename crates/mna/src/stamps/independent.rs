use ferrispice_circuit::Element;

use crate::{AssemblyError, MatrixBuilder};

pub(crate) fn current_source(builder: &mut MatrixBuilder, element: &Element, dc: f64) {
    let positive = builder.node_index(element.positive);
    let negative = builder.node_index(element.negative);

    builder.add_b(positive, -dc);
    builder.add_b(negative, dc);
}

pub(crate) fn voltage_source(
    builder: &mut MatrixBuilder,
    element: &Element,
    dc: f64,
) -> Result<(), AssemblyError> {
    let positive = builder.node_index(element.positive);
    let negative = builder.node_index(element.negative);
    let branch = builder.branch_index(element, element.id)?;

    builder.add_branch_connections(positive, negative, branch);
    builder.add_b(Some(branch), dc);

    Ok(())
}
