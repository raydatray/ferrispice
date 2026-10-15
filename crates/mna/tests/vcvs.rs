mod common;

use common::{assert_system, system};
use nalgebra::DVector;

#[test]
fn floating_controls_have_signed_gain_in_the_branch_equation() {
    for gain in [-2.0, 0.0, 2.0] {
        assert_system(
            &system(&format!("VCVS1 p n cp cn {gain}")),
            &[
                &[0.0, 0.0, 0.0, 0.0, 1.0],
                &[0.0, 0.0, 0.0, 0.0, -1.0],
                &[0.0; 5],
                &[0.0; 5],
                &[1.0, -1.0, -gain, gain, 0.0],
            ],
            &[&[0.0; 5], &[0.0; 5], &[0.0; 5], &[0.0; 5], &[0.0; 5]],
            &[0.0; 5],
        );
    }
}

#[test]
fn grounding_and_reversing_ports_preserves_the_voltage_constraint() {
    assert_system(
        &system("VCVS1 0 p 0 cp 2"),
        &[&[0.0, 0.0, -1.0], &[0.0; 3], &[-1.0, 2.0, 0.0]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0; 3],
    );
    assert_system(
        &system("VCVS1 0 0 cp 0 2"),
        &[&[0.0; 2], &[-2.0, 0.0]],
        &[&[0.0; 2], &[0.0; 2]],
        &[0.0; 2],
    );
    assert_system(
        &system("VCVS1 p 0 0 0 2"),
        &[&[0.0, 1.0], &[1.0, 0.0]],
        &[&[0.0; 2], &[0.0; 2]],
        &[0.0; 2],
    );
}

#[test]
fn aliased_sensing_and_output_terminals_accumulate_and_cancel() {
    assert_system(
        &system("VCVS1 p 0 p 0 2"),
        &[&[0.0, 1.0], &[-1.0, 0.0]],
        &[&[0.0; 2], &[0.0; 2]],
        &[0.0; 2],
    );
    assert_system(
        &system("VCVS1 p p cp cp 2"),
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0; 3],
    );
}

#[test]
fn signed_gain_and_load_satisfy_the_controlled_voltage_equation() {
    let system = system("VCVS1 out 0 cp 0 -2\nR1 out 0 2\nVcontrol cp 0 DC 3");
    assert_system(
        &system,
        &[
            &[0.5, 0.0, 1.0, 0.0],
            &[0.0, 0.0, 0.0, 1.0],
            &[1.0, 2.0, 0.0, 0.0],
            &[0.0, 1.0, 0.0, 0.0],
        ],
        &[&[0.0; 4], &[0.0; 4], &[0.0; 4], &[0.0; 4]],
        &[0.0, 0.0, 0.0, 3.0],
    );
    let x = DVector::from_vec(vec![-6.0, 3.0, 3.0, 0.0]);
    assert!((&system.g * x - &system.b).norm() < 1e-12);
}
