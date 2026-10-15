mod common;

use common::{assert_system, system};
use nalgebra::DVector;

#[test]
fn forward_controller_uses_its_preallocated_current_column() {
    for resistance in [-3.0, 0.0, 3.0] {
        let system = system(&format!("CCVS1 p n Vsense {resistance}\nVsense cp 0 DC 0"));
        assert_eq!(
            system.layout.labels().collect::<Vec<_>>(),
            ["V(p)", "V(n)", "V(cp)", "I(CCVS1)", "I(Vsense)"]
        );
        assert_system(
            &system,
            &[
                &[0.0, 0.0, 0.0, 1.0, 0.0],
                &[0.0, 0.0, 0.0, -1.0, 0.0],
                &[0.0, 0.0, 0.0, 0.0, 1.0],
                &[1.0, -1.0, 0.0, 0.0, -resistance],
                &[0.0, 0.0, 1.0, 0.0, 0.0],
            ],
            &[&[0.0; 5], &[0.0; 5], &[0.0; 5], &[0.0; 5], &[0.0; 5]],
            &[0.0; 5],
        );
    }
}

#[test]
fn reversed_output_and_sensing_source_keep_their_own_polarities() {
    assert_system(
        &system("CCVS1 0 p Vsense 2\nVsense 0 cp DC 0"),
        &[
            &[0.0, 0.0, -1.0, 0.0],
            &[0.0, 0.0, 0.0, -1.0],
            &[-1.0, 0.0, 0.0, -2.0],
            &[0.0, -1.0, 0.0, 0.0],
        ],
        &[&[0.0; 4], &[0.0; 4], &[0.0; 4], &[0.0; 4]],
        &[0.0; 4],
    );
}

#[test]
fn repeated_output_terminals_cancel_incidence_but_keep_current_control() {
    assert_system(
        &system("CCVS1 p p Vsense 2\nVsense 0 0 DC 0"),
        &[&[0.0; 3], &[0.0, 0.0, -2.0], &[0.0; 3]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0; 3],
    );
}

#[test]
fn self_control_couples_voltage_to_the_same_branch_current() {
    assert_system(
        &system("CCVS1 p 0 CCVS1 2"),
        &[&[0.0, 1.0], &[1.0, -2.0]],
        &[&[0.0; 2], &[0.0; 2]],
        &[0.0; 2],
    );
}

#[test]
fn nonzero_sensing_current_sets_the_loaded_output_voltage() {
    let system = system("CCVS1 out 0 Vsense -2\nR1 out 0 4\nVsense sense 0 DC 0\nI1 0 sense DC 3");
    assert_system(
        &system,
        &[
            &[0.25, 0.0, 1.0, 0.0],
            &[0.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 2.0],
            &[0.0, 1.0, 0.0, 0.0],
        ],
        &[&[0.0; 4], &[0.0; 4], &[0.0; 4], &[0.0; 4]],
        &[0.0, 3.0, 0.0, 0.0],
    );
    let x = DVector::from_vec(vec![-6.0, 0.0, 1.5, 3.0]);
    assert!((&system.g * x - &system.b).norm() < 1e-12);
}
