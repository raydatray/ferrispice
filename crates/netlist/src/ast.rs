use crate::{ByteSpan, NetlistPosition};

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedNetlist {
    pub title: String,
    pub elements: Vec<ParsedElement>,
    pub analyses: Vec<AnalysisRequest>,
}

impl ParsedNetlist {
    pub fn requests_dc(&self) -> bool {
        self.analyses
            .iter()
            .any(|request| request.kind == Analysis::Dc)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Analysis {
    Dc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalysisRequest {
    pub kind: Analysis,
    pub location: NetlistPosition,
    pub span: ByteSpan,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedElement {
    pub location: NetlistPosition,
    pub span: ByteSpan,
    pub name: String,
    pub positive: String,
    pub negative: String,
    pub kind: ParsedKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParsedKind {
    Resistor { resistance: f64 },
    Capacitor { capacitance: f64 },
    Inductor { inductance: f64 },
    VoltageSource { dc: f64 },
    CurrentSource { dc: f64 },
}
