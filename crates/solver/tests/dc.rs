mod common;

use common::{assert_close, assert_currents, assert_voltages, circuit, solve};
use ferrispice_circuit::GROUND;
use ferrispice_mna::assemble;
use ferrispice_solver::{DcError, SolveError, solve_dc};

#[test]
fn divider_produces_the_hand_calculated_operating_point() {
    let parsed =
        ferrispice_netlist::parse(include_str!("../../../examples/voltage-divider.cir")).unwrap();
    let circuit = ferrispice_circuit::compile(&parsed).unwrap();
    let solution = solve_dc(&circuit).unwrap();
    assert_voltages(&circuit, &solution, &[("n1", 6.0), ("n2", 2.0)]);
    assert_currents(&circuit, &solution, &[("Vin", -0.04)]);

    let system = assemble(&circuit).unwrap();
    let residual = &system.g * solution.unknowns() - &system.b;
    assert!(residual.amax() < 1e-12);
    assert_eq!(solution.layout(), &system.layout);
}

#[test]
fn ground_is_exactly_zero_and_results_follow_circuit_order() {
    let (circuit, solution) = solve("V1 b 0 DC 3\nR1 a b 1\nR2 a 0 1");
    assert_eq!(solution.node_voltage(GROUND), Some(0.0));
    let nodes: Vec<_> = solution.node_voltages().iter().map(|&(id, _)| id).collect();
    let expected: Vec<_> = circuit.nodes().iter().map(|node| node.id).collect();
    assert_eq!(nodes, expected);
    assert_voltages(&circuit, &solution, &[("b", 3.0), ("a", 1.5)]);
}

#[test]
fn explicit_branch_currents_include_inductors_and_voltage_sources() {
    let (circuit, solution) = solve("V1 a 0 DC 6\nR1 a b 3\nL1 b 0 1e-3");
    assert_voltages(&circuit, &solution, &[("a", 6.0), ("b", 0.0)]);
    assert_currents(&circuit, &solution, &[("L1", 2.0), ("V1", -2.0)]);
}

#[test]
fn ids_from_another_circuit_are_missing_not_zero() {
    let (_, solution) = solve("V1 a 0 DC 1");
    let other = circuit("V1 a 0 DC 1\nV2 b 0 DC 2\nV3 c 0 DC 3");
    let foreign_node = other.find_node("c").unwrap();
    let foreign_element = other.find_element("V3").unwrap();
    assert_eq!(solution.node_voltage(foreign_node), None);
    assert_eq!(solution.element_current(foreign_element), None);
}

#[test]
fn zero_volt_sources_have_zero_valued_results() {
    let (circuit, solution) = solve("V1 a 0 DC 0\nR1 a 0 5");
    assert_eq!(
        solution.node_voltage(circuit.find_node("a").unwrap()),
        Some(0.0)
    );
    assert_close(
        solution
            .element_current(circuit.find_element("V1").unwrap())
            .unwrap(),
        0.0,
    );
}

#[test]
fn floating_nodes_and_conflicting_ideal_sources_are_singular() {
    for records in [
        "R1 a b 10\nR2 c 0 1",
        "V1 p 0 DC 1\nV2 p 0 DC 2",
        "I1 a 0 DC 1",
    ] {
        let error = solve_dc(&circuit(records)).unwrap_err();
        assert!(
            matches!(error, DcError::Solve(SolveError::Singular { .. })),
            "{records}: {error:?}"
        );
    }
}

#[test]
fn errors_keep_their_underlying_cause() {
    let error = solve_dc(&circuit("R1 a b 10\nR2 c 0 1")).unwrap_err();
    assert!(error.to_string().contains("singular"));
    let DcError::Solve(cause) = error else {
        panic!("expected solve error");
    };
    assert!(matches!(cause, SolveError::Singular { .. }));
}

#[test]
fn empty_circuit_solves_to_ground_only() {
    let (circuit, solution) = solve("");
    assert_eq!(solution.node_voltages(), &[(GROUND, 0.0)]);
    assert!(solution.element_currents().is_empty());
    assert_eq!(circuit.elements().len(), 0);
}
