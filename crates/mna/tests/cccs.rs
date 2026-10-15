mod common;

use common::{assert_system, system};
use nalgebra::DVector;

#[test]
fn forward_controller_current_drives_kcl_without_an_output_branch() {
    for gain in [-2.0, 0.0, 2.0] {
        let system = system(&format!("CCCS1 p n Vsense {gain}\nVsense cp 0 DC 0"));
        assert_eq!(
            system.layout.labels().collect::<Vec<_>>(),
            ["V(p)", "V(n)", "V(cp)", "I(Vsense)"]
        );
        assert_system(
            &system,
            &[
                &[0.0, 0.0, 0.0, gain],
                &[0.0, 0.0, 0.0, -gain],
                &[0.0, 0.0, 0.0, 1.0],
                &[0.0, 0.0, 1.0, 0.0],
            ],
            &[&[0.0; 4], &[0.0; 4], &[0.0; 4], &[0.0; 4]],
            &[0.0; 4],
        );
    }
}

#[test]
fn reversed_output_and_sensing_source_keep_current_polarity() {
    assert_system(
        &system("CCCS1 0 p Vsense 2\nVsense 0 cp DC 0"),
        &[&[0.0, 0.0, -2.0], &[0.0, 0.0, -1.0], &[0.0, -1.0, 0.0]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0; 3],
    );
}

#[test]
fn repeated_or_grounded_output_terminals_cancel_controlled_kcl() {
    assert_system(
        &system("CCCS1 p p Vsense 2\nVsense cp 0 DC 0"),
        &[&[0.0; 3], &[0.0, 0.0, 1.0], &[0.0, 1.0, 0.0]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0; 3],
    );
    assert_system(
        &system("CCCS1 0 0 Vsense 2\nVsense cp 0 DC 0"),
        &[&[0.0, 1.0], &[1.0, 0.0]],
        &[&[0.0; 2], &[0.0; 2]],
        &[0.0; 2],
    );
}

#[test]
fn multiple_controls_accumulate_with_the_controllers_own_incidence() {
    assert_system(
        &system("CCCS1 p 0 Vsense 2\nCCCS2 p 0 Vsense -0.5\nVsense p 0 DC 0\nR1 p 0 4"),
        &[&[0.25, 2.5], &[1.0, 0.0]],
        &[&[0.0; 2], &[0.0; 2]],
        &[0.0; 2],
    );
}

#[test]
fn nonzero_sensing_current_satisfies_loaded_output_kcl() {
    let system = system("CCCS1 out 0 Vsense -2\nR1 out 0 4\nVsense sense 0 DC 0\nI1 0 sense DC 3");
    assert_system(
        &system,
        &[&[0.25, 0.0, -2.0], &[0.0, 0.0, 1.0], &[0.0, 1.0, 0.0]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0, 3.0, 0.0],
    );
    let x = DVector::from_vec(vec![24.0, 0.0, 3.0]);
    assert!((&system.g * x - &system.b).norm() < 1e-12);
}
