use boxen::{BorderStyle, TextAlignment, text::text_width};
use ferrispice_circuit::{Circuit, ElementKind};
use ferrispice_mna::{MnaSystem, NonlinearDevice, TermSign, UnknownKind, UnknownLayout};
use ferrispice_solver::{
    DcMethod, DcSolution, NewtonError, NewtonIteration, NewtonSolution, SourceSteppingFailure,
    StepAttempt,
};
use miette::{IntoDiagnostic, WrapErr};
use nalgebra::{DMatrix, DVector};

/// significant digits for printed quantities and matrix entries.
const SIGNIFICANT_DIGITS: i32 = 6;

/// magnitudes below this print as zero, matching the solver's absolute residual tolerance
const ZERO_THRESHOLD: f64 = 1e-12;

pub(crate) fn print_validation(circuit: &Circuit, layout: &UnknownLayout) {
    let diodes = circuit
        .elements()
        .iter()
        .filter(|element| matches!(element.kind, ElementKind::Diode { .. }))
        .count();
    let nonlinear = match diodes {
        0 => String::new(),
        1 => " (1 diode)".to_owned(),
        count => format!(" ({count} diodes)"),
    };

    println!("{}", circuit.title());
    println!(
        "validated circuit: {} non-ground nodes, {} elements{nonlinear}, {} unknowns",
        circuit.non_ground_nodes().count(),
        circuit.elements().len(),
        layout.len(),
    );
    println!("no analysis requested; add .dc to run DC analysis");
}

pub(crate) fn print_solution(circuit: &Circuit, solution: &DcSolution) -> miette::Result<()> {
    let layout = solution.layout();
    let width = layout.labels().map(str::len).max().unwrap_or(0);

    let mut lines = vec![circuit.title().to_owned()];

    if !solution.unknowns().is_empty() {
        lines.push(String::new());
    }

    for (unknown, &value) in layout.unknowns().iter().zip(solution.unknowns().iter()) {
        let unit = match unknown.kind {
            UnknownKind::NodeVoltage(_) => "V",
            UnknownKind::BranchCurrent(_) => "A",
        };

        lines.push(format!(
            "{:<width$} = {} {unit}",
            unknown.label,
            number(value)
        ));
    }

    let title = match solution.method() {
        DcMethod::Linear => "linear DC analysis",
        DcMethod::Newton(_) | DcMethod::SourceStepping { .. } => {
            lines.push(String::new());
            lines.push(convergence_summary(solution));
            "nonlinear DC analysis"
        }
    };

    print_panel(title, &lines.join("\n"))
}

/// how Newton-Raphson reached the operating point
fn convergence_summary(solution: &DcSolution) -> String {
    let iterations = solution.newton_iterations();
    let plural =
        |count: usize, word: &str| format!("{count} {word}{}", if count == 1 { "" } else { "s" });

    match solution.method() {
        DcMethod::SourceStepping { stepping, .. } => format!(
            "converged with source stepping: {} accepted, {} rejected, {}",
            plural(stepping.accepted().count(), "step"),
            stepping.rejected(),
            plural(iterations, "Newton iteration"),
        ),
        _ => format!("converged in {}", plural(iterations, "Newton iteration")),
    }
}

/// diode columns beyond this are left out of the trace to keep it readable
const TRACED_DIODES: usize = 4;

/// what a Newton trace needs to label its columns
struct TraceContext<'a> {
    layout: &'a UnknownLayout,
    /// name and matrix indices of the anode and cathode of each diode
    diodes: Vec<(&'a str, Option<usize>, Option<usize>)>,
}

impl<'a> TraceContext<'a> {
    fn new(circuit: &'a Circuit, layout: &'a UnknownLayout) -> Self {
        let diodes = circuit
            .elements()
            .iter()
            .filter(|element| matches!(element.kind, ElementKind::Diode { .. }))
            .map(|element| {
                (
                    element.name.as_str(),
                    layout.node_index(element.positive),
                    layout.node_index(element.negative),
                )
            })
            .collect();

        Self { layout, diodes }
    }
}

