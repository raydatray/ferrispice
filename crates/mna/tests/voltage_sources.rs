mod common;

use common::{assert_system, system};
use ferrispice_mna::assemble;

#[test]
fn floating_source_adds_incidence_and_a_voltage_constraint() {
    assert_system(
        &system("V1 p n DC 6"),
        &[&[0.0, 0.0, 1.0], &[0.0, 0.0, -1.0], &[1.0, -1.0, 0.0]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0, 0.0, 6.0],
    );
}

#[test]
fn reversed_polarity_and_signed_values_change_the_constraint() {
    for (record, incidence, rhs) in [
        ("V1 p 0 DC 6", 1.0, 6.0),
        ("V1 0 p DC 6", -1.0, 6.0),
        ("V1 p 0 DC -6", 1.0, -6.0),
    ] {
        assert_system(
            &system(record),
            &[&[0.0, incidence], &[incidence, 0.0]],
            &[&[0.0; 2], &[0.0; 2]],
            &[0.0, rhs],
        );
    }
}

#[test]
fn repeated_and_grounded_terminals_retain_the_branch_rows() {
    assert_system(
        &system("V1 p p DC 6\nV2 0 0 DC -2"),
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0, 6.0, -2.0],
    );
}

#[test]
fn divider_matches_the_hand_calculated_matrices() {
    let parsed =
        ferrispice_netlist::parse(include_str!("../../../examples/voltage-divider.cir")).unwrap();
    let circuit = ferrispice_circuit::compile(&parsed).unwrap();
    let system = assemble(&circuit).unwrap();
    assert_eq!(
        system.layout.labels().collect::<Vec<_>>(),
        ["V(n1)", "V(n2)", "I(Vin)"]
    );
    assert_system(
        &system,
        &[&[0.01, -0.01, 1.0], &[-0.01, 0.03, 0.0], &[1.0, 0.0, 0.0]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0, 0.0, 6.0],
    );
}

#[test]
fn current_source_load_and_voltage_constraint_share_the_same_system() {
    assert_system(
        &system("R1 p 0 3\nI1 p 0 DC 2\nV1 p 0 DC 6"),
        &[&[1.0 / 3.0, 1.0], &[1.0, 0.0]],
        &[&[0.0; 2], &[0.0; 2]],
        &[-2.0, 6.0],
    );
}
