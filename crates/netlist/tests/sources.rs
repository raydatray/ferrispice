use ferrispice_netlist::{ParseErrorKind, ParsedKind, ParsedWaveform, parse};

#[test]
fn parses_cos_sin_and_dc_sources_with_case_insensitive_modes() {
    let parsed = parse(
        "Sources\nVin n1 0 COS 1 60\nV2 n2 0 cos -2.5 1k\nV3 n3 0 SIN 1.5 120\nI1 n1 0 Cos 1m 50Hz\nI2 n2 0 sin -2m 2k\nI3 n3 0 dc 3\n.end",
    )
    .unwrap();

    let kinds: Vec<_> = parsed
        .elements
        .iter()
        .map(|element| element.kind.clone())
        .collect();

    assert_eq!(
        kinds,
        [
            ParsedKind::VoltageSource {
                waveform: ParsedWaveform::Cos {
                    amplitude: 1.0,
                    frequency: 60.0
                }
            },
            ParsedKind::VoltageSource {
                waveform: ParsedWaveform::Cos {
                    amplitude: -2.5,
                    frequency: 1e3
                }
            },
            ParsedKind::VoltageSource {
                waveform: ParsedWaveform::Sin {
                    amplitude: 1.5,
                    frequency: 120.0
                }
            },
            ParsedKind::CurrentSource {
                waveform: ParsedWaveform::Cos {
                    amplitude: 1e-3,
                    frequency: 50.0
                }
            },
            ParsedKind::CurrentSource {
                waveform: ParsedWaveform::Sin {
                    amplitude: -2e-3,
                    frequency: 2e3
                }
            },
            ParsedKind::CurrentSource {
                waveform: ParsedWaveform::Dc(3.0)
            },
        ]
    );
}

#[test]
fn each_source_mode_has_its_own_field_count() {
    for (record, expected) in [
        ("V1 a 0", "Vname positive negative DC value,"),
        ("I1 a 0 COS", "Iname positive negative DC value,"),
        ("I1 a 0 SIN", "Iname positive negative DC value,"),
        (
            "V1 a 0 COS 1",
            "Vname positive negative COS amplitude frequency",
        ),
        (
            "V1 a 0 SIN 1",
            "Vname positive negative SIN amplitude frequency",
        ),
        (
            "I1 a 0 COS 1 60 0",
            "Iname positive negative COS amplitude frequency",
        ),
        (
            "I1 a 0 SIN 1 60 0",
            "Iname positive negative SIN amplitude frequency",
        ),
        ("V1 a 0 DC 1 60", "Vname positive negative DC value"),
    ] {
        let text = format!("Title\n{record}\n.end");
        let error = parse(&text).unwrap_err();

        assert_eq!(&text[error.span.range()], record);
        let ParseErrorKind::WrongFieldCount {
            expected: shape, ..
        } = error.kind
        else {
            panic!("{record}: {error:?}");
        };
        assert!(shape.starts_with(expected), "{record}: {shape}");
    }
}

#[test]
fn unknown_modes_highlight_the_mode_keyword() {
    for (text, mode) in [
        ("Title\nV1 a 0 PULSE 1 60\n.end", "PULSE"),
        ("Title\nI1 a 0 AC 1\n.end", "AC"),
        ("Title\nV1 a 0 6 1\n.end", "6"),
    ] {
        let error = parse(text).unwrap_err();

        assert_eq!(&text[error.span.range()], mode);
        assert_eq!(
            error.kind,
            ParseErrorKind::UnknownSourceMode {
                found: mode.to_owned()
            }
        );
    }
}

#[test]
fn periodic_source_values_report_their_own_spans() {
    for (text, value) in [
        ("Title\nV1 a 0 COS one 60\n.end", "one"),
        ("Title\nV1 a 0 COS 1 sixty\n.end", "sixty"),
        ("Title\nV1 a 0 COS 1 inf\n.end", "inf"),
        ("Title\nV1 a 0 SIN one 60\n.end", "one"),
        ("Title\nV1 a 0 SIN 1 sixty\n.end", "sixty"),
        ("Title\nV1 a 0 SIN 1 inf\n.end", "inf"),
    ] {
        let error = parse(text).unwrap_err();

        assert_eq!(&text[error.span.range()], value);
    }
}
