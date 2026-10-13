use crate::{ByteSpan, NetlistPosition};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Token<'a> {
    pub text: &'a str,
    pub location: NetlistPosition,
    pub span: ByteSpan,
}

pub(crate) fn record_byte_span(tokens: &[Token<'_>]) -> ByteSpan {
    let first = tokens[0].span;
    let last = tokens.last().expect("nonempty record").span;

    ByteSpan::new(first.offset, last.offset + last.length - first.offset)
}

pub(crate) fn tokenize(line: &str, line_number: usize, line_offset: usize) -> Vec<Token<'_>> {
    line.split_inclusive(char::is_whitespace)
        .scan((0, 1), |(byte, column), chunk| {
            let text = chunk.trim_end_matches(char::is_whitespace);

            let token = (!text.is_empty()).then(|| Token {
                text,
                location: NetlistPosition::new(line_number, *column),
                span: ByteSpan::new(line_offset + *byte, text.len()),
            });

            *byte += chunk.len();
            *column += chunk.chars().count();

            Some(token)
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_positions_are_character_columns_and_spans_are_absolute_bytes() {
        let text = "Τitle\r\n  R1 α 0 nope ";
        let offset = "Τitle\r\n".len();
        let tokens = tokenize(&text[offset..], 2, offset);

        assert_eq!(
            tokens.iter().map(|token| token.text).collect::<Vec<_>>(),
            ["R1", "α", "0", "nope"]
        );
        assert_eq!(
            tokens
                .iter()
                .map(|token| token.location.column)
                .collect::<Vec<_>>(),
            [3, 6, 8, 10]
        );

        for token in &tokens {
            assert_eq!(token.location.line, 2);
            assert_eq!(&text[token.span.range()], token.text);
        }

        assert_eq!(tokens[1].span.length, "α".len());
    }

    #[test]
    fn record_spans_preserve_internal_whitespace_and_exclude_outer_whitespace() {
        let text = "  R1\tα  0\t100  ";
        let tokens = tokenize(text, 1, 0);

        assert_eq!(&text[record_byte_span(&tokens).range()], "R1\tα  0\t100");
    }

    #[test]
    fn blank_lines_produce_no_tokens() {
        for text in ["", "  ", "\t \u{2003}"] {
            assert!(tokenize(text, 1, 0).is_empty());
        }
    }
}
