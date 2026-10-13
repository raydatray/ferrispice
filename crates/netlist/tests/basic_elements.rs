use ferrispice_netlist::{Analysis, NetlistPosition, ParseErrorKind, ParsedKind, parse};

#[test]
fn parses_the_handout_divider() {
    let parsed =
        parse("Name of circuit\nVin n1 0 DC 6\nR1 n1 n2 100\nR2 n2 0 50\n.dc\n.end").unwrap();

    assert_eq!(parsed.title, "Name of circuit");
    assert_eq!(parsed.elements.len(), 3);
    assert_eq!(parsed.elements[0].name, "Vin");
    assert_eq!(parsed.elements[0].positive, "n1");
    assert_eq!(parsed.elements[0].negative, "0");
    assert_eq!(
        parsed.elements[0].kind,
        ParsedKind::VoltageSource { dc: 6.0 }
    );
    assert_eq!(
        parsed.elements[1].kind,
        ParsedKind::Resistor { resistance: 100.0 }
    );
    assert_eq!(
        parsed.elements[2].kind,
        ParsedKind::Resistor { resistance: 50.0 }
    );
    assert_eq!(parsed.analyses[0].kind, Analysis::Dc);
    assert_eq!(parsed.analyses[0].location, NetlistPosition::new(5, 1));
    assert!(parsed.requests_dc());
}

#[test]
fn parses_passives_and_signed_dc_sources() {
    let parsed =
        parse("Types\nr1 P N -100\nc1 N 0 1e-6\nl1 N M 0\nv1 M 0 dC +6\ni1 N 0 Dc -.5\n.end")
            .unwrap();

    let kinds: Vec<_> = parsed
        .elements
        .iter()
        .map(|element| &element.kind)
        .collect();

    assert_eq!(
        kinds,
        vec![
            &ParsedKind::Resistor { resistance: -100.0 },
            &ParsedKind::Capacitor { capacitance: 1e-6 },
            &ParsedKind::Inductor { inductance: 0.0 },
            &ParsedKind::VoltageSource { dc: 6.0 },
            &ParsedKind::CurrentSource { dc: -0.5 },
        ]
    );
    assert_eq!(parsed.elements[0].positive, "P");
    assert!(!parsed.requests_dc());
}

#[test]
fn accepts_dc_before_between_and_after_elements() {
    for text in [
        "Title\n.dc\nR1 a 0 1\nR2 b 0 2\n.end",
        "Title\nR1 a 0 1\n.dc\nR2 b 0 2\n.end",
        "Title\nR1 a 0 1\nR2 b 0 2\n.DC\n.end",
    ] {
        let parsed = parse(text).unwrap();

        assert!(parsed.requests_dc());
        assert_eq!(parsed.elements.len(), 2);
    }

    assert_eq!(parse("Title\n.dc\n.dc\n.end").unwrap().analyses.len(), 2);
}

#[test]
fn enforces_record_shapes_and_explicit_dc_keyword() {
    for record in [
        "R1 a 0",
        "C1 a 0 1 extra",
        "L1 a 0",
        "V1 a 0 6",
        "I1 a 0 DC 1 extra",
    ] {
        let error = parse(&format!("Title\n{record}\n.end")).unwrap_err();

        assert_eq!(error.location.line, 2);
        assert!(matches!(error.kind, ParseErrorKind::WrongFieldCount { .. }));
    }

    let error = parse("Title\nV1 a 0 AC 1\n.end").unwrap_err();

    assert_eq!(error.location, NetlistPosition::new(2, 8));
    assert!(matches!(error.kind, ParseErrorKind::ExpectedDc { .. }));
}

#[test]
fn invalid_numbers_point_to_the_value_token() {
    let error = parse("Title\n  R1 α 0 nope\n.end").unwrap_err();

    assert_eq!(error.location, NetlistPosition::new(2, 10));
    assert!(matches!(error.kind, ParseErrorKind::InvalidNumber { .. }));
    assert!(error.to_string().contains("nope"));
}

#[test]
fn rejects_unimplemented_elements_instead_of_dropping_them() {
    let error = parse("Title\nD1 a 0 model\n.end").unwrap_err();

    assert!(matches!(
        error.kind,
        ParseErrorKind::UnsupportedElement { .. }
    ));
}
