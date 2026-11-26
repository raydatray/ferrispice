mod common;

use common::{circuit, system};
use ferrispice_circuit::Waveform;
use ferrispice_mna::SourceTerm;

const COS_1_60: Waveform = Waveform::Cos {
    amplitude: 1.0,
    frequency: 60.0,
};

const SIN_1_60: Waveform = Waveform::Sin {
    amplitude: 1.0,
    frequency: 60.0,
};

fn assert_vector(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (&actual, &expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 1e-12,
            "expected {expected:?}, got {actual:?}"
        );
    }
}

#[test]
fn a_cos_voltage_source_drives_its_branch_row() {
    let circuit = circuit("Vin n1 0 COS 1 60\nR1 n1 0 50");
    let system = ferrispice_mna::assemble(&circuit).unwrap();

    assert_eq!(
        system.sources,
        [SourceTerm {
            element: circuit.find_element("Vin").unwrap(),
            row: 1,
            sign: 1.0,
            waveform: COS_1_60,
        }]
    );
    assert!(!system.is_time_invariant());
    assert_vector(system.b().as_slice(), &[0.0, 1.0]);
    assert_vector(system.b_at(1.0 / 120.0).as_slice(), &[0.0, -1.0]);
    assert_vector(system.b_at(1.0 / 240.0).as_slice(), &[0.0, 0.0]);
}

#[test]
fn a_sin_voltage_source_starts_at_zero_and_drives_its_branch_row() {
    let circuit = circuit("Vin n1 0 SIN 1 60\nR1 n1 0 50");
    let system = ferrispice_mna::assemble(&circuit).unwrap();

    assert_eq!(
        system.sources,
        [SourceTerm {
            element: circuit.find_element("Vin").unwrap(),
            row: 1,
            sign: 1.0,
            waveform: SIN_1_60,
        }]
    );
    assert!(!system.is_time_invariant());
    assert_vector(system.b().as_slice(), &[0.0, 0.0]);
    assert_vector(system.b_at(1.0 / 240.0).as_slice(), &[0.0, 1.0]);
    assert_vector(system.b_at(1.0 / 120.0).as_slice(), &[0.0, 0.0]);
    assert_vector(system.b_at(3.0 / 240.0).as_slice(), &[0.0, -1.0]);
}

#[test]
fn a_cos_current_source_signs_both_nodes_and_skips_ground() {
    let floating = system("I1 a b COS 2 50\nR1 a 0 1\nR2 b 0 1");
    let signs: Vec<_> = floating
        .sources
        .iter()
        .map(|term| (term.row, term.sign))
        .collect();
    assert_eq!(signs, [(0, -1.0), (1, 1.0)]);
    assert_vector(floating.b().as_slice(), &[-2.0, 2.0]);
    assert_vector(floating.b_at(0.01).as_slice(), &[2.0, -2.0]);

    let grounded = system("I1 0 a COS 2 50\nR1 a 0 1");
    assert_eq!(grounded.sources.len(), 1);
    assert_eq!(
        (grounded.sources[0].row, grounded.sources[0].sign),
        (0, 1.0)
    );
    assert_vector(grounded.b_at(0.01).as_slice(), &[-2.0]);
}

#[test]
fn with_mixed_sources_only_the_cos_rows_vary() {
    let system =
        system("Vin a 0 COS 1 60\nV2 b 0 DC 3\nI1 0 c DC 2m\nR1 a b 1\nR2 b c 1\nR3 c 0 1");
    let at_zero = system.b();

    for t in [1e-4, 1.0 / 240.0, 1.0 / 120.0, 0.0123] {
        let b = system.b_at(t);
        // unknowns: V(a), V(b), V(c), I(Vin), I(V2)
        assert_eq!(b[1], 0.0);
        assert_eq!(b[2], at_zero[2]);
        assert_eq!(b[4], at_zero[4]);
        assert!((b[3] - COS_1_60.value_at(t)).abs() < 1e-15, "{t}");
    }
}

#[test]
fn dc_sources_are_time_invariant_and_b_is_b_at_zero() {
    for records in [
        "V1 a 0 DC 6\nR1 a 0 1",
        "I1 a 0 DC 2\nR1 a 0 1",
        "R1 a 0 1",
        "Vin a 0 COS 0 60\nR1 a 0 1",
        "Vin a 0 SIN 0 60\nR1 a 0 1",
    ] {
        let system = system(records);

        assert!(system.is_time_invariant(), "{records}");
        assert_eq!(system.b(), system.b_at(0.0));
        assert_eq!(system.b_at(0.0), system.b_at(1.0));
    }
}
