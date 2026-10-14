use ferrispice_circuit::{
    CircuitError, ElementError, ElementErrorKind, ElementKind, GROUND, compile,
};
use ferrispice_netlist::{NetlistPosition, ParsedKind, ParsedNetlist, parse};

fn element_error(parsed: &ParsedNetlist) -> ElementError {
    match compile(parsed).unwrap_err() {
        CircuitError::Element(error) => error,
        other => panic!("expected an element error, found {other:?}"),
    }
}

#[test]
fn divider_runs_from_netlist_to_validated_circuit() {
    let parsed = parse(include_str!("../../../examples/voltage-divider.cir")).unwrap();
    assert!(parsed.requests_dc());
    let circuit = compile(&parsed).unwrap();

    assert_eq!(circuit.title(), "Name of circuit");
    assert_eq!(
        circuit
            .nodes()
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        ["0", "n1", "n2"]
    );
    assert_eq!(circuit.non_ground_nodes().count(), 2);
    assert_eq!(
        circuit
            .elements()
            .iter()
            .map(|element| element.name.as_str())
            .collect::<Vec<_>>(),
        ["Vin", "R1", "R2"]
    );
    let n1 = circuit.find_node("N1").unwrap();
    let n2 = circuit.find_node("n2").unwrap();
    assert_eq!(circuit.find_node("0"), Some(GROUND));
    assert!(GROUND.is_ground());
    assert!(!n1.is_ground());

    let voltage = circuit
        .element(circuit.find_element("vin").unwrap())
        .unwrap();
    assert_eq!(voltage.positive, n1);
    assert_eq!(voltage.negative, GROUND);
    assert_eq!(voltage.kind, ElementKind::VoltageSource { dc: 6.0 });
    assert!(voltage.kind.has_branch_current());

    let resistor = &circuit.elements()[1];
    assert_eq!(resistor.positive, n1);
    assert_eq!(resistor.negative, n2);
    assert_eq!(resistor.kind, ElementKind::Resistor { resistance: 100.0 });
    assert!(!resistor.kind.has_branch_current());
    assert_eq!(resistor.location, NetlistPosition::new(3, 1));
    assert_eq!(circuit.find_node("missing"), None);
    assert_eq!(circuit.find_element("missing"), None);
}

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
    let error = element_error(&parse("Duplicate\nR1 a 0 1\n  r1 b 0 2\n.end").unwrap());
    assert_eq!(error.location, NetlistPosition::new(3, 3));
    assert_eq!(error.element, "r1");
    assert_eq!(
        error.kind,
        ElementErrorKind::DuplicateElement {
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
    let error = element_error(&parsed);
    assert_eq!(error.location.line, 2);
    assert_eq!(error.element, "CCCS1");
    assert_eq!(
        error.kind,
        ElementErrorKind::UnknownController {
            name: "absent".into()
        }
    );
}

#[test]
fn rejects_controller_kinds_without_explicit_branch_currents() {
    for (controller, name) in [
        ("R1 b 0 1", "R1"),
        ("C1 b 0 1", "C1"),
        ("I1 b 0 DC 1", "I1"),
        ("VCCS1 b 0 b 0 1", "VCCS1"),
        ("CCCS2 b 0 Vsense 1", "CCCS2"),
    ] {
        let parsed = parse(&format!(
            "Controller\nCCCS1 a 0 {name} 2\n{controller}\nVsense b 0 DC 0\n.end"
        ))
        .unwrap();
        let error = element_error(&parsed);
        assert_eq!(error.element, "CCCS1");
        assert!(matches!(
            error.kind,
            ElementErrorKind::UnsupportedController { .. }
        ));
    }
}

#[test]
fn accepts_each_agreed_branch_current_controller_kind() {
    for (controller, name) in [
        ("V1 b 0 DC 0", "V1"),
        ("L1 b 0 1", "L1"),
        ("VCVS1 b 0 b 0 1", "VCVS1"),
        ("CCVS1 b 0 Vsense 1", "CCVS1"),
    ] {
        let parsed = parse(&format!(
            "Controller\nCCCS1 a 0 {name} 2\n{controller}\nVsense b 0 DC 0\n.end"
        ))
        .unwrap();
        let circuit = compile(&parsed).unwrap();
        let ElementKind::Cccs { control, .. } = circuit.elements()[0].kind else {
            panic!("expected CCCS");
        };
        assert_eq!(circuit.element(control).unwrap().name, name);
    }
}

#[test]
fn rejects_zero_resistance_and_negative_storage_parameters() {
    for record in ["R1 a 0 0", "R1 a 0 -0"] {
        let error = element_error(&parse(&format!("Invalid\n{record}\n.end")).unwrap());
        assert_eq!(error.kind, ElementErrorKind::ZeroResistance);
    }
    for record in ["C1 a 0 -1e-6", "L1 a 0 -1"] {
        let error = element_error(&parse(&format!("Invalid\n{record}\n.end")).unwrap());
        assert!(matches!(
            error.kind,
            ElementErrorKind::NegativeParameter { .. }
        ));
    }
}

#[test]
fn manually_constructed_syntax_cannot_bypass_finite_parameter_validation() {
    let base = parse("Manual AST\nR1 a 0 1\nVsense b 0 DC 0\n.end").unwrap();
    for kind in [
        ParsedKind::Resistor {
            resistance: f64::NAN,
        },
        ParsedKind::Capacitor {
            capacitance: f64::INFINITY,
        },
        ParsedKind::Inductor {
            inductance: f64::NEG_INFINITY,
        },
        ParsedKind::VoltageSource { dc: f64::NAN },
        ParsedKind::CurrentSource { dc: f64::NAN },
        ParsedKind::Vcvs {
            control_positive: "b".into(),
            control_negative: "0".into(),
            gain: f64::NAN,
        },
        ParsedKind::Vccs {
            control_positive: "b".into(),
            control_negative: "0".into(),
            transconductance: f64::NAN,
        },
        ParsedKind::Ccvs {
            control: "Vsense".into(),
            transresistance: f64::NAN,
        },
        ParsedKind::Cccs {
            control: "Vsense".into(),
            gain: f64::NAN,
        },
    ] {
        let mut parsed = base.clone();
        parsed.elements[0].kind = kind;
        let error = element_error(&parsed);
        assert_eq!(error.location.line, 2);
        assert!(matches!(
            error.kind,
            ElementErrorKind::NonFiniteParameter { .. }
        ));
    }
}

#[test]
fn manually_constructed_syntax_cannot_introduce_invalid_names() {
    let base = parse("Manual AST\nR1 a 0 1\n.end").unwrap();
    let mut parsed = base.clone();
    parsed.elements[0].name.clear();
    assert!(matches!(
        element_error(&parsed).kind,
        ElementErrorKind::InvalidName {
            field: "element",
            ..
        }
    ));

    let mut parsed = base;
    parsed.elements[0].positive = "two names".into();
    assert!(matches!(
        element_error(&parsed).kind,
        ElementErrorKind::InvalidName { field: "node", .. }
    ));
}

#[test]
fn compilation_does_not_guess_whether_a_grounded_topology_is_singular() {
    let circuit = compile(&parse("Floating\nR1 a b 100\nR2 c 0 1\n.end").unwrap()).unwrap();
    assert_eq!(circuit.non_ground_nodes().count(), 3);
    assert_eq!(circuit.elements().len(), 2);
}

#[test]
fn rejects_circuits_without_a_ground_terminal() {
    for records in [
        "R1 a b 100",
        "R1 a b 100\nV1 b c DC 1",
        // Sensing ground through a control port does not reference the circuit to it.
        "R1 a b 1\nVCCS1 a b c 0 1\nVCVS1 c d 0 a 2",
    ] {
        let error = compile(&parse(&format!("Ungrounded\n{records}\n.end")).unwrap()).unwrap_err();
        assert_eq!(error, CircuitError::MissingGround, "{records}");
        assert!(error.to_string().contains("ground node 0"), "{records}");
    }
}

#[test]
fn any_ground_terminal_satisfies_the_reference_requirement() {
    for records in ["R1 a 0 1", "I1 0 a DC 1\nR1 a b 1", "R1 0 0 1\nR2 a b 1"] {
        compile(&parse(&format!("Grounded\n{records}\n.end")).unwrap())
            .unwrap_or_else(|error| panic!("{records}: {error}"));
    }
}

#[test]
fn an_empty_circuit_needs_no_ground_terminal() {
    let circuit = compile(&parse("Empty\n.end").unwrap()).unwrap();
    assert_eq!(circuit.nodes().len(), 1);
}

#[test]
fn element_errors_take_precedence_over_a_missing_ground() {
    let error = element_error(&parse("Both\nR1 a b 0\n.end").unwrap());
    assert_eq!(error.kind, ElementErrorKind::ZeroResistance);
}

#[test]
fn semantic_errors_and_elements_preserve_their_record_spans() {
    for (text, record) in [
        (
            "Title\r\n  CCCS1 α 0 missing 2\r\n.end\r\n",
            "CCCS1 α 0 missing 2",
        ),
        ("Title\r\n  R1 α 0 0\r\n.end\r\n", "R1 α 0 0"),
    ] {
        let error = element_error(&parse(text).unwrap());
        assert_eq!(error.location, NetlistPosition::new(2, 3));
        assert_eq!(&text[error.span.range()], record);
    }

    let text = "Title\r\n  R1 α 0 100\r\n.end\r\n";
    let circuit = compile(&parse(text).unwrap()).unwrap();
    assert_eq!(&text[circuit.elements()[0].span.range()], "R1 α 0 100");
}

#[test]
fn duplicate_errors_retain_both_declaration_spans() {
    let text = "Title\r\nR1 α 0 100\r\n  r1 β 0 50\r\n.end\r\n";
    let error = element_error(&parse(text).unwrap());
    assert_eq!(&text[error.span.range()], "r1 β 0 50");
    let ElementErrorKind::DuplicateElement { first_span, .. } = error.kind else {
        panic!("expected duplicate name");
    };
    assert_eq!(&text[first_span.range()], "R1 α 0 100");
}
