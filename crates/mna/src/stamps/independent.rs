use ferrispice_circuit::Element;

use crate::MatrixBuilder;

pub(crate) fn current_source(builder: &mut MatrixBuilder, element: &Element, dc: f64) {
    let positive = builder.node_index(element.positive);
    let negative = builder.node_index(element.negative);

    builder.add_b(positive, -dc);
    builder.add_b(negative, dc);
}
