use std::fmt::Write as _;
use std::path::Path;

use ferrispice_solver::TransientSolution;

/// write every unknown at every time point: a `time` column, then one column
/// per unknown with its label as the header
///
/// values use the shortest text that parses back to the same `f64`, so no
/// precision is lost; parent directories are created
pub(crate) fn write_csv(path: &Path, solution: &TransientSolution) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, render(solution))
}

fn render(solution: &TransientSolution) -> String {
    let columns: Vec<_> = solution.columns().collect();

    let mut text = String::from("time");
    for (unknown, _) in &columns {
        text.push(',');
        text.push_str(&field(&unknown.label));
    }
    text.push('\n');

    for (row, &time) in solution.times().iter().enumerate() {
        text.push_str(&value(time));
        for (_, values) in &columns {
            write!(text, ",{}", value(values[row])).expect("writing to a string");
        }
        text.push('\n');
    }

    text
}

/// `f64`'s shortest round-trip form, which switches to an exponent for very
/// small or large magnitudes; whole numbers drop their `.0`
fn value(value: f64) -> String {
    let text = format!("{value:?}");
    match text.strip_suffix(".0") {
        Some(whole) => whole.to_owned(),
        None => text,
    }
}

/// a header field, quoted when a node or element name needs it
fn field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{field, value};

    #[test]
    fn values_round_trip_exactly() {
        for number in [
            0.0,
            1.0,
            -1.0,
            0.1,
            1e-4,
            0.26731440904763254,
            1e-20,
            1.5e300,
        ] {
            let text = value(number);
            assert_eq!(text.parse::<f64>().unwrap(), number, "{text}");
        }
        assert_eq!(value(0.0), "0");
        assert_eq!(value(-2.0), "-2");
        assert_eq!(value(0.0001), "0.0001");
        assert_eq!(value(1e-20), "1e-20");
    }

    #[test]
    fn header_fields_are_quoted_only_when_needed() {
        assert_eq!(field("V(n1)"), "V(n1)");
        assert_eq!(field("V(a,b)"), "\"V(a,b)\"");
        assert_eq!(field("V(\"q\")"), "\"V(\"\"q\"\")\"");
    }
}
