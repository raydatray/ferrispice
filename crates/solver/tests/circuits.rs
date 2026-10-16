mod common;

use common::{assert_currents, assert_kcl, assert_voltages, solve, voltage};
use ferrispice_mna::assemble;

#[test]
fn divider_reports_every_voltage_and_current() {
    let (circuit, solution) = solve("Vin n1 0 DC 6\nR1 n1 n2 100\nR2 n2 0 50");
    assert_voltages(&circuit, &solution, &[("0", 0.0), ("n1", 6.0), ("n2", 2.0)]);
    assert_currents(
        &circuit,
        &solution,
        &[("Vin", -0.04), ("R1", 0.04), ("R2", 0.04)],
    );
    assert_kcl(&circuit, &solution);
    let ids: Vec<_> = solution
        .element_currents()
        .iter()
        .map(|&(id, _)| id)
        .collect();
    let expected: Vec<_> = circuit
        .elements()
        .iter()
        .map(|element| element.id)
        .collect();
    assert_eq!(ids, expected);
}

#[test]
fn current_source_drives_a_resistor_in_both_directions() {
    let (circuit, solution) = solve("I1 0 p DC 2\nR1 p 0 5");
    assert_voltages(&circuit, &solution, &[("p", 10.0)]);
    assert_currents(&circuit, &solution, &[("I1", 2.0), ("R1", 2.0)]);
    assert_kcl(&circuit, &solution);

    let (circuit, solution) = solve("I1 p 0 DC 2\nR1 p 0 5");
    assert_voltages(&circuit, &solution, &[("p", -10.0)]);
    assert_currents(&circuit, &solution, &[("I1", 2.0), ("R1", -2.0)]);
    assert_kcl(&circuit, &solution);
}

#[test]
fn capacitor_is_open_at_dc() {
    let (circuit, solution) = solve("I1 0 p DC 1\nR1 p 0 4\nC1 p 0 1e-6");
    assert_voltages(&circuit, &solution, &[("p", 4.0)]);
    assert_currents(&circuit, &solution, &[("C1", 0.0), ("R1", 1.0)]);
    assert_kcl(&circuit, &solution);
}

#[test]
fn inductor_is_a_short_with_its_own_current() {
    let (circuit, solution) = solve("V1 a 0 DC 6\nR1 a b 3\nL1 b 0 1e-3");
    assert_voltages(&circuit, &solution, &[("b", 0.0)]);
    assert_currents(
        &circuit,
        &solution,
        &[("R1", 2.0), ("L1", 2.0), ("V1", -2.0)],
    );
    assert_kcl(&circuit, &solution);
}

#[test]
fn vcvs_amplifies_a_control_voltage_with_signed_gain() {
    for (gain, out) in [(3.0, 6.0), (-3.0, -6.0)] {
        let (circuit, solution) =
            solve(&format!("Vin c 0 DC 2\nVCVS1 out 0 c 0 {gain}\nR1 out 0 2"));
        assert_voltages(&circuit, &solution, &[("out", out)]);
        assert_currents(
            &circuit,
            &solution,
            &[("R1", out / 2.0), ("VCVS1", -out / 2.0), ("Vin", 0.0)],
        );
        assert_kcl(&circuit, &solution);
    }
}

#[test]
fn vccs_drives_current_from_a_control_voltage_with_signed_gain() {
    for (gm, out) in [(0.5, -4.0), (-0.5, 4.0)] {
        let (circuit, solution) = solve(&format!("Vin c 0 DC 2\nVCCS1 out 0 c 0 {gm}\nR1 out 0 4"));
        assert_voltages(&circuit, &solution, &[("out", out)]);
        assert_currents(
            &circuit,
            &solution,
            &[("VCCS1", 2.0 * gm), ("R1", out / 4.0)],
        );
        assert_kcl(&circuit, &solution);
    }
}

#[test]
fn ccvs_uses_a_sensing_source_current_with_signed_gain() {
    for (rm, out) in [(3.0, 6.0), (-3.0, -6.0)] {
        let (circuit, solution) = solve(&format!(
            "Vin a 0 DC 10\nRs a b 5\nVsense b 0 DC 0\nCCVS1 out 0 Vsense {rm}\nR1 out 0 6"
        ));
        assert_voltages(&circuit, &solution, &[("b", 0.0), ("out", out)]);
        assert_currents(
            &circuit,
            &solution,
            &[
                ("Vsense", 2.0),
                ("Vin", -2.0),
                ("R1", out / 6.0),
                ("CCVS1", -out / 6.0),
            ],
        );
        assert_kcl(&circuit, &solution);
    }
}

#[test]
fn cccs_uses_a_sensing_source_declared_later() {
    for (gain, out) in [(2.0, -24.0), (-2.0, 24.0)] {
        let (circuit, solution) = solve(&format!(
            "CCCS1 out 0 Vsense {gain}\nR1 out 0 6\nVin a 0 DC 10\nRs a b 5\nVsense b 0 DC 0"
        ));
        assert_voltages(&circuit, &solution, &[("out", out)]);
        assert_currents(
            &circuit,
            &solution,
            &[("Vsense", 2.0), ("CCCS1", 2.0 * gain), ("R1", out / 6.0)],
        );
        assert_kcl(&circuit, &solution);
    }
}

#[test]
fn mixed_circuit_with_all_nine_elements_is_consistent() {
    let records = "Vin a 0 DC 10\nR1 a b 2\nC1 b 0 1e-6\nL1 b c 1e-3\nR2 c 0 3\n\
                   I1 0 c DC 1\nVCVS1 d 0 c 0 2\nRd d 0 4\nVCCS1 0 e b 0 0.1\nRe e 0 10\n\
                   CCVS1 f 0 L1 5\nRf f 0 5\nCCCS1 0 g Vin 0.5\nRg g 0 2";
    let (circuit, solution) = solve(records);
    // Nodes b and c are shorted by L1: (10 - v)/2 + 1 = v/3 gives v = 7.2.
    let v = 7.2;
    let i_l = v / 3.0 - 1.0;
    let i_vin = -(10.0 - v) / 2.0;
    assert_voltages(
        &circuit,
        &solution,
        &[
            ("b", v),
            ("c", v),
            ("d", 2.0 * v),
            ("e", 0.1 * v * 10.0),
            ("f", 5.0 * i_l),
            ("g", 0.5 * i_vin * 2.0),
        ],
    );
    assert_currents(
        &circuit,
        &solution,
        &[
            ("C1", 0.0),
            ("L1", i_l),
            ("Vin", i_vin),
            ("I1", 1.0),
            ("VCVS1", -2.0 * v / 4.0),
            ("VCCS1", 0.1 * v),
            ("CCVS1", -i_l),
            ("CCCS1", 0.5 * i_vin),
        ],
    );
    assert_kcl(&circuit, &solution);

    let system = assemble(&circuit).unwrap();
    assert!((&system.g * solution.unknowns() - &system.b).amax() < 1e-12);
    assert_eq!(voltage(&circuit, &solution, "0"), 0.0);
}
