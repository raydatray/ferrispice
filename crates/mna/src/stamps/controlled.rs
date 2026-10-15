use ferrispice_circuit::{Element, NodeId};

use crate::{AssemblyError, MatrixBuilder};

pub(crate) fn vcvs(
    builder: &mut MatrixBuilder,
    element: &Element,
    control_positive: NodeId,
    control_negative: NodeId,
    gain: f64,
) -> Result<(), AssemblyError> {
    let positive = builder.node_index(element.positive);
    let negative = builder.node_index(element.negative);
    let cp = builder.node_index(control_positive);
    let cn = builder.node_index(control_negative);
    let branch = builder.branch_index(element, element.id)?;

    builder.add_branch_connections(positive, negative, branch);
    builder.add_g(Some(branch), cp, -gain);
    builder.add_g(Some(branch), cn, gain);

    Ok(())
}

pub(crate) fn vccs(
    builder: &mut MatrixBuilder,
    element: &Element,
    control_positive: NodeId,
    control_negative: NodeId,
    transconductance: f64,
) {
    let positive = builder.node_index(element.positive);
    let negative = builder.node_index(element.negative);
    let cp = builder.node_index(control_positive);
    let cn = builder.node_index(control_negative);

    builder.add_g(positive, cp, transconductance);
    builder.add_g(positive, cn, -transconductance);
    builder.add_g(negative, cp, -transconductance);
    builder.add_g(negative, cn, transconductance);
}
