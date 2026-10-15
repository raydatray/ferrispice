use ferrispice_circuit::{CircuitError, ElementErrorKind};
use ferrispice_netlist::{ByteSpan, ParseError, ParseErrorKind};
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

/// preserve the library error, attach source text only at presentation time
#[derive(Debug, Error)]
pub(crate) enum NetlistCause {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    Circuit(#[from] CircuitError),
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
                    ParseErrorKind::ExpectedDc { .. } => {
                        Some("independent sources use: name positive negative DC value".to_owned())
                    }
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
    fn empty_input_has_a_readable_diagnostic() {
        let error = ferrispice_netlist::parse("").unwrap_err();
        let diagnostic = NetlistDiagnostic::new(error.into(), "empty.cir".into(), String::new());
        let output = render(&diagnostic);
        assert!(output.contains("missing the first-line circuit title"));
    }
}
