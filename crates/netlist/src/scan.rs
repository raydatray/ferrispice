use crate::token::{Token, record_byte_span, tokenize};
use crate::{ByteSpan, NetlistPosition, ParseError, ParseErrorKind};

#[derive(Debug)]
pub(crate) struct RawNetlist<'a> {
    pub title: &'a str,
    pub records: Vec<RawRecord<'a>>,
}

#[derive(Debug)]
pub(crate) enum RawRecord<'a> {
    Dc(NetlistPosition, ByteSpan),
    Element(RawElement<'a>),
}

#[derive(Debug)]
pub(crate) struct RawElement<'a> {
    pub location: NetlistPosition,
    pub span: ByteSpan,
    pub tokens: Vec<Token<'a>>,
}

pub(crate) fn scan(text: &str) -> Result<RawNetlist<'_>, ParseError> {
    let _span = tracing::debug_span!("scan_netlist").entered();

    let mut lines = text.split_inclusive('\n');
    let first_line = lines.next().filter(|_| !text.is_empty()).ok_or_else(|| {
        ParseError::new(
            NetlistPosition::new(1, 1),
            ByteSpan::new(0, 0),
            ParseErrorKind::MissingTitle,
        )
    })?;
    let title = strip_line_ending(first_line);

    let mut ended = false;

    let records = lines
        .enumerate()
        .map(|(index, line)| (index + 2, strip_line_ending(line)))
        .filter(|(_, line)| {
            let line = line.trim_start();
            !line.is_empty() && !line.starts_with('*')
        })
        .map(|(line_number, line)| {
            let byte_offset = line.as_ptr() as usize - text.as_ptr() as usize;
            let tokens = tokenize(line, line_number, byte_offset);

            parse_record(tokens, &mut ended)
        })
        .collect::<Result<Vec<_>, ParseError>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    if !ended {
        return Err(ParseError::new(
            NetlistPosition::end_of_input(text),
            ByteSpan::new(text.len(), 0),
            ParseErrorKind::MissingEnd,
        ));
    }

    tracing::debug!(records = records.len(), "scanned netlist");

    Ok(RawNetlist { title, records })
}

fn parse_record<'a>(
    tokens: Vec<Token<'a>>,
    ended: &mut bool,
) -> Result<Option<RawRecord<'a>>, ParseError> {
    let first = tokens[0];

    if *ended {
        return Err(ParseError::new(
            first.location,
            record_byte_span(&tokens),
            ParseErrorKind::RecordAfterEnd,
        ));
    }

    tracing::trace!(
        line = first.location.line,
        record = first.text,
        "read record"
    );

    match first.text.to_ascii_lowercase().as_str() {
        ".end" => {
            require_fields(&tokens, 1, ".end")?;
            *ended = true;
            Ok(None)
        }
        ".dc" => {
            require_fields(&tokens, 1, ".dc")?;
            Ok(Some(RawRecord::Dc(first.location, first.span)))
        }
        directive if directive.starts_with('.') => Err(ParseError::new(
            first.location,
            first.span,
            ParseErrorKind::UnsupportedDirective {
                directive: first.text.to_owned(),
            },
        )),
        _ => Ok(Some(RawRecord::Element(RawElement {
            location: first.location,
            span: record_byte_span(&tokens),
            tokens,
        }))),
    }
}

