use ferrispice_circuit::Circuit;
use ferrispice_mna::{MnaSystem, UnknownKind};
use ferrispice_solver::DcSolution;
use nalgebra::DMatrix;

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

pub(crate) fn print_solution(circuit: &Circuit, solution: &DcSolution) {
    let layout = solution.layout();
    let width = layout.labels().map(str::len).max().unwrap_or(0);

    println!("{}", circuit.title());
    println!("linear DC analysis");

    if !solution.unknowns().is_empty() {
        println!();
    }

    for (unknown, &value) in layout.unknowns().iter().zip(solution.unknowns().iter()) {
        let unit = match unknown.kind {
            UnknownKind::NodeVoltage(_) => "V",
            UnknownKind::BranchCurrent(_) => "A",
        };

        println!("{:<width$} = {} {unit}", unknown.label, number(value));
    }
}

/// `G x + C dx/dt = b`, with rows labeled as KCL or branch equations.
pub(crate) fn print_mna(circuit: &Circuit, system: &MnaSystem) {
    let columns: Vec<_> = system.layout.labels().map(str::to_owned).collect();

    let rows: Vec<_> = system
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
        .collect();

    println!("MNA system: G x + C dx/dt = b");
    println!();
    println!("unknowns x:");

    for (index, (label, row)) in columns.iter().zip(&rows).enumerate() {
        println!("  [{index}] {label:<w$}  row: {row}", w = width(&columns));
    }

    print_matrix("G", &system.g, &rows, &columns);
    print_matrix("C", &system.c, &rows, &columns);

    let b = DMatrix::from_column_slice(system.b.len(), 1, system.b.as_slice());

    print_matrix("b", &b, &rows, &["b".to_owned()]);
    println!();
}

fn print_matrix(name: &str, matrix: &DMatrix<f64>, rows: &[String], columns: &[String]) {
    println!();
    println!("{name}:");

    if matrix.is_empty() {
        println!("  (empty)");
        return;
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

    print!("  {:row_width$}", "");

    for (column, label) in columns.iter().enumerate() {
        print!("  {label:>w$}", w = column_widths[column]);
    }

    println!();

    for (label, row) in rows.iter().zip(&cells) {
        print!("  {label:<row_width$}");

        for (column, cell) in row.iter().enumerate() {
            print!("  {cell:>w$}", w = column_widths[column]);
        }

        println!();
    }
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
