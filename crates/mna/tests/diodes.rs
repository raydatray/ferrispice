mod common;

use common::{assert_system, circuit, system};
use ferrispice_mna::{NonlinearDevice, assemble};

fn diodes(system: &ferrispice_mna::MnaSystem) -> Vec<(&str, Option<usize>, Option<usize>)> {
    system
        .nonlinear
        .iter()
        .map(|device| match device {
            NonlinearDevice::Diode(diode) => (diode.name.as_str(), diode.anode, diode.cathode),
        })
        .collect()
}

#[test]
fn diodes_add_no_unknowns_or_linear_stamps() {
    let system = system("Vin n1 0 DC 1\nD1 n1 n2\nR n2 0 50");

    assert_eq!(
        system.layout.labels().collect::<Vec<_>>(),
        ["V(n1)", "V(n2)", "I(Vin)"]
    );
    assert_system(
        &system,
        &[&[0.0, 0.0, 1.0], &[0.0, 0.02, 0.0], &[1.0, 0.0, 0.0]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0, 0.0, 1.0],
    );
    assert!(!system.is_linear());
    assert_eq!(diodes(&system), [("D1", Some(0), Some(1))]);
}

#[test]
fn recorded_diodes_keep_their_identity_and_parameters() {
    let circuit = circuit("Vin n1 0 DC 1\nD1 n1 n2\nR n2 0 50");
    let system = assemble(&circuit).unwrap();
    let NonlinearDevice::Diode(diode) = &system.nonlinear[0];
    let element = circuit
        .element(circuit.find_element("D1").unwrap())
        .unwrap();

    assert_eq!(diode.element, element.id);
    assert_eq!(diode.name, element.name);
    assert_eq!(diode.saturation_current, 1e-15);
    assert_eq!(diode.thermal_voltage, 0.025);
}

#[test]
fn grounded_terminals_have_no_matrix_index_and_order_follows_the_circuit() {
    let system = system("D2 0 a\nR1 a 0 1\nD1 a 0\nD3 a a");

    assert_eq!(
        diodes(&system),
        [
            ("D2", None, Some(0)),
            ("D1", Some(0), None),
            ("D3", Some(0), Some(0)),
        ]
    );
}

#[test]
fn circuits_without_diodes_are_linear_and_unchanged() {
    let system = system("Vin n1 0 DC 6\nR1 n1 n2 100\nR2 n2 0 50");

    assert!(system.is_linear());
    assert!(system.nonlinear.is_empty());
    assert_system(
        &system,
        &[&[0.01, -0.01, 1.0], &[-0.01, 0.03, 0.0], &[1.0, 0.0, 0.0]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0, 0.0, 6.0],
    );
}
