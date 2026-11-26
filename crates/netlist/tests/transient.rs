use ferrispice_netlist::{
    Analysis, NetlistPosition, ParseErrorKind, ParsedKind, ParsedWaveform, parse,
};

const HANDOUT: &str = "* a half-wave recitifier circuit
Vin n1 0 COS 1 60
* Vin is a cosine with a 1V amplitude and 60Hz frequency
D1 n1 n2
R1 n2 0 50
C1 n2 0 100e-6
.tran 0.05s  0.1ms
* run transient simulation for 0.05s with step size of 0.1ms
.end
";

#[test]
fn parses_the_handout_rectifier_verbatim() {
    let parsed = parse(HANDOUT).unwrap();

    assert_eq!(parsed.title, "* a half-wave recitifier circuit");
    let names: Vec<_> = parsed
        .elements
        .iter()
        .map(|element| element.name.as_str())
        .collect();
    assert_eq!(names, ["Vin", "D1", "R1", "C1"]);
    assert_eq!(
        parsed.elements[0].kind,
        ParsedKind::VoltageSource {
            waveform: ParsedWaveform::Cos {
                amplitude: 1.0,
                frequency: 60.0
            }
        }
    );
    assert_eq!(
        parsed.elements[3].kind,
        ParsedKind::Capacitor { capacitance: 1e-4 }
    );

    assert_eq!(parsed.analyses.len(), 1);
    assert_eq!(
        parsed.analyses[0].kind,
        Analysis::Tran {
            stop: 0.05,
            step: 1e-4
        }
    );
    assert_eq!(parsed.analyses[0].location, NetlistPosition::new(7, 1));
    assert_eq!(
        &HANDOUT[parsed.analyses[0].span.range()],
        ".tran 0.05s  0.1ms"
    );
    assert_eq!(parsed.transient_request(), Some((0.05, 1e-4)));
    assert!(!parsed.requests_dc());
}

#[test]
fn dc_and_tran_may_both_be_requested_in_any_order() {
    for text in [
        "Title\nR1 a 0 1\n.dc\n.TRAN 1m 1u\n.end",
        "Title\n.tran 1m 1u\nR1 a 0 1\n.dc\n.end",
    ] {
        let parsed = parse(text).unwrap();

        assert!(parsed.requests_dc(), "{text}");
        assert_eq!(parsed.transient_request(), Some((1e-3, 1e-6)), "{text}");
    }

    assert_eq!(parse("Title\n.dc\n.end").unwrap().transient_request(), None);
}

#[test]
fn a_second_tran_points_at_itself_and_names_the_first() {
    let text = "Title\n.tran 1 0.1\nR1 a 0 1\n  .tran 1 0.1\n.end";
    let error = parse(text).unwrap_err();

    assert_eq!(error.location, NetlistPosition::new(4, 3));
    assert_eq!(&text[error.span.range()], ".tran 1 0.1");
    assert_eq!(
        error.kind,
        ParseErrorKind::DuplicateTransient {
            first: NetlistPosition::new(2, 1)
        }
    );
}

#[test]
fn tran_values_report_their_own_spans() {
    for (text, value) in [
        ("Title\n.tran soon 1m\n.end", "soon"),
        ("Title\n.tran 1 1k5\n.end", "1k5"),
        ("Title\n.tran 1 NaN\n.end", "NaN"),
    ] {
        let error = parse(text).unwrap_err();

        assert_eq!(&text[error.span.range()], value);
    }
}

#[test]
fn tran_needs_exactly_a_stop_time_and_a_step() {
    for record in [".tran", ".tran 1", ".tran 1 0.1 0"] {
        let text = format!("Title\n{record}\n.end");
        let error = parse(&text).unwrap_err();

        assert_eq!(&text[error.span.range()], record);
        assert_eq!(
            error.kind,
            ParseErrorKind::WrongFieldCount {
                expected: ".tran tstop tstep",
                found: record.split_whitespace().count(),
            }
        );
    }
}
