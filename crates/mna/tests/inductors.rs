mod common;

use common::{assert_system, system};
use nalgebra::DVector;

#[test]
fn inductor_has_voltage_incidence_and_negative_branch_storage() {
    assert_system(
        &system("L1 p n 2.5"),
        &[&[0.0, 0.0, 1.0], &[0.0, 0.0, -1.0], &[1.0, -1.0, 0.0]],
        &[&[0.0; 3], &[0.0; 3], &[0.0, 0.0, -2.5]],
        &[0.0; 3],
    );
}

#[test]
fn reversed_grounded_inductor_preserves_current_orientation() {
    assert_system(
        &system("L1 0 p 3"),
        &[&[0.0, -1.0], &[-1.0, 0.0]],
        &[&[0.0; 2], &[0.0, -3.0]],
        &[0.0; 2],
    );
}

#[test]
fn zero_inductance_retains_the_dc_short_and_current_unknown() {
    assert_system(
        &system("L1 p 0 0"),
        &[&[0.0, 1.0], &[1.0, 0.0]],
        &[&[0.0; 2], &[0.0; 2]],
        &[0.0; 2],
    );
}

#[test]
fn repeated_terminals_cancel_incidence_but_keep_inductive_storage() {
    assert_system(
        &system("L1 p p 2.5\nL2 0 0 7"),
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[&[0.0; 3], &[0.0, -2.5, 0.0], &[0.0, 0.0, -7.0]],
        &[0.0; 3],
    );
}

#[test]
fn storage_equation_matches_voltage_equals_l_times_current_derivative() {
    let system = system("R1 p 0 2\nL1 p 0 3\nI1 0 p DC 4");
    assert_system(
        &system,
        &[&[0.5, 1.0], &[1.0, 0.0]],
        &[&[0.0; 2], &[0.0, -3.0]],
        &[4.0, 0.0],
    );
    let x = DVector::from_vec(vec![6.0, 1.0]);
    let derivative = DVector::from_vec(vec![0.0, 2.0]);
    assert!((&system.g * x + &system.c * derivative - &system.b).norm() < 1e-12);
}
