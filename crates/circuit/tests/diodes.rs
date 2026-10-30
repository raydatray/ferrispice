use ferrispice_circuit::{
    CircuitError, DEFAULT_SATURATION_CURRENT, DEFAULT_THERMAL_VOLTAGE, ElementErrorKind,
    ElementKind, GROUND, compile,
};
use ferrispice_netlist::{NetlistPosition, parse};

const HANDOUT: &str = "* diode circuit\nVin n1 0 DC 1\nD1 n1 n2\nR n2 0 50\n.dc\n.end\n";

#[test]
fn compiles_the_handout_diode_circuit() {
    let circuit = compile(&parse(HANDOUT).unwrap()).unwrap();

    assert_eq!(circuit.title(), "* diode circuit");
    let names: Vec<_> = circuit
        .nodes()
        .iter()
        .map(|node| node.name.as_str())
        .collect();
    assert_eq!(names, ["0", "n1", "n2"]);

    let diode = circuit
        .element(circuit.find_element("d1").unwrap())
        .unwrap();
    assert_eq!(diode.positive, circuit.find_node("n1").unwrap());
    assert_eq!(diode.negative, circuit.find_node("n2").unwrap());
    assert_eq!(
        diode.kind,
        ElementKind::Diode {
            saturation_current: DEFAULT_SATURATION_CURRENT,
            thermal_voltage: DEFAULT_THERMAL_VOLTAGE,
        }
    );
    assert_eq!(diode.kind.name(), "diode");
    assert!(!diode.kind.has_branch_current());
    assert_eq!(diode.location, NetlistPosition::new(3, 1));
}

#[test]
fn defaults_reproduce_the_handout_results() {
    assert_eq!(DEFAULT_SATURATION_CURRENT, 1e-15);
    assert_eq!(DEFAULT_THERMAL_VOLTAGE, 0.025);
}

#[test]
fn orientation_follows_terminal_order_and_same_node_diodes_are_valid() {
    let circuit = compile(&parse("Orientation\nD1 a 0\nD2 0 a\nD3 a a\n.end").unwrap()).unwrap();
    let a = circuit.find_node("a").unwrap();

    let terminals: Vec<_> = circuit
        .elements()
        .iter()
        .map(|element| (element.positive, element.negative))
        .collect();

    assert_eq!(terminals, [(a, GROUND), (GROUND, a), (a, a)]);
}

#[test]
fn diodes_cannot_control_current_controlled_sources() {
    for record in ["CCCS1 out 0 D1 2", "CCVS1 out 0 D1 2"] {
        let parsed = parse(&format!("Controller\n{record}\nD1 out 0\n.end")).unwrap();
        let CircuitError::Element(error) = compile(&parsed).unwrap_err() else {
            panic!("expected an element error");
        };

        assert_eq!(
            error.kind,
            ElementErrorKind::UnsupportedController {
                name: "D1".to_owned(),
                kind: "diode"
            }
        );
        assert_eq!(error.location, NetlistPosition::new(2, 1));
    }
}
