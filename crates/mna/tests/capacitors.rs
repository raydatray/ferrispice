mod common;

use common::{assert_system, system};

#[test]
fn capacitances_accumulate_only_in_storage() {
    assert_system(
        &system("R1 p n 100\nC1 p n 2\nC2 p n 3\nC3 n 0 7"),
        &[&[0.01, -0.01], &[-0.01, 0.01]],
        &[&[5.0, -5.0], &[-5.0, 12.0]],
        &[0.0, 0.0],
    );
}

#[test]
fn either_grounded_terminal_has_the_same_storage() {
    for record in ["C1 p 0 4", "C1 0 p 4"] {
        assert_system(&system(record), &[&[0.0]], &[&[4.0]], &[0.0]);
    }
}

#[test]
fn repeated_terminals_cancel_storage() {
    assert_system(&system("C1 p p 4\nC2 0 0 5"), &[&[0.0]], &[&[0.0]], &[0.0]);
}

#[test]
fn zero_capacitance_still_retains_its_terminal_node() {
    assert_system(&system("C1 p 0 0"), &[&[0.0]], &[&[0.0]], &[0.0]);
}
