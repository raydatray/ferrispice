mod common;

use common::{assert_system, circuit, system};
use ferrispice_mna::{NonlinearDevice, TermSign, assemble};
use nalgebra::{DMatrix, DVector};

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

const IS: f64 = 1e-15;
const VT: f64 = 0.025;

fn id(vd: f64) -> f64 {
    IS * (vd / VT).exp_m1()
}

fn gd(vd: f64) -> f64 {
    IS / VT * (vd / VT).exp()
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-12 * expected.abs().max(1e-30),
        "expected {expected}, got {actual}"
    );
}

fn assert_matrix(actual: &DMatrix<f64>, expected: &[&[f64]]) {
    assert_eq!(actual.nrows(), expected.len());
    for (row, values) in expected.iter().enumerate() {
        assert_eq!(actual.ncols(), values.len());
        for (column, &value) in values.iter().enumerate() {
            assert_close(actual[(row, column)], value);
        }
    }
}

#[test]
fn handout_f_and_jacobian_match_the_hand_derived_stamps() {
    let system = system("Vin n1 0 DC 1\nD1 n1 n2\nR n2 0 50");
    let x = DVector::from_vec(vec![1.0, 0.2673, -0.005]);
    let vd = 1.0 - 0.2673;

    let f = system.f(&x).unwrap();
    assert_close(f[0], id(vd));
    assert_close(f[1], -id(vd));
    assert_eq!(f[2], 0.0);

    let g = gd(vd);
    assert_matrix(
        &system.df_dx(&x).unwrap(),
        &[&[g, -g, 0.0], &[-g, g, 0.0], &[0.0, 0.0, 0.0]],
    );
}

#[test]
fn grounded_floating_reversed_and_shared_diodes_accumulate() {
    // unknowns: V(a), V(b)
    let system = system("D1 a 0\nD2 a b\nD3 0 b\nD4 b a\nD5 a a");
    let x = DVector::from_vec(vec![0.6, 0.1]);
    let (va, vb) = (0.6, 0.1);

    let f = system.f(&x).unwrap();
    // currents leaving a: D1 and D2 forward out, D4 enters a from b
    assert_close(f[0], id(va) + id(va - vb) - id(vb - va));
    // currents leaving b: D2 enters, D3 enters from ground, D4 leaves
    assert_close(f[1], -id(va - vb) - id(-vb) + id(vb - va));

    let (g1, g2, g3, g4) = (gd(va), gd(va - vb), gd(-vb), gd(vb - va));
    assert_matrix(
        &system.df_dx(&x).unwrap(),
        &[&[g1 + g2 + g4, -g2 - g4], &[-g2 - g4, g2 + g3 + g4]],
    );
}

#[test]
fn jacobian_matches_a_central_finite_difference_of_f() {
    let system = system("Vin n1 0 DC 1\nD1 n1 n2\nD2 0 n2\nD3 n2 n3\nR n3 0 50\nCCCS1 n3 0 Vin 2");

    for point in [
        [1.0, 0.3, 0.1, -0.01],
        [0.0, 0.0, 0.0, 0.0],
        [0.7, -0.2, -0.5, 0.0],
        [0.9, 0.05, 0.02, 0.003],
    ] {
        let x = DVector::from_row_slice(&point);
        let jacobian = system.df_dx(&x).unwrap();

        for column in 0..x.len() {
            let step = 1e-7;
            let mut forward = x.clone();
            let mut backward = x.clone();
            forward[column] += step;
            backward[column] -= step;
            let (f_forward, f_backward) =
                (system.f(&forward).unwrap(), system.f(&backward).unwrap());
            let derivative = (&f_forward - &f_backward) / (2.0 * step);

            for row in 0..x.len() {
                let expected = derivative[row];
                let actual = jacobian[(row, column)];
                // rounding in f is amplified by 1/step in the difference
                let rounding =
                    8.0 * f64::EPSILON * f_forward[row].abs().max(f_backward[row].abs()) / step;
                assert!(
                    (actual - expected).abs() <= 1e-6 * expected.abs() + rounding,
                    "J[{row},{column}] at {point:?}: expected {expected}, got {actual}"
                );
            }
        }
    }
}

#[test]
fn overflowing_junction_voltages_report_the_device() {
    let circuit = circuit("Vin n1 0 DC 1\nD1 n1 n2\nR n2 0 50");
    let system = assemble(&circuit).unwrap();
    let x = DVector::from_vec(vec![100.0, 0.0, 0.0]);

    for error in [system.f(&x).unwrap_err(), system.df_dx(&x).unwrap_err()] {
        assert_eq!(error.element, "D1");
        assert_eq!(error.cause.voltage, 100.0);
        assert_eq!(error.id, circuit.find_element("D1").unwrap());
        assert!(error.to_string().contains("not finite"), "{error}");
    }
}

#[test]
fn describe_f_lists_signed_device_terms_per_row() {
    let system = system("Vin n1 0 DC 1\nD1 n1 n2\nR n2 0 50\nD2 0 n2\nD3 n1 n1");
    let rows = system.describe_f();
    let terms: Vec<Vec<_>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|term| (term.name.as_str(), term.sign))
                .collect()
        })
        .collect();

    assert_eq!(
        terms,
        [
            vec![("D1", TermSign::Positive)],
            vec![("D1", TermSign::Negative), ("D2", TermSign::Negative)],
            vec![],
        ]
    );
}

#[test]
fn linear_systems_have_zero_f_and_jacobian() {
    let system = system("Vin n1 0 DC 6\nR1 n1 n2 100\nR2 n2 0 50");
    let x = DVector::from_vec(vec![6.0, 2.0, -0.04]);

    assert_eq!(system.f(&x).unwrap(), DVector::zeros(3));
    assert_eq!(system.df_dx(&x).unwrap(), DMatrix::zeros(3, 3));
    assert!(system.describe_f().iter().all(Vec::is_empty));
}