/// Newton-Raphson progress for a solved circuit
pub(crate) fn print_trace(circuit: &Circuit, solution: &DcSolution) -> miette::Result<()> {
    let context = TraceContext::new(circuit, solution.layout());
    let body = match solution.method() {
        DcMethod::Linear => "linear circuit: solved directly by one LU factorization of G; \
                             Newton-Raphson was not used"
            .to_owned(),
        DcMethod::Newton(newton) => [
            "direct Newton on the full circuit, from x = 0: converged".to_owned(),
            newton_table(&context, &newton.iterations),
        ]
        .join("\n"),
        DcMethod::SourceStepping { direct, stepping } => [
            direct_failure(&context, direct),
            stepping_section(&context, &stepping.attempts),
        ]
        .join("\n\n"),
    };

    print_panel("Newton trace", &[TRACE_LEGEND, &body].join("\n\n"))
}

/// Newton-Raphson progress up to a failed nonlinear solve
pub(crate) fn print_failed_trace(
    circuit: &Circuit,
    failure: &SourceSteppingFailure,
) -> miette::Result<()> {
    let layout = UnknownLayout::new(circuit);
    let context = TraceContext::new(circuit, &layout);
    let body = [
        TRACE_LEGEND.to_owned(),
        direct_failure(&context, &failure.direct),
        stepping_section(&context, &failure.stepping.attempts),
        format!(
            "source stepping stopped: the step fell to {} with the sources scaled by {}",
            parameter(failure.stepping.step),
            parameter(failure.stepping.reached),
        ),
    ]
    .join("\n\n");

    print_panel("Newton trace", &body)
}

const TRACE_LEGEND: &str = "each iteration linearizes at x_k and solves J(x_k) Δx = -Φ(x_k)\n\
    vd: diode voltages at x_k, where f and J are evaluated\n\
    Φ in: max |Φ(x_k)|, the residual put into the solve\n\
    ΔV: the largest node-voltage change in Δx\n\
    Φ out: max |Φ(x_k + Δx)|, the next iteration's Φ in";

fn direct_failure(context: &TraceContext<'_>, error: &NewtonError) -> String {
    [
        format!("direct Newton on the full circuit, from x = 0: failed\n  {error}"),
        newton_table(context, &error.iterations),
    ]
    .join("\n")
}

/// a summary of every attempt, then each attempt's Newton iterations
fn stepping_section(
    context: &TraceContext<'_>,
    attempts: &[StepAttempt<NewtonSolution, NewtonError>],
) -> String {
    let mut lines = vec![
        "source stepping: solve G x + f(x) = λ b, ramping λ from 0 to 1".to_owned(),
        format!(
            "  {:>7}  {:>21}  {:>10}  {:<8}  {:>10}",
            "attempt", "λ from -> to", "step", "result", "iterations"
        ),
    ];

    for (index, attempt) in attempts.iter().enumerate() {
        let (result, iterations) = match &attempt.outcome {
            Ok(solution) => ("accepted", solution.iterations.len()),
            Err(error) => ("rejected", error.iterations.len()),
        };
        lines.push(format!(
            "  {:>7}  {:>21}  {:>10}  {result:<8}  {iterations:>10}",
            index + 1,
            format!("{} -> {}", parameter(attempt.from), parameter(attempt.to)),
            parameter(attempt.step()),
        ));
    }

    for (index, attempt) in attempts.iter().enumerate() {
        let (status, iterations) = match &attempt.outcome {
            Ok(solution) => ("accepted".to_owned(), &solution.iterations),
            Err(error) => (format!("rejected: {error}"), &error.iterations),
        };
        lines.push(String::new());
        lines.push(format!(
            "attempt {}, λ = {}: {status}",
            index + 1,
            parameter(attempt.to)
        ));
        lines.push(newton_table(context, iterations));
    }

    lines.join("\n")
}

