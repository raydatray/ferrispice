use ferrispice_netlist::{ParseErrorKind, ParsedKind, parse};

#[test]
fn parses_all_four_controlled_source_forms() {
    let parsed = parse("Controlled\nVCVS1 out 0 cp cn -2\nVCCS1 out 0 cp cn 1e-3\nCCVS1 out 0 Vsense -100\nCCCS1 out 0 Vsense 2\n.end").unwrap();

    assert_eq!(parsed.elements.len(), 4);
    assert_eq!(
        parsed.elements[0].kind,
        ParsedKind::Vcvs {
            control_positive: "cp".into(),
            control_negative: "cn".into(),
            gain: -2.0,
        }
    );
    assert_eq!(
        parsed.elements[1].kind,
        ParsedKind::Vccs {
            control_positive: "cp".into(),
            control_negative: "cn".into(),
            transconductance: 1e-3,
        }
    );
    assert_eq!(
        parsed.elements[2].kind,
        ParsedKind::Ccvs {
            control: "Vsense".into(),
            transresistance: -100.0,
        }
    );
    assert_eq!(
        parsed.elements[3].kind,
        ParsedKind::Cccs {
            control: "Vsense".into(),
            gain: 2.0,
        }
    );

    for element in &parsed.elements {
        assert_eq!(element.positive, "out");
        assert_eq!(element.negative, "0");
    }
}

#[test]
fn long_prefixes_take_precedence_and_preserve_spelling() {
    let parsed = parse("Case\nvcvsOne A B C D 2\nvCcSOne A B C D 3\ncCvSOne A B Vctrl 4\ncccsOne A B Vctrl 5\n.end").unwrap();

    assert_eq!(parsed.elements[0].name, "vcvsOne");
    assert!(matches!(parsed.elements[0].kind, ParsedKind::Vcvs { .. }));
    assert!(matches!(parsed.elements[1].kind, ParsedKind::Vccs { .. }));
    assert!(matches!(parsed.elements[2].kind, ParsedKind::Ccvs { .. }));
    assert!(matches!(parsed.elements[3].kind, ParsedKind::Cccs { .. }));
}

#[test]
fn references_are_syntax_until_circuit_compilation() {
    let parsed = parse("Reference\nCCCS1 out 0 missing 2\n.end").unwrap();

    assert_eq!(
        parsed.elements[0].kind,
        ParsedKind::Cccs {
            control: "missing".into(),
            gain: 2.0,
        }
    );
}

#[test]
fn validates_controlled_record_arity_without_falling_back_to_v_or_c() {
    for record in [
        "VCVS1 a 0 b 0",
        "VCVS1 a 0 b 0 1 extra",
        "VCCS1 a 0 b",
        "VCCS1 a 0 b 0 1 extra",
        "CCVS1 a 0 Vsense",
        "CCVS1 a 0 Vsense 1 extra",
        "CCCS1 a 0 Vsense",
        "CCCS1 a 0 Vsense 1 extra",
    ] {
        let error = parse(&format!("Title\n{record}\n.end")).unwrap_err();

        assert_eq!(error.location.line, 2);
        assert!(matches!(error.kind, ParseErrorKind::WrongFieldCount { .. }));
    }
}

#[test]
fn rejects_bad_controlled_parameters_with_source_context() {
    let error = parse("Title\nVCCS1 a 0 b 0 invalid\n.end").unwrap_err();

    assert_eq!(error.location.line, 2);
    assert!(matches!(error.kind, ParseErrorKind::InvalidNumber { .. }));
    assert!(error.to_string().contains("invalid"));

    let error = parse("Title\nCCCS1 a 0 Vsense NaN\n.end").unwrap_err();

    assert!(matches!(error.kind, ParseErrorKind::NonFiniteNumber { .. }));
}

#[test]
fn controlled_source_errors_and_records_preserve_exact_ranges() {
    for record in [
        "VCVS1 α 0 b 0 invalid",
        "VCCS1 α 0 b 0 invalid",
        "CCVS1 α 0 Vsense invalid",
        "CCCS1 α 0 Vsense invalid",
    ] {
        let text = format!("Title\r\n  {record}\r\n.end\r\n");
        let error = parse(&text).unwrap_err();

        assert_eq!(&text[error.span.range()], "invalid");
    }
    let text = "Title\r\n  CCCS1 out 0 Vsense 2\r\n.end\r\n";
    let parsed = parse(text).unwrap();

    assert_eq!(
        &text[parsed.elements[0].span.range()],
        "CCCS1 out 0 Vsense 2"
    );
}
