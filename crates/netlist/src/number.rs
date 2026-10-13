use crate::{ByteSpan, NetlistPosition, ParseError, ParseErrorKind};

pub(crate) fn parse_number(
    token: &str,
    location: NetlistPosition,
    span: ByteSpan,
) -> Result<f64, ParseError> {
    let value: f64 = token.parse().map_err(|_| {
        ParseError::new(
            location,
            span,
            ParseErrorKind::InvalidNumber {
                value: token.to_owned(),
            },
        )
    })?;

    if !value.is_finite() {
        return Err(ParseError::new(
            location,
            span,
            ParseErrorKind::NonFiniteNumber {
                value: token.to_owned(),
            },
        ));
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_signed_decimals_and_scientific_notation() {
        for (text, expected) in [
            ("100", 100.0),
            ("+6", 6.0),
            ("-.5", -0.5),
            ("5.", 5.0),
            ("1e-6", 0.000001),
            ("-2.5E+3", -2500.0),
        ] {
            assert_eq!(
                parse_number(
                    text,
                    NetlistPosition::new(2, 10),
                    ByteSpan::new(20, text.len())
                ),
                Ok(expected)
            );
        }
    }

    #[test]
    fn rejects_malformed_and_suffix_values_with_their_location() {
        let location = NetlistPosition::new(4, 12);

        for text in ["1k", "1e", "--1", "1_000", "value"] {
            let span = ByteSpan::new(20, text.len());
            let error = parse_number(text, location, span).unwrap_err();

            assert_eq!(error.location, location);
            assert_eq!(error.span, span);
            assert!(matches!(error.kind, ParseErrorKind::InvalidNumber { .. }));
        }
    }

    #[test]
    fn rejects_non_finite_values_and_overflow() {
        for text in ["NaN", "inf", "-inf", "1e999"] {
            let error = parse_number(
                text,
                NetlistPosition::new(2, 1),
                ByteSpan::new(0, text.len()),
            )
            .unwrap_err();

            assert!(matches!(error.kind, ParseErrorKind::NonFiniteNumber { .. }));
        }
    }
}
