//! transient results checked against closed-form solutions and an independent
//! backward-Euler reference

mod common;

use std::f64::consts::TAU;

use common::circuit;
use ferrispice_circuit::Circuit;
use ferrispice_solver::{TransientSolution, solve_transient};

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

/// a first-order low-pass response to `cos(ω t)` from an initial value:
/// `A cos(ω t - φ) + (initial - A cos φ) e^{-t/τ}`, with
/// `A = 1 / sqrt(1 + (ω τ)²)` and `φ = atan(ω τ)`
fn low_pass(omega: f64, tau: f64, initial: f64, t: f64) -> f64 {
    let amplitude = 1.0 / (1.0 + (omega * tau).powi(2)).sqrt();
    let phase = (omega * tau).atan();
    amplitude * (omega * t - phase).cos() + (initial - amplitude * phase.cos()) * (-t / tau).exp()
}

/// a first-order low-pass response to `sin(ω t)` from `0`:
/// `A sin(ω t - φ) + A sin φ · e^{-t/τ}`, with
/// `A = 1 / sqrt(1 + (ω τ)²)` and `φ = atan(ω τ)`
fn low_pass_sin(omega: f64, tau: f64, t: f64) -> f64 {
    let amplitude = 1.0 / (1.0 + (omega * tau).powi(2)).sqrt();
    let phase = (omega * tau).atan();
    amplitude * (omega * t - phase).sin() + amplitude * phase.sin() * (-t / tau).exp()
}

fn max_error(actual: &[f64], times: &[f64], exact: impl Fn(f64) -> f64) -> f64 {
    actual
        .iter()
        .zip(times)
        .map(|(&value, &t)| (value - exact(t)).abs())
        .fold(0.0, f64::max)
}

/// `R = 1k` and `C = 1u`: `τ = 1 ms`, driven at 100 Hz
const RC: &str = "Vin a 0 COS 1 100\nR1 a b 1k\nC1 b 0 1u";

#[test]
fn rc_error_against_the_continuous_solution_is_first_order() {
    let (omega, tau) = (TAU * 100.0, 1e-3);
    let errors: Vec<f64> = [1e-4, 5e-5, 2.5e-5]
        .iter()
        .map(|&step| {
            let (circuit, solution) = transient(RC, 0.02, step);
            let v = voltage(&circuit, &solution, "b");
            max_error(&v, solution.times(), |t| low_pass(omega, tau, 1.0, t))
        })
        .collect();

    assert!(errors[0] < 0.05, "{errors:?}");
    for pair in errors.windows(2) {
        let ratio = pair[0] / pair[1];
        assert!((1.8..2.2).contains(&ratio), "{errors:?}");
    }
}

#[test]
fn rc_steady_state_approaches_the_analytical_amplitude_and_phase() {
    let (omega, tau, step) = (TAU * 100.0, 1e-3, 1e-5);
    let (circuit, solution) = transient(RC, 0.05, step);
    let v = voltage(&circuit, &solution, "b");

    // project the last two periods, long after e^{-t/τ} has died out, onto
    // cos and sin: v ≈ a cos(ω t) + b sin(ω t)
    let samples: Vec<_> = solution
        .times()
        .iter()
        .zip(&v)
        .filter(|(t, _)| **t > 0.03)
        .collect();
    let count = samples.len() as f64;
    let a = 2.0 / count
        * samples
            .iter()
            .map(|(t, v)| *v * (omega * *t).cos())
            .sum::<f64>();
    let b = 2.0 / count
        * samples
            .iter()
            .map(|(t, v)| *v * (omega * *t).sin())
            .sum::<f64>();

    let amplitude = (a * a + b * b).sqrt();
    // v = A cos(ω t + θ) has a = A cos θ and b = -A sin θ
    let phase = (-b).atan2(a);

    let expected_amplitude = 1.0 / (1.0 + (omega * tau).powi(2)).sqrt();
    let expected_phase = -(omega * tau).atan();
    assert!(
        (amplitude - expected_amplitude).abs() < 5e-3,
        "{amplitude} vs {expected_amplitude}"
    );
    assert!(
        (phase - expected_phase).abs() < 5e-3,
        "{phase} vs {expected_phase}"
    );
}

