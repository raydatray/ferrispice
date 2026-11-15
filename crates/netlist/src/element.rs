use crate::number::parse_number;
use crate::scan::{RawElement, require_fields};
use crate::{ParseError, ParseErrorKind, ParsedElement, ParsedKind, ParsedWaveform};

#[derive(Clone, Copy)]
enum Family {
    Resistor,
    Capacitor,
    Inductor,
    VoltageSource,
    CurrentSource,
    Vcvs,
    Vccs,
    Ccvs,
    Cccs,
    Diode,
}

pub(crate) fn parse_element(record: RawElement<'_>) -> Result<ParsedElement, ParseError> {
    let name = record.tokens[0].text;
    let family = classify(name).ok_or_else(|| unsupported(&record))?;

    let kind = match family {
        Family::Resistor => ParsedKind::Resistor {
            resistance: parse_element_value(&record, 4, "Rname positive negative resistance")?,
        },
        Family::Capacitor => ParsedKind::Capacitor {
            capacitance: parse_element_value(&record, 4, "Cname positive negative capacitance")?,
        },
        Family::Inductor => ParsedKind::Inductor {
            inductance: parse_element_value(&record, 4, "Lname positive negative inductance")?,
        },
        Family::VoltageSource => ParsedKind::VoltageSource {
            waveform: parse_waveform(&record, &VOLTAGE_SOURCE_SHAPES)?,
        },
        Family::CurrentSource => ParsedKind::CurrentSource {
            waveform: parse_waveform(&record, &CURRENT_SOURCE_SHAPES)?,
        },
        Family::Vcvs => {
            let gain = parse_element_value(
                &record,
                6,
                "VCVSname positive negative control_positive control_negative gain",
            )?;
            ParsedKind::Vcvs {
                control_positive: record.tokens[3].text.to_owned(),
                control_negative: record.tokens[4].text.to_owned(),
                gain,
            }
        }
        Family::Vccs => {
            let transconductance = parse_element_value(
                &record,
                6,
                "VCCSname positive negative control_positive control_negative transconductance",
            )?;
            ParsedKind::Vccs {
                control_positive: record.tokens[3].text.to_owned(),
                control_negative: record.tokens[4].text.to_owned(),
                transconductance,
            }
        }
        Family::Ccvs => {
            let transresistance = parse_element_value(
                &record,
                5,
                "CCVSname positive negative controlling_element transresistance",
            )?;
            ParsedKind::Ccvs {
                control: record.tokens[3].text.to_owned(),
                transresistance,
            }
        }
        Family::Cccs => {
            let gain = parse_element_value(
                &record,
                5,
                "CCCSname positive negative controlling_element gain",
            )?;
            ParsedKind::Cccs {
                control: record.tokens[3].text.to_owned(),
                gain,
            }
        }
        Family::Diode => {
            require_fields(&record.tokens, 3, "Dname anode cathode")?;
            ParsedKind::Diode
        }
    };

    tracing::trace!(
        element = name,
        line = record.location.line,
        "parsed element"
    );

    Ok(ParsedElement {
        location: record.location,
        span: record.span,
        name: name.to_owned(),
        positive: record.tokens[1].text.to_owned(),
        negative: record.tokens[2].text.to_owned(),
        kind,
    })
}

fn classify(name: &str) -> Option<Family> {
    let name = name.to_ascii_uppercase();

    for (prefix, family) in [
        ("VCVS", Family::Vcvs),
        ("VCCS", Family::Vccs),
        ("CCVS", Family::Ccvs),
        ("CCCS", Family::Cccs),
    ] {
        if name.starts_with(prefix) {
            return Some(family);
        }
    }

    match name.as_bytes().first()? {
        b'R' => Some(Family::Resistor),
        b'C' => Some(Family::Capacitor),
        b'L' => Some(Family::Inductor),
        b'V' => Some(Family::VoltageSource),
        b'I' => Some(Family::CurrentSource),
        b'D' => Some(Family::Diode),
        _ => None,
    }
}

fn parse_element_value(
    record: &RawElement<'_>,
    fields: usize,
    expected: &'static str,
) -> Result<f64, ParseError> {
    require_fields(&record.tokens, fields, expected)?;

    let token = record.tokens[fields - 1];
    parse_number(token.text, token.location, token.span)
}

/// the record shapes of one independent-source family, for field-count errors
struct SourceShapes {
    any: &'static str,
    dc: &'static str,
    cos: &'static str,
    sin: &'static str,
}

const VOLTAGE_SOURCE_SHAPES: SourceShapes = SourceShapes {
    any: "Vname positive negative DC value, COS amplitude frequency, or SIN amplitude frequency",
    dc: "Vname positive negative DC value",
    cos: "Vname positive negative COS amplitude frequency",
    sin: "Vname positive negative SIN amplitude frequency",
};

const CURRENT_SOURCE_SHAPES: SourceShapes = SourceShapes {
    any: "Iname positive negative DC value, COS amplitude frequency, or SIN amplitude frequency",
    dc: "Iname positive negative DC value",
    cos: "Iname positive negative COS amplitude frequency",
    sin: "Iname positive negative SIN amplitude frequency",
};

/// `DC value`, `COS amplitude frequency`, or `SIN amplitude frequency`, with a
/// case-insensitive keyword
fn parse_waveform(
    record: &RawElement<'_>,
    shapes: &SourceShapes,
) -> Result<ParsedWaveform, ParseError> {
    // every mode has a keyword and at least one value
    if record.tokens.len() < 5 {
        require_fields(&record.tokens, 5, shapes.any)?;
    }

    let mode = record.tokens[3];
    let value = |index: usize| {
        let token = record.tokens[index];
        parse_number(token.text, token.location, token.span)
    };

    if mode.text.eq_ignore_ascii_case("DC") {
        require_fields(&record.tokens, 5, shapes.dc)?;
        Ok(ParsedWaveform::Dc(value(4)?))
    } else if mode.text.eq_ignore_ascii_case("COS") {
        require_fields(&record.tokens, 6, shapes.cos)?;
        Ok(ParsedWaveform::Cos {
            amplitude: value(4)?,
            frequency: value(5)?,
        })
    } else if mode.text.eq_ignore_ascii_case("SIN") {
        require_fields(&record.tokens, 6, shapes.sin)?;
        Ok(ParsedWaveform::Sin {
            amplitude: value(4)?,
            frequency: value(5)?,
        })
    } else {
        Err(ParseError::new(
            mode.location,
            mode.span,
            ParseErrorKind::UnknownSourceMode {
                found: mode.text.to_owned(),
            },
        ))
    }
}

fn unsupported(record: &RawElement<'_>) -> ParseError {
    ParseError::new(
        record.location,
        record.tokens[0].span,
        ParseErrorKind::UnsupportedElement {
            name: record.tokens[0].text.to_owned(),
        },
    )
}
