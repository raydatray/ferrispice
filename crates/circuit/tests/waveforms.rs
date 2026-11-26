use ferrispice_circuit::{
    CircuitError, ElementErrorKind, ElementKind, TransientRequestErrorKind, Waveform, compile,
};
use ferrispice_netlist::{ParsedKind, ParsedWaveform, parse};

const COS_1_60: Waveform = Waveform::Cos {
    amplitude: 1.0,
    frequency: 60.0,
};

const SIN_1_60: Waveform = Waveform::Sin {
    amplitude: 1.0,
    frequency: 60.0,
};

#[test]
fn cos_and_sin_waveforms_follow_amplitude_times_trig() {
    assert_eq!(COS_1_60.value_at(0.0), 1.0);
    assert!((COS_1_60.value_at(1.0 / 120.0) + 1.0).abs() < 1e-12);
    assert!(COS_1_60.value_at(1.0 / 240.0).abs() < 1e-12);
    assert!((COS_1_60.value_at(1.0 / 60.0) - 1.0).abs() < 1e-12);

    assert_eq!(SIN_1_60.value_at(0.0), 0.0);
    assert!((SIN_1_60.value_at(1.0 / 240.0) - 1.0).abs() < 1e-12);
    assert!(SIN_1_60.value_at(1.0 / 120.0).abs() < 1e-12);
    assert!((SIN_1_60.value_at(3.0 / 240.0) + 1.0).abs() < 1e-12);
    assert!(SIN_1_60.value_at(1.0 / 60.0).abs() < 1e-12);

    let scaled_cos = Waveform::Cos {
        amplitude: -2.0,
        frequency: 1e3,
    };
    assert!((scaled_cos.value_at(1e-3 / 3.0) - 1.0).abs() < 1e-12);

    let scaled_sin = Waveform::Sin {
        amplitude: -2.0,
        frequency: 1e3,
    };
    assert!((scaled_sin.value_at(1e-3 / 12.0) + 1.0).abs() < 1e-12);
}

#[test]
fn dc_analysis_uses_the_value_at_time_zero() {
    assert_eq!(COS_1_60.dc_value(), 1.0);
    assert_eq!(SIN_1_60.dc_value(), 0.0);
    assert_eq!(Waveform::Dc(-3.5).dc_value(), -3.5);
    assert_eq!(Waveform::Dc(-3.5).value_at(1.0), -3.5);

    assert!(Waveform::Dc(2.0).is_constant());
    assert!(!COS_1_60.is_constant());
    assert!(!SIN_1_60.is_constant());
    assert!(
        Waveform::Cos {
            amplitude: 0.0,
            frequency: 60.0
        }
        .is_constant()
    );
    assert!(
        Waveform::Sin {
            amplitude: 0.0,
            frequency: 60.0
        }
        .is_constant()
    );
}

#[test]
fn sources_carry_their_waveforms_into_the_circuit() {
    let circuit = compile(
        &parse("Sources\nVin n1 0 COS 1 60\nV2 n2 0 SIN 1 60\nI1 n1 0 DC 2m\nR1 n1 n2 1k\n.end")
            .unwrap(),
    )
    .unwrap();

    assert_eq!(
        circuit.elements()[0].kind,
        ElementKind::VoltageSource { waveform: COS_1_60 }
    );
    assert_eq!(
        circuit.elements()[1].kind,
        ElementKind::VoltageSource { waveform: SIN_1_60 }
    );
    assert_eq!(
        circuit.elements()[2].kind,
        ElementKind::CurrentSource {
            waveform: Waveform::Dc(2e-3)
        }
    );
}

#[test]
fn periodic_source_frequency_must_be_positive() {
    for (record, parameter, value) in [
        ("V1 a 0 COS 1 0", "COS frequency", 0.0),
        ("I1 a 0 COS 1 -60", "COS frequency", -60.0),
        ("V1 a 0 SIN 1 0", "SIN frequency", 0.0),
        ("I1 a 0 SIN 1 -60", "SIN frequency", -60.0),
    ] {
        let text = format!("Frequency\n{record}\nR1 a 0 1\n.end");
        let CircuitError::Element(error) = compile(&parse(&text).unwrap()).unwrap_err() else {
            panic!("{record}: expected an element error");
        };

        assert_eq!(&text[error.span.range()], record);
        assert_eq!(
            error.kind,
            ElementErrorKind::NonPositiveParameter { parameter, value }
        );
    }
}

#[test]
fn manually_constructed_periodic_values_must_be_finite() {
    let base = parse("Manual AST\nV1 a 0 DC 1\nR1 a 0 1\n.end").unwrap();

    for (waveform, parameter) in [
        (
            ParsedWaveform::Cos {
                amplitude: f64::NAN,
                frequency: 60.0,
            },
            "COS amplitude",
        ),
        (
            ParsedWaveform::Cos {
                amplitude: 1.0,
                frequency: f64::INFINITY,
            },
            "COS frequency",
        ),
        (
            ParsedWaveform::Sin {
                amplitude: f64::NAN,
                frequency: 60.0,
            },
            "SIN amplitude",
        ),
        (
            ParsedWaveform::Sin {
                amplitude: 1.0,
                frequency: f64::INFINITY,
            },
            "SIN frequency",
        ),
    ] {
        let mut parsed = base.clone();
        parsed.elements[0].kind = ParsedKind::VoltageSource { waveform };
        let CircuitError::Element(error) = compile(&parsed).unwrap_err() else {
            panic!("{parameter}: expected an element error");
        };

        assert!(
            matches!(
                error.kind,
                ElementErrorKind::NonFiniteParameter { parameter: found, .. } if found == parameter
            ),
            "{error:?}"
        );
    }
}

#[test]
fn transient_times_must_be_positive_and_ordered() {
    for (record, expected) in [
        (
            ".tran 0 1m",
            TransientRequestErrorKind::NonPositiveStop { stop: 0.0 },
        ),
        (
            ".tran -1 1m",
            TransientRequestErrorKind::NonPositiveStop { stop: -1.0 },
        ),
        (
            ".tran 1 0",
            TransientRequestErrorKind::NonPositiveStep { step: 0.0 },
        ),
        (
            ".tran 1 -1u",
            TransientRequestErrorKind::NonPositiveStep { step: -1e-6 },
        ),
        (
            ".tran 1m 2m",
            TransientRequestErrorKind::StepExceedsStop {
                stop: 1e-3,
                step: 2e-3,
            },
        ),
    ] {
        let text = format!("Times\nR1 a 0 1\n  {record}\n.end");
        let CircuitError::Transient(error) = compile(&parse(&text).unwrap()).unwrap_err() else {
            panic!("{record}: expected a transient request error");
        };

        assert_eq!(&text[error.span.range()], record);
        assert_eq!(error.location.line, 3);
        assert_eq!(error.kind, expected);
    }

    for record in [".tran 1m 1m", ".tran 0.05s 0.1ms", ".tran 1 0.3"] {
        let text = format!("Times\nR1 a 0 1\n{record}\n.end");
        assert!(compile(&parse(&text).unwrap()).is_ok(), "{record}");
    }
}
