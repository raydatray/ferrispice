mod common;

use common::circuit;
use ferrispice_circuit::Circuit;
use ferrispice_solver::{DcError, TransientError, TransientSolution, solve_transient};

fn transient(records: &str, stop: f64, step: f64) -> (Circuit, TransientSolution) {
    let circuit = circuit(records);
    let solution = solve_transient(&circuit, stop, step).unwrap();
    (circuit, solution)
}

fn voltage(circuit: &Circuit, solution: &TransientSolution, node: &str) -> Vec<f64> {
    solution
        .node_voltage(circuit.find_node(node).unwrap())
        .unwrap()
}

fn current(circuit: &Circuit, solution: &TransientSolution, element: &str) -> Vec<f64> {
    solution
        .element_current(circuit.find_element(element).unwrap())
        .unwrap()
}

#[test]
fn the_grid_is_n_times_h_and_ends_exactly_at_stop() {
    let (_, solution) = transient("R1 a 0 1\nI1 0 a DC 1", 1e-3, 1e-4);
    assert_eq!(solution.times().len(), 11);
    for (n, &time) in solution.times().iter().enumerate() {
        assert_eq!(time, if n == 10 { 1e-3 } else { n as f64 * 1e-4 });
    }
    assert_eq!(solution.stats().steps, 10);

    let (_, solution) = transient("R1 a 0 1\nI1 0 a DC 1", 0.05, 1e-4);
    assert_eq!(solution.times().len(), 501);
    assert_eq!(*solution.times().last().unwrap(), 0.05);
}

#[test]
fn a_partial_final_step_lands_on_stop_with_its_own_factorization() {
    let (_, solution) = transient("R1 a 0 1\nC1 a 0 1u\nI1 0 a COS 1 1k", 1e-3, 3e-4);
    let times = solution.times();
    assert_eq!(times.len(), 5);
    assert_eq!(times[3], 3.0 * 3e-4);
    assert_eq!(times[4], 1e-3);
    assert_eq!(solution.stats().factorizations, 2);

    let (_, solution) = transient("R1 a 0 1\nC1 a 0 1u\nI1 0 a COS 1 1k", 1e-3, 1e-4);
    assert_eq!(solution.stats().factorizations, 1);
}

#[test]
fn starts_from_the_dc_operating_point() {
    let (circuit, solution) = transient("Vin a 0 COS 2 60\nR1 a b 1k\nC1 b 0 1u", 1e-3, 1e-4);
    assert_eq!(voltage(&circuit, &solution, "a")[0], 2.0);
    assert_eq!(voltage(&circuit, &solution, "b")[0], 2.0);
    assert_eq!(current(&circuit, &solution, "C1")[0], 0.0);
    assert_eq!(
        solution.states()[0],
        solution.operating_point().unknowns().clone()
    );
}

#[test]
fn rc_matches_the_backward_euler_recurrence_to_rounding() {
    // (v_{n+1} - Vs(t_{n+1})) / R + C (v_{n+1} - v_n) / h = 0
    let (r, c, h, f) = (1e3, 1e-6, 1e-4, 60.0);
    let (circuit, solution) = transient("Vin a 0 COS 1 60\nR1 a b 1k\nC1 b 0 1u", 0.02, h);
    let source = |t: f64| (std::f64::consts::TAU * f * t).cos();

    let v = voltage(&circuit, &solution, "b");
    let mut expected = 1.0;
    for (n, &time) in solution.times().iter().enumerate().skip(1) {
        expected = (source(time) / r + c / h * expected) / (1.0 / r + c / h);
        assert!(
            (v[n] - expected).abs() < 1e-12,
            "t = {time}: {} vs {expected}",
            v[n]
        );
    }

    // the capacitor's current is the resistor's: KCL at b
    let capacitor = current(&circuit, &solution, "C1");
    let resistor = current(&circuit, &solution, "R1");
    for n in 1..v.len() {
        assert!((capacitor[n] - resistor[n]).abs() < 1e-12, "{n}");
    }
}

#[test]
fn resistive_circuits_follow_the_source_at_every_point() {
    let (circuit, solution) = transient(
        "Vin a 0 COS 3 50\nR1 a b 1k\nR2 b 0 2k\nI1 0 b COS 1m 200",
        0.02,
        1e-4,
    );
    let source = current(&circuit, &solution, "I1");
    for (n, &time) in solution.times().iter().enumerate() {
        let vin = 3.0 * (std::f64::consts::TAU * 50.0 * time).cos();
        let injected = 1e-3 * (std::f64::consts::TAU * 200.0 * time).cos();
        assert!((source[n] - injected).abs() < 1e-15);
        // (b - vin) / 1k + b / 2k = injected
        let b = (vin / 1e3 + injected) / (1.0 / 1e3 + 1.0 / 2e3);
        assert!((voltage(&circuit, &solution, "a")[n] - vin).abs() < 1e-12);
        assert!((voltage(&circuit, &solution, "b")[n] - b).abs() < 1e-12);
    }
}

#[test]
fn columns_follow_the_layout() {
    let (circuit, solution) = transient("Vin a 0 COS 1 60\nR1 a b 1k\nC1 b 0 1u", 1e-3, 1e-4);
    let labels: Vec<_> = solution
        .columns()
        .map(|(unknown, values)| {
            assert_eq!(values.len(), solution.times().len());
            unknown.label.clone()
        })
        .collect();
    assert_eq!(labels, ["V(a)", "V(b)", "I(Vin)"]);

    let ground = solution.node_voltage(ferrispice_circuit::GROUND).unwrap();
    assert!(ground.iter().all(|&value| value == 0.0));
    assert_eq!(current(&circuit, &solution, "Vin").len(), 11);
}

#[test]
fn a_singular_operating_point_is_reported_with_its_cause() {
    // node b connects to the rest only through a capacitor, which is open at DC
    let circuit = circuit("Vin a 0 COS 1 60\nR1 a 0 1k\nC1 a b 1u");
    let error = solve_transient(&circuit, 1e-3, 1e-4).unwrap_err();
    assert!(
        matches!(error, TransientError::OperatingPoint(DcError::Solve(_))),
        "{error:?}"
    );
}

const RECTIFIER: &str = "Vin n1 0 COS 1 60\nD1 n1 n2\nR1 n2 0 50\nC1 n2 0 100e-6";

#[test]
fn nonlinear_circuits_take_a_newton_solve_at_every_step() {
    let (circuit, solution) = transient(RECTIFIER, 0.05, 1e-4);
    let stats = solution.stats();
    assert_eq!(stats.steps, 500);
    assert_eq!(stats.factorizations, 0);
    assert!(stats.newton_iterations >= stats.steps, "{stats:?}");
    assert!(stats.max_step_iterations >= 1, "{stats:?}");

    let n2 = voltage(&circuit, &solution, "n2");
    assert!((n2[0] - 0.267314).abs() < 1e-6);
    // the source drives n1 exactly; the diode and capacitor hold n2 up
    for (n, &time) in solution.times().iter().enumerate() {
        let vin = (std::f64::consts::TAU * 60.0 * time).cos();
        assert!((voltage(&circuit, &solution, "n1")[n] - vin).abs() < 1e-9);
        assert!(n2[n] > 0.0 && n2[n] < 0.3, "t = {time}: {}", n2[n]);
    }
}
