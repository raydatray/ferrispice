use crate::{ByteSpan, NetlistPosition, ParseError, ParseErrorKind};

/// SPICE scale factors, matched case-insensitively against the letters after
/// the mantissa; longer names come first so `meg` and `mil` win over `m`
const SCALES: [(&str, f64); 10] = [
    ("meg", 1e6),
    ("mil", 25.4e-6),
    ("t", 1e12),
    ("g", 1e9),
    ("k", 1e3),
    ("m", 1e-3),
    ("u", 1e-6),
    ("n", 1e-9),
    ("p", 1e-12),
    ("f", 1e-15),
];

/// parse `mantissa [scale] [unit letters]`, for example `0.1ms` or `100uF`
///
/// the mantissa is a complete decimal or scientific-notation number. An
/// optional SPICE scale follows, then any ASCII letters, which are ignored as
/// units. `M` is milli, as in SPICE; mega is `meg`.
pub(crate) fn parse_number(
    token: &str,
    location: NetlistPosition,
    span: ByteSpan,
) -> Result<f64, ParseError> {
    let error = |kind| ParseError::new(location, span, kind);
    let invalid = || {
        error(ParseErrorKind::InvalidNumber {
            value: token.to_owned(),
        })
    };

    // a plain float, including `inf` and `NaN`, which are rejected below as
    // non-finite rather than as malformed
    let value = match token.parse::<f64>() {
        Ok(value) => value,
        Err(_) => {
            let length = mantissa_length(token).ok_or_else(invalid)?;
            let (mantissa, suffix) = token.split_at(length);
            if !suffix.bytes().all(|byte| byte.is_ascii_alphabetic()) {
                return Err(invalid());
            }

            scaled(mantissa, suffix).ok_or_else(invalid)?
        }
    };

    if !value.is_finite() {
        return Err(error(ParseErrorKind::NonFiniteNumber {
            value: token.to_owned(),
        }));
    }

    Ok(value)
}

/// byte length of the leading `[sign] digits [. digits] [e [sign] digits]`,
/// which needs at least one mantissa digit
///
/// an `e` that does not start a complete exponent makes the number invalid
/// rather than starting a suffix, so `1e` and `1ex` are rejected
fn mantissa_length(token: &str) -> Option<usize> {
    let bytes = token.as_bytes();
    let digits_from = |start: usize| {
        bytes[start..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count()
    };

    let mut end = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let whole = digits_from(end);
    end += whole;

    let mut fraction = 0;
    if bytes.get(end) == Some(&b'.') {
        fraction = digits_from(end + 1);
        end += 1 + fraction;
    }

    if whole + fraction == 0 {
        return None;
    }

    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(bytes.get(end + 1), Some(b'+' | b'-')));
        let exponent = digits_from(end + 1 + sign);
        if exponent == 0 {
            return None;
        }
        end += 1 + sign + exponent;
    }

    Some(end)
}

/// `mantissa` scaled by the factor named at the start of `suffix`
///
/// power-of-ten scales shift the decimal exponent before parsing, so `100u`
/// is exactly the double nearest `1e-4`, as `100e-6` is; multiplying would
/// round twice
fn scaled(mantissa: &str, suffix: &str) -> Option<f64> {
    let suffix = suffix.to_ascii_lowercase();
    let factor = SCALES
        .iter()
        .find(|(name, _)| suffix.starts_with(name))
        .map_or(1.0, |&(_, factor)| factor);

    if factor == 25.4e-6 {
        return Some(mantissa.parse::<f64>().ok()? * factor);
    }

    let (significand, exponent) = match mantissa.find(['e', 'E']) {
        Some(index) => (&mantissa[..index], &mantissa[index + 1..]),
        None => (mantissa, "0"),
    };
    // an exponent too long for i64 is far outside f64 range either way
    let exponent: i64 = exponent.parse().unwrap_or(if exponent.starts_with('-') {
        -100_000
    } else {
        100_000
    });
    let shift = factor.log10().round() as i64;

    format!("{significand}e{}", exponent + shift).parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<f64, ParseError> {
        parse_number(
            text,
            NetlistPosition::new(2, 10),
            ByteSpan::new(20, text.len()),
        )
    }

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
            assert_eq!(parse(text), Ok(expected));
        }
    }

    #[test]
    fn applies_spice_scale_suffixes_and_ignores_units() {
        for (text, expected) in [
            ("0.05s", 0.05),
            ("0.1ms", 1e-4),
            ("100u", 1e-4),
            ("100uF", 1e-4),
            ("1k", 1e3),
            ("1K", 1e3),
            ("1meg", 1e6),
            ("1MEG", 1e6),
            ("1M", 1e-3),
            ("100e-6", 1e-4),
            ("60Hz", 60.0),
            ("2t", 2e12),
            ("2G", 2e9),
            ("3n", 3e-9),
            ("3p", 3e-12),
            ("3f", 3e-15),
            ("1mil", 25.4e-6),
            ("5.u", 5e-6),
            ("-.5k", -500.0),
            ("1.5e3m", 1.5),
            ("-4.7kOhm", -4700.0),
            ("1e-3k", 1.0),
            ("10V", 10.0),
        ] {
            assert_eq!(parse(text), Ok(expected), "{text}");
        }
    }

    #[test]
    fn rejects_malformed_values_and_suffixes_with_their_location() {
        let location = NetlistPosition::new(4, 12);

        for text in [
            "1e", "1ek", "1e+", "--1", "1_000", "1k5", "1.0.0", "value", "k", ".", "-", "1 k",
            "1kΩ",
        ] {
            let span = ByteSpan::new(20, text.len());
            let error = parse_number(text, location, span).unwrap_err();

            assert_eq!(error.location, location, "{text}");
            assert_eq!(error.span, span, "{text}");
            assert!(
                matches!(error.kind, ParseErrorKind::InvalidNumber { .. }),
                "{text}: {error:?}"
            );
        }
    }

    #[test]
    fn rejects_non_finite_values_and_overflow() {
        for text in ["NaN", "inf", "-inf", "1e999", "1e308k", "-1e308meg"] {
            let error = parse(text).unwrap_err();

            assert!(
                matches!(error.kind, ParseErrorKind::NonFiniteNumber { .. }),
                "{text}: {error:?}"
            );
        }
    }
}
