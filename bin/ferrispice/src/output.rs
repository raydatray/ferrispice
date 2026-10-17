use boxen::{BorderStyle, TextAlignment, text::text_width};
use ferrispice_circuit::Circuit;
use ferrispice_mna::{MnaSystem, UnknownKind};
use ferrispice_solver::DcSolution;
use miette::{IntoDiagnostic, WrapErr};
use nalgebra::{DMatrix, DVector};

/// significant digits for printed quantities and matrix entries.
const SIGNIFICANT_DIGITS: i32 = 6;

/// magnitudes below this print as zero, matching the solver's absolute residual tolerance
const ZERO_THRESHOLD: f64 = 1e-12;

pub(crate) fn print_validation(circuit: &Circuit, system: &MnaSystem) {
    println!("{}", circuit.title());
    println!(
        "validated circuit: {} non-ground nodes, {} elements, {} unknowns",
        circuit.non_ground_nodes().count(),
        circuit.elements().len(),
        system.layout.len(),
    );
    println!("no analysis requested; add .dc to run linear DC analysis");
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

    print_panel("linear DC analysis", &lines.join("\n"))
}

/// `G x + C dx/dt = b`, with rows labeled as KCL or branch equations.
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
        "G x + C dx/dt = b".to_owned(),
        unknowns.join("\n"),
        format_matrix("G", &system.g, &rows, &columns),
        format_matrix("C", &system.c, &rows, &columns),
        format_matrix("b", &b, &rows, &["b".to_owned()]),
    ]
    .join("\n\n");

    print_panel("MNA system", &body)
}

/// print a diagnostic factorization of G, with `P * G = L * U`
pub(crate) fn print_lu(circuit: &Circuit, system: &MnaSystem) -> miette::Result<()> {
    // solve_dc computes its own factorization; this one is only for inspection
    let lu = system.g.clone().lu();
    let n = system.g.nrows();
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
        "P * G = L * U".to_owned(),
        "factor rows: [position <- original MNA row] equation".to_owned(),
        format_matrix("P", &p, &factor_positions, &original_positions),
        format_matrix("L", &lu.l(), &rows, &pivots),
        format_matrix("U", &lu.u(), &rows, &columns),
    ]
    .join("\n\n");

    print_panel("LU decomposition", &body)
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
