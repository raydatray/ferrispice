use ferrispice_circuit::Element;

use crate::MatrixBuilder;

pub(crate) fn resistor(builder: &mut MatrixBuilder, element: &Element, resistance: f64) {
    let positive = builder.node_index(element.positive);
    let negative = builder.node_index(element.negative);

    builder.add_g_two_terminal(positive, negative, 1.0 / resistance);
}

pub(crate) fn capacitor(builder: &mut MatrixBuilder, element: &Element, capacitance: f64) {
    let positive = builder.node_index(element.positive);
    let negative = builder.node_index(element.negative);

    builder.add_c_two_terminal(positive, negative, capacitance);
}
