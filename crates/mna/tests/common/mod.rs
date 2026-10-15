use ferrispice_circuit::{Circuit, compile};
use ferrispice_mna::{MnaSystem, assemble};
use ferrispice_netlist::parse;
use nalgebra::DMatrix;

/// Compilation rejects circuits with no ground terminal, but stamps are tested
/// in isolation, including on floating terminals. The appended resistor
/// touches only ground, which has no row or column, so it stamps nothing.
const INERT_GROUND: &str = "Rinert_ground 0 0 1";

pub fn circuit(records: &str) -> Circuit {
    compile(&parse(&format!("MNA test\n{records}\n{INERT_GROUND}\n.end")).unwrap()).unwrap()
}

pub fn system(records: &str) -> MnaSystem {
    assemble(&circuit(records)).unwrap()
}

pub fn assert_system(system: &MnaSystem, g: &[&[f64]], c: &[&[f64]], b: &[f64]) {
    assert_matrix(&system.g, g);
    assert_matrix(&system.c, c);
    assert_eq!(system.b.len(), b.len());
    for (&actual, &expected) in system.b.iter().zip(b) {
        assert_close(actual, expected);
    }
}

fn assert_matrix(actual: &DMatrix<f64>, expected: &[&[f64]]) {
    let columns = expected.first().map_or(0, |row| row.len());
    assert_eq!(actual.shape(), (expected.len(), columns));
    for (row, values) in expected.iter().enumerate() {
        assert_eq!(values.len(), columns);
        for (column, &value) in values.iter().enumerate() {
            assert_close(actual[(row, column)], value);
        }
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-12 * (1.0 + expected.abs()),
        "expected {expected}, got {actual}"
    );
}
