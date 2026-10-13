use ferrispice_circuit::{CircuitErrorKind, ElementKind, GROUND, compile};
use ferrispice_netlist::{NetlistPosition, parse};

#[test]
fn compiles_all_nine_kinds_and_control_only_nodes() {
    let parsed = parse("All kinds\nVCVS1 out 0 cp cn -2\nVCCS1 out 0 cp cn -1e-3\nCCVS1 out 0 Vsense -100\nCCCS1 out 0 Vsense -3\nVsense sense 0 DC 0\nR1 out 0 -10\nC1 cp 0 0\nL1 cn 0 0\nI1 out 0 DC -1\n.end").unwrap();
    let circuit = compile(&parsed).unwrap();
    let cp = circuit.find_node("cp").unwrap();
    let cn = circuit.find_node("cn").unwrap();
    let control = circuit.find_element("Vsense").unwrap();
    assert_eq!(
        circuit.elements()[0].kind,
        ElementKind::Vcvs {
            control_positive: cp,
            control_negative: cn,
            gain: -2.0
        }
    );
    assert_eq!(
        circuit.elements()[1].kind,
        ElementKind::Vccs {
            control_positive: cp,
            control_negative: cn,
            transconductance: -1e-3
        }
    );
    assert_eq!(
        circuit.elements()[2].kind,
        ElementKind::Ccvs {
            control,
            transresistance: -100.0
        }
    );
    assert_eq!(
        circuit.elements()[3].kind,
        ElementKind::Cccs {
            control,
            gain: -3.0
        }
    );
    assert_eq!(
        circuit.elements()[4].kind,
        ElementKind::VoltageSource { dc: 0.0 }
    );
    assert_eq!(
        circuit.elements()[5].kind,
        ElementKind::Resistor { resistance: -10.0 }
    );
    assert_eq!(
        circuit.elements()[6].kind,
        ElementKind::Capacitor { capacitance: 0.0 }
    );
    assert_eq!(
        circuit.elements()[7].kind,
        ElementKind::Inductor { inductance: 0.0 }
    );
    assert_eq!(
        circuit.elements()[8].kind,
        ElementKind::CurrentSource { dc: -1.0 }
    );
}

#[test]
fn first_appearance_and_ids_are_deterministic() {
    let parsed =
        parse("Ordering\nVCVS1 Out 0 ControlP ControlN 2\nR1 Other out 1\nR2 controln 00 2\n.end")
            .unwrap();
    let expected_names = ["0", "Out", "ControlP", "ControlN", "Other", "00"];
    let first = compile(&parsed).unwrap();
    let expected_ids: Vec<_> = first.nodes().iter().map(|node| node.id).collect();
    for _ in 0..8 {
        let circuit = compile(&parsed).unwrap();
        assert_eq!(
            circuit
                .nodes()
                .iter()
                .map(|node| node.name.as_str())
                .collect::<Vec<_>>(),
            expected_names
        );
        assert_eq!(
            circuit
                .nodes()
                .iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            expected_ids
        );
        assert_ne!(circuit.find_node("00"), Some(GROUND));
        assert_eq!(
            circuit
                .node(circuit.find_node("OUT").unwrap())
                .unwrap()
                .name,
            "Out"
        );
    }
}

#[test]
fn case_insensitive_forward_references_preserve_declared_names() {
    let parsed =
        parse("Forward\nCCCS1 OUT 0 vsense 2\nR1 out 0 100\nVSense Sense 0 DC 0\n.end").unwrap();
    let circuit = compile(&parsed).unwrap();
    let ElementKind::Cccs { control, .. } = circuit.elements()[0].kind else {
        panic!("expected CCCS");
    };
    assert_eq!(circuit.element(control).unwrap().name, "VSense");
    assert_eq!(
        circuit.elements()[0].positive,
        circuit.elements()[1].positive
    );
    assert_eq!(
        circuit.node(circuit.elements()[0].positive).unwrap().name,
        "OUT"
    );
}

#[test]
fn duplicate_elements_include_both_declaration_locations() {
    let error = compile(&parse("Duplicate\nR1 a 0 1\n  r1 b 0 2\n.end").unwrap()).unwrap_err();
    assert_eq!(error.location, NetlistPosition::new(3, 3));
    assert_eq!(error.element, "r1");
    assert_eq!(
        error.kind,
        CircuitErrorKind::DuplicateElement {
            first: NetlistPosition::new(2, 1),
            first_span: ferrispice_netlist::ByteSpan::new("Duplicate\n".len(), "R1 a 0 1".len()),
        }
    );
    assert!(error.to_string().contains("line 2"));
    assert!(error.to_string().contains("line 3"));
}

#[test]
fn unknown_current_controller_is_a_semantic_error() {
    let parsed = parse("Missing\nCCCS1 a 0 absent 2\n.end").unwrap();
    let error = compile(&parsed).unwrap_err();
    assert_eq!(error.location.line, 2);
    assert_eq!(error.element, "CCCS1");
    assert_eq!(
        error.kind,
        CircuitErrorKind::UnknownController {
            name: "absent".into()
        }
    );
}

#[test]
fn manually_constructed_syntax_cannot_introduce_invalid_names() {
    let base = parse("Manual AST\nR1 a 0 1\n.end").unwrap();
    let mut parsed = base.clone();
    parsed.elements[0].name.clear();
    assert!(matches!(
        compile(&parsed).unwrap_err().kind,
        CircuitErrorKind::InvalidName {
            field: "element",
            ..
        }
    ));

    let mut parsed = base;
    parsed.elements[0].positive = "two names".into();
    assert!(matches!(
        compile(&parsed).unwrap_err().kind,
        CircuitErrorKind::InvalidName { field: "node", .. }
    ));
}

#[test]
fn compilation_does_not_guess_whether_a_topology_is_singular() {
    let circuit = compile(&parse("Floating\nR1 a b 100\n.end").unwrap()).unwrap();
    assert_eq!(circuit.non_ground_nodes().count(), 2);
    assert_eq!(circuit.elements().len(), 1);
}

#[test]
fn duplicate_errors_retain_both_declaration_spans() {
    let text = "Title\r\nR1 α 0 100\r\n  r1 β 0 50\r\n.end\r\n";
    let error = compile(&parse(text).unwrap()).unwrap_err();
    assert_eq!(&text[error.span.range()], "r1 β 0 50");
    let CircuitErrorKind::DuplicateElement { first_span, .. } = error.kind else {
        panic!("expected duplicate name");
    };
    assert_eq!(&text[first_span.range()], "R1 α 0 100");
}
