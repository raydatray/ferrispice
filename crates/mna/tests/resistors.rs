mod common;

use common::{assert_system, system};

#[test]
fn parallel_and_grounded_conductances_accumulate() {
    let system = system("R1 a b 100\nR2 b 0 50\nR3 a b 100");
    assert_system(
        &system,
        &[&[0.02, -0.02], &[-0.02, 0.04]],
        &[&[0.0, 0.0], &[0.0, 0.0]],
        &[0.0, 0.0],
    );
}

#[test]
fn either_grounded_terminal_has_the_same_conductance() {
    for record in ["R1 p 0 100", "R1 0 p 100"] {
        assert_system(&system(record), &[&[0.01]], &[&[0.0]], &[0.0]);
    }
}

#[test]
fn repeated_terminals_cancel_and_ground_adds_no_unknowns() {
    assert_system(
        &system("R1 p p 10\nR2 0 0 20"),
        &[&[0.0]],
        &[&[0.0]],
        &[0.0],
    );
}

#[test]
fn signed_resistance_produces_signed_conductance() {
    assert_system(&system("R1 p 0 -10"), &[&[-0.1]], &[&[0.0]], &[0.0]);
}