#[test]
fn sin_driven_rc_starts_at_zero_and_converges_first_order() {
    let rc_sin = "Vin a 0 SIN 1 100\nR1 a b 1k\nC1 b 0 1u";
    let (omega, tau) = (TAU * 100.0, 1e-3);

    let errors: Vec<f64> = [1e-4, 5e-5, 2.5e-5]
        .iter()
        .map(|&step| {
            let (circuit, solution) = transient(rc_sin, 0.02, step);
            let a = voltage(&circuit, &solution, "a");
            let b = voltage(&circuit, &solution, "b");
            assert_eq!(a[0], 0.0);
            assert_eq!(b[0], 0.0);

            // exact backward-Euler recurrence
            let mut expected = 0.0;
            for (n, &time) in solution.times().iter().enumerate().skip(1) {
                let source = (omega * time).sin();
                assert!((a[n] - source).abs() < 1e-12, "t = {time}");
                expected = (expected + (step / tau) * source) / (1.0 + step / tau);
                assert!((b[n] - expected).abs() < 1e-12, "t = {time}");
            }

            max_error(&b, solution.times(), |t| low_pass_sin(omega, tau, t))
        })
        .collect();

    assert!(errors[0] < 0.05, "{errors:?}");
    for pair in errors.windows(2) {
        let ratio = pair[0] / pair[1];
        assert!((1.8..2.2).contains(&ratio), "{errors:?}");
    }
}

/// `R = 10` and `L = 10m`: `τ = 1 ms`, driven at 100 Hz
const RL: &str = "Vin a 0 COS 1 100\nR1 a p 10\nL1 p 0 10m";

#[test]
fn rl_branch_current_follows_the_minus_l_convention() {
    // the branch row is v - (L/h)(i_{n+1} - i_n) = 0, so with the resistor
    // (L/h)(i_{n+1} - i_n) - Vs(t_{n+1}) + R i_{n+1} = 0
    let (r, l, h) = (10.0, 10e-3, 1e-4);
    let (circuit, solution) = transient(RL, 0.02, h);
    let i = current(&circuit, &solution, "L1");

    // the inductor is a short at the operating point
    assert!((i[0] - 0.1).abs() < 1e-15);
    let mut expected = 0.1;
    for (n, &time) in solution.times().iter().enumerate().skip(1) {
        let source = (TAU * 100.0 * time).cos();
        expected = (source + l / h * expected) / (r + l / h);
        assert!((i[n] - expected).abs() < 1e-12, "t = {time}");
    }

    // and the current converges on the continuous solution, scaled by 1 / R
    let error = max_error(&i, solution.times(), |t| {
        low_pass(TAU * 100.0, l / r, 1.0, t) / r
    });
    assert!(error < 5e-3, "{error}");
}

const RECTIFIER: &str = "Vin n1 0 COS 1 60\nD1 n1 n2\nR1 n2 0 50\nC1 n2 0 100e-6";

/// `V(n2)` from an independent Python backward-Euler reference with the DC
/// operating point as the initial state and `Is = 1e-15 A`
const RECTIFIER_REFERENCE: [(usize, f64); 7] = [
    (0, 0.267314),
    (1, 0.267192),
    (10, 0.245161),
    (50, 0.111627),
    (100, 0.041472),
    (250, 0.057032),
    (500, 0.255129),
];

#[test]
fn the_handout_rectifier_reproduces_the_reference() {
    let (circuit, solution) = transient(RECTIFIER, 0.05, 1e-4);
    let times = solution.times();
    assert_eq!(times.len(), 501);
    assert_eq!(solution.stats().subdivided_steps, 0);

    let n2 = voltage(&circuit, &solution, "n2");
    for (n, expected) in RECTIFIER_REFERENCE {
        assert!(
            (n2[n] - expected).abs() < 5e-7,
            "t = {}: {} vs {expected}",
            times[n],
            n2[n]
        );
    }

    let n1 = voltage(&circuit, &solution, "n1");
    for (n, &time) in times.iter().enumerate() {
        assert!((n1[n] - (TAU * 60.0 * time).cos()).abs() < 1e-12, "{time}");
    }

    let settled = n2[100..].iter().copied().fold(f64::MIN, f64::max);
    let minimum = n2.iter().copied().fold(f64::MAX, f64::min);
    assert!((settled - 0.2595).abs() < 1e-4, "{settled}");
    assert!((minimum - 0.0173).abs() < 1e-4, "{minimum}");
}

#[test]
fn kcl_holds_at_the_rectifier_output_at_every_point() {
    // id = v / R + C dv/dt
    let (circuit, solution) = transient(RECTIFIER, 0.05, 1e-4);
    let diode = current(&circuit, &solution, "D1");
    let resistor = current(&circuit, &solution, "R1");
    let capacitor = current(&circuit, &solution, "C1");
    let n2 = voltage(&circuit, &solution, "n2");

    for n in 0..diode.len() {
        assert!((resistor[n] - n2[n] / 50.0).abs() < 1e-15);
        let leaving = resistor[n] + capacitor[n] - diode[n];
        assert!(
            leaving.abs() < 1e-9,
            "t = {}: {leaving}",
            solution.times()[n]
        );
    }

    // the source supplies the diode current
    let source = current(&circuit, &solution, "Vin");
    for n in 0..diode.len() {
        assert!((source[n] + diode[n]).abs() < 1e-9);
    }
}
