use ferrispice_netlist::{NetlistPosition, ParseErrorKind, ParsedKind, parse};

const HANDOUT: &str = "* diode circuit\nVin n1 0 DC 1\nD1 n1 n2\nR n2 0 50\n.dc\n.end\n";

#[test]
fn parses_the_handout_diode_circuit() {
    let parsed = parse(HANDOUT).unwrap();

    // the first physical line is the title even though it looks like a comment
    assert_eq!(parsed.title, "* diode circuit");
    assert_eq!(parsed.elements.len(), 3);

    let diode = &parsed.elements[1];
    assert_eq!(diode.name, "D1");
    assert_eq!(diode.positive, "n1");
    assert_eq!(diode.negative, "n2");
    assert_eq!(diode.kind, ParsedKind::Diode);
    assert_eq!(diode.location, NetlistPosition::new(3, 1));

    // a bare prefix is a complete element name
    assert_eq!(parsed.elements[2].name, "R");
    assert_eq!(
        parsed.elements[2].kind,
        ParsedKind::Resistor { resistance: 50.0 }
    );
    assert!(parsed.requests_dc());
}

#[test]
fn diode_prefix_is_case_insensitive_and_keeps_terminal_order() {
    let parsed = parse("Title\nd1 a 0\nDreverse 0 a\nD b c\n.end").unwrap();

    let terminals: Vec<_> = parsed
        .elements
        .iter()
        .map(|element| {
            assert_eq!(element.kind, ParsedKind::Diode);
            (
                element.name.as_str(),
                element.positive.as_str(),
                element.negative.as_str(),
            )
        })
        .collect();

    assert_eq!(
        terminals,
        [("d1", "a", "0"), ("Dreverse", "0", "a"), ("D", "b", "c")]
    );
}

#[test]
fn diode_records_have_exactly_two_nodes() {
    for (record, found) in [("D1 a", 2), ("D1 a 0 model", 4), ("D1 a 0 1e-14 0.025", 5)] {
        let text = format!("Title\n{record}\n.end");
        let error = parse(&text).unwrap_err();

        assert_eq!(error.location, NetlistPosition::new(2, 1));
        assert_eq!(
            error.kind,
            ParseErrorKind::WrongFieldCount {
                expected: "Dname anode cathode",
                found,
            }
        );
        assert_eq!(&text[error.span.range()], record);
    }
}

#[test]
fn diode_spans_slice_the_original_input_with_crlf() {
    let text = "* diode circuit\r\n  D1 α n2  \r\n.end\r\n";
    let parsed = parse(text).unwrap();

    assert_eq!(&text[parsed.elements[0].span.range()], "D1 α n2");
    assert_eq!(parsed.elements[0].location, NetlistPosition::new(2, 3));

    let text = "Title\r\n  D1 α\r\n.end\r\n";
    let error = parse(text).unwrap_err();

    assert_eq!(&text[error.span.range()], "D1 α");
}
