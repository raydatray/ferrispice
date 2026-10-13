use ferrispice_netlist::{ByteSpan, NetlistPosition, ParseErrorKind, parse};

#[test]
fn numeric_errors_highlight_only_the_value_after_unicode_and_crlf() {
    let text = "Τitle\r\n  R1 α 0 invalid\r\n.end\r\n";
    let error = parse(text).unwrap_err();

    assert_eq!(error.location, NetlistPosition::new(2, 10));
    assert_eq!(&text[error.span.range()], "invalid");
    assert_eq!(error.span.offset, text.find("invalid").unwrap());
}

#[test]
fn syntax_errors_highlight_the_relevant_token_or_record() {
    for (text, expected) in [
        ("Title\nV1 a 0 AC 1\n.end", "AC"),
        ("Title\nD1 a 0 model\n.end", "D1"),
        ("Title\n  R1 a 0 1 extra  \n.end", "R1 a 0 1 extra"),
        ("Title\nI1 a 0 DC NaN\n.end", "NaN"),
    ] {
        let error = parse(text).unwrap_err();

        assert_eq!(&text[error.span.range()], expected);
    }
}

#[test]
fn parsed_records_and_analysis_requests_keep_their_byte_ranges() {
    let text = "Title\r\n  R1 α 0 100 \r\n  .dc\r\n.end\r\n";
    let parsed = parse(text).unwrap();

    assert_eq!(&text[parsed.elements[0].span.range()], "R1 α 0 100");
    assert_eq!(&text[parsed.analyses[0].span.range()], ".dc");
}

#[test]
fn missing_end_points_at_eof_with_and_without_a_trailing_newline() {
    for text in ["Title\nR1 α 0 1", "Title\r\nR1 α 0 1\r\n"] {
        let error = parse(text).unwrap_err();

        assert_eq!(error.kind, ParseErrorKind::MissingEnd);
        assert_eq!(error.span, ByteSpan::new(text.len(), 0));
        assert_eq!(&text[error.span.range()], "");
    }
}
