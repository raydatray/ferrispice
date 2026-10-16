#![allow(dead_code)]

use ferrispice_circuit::{Circuit, compile};
use ferrispice_netlist::parse;
use ferrispice_solver::{DcSolution, solve_dc};

pub fn circuit(records: &str) -> Circuit {
    compile(&parse(&format!("DC test\n{records}\n.end")).unwrap()).unwrap()
}

pub fn solve(records: &str) -> (Circuit, DcSolution) {
    let circuit = circuit(records);
    let solution = solve_dc(&circuit).unwrap();
    (circuit, solution)
}

pub fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9 * (1.0 + expected.abs()),
        "expected {expected}, got {actual}"
    );
}

pub fn voltage(circuit: &Circuit, solution: &DcSolution, node: &str) -> f64 {
    solution
        .node_voltage(circuit.find_node(node).unwrap())
        .unwrap()
}

pub fn current(circuit: &Circuit, solution: &DcSolution, element: &str) -> f64 {
    solution
        .element_current(circuit.find_element(element).unwrap())
        .unwrap()
}

pub fn assert_voltages(circuit: &Circuit, solution: &DcSolution, expected: &[(&str, f64)]) {
    for &(node, value) in expected {
        assert_close(voltage(circuit, solution, node), value);
    }
}

pub fn assert_currents(circuit: &Circuit, solution: &DcSolution, expected: &[(&str, f64)]) {
    for &(element, value) in expected {
        assert_close(current(circuit, solution, element), value);
    }
}

/// Sum currents leaving each node through every element; each must be zero.
pub fn assert_kcl(circuit: &Circuit, solution: &DcSolution) {
    for node in circuit.non_ground_nodes() {
        let leaving: f64 = circuit
            .elements()
            .iter()
            .map(|element| {
                let current = solution.element_current(element.id).unwrap();
                f64::from(element.positive == node.id) * current
                    - f64::from(element.negative == node.id) * current
            })
            .sum();
        assert!(leaving.abs() < 1e-9, "KCL at {}: {leaving}", node.name);
    }
}