/// one row per Newton iteration: the diode voltages it linearized at, the
/// residual it solved with, the largest node-voltage change, and the
/// residual it left for the next iteration
fn newton_table(context: &TraceContext<'_>, iterations: &[NewtonIteration]) -> String {
    if iterations.is_empty() {
        return "  (no completed iterations)".to_owned();
    }

    let unknowns = context.layout.unknowns();
    let diodes = &context.diodes[..context.diodes.len().min(TRACED_DIODES)];

    let mut header = vec!["iter".to_owned()];
    header.extend(diodes.iter().map(|(name, _, _)| format!("vd({name})")));
    header.extend(["Φ in", "ΔV", "node", "Φ out"].map(str::to_owned));

    let rows: Vec<Vec<String>> = iterations
        .iter()
        .enumerate()
        .map(|(index, iteration)| {
            let x = &iteration.linearized_at;
            let voltage = |node: Option<usize>| node.map_or(0.0, |node| x[node]);

            let mut row = vec![(index + 1).to_string()];
            row.extend(
                diodes
                    .iter()
                    .map(|&(_, anode, cathode)| number(voltage(anode) - voltage(cathode))),
            );

            // node voltages are what a reader can follow; a circuit without
            // any falls back to the unknown that limited convergence
            let changed = (0..unknowns.len())
                .filter(|&index| matches!(unknowns[index].kind, UnknownKind::NodeVoltage(_)))
                .max_by(|&a, &b| {
                    iteration.delta[a]
                        .abs()
                        .total_cmp(&iteration.delta[b].abs())
                })
                .or(iteration.worst_unknown);
            let (change, label) = changed.map_or((String::new(), "-"), |index| {
                (
                    format!("{:.3e}", iteration.delta[index]),
                    unknowns[index].label.as_str(),
                )
            });

            row.extend([
                format!("{:.3e}", iteration.residual_in),
                change,
                label.to_owned(),
                format!("{:.3e}", iteration.residual),
            ]);
            row
        })
        .collect();

    let widths: Vec<_> = (0..header.len())
        .map(|column| {
            rows.iter()
                .map(|row| text_width(&row[column]))
                .chain([text_width(&header[column])])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let node_column = header.len() - 2;
    let format_row = |row: &[String]| {
        let cells: Vec<_> = row
            .iter()
            .enumerate()
            .map(|(column, cell)| {
                let padding = " ".repeat(widths[column] - text_width(cell));
                if column == node_column {
                    format!("{cell}{padding}")
                } else {
                    format!("{padding}{cell}")
                }
            })
            .collect();
        format!("  {}", cells.join("  ").trim_end())
    };

    let mut lines = vec![format_row(&header)];
    lines.extend(rows.iter().map(|row| format_row(row)));
    if context.diodes.len() > diodes.len() {
        lines.push(format!(
            "  (vd shown for the first {} of {} diodes)",
            diodes.len(),
            context.diodes.len()
        ));
    }

    lines.join("\n")
}

/// source scales and step sizes, which can be far below the zero threshold
fn parameter(value: f64) -> String {
    if value != 0.0 && value.abs() < ZERO_THRESHOLD {
        format!("{value:.3e}")
    } else {
        number(value)
    }
}

/// `G x + C dx/dt + f(x) = b`, with rows labeled as KCL or branch equations.
pub(crate) fn print_mna(circuit: &Circuit, system: &MnaSystem) -> miette::Result<()> {
    let columns: Vec<_> = system.layout.labels().map(str::to_owned).collect();

    let rows = equation_labels(circuit, system);

    let mut unknowns = vec!["unknowns x:".to_owned()];

    for (index, (label, row)) in columns.iter().zip(&rows).enumerate() {
        unknowns.push(format!(
            "  [{index}] {label:<w$}  row: {row}",
            w = width(&columns)
        ));
    }

    let b = DMatrix::from_column_slice(system.b.len(), 1, system.b.as_slice());
    let body = [
        "G x + C dx/dt + f(x) = b".to_owned(),
        unknowns.join("\n"),
        format_matrix("G", &system.g, &rows, &columns),
        format_matrix("C", &system.c, &rows, &columns),
        format_f(system, &rows),
        format_matrix("b", &b, &rows, &["b".to_owned()]),
    ]
    .join("\n\n");

    print_panel("MNA system", &body)
}

/// `f(x)` evaluated at the solved operating point; nothing for linear circuits
pub(crate) fn print_f_at_solution(
    circuit: &Circuit,
    system: &MnaSystem,
    solution: &DcSolution,
) -> miette::Result<()> {
    if system.is_linear() {
        return Ok(());
    }

    let f = system
        .f(solution.unknowns())
        .into_diagnostic()
        .wrap_err("could not evaluate f(x) at the operating point")?;
    let f = DMatrix::from_column_slice(f.len(), 1, f.as_slice());
    let rows = equation_labels(circuit, system);

    print_panel(
        "f(x) at the operating point",
        &format_matrix("f(x*)", &f, &rows, &["f(x*)".to_owned()]),
    )
}

/// `f(x)` symbolically: the signed device currents in each row, then each
/// device's equation in terms of the unknowns
fn format_f(system: &MnaSystem, rows: &[String]) -> String {
    if system.is_linear() {
        return "f(x) = 0".to_owned();
    }

    let terms: Vec<String> = system
        .describe_f()
        .iter()
        .map(|row| {
            if row.is_empty() {
                return "0".to_owned();
            }
            row.iter()
                .map(|term| {
                    let sign = match term.sign {
                        TermSign::Positive => '+',
                        TermSign::Negative => '-',
                    };
                    format!("{sign}id({})", term.name)
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();

    let row_width = width(rows);
    let mut output = "f(x):".to_owned();
    for (label, term) in rows.iter().zip(&terms) {
        output.push_str(&format!("\n  {label:<row_width$}  {term}"));
    }
    output.push('\n');

    for device in &system.nonlinear {
        match device {
            NonlinearDevice::Diode(diode) => {
                let voltage = |index: Option<usize>| {
                    index.map(|index| system.layout.unknowns()[index].label.clone())
                };
                let junction = match (voltage(diode.anode), voltage(diode.cathode)) {
                    (Some(anode), Some(cathode)) => format!("{anode} - {cathode}"),
                    (Some(anode), None) => anode,
                    (None, Some(cathode)) => format!("-{cathode}"),
                    (None, None) => "0".to_owned(),
                };
                // `number` would print Is as zero
                output.push_str(&format!(
                    "\nid({}) = {:e} * (exp(({junction}) / {}) - 1)",
                    diode.name,
                    diode.saturation_current,
                    number(diode.thermal_voltage),
                ));
            }
        }
    }

    output
}

/// print a diagnostic factorization of G, with `P * G = L * U`
pub(crate) fn print_lu(circuit: &Circuit, system: &MnaSystem) -> miette::Result<()> {
    print_factorization(circuit, system, &system.g, "G", "LU decomposition")
}

/// print a diagnostic factorization of the Jacobian at the operating point,
/// the matrix Newton-Raphson solved last
pub(crate) fn print_jacobian_lu(
    circuit: &Circuit,
    system: &MnaSystem,
    solution: &DcSolution,
) -> miette::Result<()> {
    match solution.jacobian() {
        Some(jacobian) => print_factorization(
            circuit,
            system,
            jacobian,
            "J",
            "LU decomposition of J = G + ∂f/∂x at the operating point",
        ),
        None => print_lu(circuit, system),
    }
}

/// a nonlinear circuit's Jacobian depends on an operating point that was not found
pub(crate) fn print_no_jacobian(reason: &str) -> miette::Result<()> {
    print_panel(
        "LU decomposition",
        &format!(
            "J = G + ∂f/∂x depends on the operating point, and {reason}; \
             no Jacobian is available to factor"
        ),
    )
}

fn print_factorization(
    circuit: &Circuit,
    system: &MnaSystem,
    matrix: &DMatrix<f64>,
    symbol: &str,
    title: &str,
) -> miette::Result<()> {
    // the solver computes its own factorization; this one is only for inspection
    let lu = matrix.clone().lu();
    let n = matrix.nrows();
    let mut p = DMatrix::<f64>::identity(n, n);
    lu.p().permute_rows(&mut p);

    let mut row_order = DVector::from_iterator(n, 0..n);
    lu.p().permute_rows(&mut row_order);
    let original_equations = equation_labels(circuit, system);
    let rows: Vec<_> = row_order
        .iter()
        .enumerate()
        .map(|(position, &original)| {
            format!(
                "[{position} <- {original}] {}",
                original_equations[original]
            )
        })
        .collect();

    let factor_positions: Vec<_> = (0..n).map(|index| format!("factor {index}")).collect();
    let original_positions: Vec<_> = (0..n).map(|index| format!("original {index}")).collect();
    let pivots: Vec<_> = (0..n).map(|index| format!("pivot {index}")).collect();
    let columns: Vec<_> = system.layout.labels().map(str::to_owned).collect();

    let body = [
        format!("P * {symbol} = L * U"),
        "factor rows: [position <- original MNA row] equation".to_owned(),
        format_matrix("P", &p, &factor_positions, &original_positions),
        format_matrix("L", &lu.l(), &rows, &pivots),
        format_matrix("U", &lu.u(), &rows, &columns),
    ]
    .join("\n\n");

    print_panel(title, &body)
}

fn equation_labels(circuit: &Circuit, system: &MnaSystem) -> Vec<String> {
    system
        .layout
        .unknowns()
        .iter()
        .map(|unknown| match unknown.kind {
            UnknownKind::NodeVoltage(id) => format!(
                "KCL {}",
                circuit
                    .node(id)
                    .expect("layout nodes belong to the circuit")
                    .name
            ),
            UnknownKind::BranchCurrent(id) => format!(
                "branch {}",
                circuit
                    .element(id)
                    .expect("layout elements belong to the circuit")
                    .name
            ),
        })
        .collect()
}

fn format_matrix(name: &str, matrix: &DMatrix<f64>, rows: &[String], columns: &[String]) -> String {
    let mut output = format!("{name}:\n");

    if matrix.is_empty() {
        output.push_str("  (empty)");
        return output;
    }

    let cells: Vec<Vec<String>> = matrix
        .row_iter()
        .map(|row| row.iter().map(|&value| number(value)).collect())
        .collect();

    let row_width = width(rows);

    let column_widths: Vec<_> = (0..columns.len())
        .map(|column| {
            cells
                .iter()
                .map(|row| row[column].len())
                .chain([columns[column].len()])
                .max()
                .unwrap_or(0)
        })
        .collect();

    output.push_str(&format!("  {:row_width$}", ""));

    for (column, label) in columns.iter().enumerate() {
        output.push_str(&format!("  {label:>w$}", w = column_widths[column]));
    }

    for (label, row) in rows.iter().zip(&cells) {
        output.push('\n');
        output.push_str(&format!("  {label:<row_width$}"));

        for (column, cell) in row.iter().enumerate() {
            output.push_str(&format!("  {cell:>w$}", w = column_widths[column]));
        }
    }

    output
}

fn print_panel(title: &str, body: &str) -> miette::Result<()> {
    let content_width = body
        .lines()
        .map(text_width)
        .chain([text_width(title) + 2])
        .max()
        .unwrap_or(1);
    let panel_width = content_width + 6;
    if panel_width > boxen::get_terminal_width() {
        println!("{title}\n\n{body}\n");
        return Ok(());
    }

    let panel = boxen::builder()
        .title(format!(" {title} "))
        .border_style(BorderStyle::Round)
        .padding((1, 2, 1, 2))
        .text_alignment(TextAlignment::Left)
        .width(panel_width)
        .render(body)
        .into_diagnostic()
        .wrap_err_with(|| format!("could not render {title} panel"))?;

    println!("{panel}\n");
    Ok(())
}

fn width(labels: &[String]) -> usize {
    labels.iter().map(String::len).max().unwrap_or(0)
}

/// six significant digits, scientific notation outside `[1e-3, 1e6)`.
fn number(value: f64) -> String {
    let magnitude = value.abs();

    if magnitude < ZERO_THRESHOLD {
        return "0".to_owned();
    }

    if (1e-3..1e6).contains(&magnitude) {
        let decimals = (SIGNIFICANT_DIGITS - 1 - magnitude.log10().floor() as i32).max(0) as usize;
        return trim_fraction(format!("{value:.decimals$}"));
    }

    let formatted = format!("{value:.*e}", (SIGNIFICANT_DIGITS - 1) as usize);
    let (mantissa, exponent) = formatted.split_once('e').expect("scientific notation");

    format!("{}e{exponent}", trim_fraction(mantissa.to_owned()))
}

fn trim_fraction(mut number: String) -> String {
    if number.contains('.') {
        let trimmed = number.trim_end_matches('0').trim_end_matches('.').len();
        number.truncate(trimmed);
    }

    number
}

#[cfg(test)]
mod tests {
    use super::number;

    #[test]
    fn numbers_use_six_significant_digits_and_scientific_extremes() {
        for (value, expected) in [
            (0.0, "0"),
            (-0.0, "0"),
            (1e-15, "0"),
            (6.0, "6"),
            (-0.039999999999999994, "-0.04"),
            (0.01, "0.01"),
            (1.0 / 3.0, "0.333333"),
            (123456.7, "123457"),
            (1e-3, "0.001"),
            (1.5e-6, "1.5e-6"),
            (-2.5e-4, "-2.5e-4"),
            (1e6, "1e6"),
            (1234567.0, "1.23457e6"),
        ] {
            assert_eq!(number(value), expected, "{value}");
        }
    }
}