pub(crate) fn require_fields(
    tokens: &[Token<'_>],
    count: usize,
    expected: &'static str,
) -> Result<(), ParseError> {
    if tokens.len() != count {
        return Err(ParseError::new(
            tokens[0].location,
            record_byte_span(tokens),
            ParseErrorKind::WrongFieldCount {
                expected,
                found: tokens.len(),
            },
        ));
    }

    Ok(())
}

fn strip_line_ending(line: &str) -> &str {
    match line.strip_suffix('\n') {
        Some(line) => line.strip_suffix('\r').unwrap_or(line),
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_physical_line_is_always_the_title() {
        let raw = scan("R1 looks like an element\n.dc\n.end").unwrap();

        assert_eq!(raw.title, "R1 looks like an element");
        assert!(matches!(
            raw.records[0],
            RawRecord::Dc(NetlistPosition { line: 2, .. }, _)
        ));
        assert_eq!(scan("\n.end").unwrap().title, "");
    }

    #[test]
    fn ignores_blank_lines_comments_and_supports_crlf() {
        let raw = scan("Title\r\n\r\n  * comment\r\n.dc\r\n.END\r\n * tail\r\n").unwrap();

        assert_eq!(raw.records.len(), 1);
    }

    #[test]
    fn retains_character_columns_and_element_tokens() {
        let raw = scan("Title\n  R1 α 0 nope\n.end").unwrap();
        let RawRecord::Element(element) = &raw.records[0] else {
            panic!("expected an element record");
        };

        assert_eq!(element.location, NetlistPosition::new(2, 3));
        assert_eq!(element.tokens[1].text, "α");
        assert_eq!(element.tokens[3].location, NetlistPosition::new(2, 10));
    }

    #[test]
    fn missing_title_and_end_have_locations() {
        let error = scan("").unwrap_err();

        assert_eq!(error.kind, ParseErrorKind::MissingTitle);
        assert_eq!(error.location, NetlistPosition::new(1, 1));
        assert_eq!(error.span, ByteSpan::new(0, 0));

        let error = scan("Title\n.dc\n").unwrap_err();

        assert_eq!(error.kind, ParseErrorKind::MissingEnd);
        assert_eq!(error.location, NetlistPosition::new(3, 1));
        assert_eq!(error.span, ByteSpan::new("Title\n.dc\n".len(), 0));
    }

    #[test]
    fn rejects_records_after_end_and_directive_arguments() {
        for text in ["Title\n.end\nR1 a 0 1", "Title\n.end\n.end"] {
            let error = scan(text).unwrap_err();

            assert_eq!(error.kind, ParseErrorKind::RecordAfterEnd);
            assert_eq!(error.location.line, 3);
        }
        for text in ["Title\n.dc extra\n.end", "Title\n.end extra"] {
            assert!(matches!(
                scan(text).unwrap_err().kind,
                ParseErrorKind::WrongFieldCount { .. }
            ));
        }
    }

    #[test]
    fn rejects_unsupported_directives() {
        let error = scan("Title\n.op\n.end").unwrap_err();

        assert_eq!(error.location.line, 2);
        assert!(matches!(
            error.kind,
            ParseErrorKind::UnsupportedDirective { .. }
        ));
    }

    #[test]
    fn spans_slice_the_original_input_with_unicode_and_crlf() {
        let text = "Τitle\r\n\r\n* comment\r\n  R1 α 0 nope\r\n.dc\r\n.end\r\n";
        let raw = scan(text).unwrap();
        let RawRecord::Element(element) = &raw.records[0] else {
            panic!("expected element");
        };

        assert_eq!(&text[element.span.range()], "R1 α 0 nope");

        for token in &element.tokens {
            assert_eq!(&text[token.span.range()], token.text);
        }

        assert_eq!(element.tokens[1].span.length, "α".len());
        assert_eq!(element.tokens[3].location, NetlistPosition::new(4, 10));

        let RawRecord::Dc(_, span) = raw.records[1] else {
            panic!("expected .dc");
        };

        assert_eq!(&text[span.range()], ".dc");
    }

    #[test]
    fn framing_errors_select_the_offending_text() {
        for (text, expected) in [
            ("Title\n  .op\n.end", ".op"),
            ("Title\n.dc extra\n.end", ".dc extra"),
            ("Title\n.end\n  R1 a 0 1", "R1 a 0 1"),
        ] {
            let error = scan(text).unwrap_err();

            assert_eq!(&text[error.span.range()], expected);
        }
    }
}
