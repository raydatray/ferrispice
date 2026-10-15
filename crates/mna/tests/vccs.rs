mod common;

use common::{assert_system, system};
use nalgebra::DVector;

#[test]
fn floating_voltage_controls_stamp_signed_outgoing_current() {
    for gm in [-2.0, 0.0, 2.0] {
        assert_system(
            &system(&format!("VCCS1 p n cp cn {gm}")),
            &[
                &[0.0, 0.0, gm, -gm],
                &[0.0, 0.0, -gm, gm],
                &[0.0; 4],
                &[0.0; 4],
            ],
            &[&[0.0; 4], &[0.0; 4], &[0.0; 4], &[0.0; 4]],
            &[0.0; 4],
        );
    }
}

#[test]
fn grounding_and_reversing_ports_preserves_control_polarity() {
    assert_system(
        &system("VCCS1 0 p 0 cp 2"),
        &[&[0.0, 2.0], &[0.0; 2]],
        &[&[0.0; 2], &[0.0; 2]],
        &[0.0; 2],
    );
    assert_system(&system("VCCS1 0 0 cp 0 2"), &[&[0.0]], &[&[0.0]], &[0.0]);
    assert_system(&system("VCCS1 p 0 0 0 2"), &[&[0.0]], &[&[0.0]], &[0.0]);
}

#[test]
fn equal_output_or_sensing_terminals_cancel() {
    for record in ["VCCS1 p p cp 0 2", "VCCS1 p 0 cp cp 2"] {
        assert_system(
            &system(record),
            &[&[0.0; 2], &[0.0; 2]],
            &[&[0.0; 2], &[0.0; 2]],
            &[0.0; 2],
        );
    }
}

#[test]
fn shared_entries_accumulate_with_other_elements() {
    assert_system(
        &system("VCCS1 p 0 p 0 2\nR1 p 0 2"),
        &[&[2.5]],
        &[&[0.0]],
        &[0.0],
    );
    assert_system(
        &system("VCCS1 p 0 cp 0 2\nVCCS2 p 0 cp 0 -0.5"),
        &[&[0.0, 1.5], &[0.0; 2]],
        &[&[0.0; 2], &[0.0; 2]],
        &[0.0; 2],
    );
}

#[test]
fn loaded_output_satisfies_kcl_for_nonzero_control_voltage() {
    let system = system("VCCS1 out 0 cp 0 2\nR1 out 0 4\nVcontrol cp 0 DC 3");
    assert_system(
        &system,
        &[&[0.25, 2.0, 0.0], &[0.0, 0.0, 1.0], &[0.0, 1.0, 0.0]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0, 0.0, 3.0],
    );
    let x = DVector::from_vec(vec![-24.0, 3.0, 0.0]);
    assert!((&system.g * x - &system.b).norm() < 1e-12);
}
