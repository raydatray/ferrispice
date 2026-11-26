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

    /// `(stop, step)` of the `.tran` record, if there is one
    pub fn transient_request(&self) -> Option<(f64, f64)> {
        self.analyses.iter().find_map(|request| match request.kind {
            Analysis::Tran { stop, step } => Some((stop, step)),
            Analysis::Dc => None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Analysis {
    Dc,
    /// `.tran tstop tstep`: stop time first, the reverse of standard SPICE
    Tran {
        stop: f64,
        step: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
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
    Resistor {
        resistance: f64,
    },
    Capacitor {
        capacitance: f64,
    },
    Inductor {
        inductance: f64,
    },
    VoltageSource {
        waveform: ParsedWaveform,
    },
    CurrentSource {
        waveform: ParsedWaveform,
    },
    Vcvs {
        control_positive: String,
        control_negative: String,
        gain: f64,
    },
    Vccs {
        control_positive: String,
        control_negative: String,
        transconductance: f64,
    },
    Ccvs {
        control: String,
        transresistance: f64,
    },
    Cccs {
        control: String,
        gain: f64,
    },
    /// `positive` is the anode and `negative` is the cathode
    Diode,
}

/// the value of an independent source over time, as written in the netlist
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParsedWaveform {
    /// `DC value`: constant
    Dc(f64),
    /// `COS amplitude frequency`: `amplitude * cos(2π · frequency · t)`
    Cos { amplitude: f64, frequency: f64 },
    /// `SIN amplitude frequency`: `amplitude * sin(2π · frequency · t)`
    Sin { amplitude: f64, frequency: f64 },
}
