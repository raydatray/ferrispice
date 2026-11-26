use crate::element::parse_element;
use crate::number::parse_number;
use crate::scan::{RawRecord, scan};
use crate::{Analysis, AnalysisRequest, ParseError, ParseErrorKind, ParsedNetlist};

pub fn parse(text: &str) -> Result<ParsedNetlist, ParseError> {
    let _span = tracing::debug_span!("parse_netlist", bytes = text.len()).entered();

    let raw = scan(text)?;

    let mut elements = Vec::new();
    let mut analyses: Vec<AnalysisRequest> = Vec::new();

    for record in raw.records {
        match record {
            RawRecord::Dc(location, span) => analyses.push(AnalysisRequest {
                kind: Analysis::Dc,
                location,
                span,
            }),
            RawRecord::Tran(location, span, tokens) => {
                // repeated `.dc` records agree, but two `.tran` records could not
                if let Some(first) = analyses
                    .iter()
                    .find(|request| matches!(request.kind, Analysis::Tran { .. }))
                {
                    return Err(ParseError::new(
                        location,
                        span,
                        ParseErrorKind::DuplicateTransient {
                            first: first.location,
                        },
                    ));
                }

                let value = |index: usize| {
                    let token = tokens[index];
                    parse_number(token.text, token.location, token.span)
                };
                analyses.push(AnalysisRequest {
                    kind: Analysis::Tran {
                        stop: value(1)?,
                        step: value(2)?,
                    },
                    location,
                    span,
                });
            }
            RawRecord::Element(record) => elements.push(parse_element(record)?),
        }
    }

    tracing::debug!(
        elements = elements.len(),
        analyses = analyses.len(),
        "parsed netlist"
    );

    Ok(ParsedNetlist {
        title: raw.title.to_owned(),
        elements,
        analyses,
    })
}
