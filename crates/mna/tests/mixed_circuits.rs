mod common;

use common::{assert_system, circuit, system};
use ferrispice_mna::assemble;
use nalgebra::DVector;

#[test]
fn all_nine_elements_satisfy_the_full_dynamic_equation() {
    let system = system(
        "R1 a b 1\nC1 a b 3\nL1 b 0 4\nVin a 0 DC 5\nI1 a b DC 2\nVCVS1 c 0 a b 2\nVCCS1 a b c 0 3\nCCVS1 d 0 L1 -2\nCCCS1 a b Vin -1",
    );
    assert_eq!(
        system.layout.labels().collect::<Vec<_>>(),
        [
            "V(a)", "V(b)", "V(c)", "V(d)", "I(L1)", "I(Vin)", "I(VCVS1)", "I(CCVS1)"
        ]
    );
    assert_system(
        &system,
        &[
            &[1.0, -1.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            &[-1.0, 1.0, -3.0, 0.0, 1.0, 1.0, 0.0, 0.0],
            &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
            &[0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            &[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            &[-2.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            &[0.0, 0.0, 0.0, 1.0, 2.0, 0.0, 0.0, 0.0],
        ],
        &[
            &[3.0, -3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            &[-3.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            &[0.0; 8],
            &[0.0; 8],
            &[0.0, 0.0, 0.0, 0.0, -4.0, 0.0, 0.0, 0.0],
            &[0.0; 8],
            &[0.0; 8],
            &[0.0; 8],
        ],
        &[-2.0, 2.0, 0.0, 0.0, 0.0, 5.0, 0.0, 0.0],
    );

    // va=5, vb=4; vc=2(va-vb); vd=-2*iL; iL=-iVin=1.
    // d(va-vb)/dt=-3 and diL/dt=1 satisfy capacitor KCL and vL=L*diL/dt.
    let x = DVector::from_vec(vec![5.0, 4.0, 2.0, -2.0, 1.0, -1.0, 0.0, 0.0]);
    let derivative = DVector::from_vec(vec![0.0, 3.0, -6.0, -2.0, 1.0, -1.0, 0.0, 0.0]);
    assert!((&system.g * x + &system.c * derivative - &system.b).norm() < 1e-12);
}

#[test]
fn grounding_all_terminals_keeps_only_explicit_currents_and_their_constraints() {
    let system = system(
        "R1 0 0 1\nC1 0 0 2\nL1 0 0 3\nV1 0 0 DC 0\nI1 0 0 DC 4\nVCVS1 0 0 0 0 5\nVCCS1 0 0 0 0 6\nCCVS1 0 0 V1 7\nCCCS1 0 0 V1 8",
    );
    assert_eq!(
        system.layout.labels().collect::<Vec<_>>(),
        ["I(L1)", "I(V1)", "I(VCVS1)", "I(CCVS1)"]
    );
    assert_system(
        &system,
        &[&[0.0; 4], &[0.0; 4], &[0.0; 4], &[0.0, -7.0, 0.0, 0.0]],
        &[&[-3.0, 0.0, 0.0, 0.0], &[0.0; 4], &[0.0; 4], &[0.0; 4]],
        &[0.0; 4],
    );
}

#[test]
fn every_explicit_branch_kind_can_supply_a_forward_current_control() {
    for (name, record) in [
        ("Vcontrol", "Vcontrol p 0 DC 0"),
        ("Lcontrol", "Lcontrol p 0 1"),
        ("VCVScontrol", "VCVScontrol p 0 p 0 1"),
        ("CCVScontrol", "CCVScontrol p 0 Vref 2\nVref p 0 DC 0"),
    ] {
        let circuit = circuit(&format!("CCCS1 out 0 {name} 2\n{record}"));
        let system = assemble(&circuit).unwrap();
        assert_eq!(
            system
                .layout
                .branch_index(circuit.find_element(name).unwrap()),
            Some(2)
        );
        assert_eq!(system.g[(0, 2)], 2.0);
        assert_eq!(
            system
                .layout
                .branch_index(circuit.find_element("CCCS1").unwrap()),
            None
        );
    }
}

#[test]
fn singular_and_incompatible_topologies_are_still_assembled() {
    assert_system(
        &system("R1 a b 10"),
        &[&[0.1, -0.1], &[-0.1, 0.1]],
        &[&[0.0; 2], &[0.0; 2]],
        &[0.0; 2],
    );
    assert_system(
        &system("V1 p 0 DC 1\nV2 p 0 DC 2"),
        &[&[0.0, 1.0, 1.0], &[1.0, 0.0, 0.0], &[1.0, 0.0, 0.0]],
        &[&[0.0; 3], &[0.0; 3], &[0.0; 3]],
        &[0.0, 1.0, 2.0],
    );
}
