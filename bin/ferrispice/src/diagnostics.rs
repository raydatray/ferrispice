use ferrispice_circuit::{Circuit, CircuitError, ElementErrorKind};
use ferrispice_mna::AssemblyError;
use ferrispice_netlist::{ByteSpan, ParseError, ParseErrorKind};
use ferrispice_solver::{
    DcError, NewtonError, NewtonErrorKind, SolveError, SourceSteppingFailure, TransientError,
    TransientStepFailure,
};
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

/// preserve the library error, attach source text only at presentation time
#[derive(Debug, Error)]
pub(crate) enum NetlistCause {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    Circuit(#[from] CircuitError),
    #[error(transparent)]
    Assembly(#[from] AssemblyError),
}

/// a solve failure has no single source location, names the input instead
#[derive(Debug, Error, Diagnostic)]
#[error("{input_name}: linear DC analysis failed")]
struct AnalysisDiagnostic {
    input_name: String,
    #[source]
    cause: SolveError,
    #[help]
    help: Option<&'static str>,
}

/// Newton failed on the full circuit and source stepping could not reach it
#[derive(Debug, Error, Diagnostic)]
#[error("{input_name}: nonlinear DC analysis failed")]
struct NonlinearDiagnostic {
    input_name: String,
    #[source]
    cause: Box<SourceSteppingFailure>,
    #[source_code]
    input: NamedSource<String>,
    /// the diode whose equation overflowed, when that ended the last attempt
    #[label("{label}")]
    span: Option<SourceSpan>,
    label: String,
    #[help]
    help: &'static str,
}

/// the diode whose equation overflowed, when that ended a Newton solve, with
/// what went wrong; the circuit, not the solver, knows where it was declared
fn failed_device(error: Option<&NewtonError>, circuit: &Circuit) -> (Option<SourceSpan>, String) {
    let device = match error.map(|error| &error.kind) {
        Some(NewtonErrorKind::Device { cause, .. }) => Some(cause),
        _ => None,
    };

    (
        device
            .and_then(|device| circuit.element(device.id))
            .map(|element| source_span(element.span)),
        device.map_or_else(String::new, |device| device.cause.to_string()),
    )
}

impl NonlinearDiagnostic {
    fn new(
        cause: Box<SourceSteppingFailure>,
        circuit: &Circuit,
        input_name: String,
        contents: String,
    ) -> Self {
        let (span, label) = failed_device(cause.stepping.last_failure(), circuit);

        Self {
            span,
            label,
            input: NamedSource::new(input_name.clone(), contents),
            input_name,
            cause,
            help: "Newton-Raphson did not converge from x = 0, and ramping every source up \
                   from zero could not reach full strength. Possible causes:\n\
                   - current is forced backwards through a diode, which blocks reverse \
                   current\n\
                   - part of the circuit is not connected to the rest, so its voltages are \
                   undetermined",
        }
    }
}

pub(crate) fn analysis(
    error: DcError,
    circuit: &Circuit,
    input_name: String,
    contents: String,
) -> miette::Report {
    match error {
        DcError::Assembly(error) => report(error, input_name, contents),
        DcError::SourceStepping(cause) => miette::Report::new(NonlinearDiagnostic::new(
            cause, circuit, input_name, contents,
        )),
        DcError::Solve(cause) => {
            let help = matches!(cause, SolveError::Singular { .. }).then_some(
                "the equations do not determine every unknown. floating nodes, \
                 current sources in series, and loops of voltage sources or inductors \
                 can cause this.",
            );
            miette::Report::new(AnalysisDiagnostic {
                input_name,
                cause,
                help,
            })
        }
    }
}

/// a backward-Euler step failed; the operating point before it was found
#[derive(Debug, Error, Diagnostic)]
#[error("{input_name}: transient analysis failed at t = {time} s")]
struct TransientDiagnostic {
    input_name: String,
    time: f64,
    #[source]
    cause: TransientCause,
    #[source_code]
    input: NamedSource<String>,
    #[label("{label}")]
    span: Option<SourceSpan>,
    label: String,
    #[help]
    help: &'static str,
}

#[derive(Debug, Error)]
enum TransientCause {
    #[error(transparent)]
    Linear(SolveError),

    #[error(transparent)]
    Step(Box<TransientStepFailure>),
}

pub(crate) fn transient(
    error: TransientError,
    circuit: &Circuit,
    input_name: String,
    contents: String,
) -> miette::Report {
    let (time, cause, span, label, help) = match error {
        // the initial state is a DC operating point, and fails like one
        TransientError::OperatingPoint(error) => {
            return analysis(error, circuit, input_name, contents);
        }
        TransientError::Singular { time, source } => (
            time,
            TransientCause::Linear(source),
            None,
            String::new(),
            "G + C/h does not determine every unknown. a loop of voltage sources, or \
             current sources in series, can cause this.",
        ),
        TransientError::Step { time, failure } => {
            let (span, label) = failed_device(failure.stepping.last_failure(), circuit);
            (
                time,
                TransientCause::Step(failure),
                span,
                label,
                "Newton-Raphson did not converge for a time step, even after dividing the \
                 step into smaller ones. a smaller .tran step may help, as may checking for \
                 current forced backwards through a diode.",
            )
        }
    };

    miette::Report::new(TransientDiagnostic {
        input: NamedSource::new(input_name.clone(), contents),
        input_name,
        time,
        cause,
        span,
        label,
        help,
    })
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
struct NetlistDiagnostic {
    cause: NetlistCause,
    /// errors without a source span name the input instead
    message: String,
    #[source_code]
    input: NamedSource<String>,
    /// absent for whole-netlist errors that no single record caused
    #[label("{label}")]
    span: Option<SourceSpan>,
    label: String,
    #[label("first declared here")]
    first_declaration: Option<SourceSpan>,
    #[help]
    help: Option<String>,
}

impl NetlistDiagnostic {
    fn new(cause: NetlistCause, input_name: String, contents: String) -> Self {
        let (span, label, first_declaration, help) = match &cause {
            NetlistCause::Parse(error) => {
                let help = match &error.kind {
                    ParseErrorKind::MissingTitle => {
                        Some("use the first line as the circuit title".to_owned())
                    }
                    ParseErrorKind::MissingEnd => Some("finish the netlist with `.end`".to_owned()),
                    ParseErrorKind::UnknownSourceMode { .. } => Some(
                        "independent sources use: name positive negative DC value, \
                         COS amplitude frequency, or SIN amplitude frequency"
                            .to_owned(),
                    ),
                    _ => None,
                };
                (Some(error.span), error.kind.to_string(), None, help)
            }
            NetlistCause::Circuit(CircuitError::Element(error)) => {
                let first = match &error.kind {
                    ElementErrorKind::DuplicateElement { first_span, .. } => {
                        Some(source_span(*first_span))
                    }
                    _ => None,
                };
                (Some(error.span), error.kind.to_string(), first, None)
            }
            NetlistCause::Circuit(CircuitError::Transient(error)) => (
                Some(error.span),
                error.kind.to_string(),
                None,
                Some(
                    ".tran takes the stop time first, then the time step: .tran tstop tstep"
                        .to_owned(),
                ),
            ),
            NetlistCause::Circuit(CircuitError::MissingGround) => (
                None,
                String::new(),
                None,
                Some(
                    "connect at least one element terminal to node 0, \
                     the reference for every voltage"
                        .to_owned(),
                ),
            ),
            NetlistCause::Assembly(error) => (Some(error.span), error.kind.to_string(), None, None),
        };
        let message = match span {
            Some(_) => cause.to_string(),
            None => format!("{input_name}: {cause}"),
        };

        Self {
            cause,
            message,
            input: NamedSource::new(input_name, contents),
            span: span.map(source_span),
            label,
            first_declaration,
            help,
        }
    }
}

pub(crate) fn report(
    error: impl Into<NetlistCause>,
    input_name: String,
    contents: String,
) -> miette::Report {
    miette::Report::new(NetlistDiagnostic::new(error.into(), input_name, contents))
}

fn source_span(span: ByteSpan) -> SourceSpan {
    (span.offset, span.length).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use miette::{GraphicalReportHandler, GraphicalTheme};

    fn render(diagnostic: &NetlistDiagnostic) -> String {
        let mut output = String::new();
        GraphicalReportHandler::new()
            .with_theme(GraphicalTheme::unicode_nocolor())
            .render_report(&mut output, diagnostic)
            .unwrap();
        output
    }

    #[test]
    fn renders_numeric_errors_with_the_input_name_and_exact_unicode_span() {
        let text = "Τitle\r\nR1 α 0 invalid\r\n.end\r\n";
        let error = ferrispice_netlist::parse(text).unwrap_err();
        let span = error.span;
        let diagnostic = NetlistDiagnostic::new(error.into(), "divider.cir".into(), text.into());
        let labels: Vec<_> = diagnostic.labels().unwrap().collect();
        assert_eq!(labels[0].offset(), span.offset);
        assert_eq!(labels[0].len(), "invalid".len());
        let output = render(&diagnostic);
        assert!(output.contains("divider.cir"));
        assert!(output.contains("R1 α 0 invalid"));
        assert!(output.contains("scientific-notation"));
    }

    #[test]
    fn renders_eof_diagnostics_for_terminal_input() {
        let text = "Terminal circuit\nR1 a 0 100";
        let error = ferrispice_netlist::parse(text).unwrap_err();
        let diagnostic = NetlistDiagnostic::new(error.into(), "<stdin>".into(), text.into());
        let labels: Vec<_> = diagnostic.labels().unwrap().collect();
        assert_eq!(labels[0].offset(), text.len());
        assert_eq!(labels[0].len(), 0);
        let output = render(&diagnostic);
        assert!(output.contains("<stdin>"));
        assert!(output.contains("R1 a 0 100"));
        assert!(output.contains("finish the netlist with `.end`"));
    }

    #[test]
    fn renders_both_duplicate_declarations() {
        let text = "Duplicate\nR1 α 0 100\nR1 β 0 50\n.end\n";
        let parsed = ferrispice_netlist::parse(text).unwrap();
        let error = ferrispice_circuit::compile(&parsed).unwrap_err();
        let diagnostic = NetlistDiagnostic::new(error.into(), "duplicate.cir".into(), text.into());
        let labels: Vec<_> = diagnostic.labels().unwrap().collect();
        assert_eq!(labels.len(), 2);
        assert_eq!(
            &text[labels[0].offset()..labels[0].offset() + labels[0].len()],
            "R1 β 0 50"
        );
        assert_eq!(
            &text[labels[1].offset()..labels[1].offset() + labels[1].len()],
            "R1 α 0 100"
        );
        let output = render(&diagnostic);
        assert!(output.contains("first declared here"));
        assert!(output.contains("R1 α 0 100"));
        assert!(output.contains("R1 β 0 50"));
    }

    #[test]
    fn renders_semantic_controller_errors() {
        let text = "Controller\nCCCS1 out 0 missing 2\n.end\n";
        let parsed = ferrispice_netlist::parse(text).unwrap();
        let error = ferrispice_circuit::compile(&parsed).unwrap_err();
        let diagnostic = NetlistDiagnostic::new(error.into(), "controller.cir".into(), text.into());
        let output = render(&diagnostic);
        assert!(output.contains("controller.cir"));
        assert!(output.contains("CCCS1 out 0 missing 2"));
        assert!(output.contains("unknown controlling element"));
    }

    #[test]
    fn renders_a_missing_ground_without_blaming_a_record() {
        let text = "Ungrounded\nR1 a b 100\n.end\n";
        let parsed = ferrispice_netlist::parse(text).unwrap();
        let error = ferrispice_circuit::compile(&parsed).unwrap_err();
        let diagnostic = NetlistDiagnostic::new(error.into(), "ungrounded.cir".into(), text.into());
        assert_eq!(diagnostic.labels().into_iter().flatten().count(), 0);
        let output = render(&diagnostic);
        assert!(output.contains("ungrounded.cir: no element terminal connects to ground node 0"));
        assert!(output.contains("connect at least one element terminal to node 0"));
        assert!(!output.contains("R1 a b 100"));
    }

    #[test]
    fn renders_malformed_diode_records() {
        let text = "* diode circuit\nD1 n1\n.end\n";
        let error = ferrispice_netlist::parse(text).unwrap_err();
        let diagnostic = NetlistDiagnostic::new(error.into(), "diode.cir".into(), text.into());
        let labels: Vec<_> = diagnostic.labels().unwrap().collect();
        assert_eq!(
            &text[labels[0].offset()..labels[0].offset() + labels[0].len()],
            "D1 n1"
        );
        let output = render(&diagnostic);
        assert!(output.contains("diode.cir"));
        assert!(output.contains("expected Dname anode cathode; found 2 fields"));
    }

    #[test]
    fn empty_input_has_a_readable_diagnostic() {
        let error = ferrispice_netlist::parse("").unwrap_err();
        let diagnostic = NetlistDiagnostic::new(error.into(), "empty.cir".into(), String::new());
        let output = render(&diagnostic);
        assert!(output.contains("missing the first-line circuit title"));
    }

    #[test]
    fn renders_invalid_transient_times_on_the_tran_record() {
        let text = "Times\nR1 a 0 1\n.tran 1m 2m\n.end\n";
        let parsed = ferrispice_netlist::parse(text).unwrap();
        let error = ferrispice_circuit::compile(&parsed).unwrap_err();
        let diagnostic = NetlistDiagnostic::new(error.into(), "times.cir".into(), text.into());
        let labels: Vec<_> = diagnostic.labels().unwrap().collect();
        assert_eq!(
            &text[labels[0].offset()..labels[0].offset() + labels[0].len()],
            ".tran 1m 2m"
        );
        let output = render(&diagnostic);
        assert!(output.contains("is longer than the stop time"), "{output}");
        assert!(output.contains(".tran tstop tstep"), "{output}");
    }

    #[test]
    fn renders_a_transient_step_failure_with_its_time() {
        let text = "Rectifier\nVin n1 0 COS 1 60\nD1 n1 n2\nR1 n2 0 50\nC1 n2 0 100u\n.end\n";
        let circuit =
            ferrispice_circuit::compile(&ferrispice_netlist::parse(text).unwrap()).unwrap();
        let options = ferrispice_solver::TransientOptions {
            newton: ferrispice_solver::NewtonOptions {
                max_iterations: 0,
                ..Default::default()
            },
        };
        let error =
            ferrispice_solver::solve_transient_with(&circuit, 1e-3, 1e-4, &options).unwrap_err();
        let report = transient(error, &circuit, "rectifier.cir".into(), text.into());

        let mut output = String::new();
        GraphicalReportHandler::new()
            .with_theme(GraphicalTheme::unicode_nocolor())
            .render_report(&mut output, report.as_ref())
            .unwrap();
        assert!(
            output.contains("rectifier.cir: transient analysis failed at t = 0 s"),
            "{output}"
        );
        assert!(output.contains("subdividing it stopped"), "{output}");
        assert!(output.contains("smaller .tran step"), "{output}");
    }
}
