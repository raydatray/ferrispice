use ferrispice_circuit::{GROUND, compile};
use ferrispice_mna::{AssemblyErrorKind, MatrixBuilder, UnknownKind, UnknownLayout, assemble};
use ferrispice_netlist::parse;
use nalgebra::{DMatrix, DVector};

#[test]
fn layout_orders_nodes_before_all_explicit_currents() {
    let circuit = compile(&parse("Layout\nCCCS1 Out 0 Vsense 2\nVCVS1 Out 0 CP CN 3\nVCCS1 Out 0 CP CN 4\nCCVS1 Out 0 Vsense 5\nR1 Out 0 10\nC1 CP 0 1\nL1 CN 0 2\nI1 Out 0 DC 1\nVsense Sense 0 DC 0\n.end").unwrap()).unwrap();
    let layout = UnknownLayout::new(&circuit);
    assert_eq!(
        layout.labels().collect::<Vec<_>>(),
        [
            "V(Out)",
            "V(CP)",
            "V(CN)",
            "V(Sense)",
            "I(VCVS1)",
            "I(CCVS1)",
            "I(L1)",
            "I(Vsense)"
        ]
    );
    assert_eq!(layout.len(), 8);
    assert!(!layout.is_empty());
    assert_eq!(layout.node_index(GROUND), None);
    for (index, node) in circuit.non_ground_nodes().enumerate() {
        assert_eq!(layout.node_index(node.id), Some(index));
        assert_eq!(
            layout.unknowns()[index].kind,
            UnknownKind::NodeVoltage(node.id)
        );
    }
    for (name, index) in [("VCVS1", 4), ("CCVS1", 5), ("L1", 6), ("Vsense", 7)] {
        let id = circuit.find_element(name).unwrap();
        assert_eq!(layout.branch_index(id), Some(index));
        assert_eq!(
            layout.unknowns()[index].kind,
            UnknownKind::BranchCurrent(id)
        );
    }
    for name in ["CCCS1", "VCCS1", "R1", "C1", "I1"] {
        assert_eq!(
            layout.branch_index(circuit.find_element(name).unwrap()),
            None
        );
    }
    assert_eq!(UnknownLayout::new(&circuit), layout);
}

#[test]
fn empty_circuit_has_no_unknowns_or_ground_equation() {
    let circuit = compile(&parse("Empty\n.end").unwrap()).unwrap();
    let system = assemble(&circuit).unwrap();
    assert!(system.layout.is_empty());
    assert_eq!(system.g.shape(), (0, 0));
    assert_eq!(system.c.shape(), (0, 0));
    assert_eq!(system.b.len(), 0);
}

#[test]
fn builder_accumulates_incidence_storage_and_rhs_without_ground_entries() {
    let circuit = compile(&parse("Builder\nV1 p n DC 0\nR1 n 0 1\n.end").unwrap()).unwrap();
    let mut builder = MatrixBuilder::new(UnknownLayout::new(&circuit));
    let p = builder.node_index(circuit.find_node("p").unwrap());
    let n = builder.node_index(circuit.find_node("n").unwrap());
    let ground = builder.node_index(GROUND);
    let branch = builder
        .branch_index(&circuit.elements()[0], circuit.elements()[0].id)
        .unwrap();
    builder.add_g_two_terminal(p, n, 2.0);
    builder.add_g_two_terminal(p, n, 3.0);
    builder.add_branch_connections(p, n, branch);
    builder.add_g_two_terminal(ground, ground, 100.0);
    builder.add_c_two_terminal(p, p, 4.0);
    builder.add_c_two_terminal(p, ground, 5.0);
    builder.add_c(ground, n, 100.0);
    builder.add_b(p, -3.0);
    builder.add_b(n, 3.0);
    builder.add_b(Some(branch), 6.0);
    builder.add_b(ground, 100.0);
    let system = builder.finish();
    assert_eq!(
        system.g,
        DMatrix::from_row_slice(3, 3, &[5.0, -5.0, 1.0, -5.0, 5.0, -1.0, 1.0, -1.0, 0.0])
    );
    assert_eq!(
        system.c,
        DMatrix::from_diagonal(&DVector::from_vec(vec![5.0, 0.0, 0.0]))
    );
    assert_eq!(system.b, DVector::from_vec(vec![-3.0, 3.0, 6.0]));
}

#[test]
fn missing_branch_lookup_is_an_element_error() {
    let circuit = compile(&parse("Missing branch\nR1 p 0 10\n.end").unwrap()).unwrap();
    let element = &circuit.elements()[0];
    let builder = MatrixBuilder::new(UnknownLayout::new(&circuit));
    let error = builder.branch_index(element, element.id).unwrap_err();
    assert_eq!(error.element, "R1");
    assert_eq!(error.location, element.location);
    assert_eq!(error.span, element.span);
    assert_eq!(
        error.kind,
        AssemblyErrorKind::MissingBranchCurrent { branch: element.id }
    );
}

#[test]
fn unimplemented_elements_are_not_silently_ignored() {
    let circuit = compile(&parse("Unsupported\nC1 p 0 1\n.end").unwrap()).unwrap();
    let error = assemble(&circuit).unwrap_err();
    assert_eq!(error.element, "C1");
    assert_eq!(error.span, circuit.elements()[0].span);
    assert_eq!(
        error.kind,
        AssemblyErrorKind::UnsupportedElement { kind: "capacitor" }
    );
}
