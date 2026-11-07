mod common;

use common::{assert_close, assert_kcl, assert_voltages, current, solve, voltage};
use ferrispice_solver::DcMethod;

const IS: f64 = 1e-15;
const VT: f64 = 0.025;

fn diode_current(vd: f64) -> f64 {
    IS * (vd / VT).exp_m1()
}

/// solve the monotonic equation `g(v) = 0` on `[low, high]` by bisection,
/// independently of the Newton solver under test
fn bisect(mut low: f64, mut high: f64, g: impl Fn(f64) -> f64) -> f64 {
    assert!(g(low) < 0.0 && g(high) > 0.0);
    for _ in 0..300 {
        let middle = 0.5 * (low + high);
        if g(middle) < 0.0 {
            low = middle;
        } else {
            high = middle;
        }
    }
    0.5 * (low + high)
}

/// junction voltage of one diode in series with `resistance` across `volts`
fn series_junction(volts: f64, resistance: f64) -> f64 {
    bisect(0.0, volts, |vd| {
        diode_current(vd) - (volts - vd) / resistance
    })
}

fn used_source_stepping(method: &DcMethod) -> bool {
    matches!(method, DcMethod::SourceStepping { .. })
}

#[test]
fn handout_circuit_reproduces_the_stated_results() {
    let (circuit, solution) = solve("Vin n1 0 DC 1\nD1 n1 n2\nR n2 0 50");

    assert!(matches!(solution.method(), DcMethod::Newton(_)));
    assert_voltages(&circuit, &solution, &[("n1", 1.0)]);
    let v2 = voltage(&circuit, &solution, "n2");
    let vin = current(&circuit, &solution, "Vin");

    assert!((v2 - 0.267314).abs() < 5e-7, "{v2}");
    assert!((vin + 0.00534629).abs() < 5e-9, "{vin}");
    // the handout rounds to V2 = 0.2673 and IE = 0.0053
    assert_eq!(format!("{v2:.4}"), "0.2673");
    assert_eq!(format!("{:.4}", -vin), "0.0053");

    assert_close(v2, 1.0 - series_junction(1.0, 50.0));
    assert_kcl(&circuit, &solution);
}

#[test]
fn reverse_bias_carries_only_the_saturation_current() {
    let (circuit, solution) = solve("Vin n1 0 DC 5\nD1 n2 n1\nR n2 0 50");

    let diode = current(&circuit, &solution, "D1");
    assert!((diode + IS).abs() < 1e-6 * IS, "{diode}");
    // V(n2) = -50 Is, far below the voltage resolution
    assert!(voltage(&circuit, &solution, "n2").abs() < 1e-9);
    assert_kcl(&circuit, &solution);
}

#[test]
fn series_diodes_share_the_source_voltage() {
    let (circuit, solution) = solve("Vin a 0 DC 2\nD1 a b\nD2 b c\nR c 0 50");

    // identical diodes carry the same current, so their junctions are equal
    let vd = bisect(0.0, 1.0, |vd| diode_current(vd) - (2.0 - 2.0 * vd) / 50.0);
    assert_voltages(
        &circuit,
        &solution,
        &[("b", 2.0 - vd), ("c", 2.0 - 2.0 * vd)],
    );
    assert_kcl(&circuit, &solution);
}

#[test]
fn anti_parallel_diodes_conduct_in_the_forward_direction() {
    let (circuit, solution) = solve("Vin n1 0 DC 1\nD1 n1 n2\nD2 n2 n1\nR n2 0 50");

    let v2 = bisect(0.0, 1.0, |v2| {
        v2 / 50.0 - (diode_current(1.0 - v2) - diode_current(v2 - 1.0))
    });
    assert_voltages(&circuit, &solution, &[("n2", v2)]);
    assert!(current(&circuit, &solution, "D1") > 0.0);
    assert!((current(&circuit, &solution, "D2") + IS).abs() < 1e-6 * IS);
    assert_kcl(&circuit, &solution);
}

#[test]
fn a_current_source_sets_the_junction_voltage_directly() {
    // I1 pushes 1 mA into node a, all of which flows forward through D1. From
    // x = 0 the diode conducts only Is / Vt, so the source must be ramped up
    let (circuit, solution) = solve("I1 0 a DC 1e-3\nD1 a 0");
    assert!(used_source_stepping(solution.method()));

    let expected = VT * (1e-3 / IS).ln_1p();
    assert_voltages(&circuit, &solution, &[("a", expected)]);
    assert_close(current(&circuit, &solution, "D1"), 1e-3);
    assert_kcl(&circuit, &solution);
}

#[test]
fn large_sources_converge_through_source_stepping() {
    let mut rejected = 0;

    for volts in [5.0, 20.0, 100.0, 1000.0] {
        let (circuit, solution) = solve(&format!("Vin n1 0 DC {volts}\nD1 n1 n2\nR n2 0 50"));

        assert!(
            used_source_stepping(solution.method()),
            "{volts} V: {:?}",
            solution.method()
        );
        if let DcMethod::SourceStepping { stepping, .. } = solution.method() {
            rejected += stepping.rejected();
        }

        let vd = series_junction(volts, 50.0);
        let v2 = voltage(&circuit, &solution, "n2");
        assert!(
            (v2 - (volts - vd)).abs() <= 1e-9 * volts,
            "{volts} V: expected {}, got {v2}",
            volts - vd
        );
        // currents reach 20 A, so KCL is checked relative to the source current
        let source = current(&circuit, &solution, "Vin");
        let diode = current(&circuit, &solution, "D1");
        assert!((source + diode).abs() <= 1e-9 * diode.abs(), "{volts} V");
    }

    assert!(rejected > 0, "at least one ramp step must be rejected");
}

#[test]
fn diodes_combine_with_controlled_sources() {
    // the VCVS doubles the 1 V input to drive a diode and 100 Ω load
    let (circuit, solution) =
        solve("Vin a 0 DC 1\nR1 a 0 1000\nVCVS1 b 0 a 0 2\nD1 b c\nR2 c 0 100");

    let vd = series_junction(2.0, 100.0);
    assert_voltages(&circuit, &solution, &[("b", 2.0), ("c", 2.0 - vd)]);
    assert_close(current(&circuit, &solution, "VCVS1"), -(2.0 - vd) / 100.0);
    assert_kcl(&circuit, &solution);
}
