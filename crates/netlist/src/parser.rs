use crate::element::parse_element;
use crate::scan::{RawRecord, scan};
use crate::{Analysis, AnalysisRequest, ParseError, ParsedNetlist};

pub fn parse(text: &str) -> Result<ParsedNetlist, ParseError> {
    let _span = tracing::debug_span!("parse_netlist", bytes = text.len()).entered();

    let raw = scan(text)?;

    let mut elements = Vec::new();
    let mut analyses = Vec::new();

    for record in raw.records {
        match record {
            RawRecord::Dc(location, span) => analyses.push(AnalysisRequest {
                kind: Analysis::Dc,
                location,
                span,
            }),
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
