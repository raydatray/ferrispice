use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use nalgebra::DMatrix;

static NEXT_INPUT: AtomicUsize = AtomicUsize::new(0);

struct Input {
    directory: PathBuf,
    path: PathBuf,
}

impl Input {
    fn new(text: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "ferrispice-cli-{}-{}",
            std::process::id(),
            NEXT_INPUT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("input with spaces.cir");
        std::fs::write(&path, text).unwrap();
        Self { directory, path }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn run(arguments: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ferrispice"))
        .args(arguments)
        .env("RUST_LOG", "error")
        .env_remove("NO_COLOR")
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap()
}

fn assert_usage(output: &[u8]) {
    let text = std::str::from_utf8(output).unwrap();
    assert!(
        ["Usage:", "ferrispice", "<netlist>"]
            .iter()
            .all(|expected| text.contains(expected)),
        "{text}"
    );
}

fn output_text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| {
            line.trim_matches(|c: char| {
                c.is_whitespace() || matches!(c, '│' | '╭' | '╮' | '╰' | '╯' | '─')
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn success(text: &str) -> String {
    let input = Input::new(text);
    let output = run(&[input.path.as_os_str()]);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    output_text(output.stdout)
}

/// Parse `NAME(...) = value` result lines, ignoring units and alignment.
fn results(stdout: &str) -> HashMap<String, f64> {
    stdout
        .lines()
        .filter_map(|line| {
            let (label, value) = line.split_once('=')?;
            let label = label.trim();
            // results are entries of x; `id(D1) = ...` lines describe f(x)
            if !(label.starts_with("V(") || label.starts_with("I(")) {
                return None;
            }
            let value = value.split_whitespace().next()?.parse().ok()?;
            Some((label.to_owned(), value))
        })
        .collect()
}

fn assert_results(stdout: &str, expected: &[(&str, f64)]) {
    let results = results(stdout);
    for &(label, value) in expected {
        let actual = results[label];
        assert!(
            (actual - value).abs() <= 1e-9 * (1.0 + value.abs()),
            "{label}: expected {value}, got {actual}\n{stdout}"
        );
    }
}

#[test]
fn solves_the_divider_from_a_path_containing_spaces() {
    let stdout = success(DIVIDER);
    assert!(stdout.contains("linear DC analysis"), "{stdout}");
    let labels: Vec<_> = stdout
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(label, _)| label.trim())
        .collect();
    assert_eq!(labels, ["V(n1)", "V(n2)", "I(Vin)"], "{stdout}");
    assert_results(
        &stdout,
        &[("V(n1)", 6.0), ("V(n2)", 2.0), ("I(Vin)", -0.04)],
    );
}

#[test]
fn dc_may_appear_anywhere_before_end_and_repeats_run_once() {
    for text in [
        "Divider\n.dc\nVin n1 0 DC 6\nR1 n1 n2 100\nR2 n2 0 50\n.end\n",
        "Divider\nVin n1 0 DC 6\n.dc\nR1 n1 n2 100\nR2 n2 0 50\n.end\n",
        "Divider\nVin n1 0 DC 6\n.DC\nR1 n1 n2 100\n.dc\nR2 n2 0 50\n.dc\n.end\n",
    ] {
        let stdout = success(text);
        assert_eq!(stdout.matches("linear DC analysis").count(), 1, "{stdout}");
        assert_eq!(stdout.matches("V(n2)").count(), 1, "{stdout}");
        assert_results(&stdout, &[("V(n2)", 2.0), ("I(Vin)", -0.04)]);
    }
}

#[test]
fn validates_a_circuit_without_an_analysis_request() {
    let stdout = success("No analysis\nR1 a 0 100\n.end\n");
    assert!(stdout.contains("no analysis requested"), "{stdout}");
    assert!(
        !stdout.lines().any(|line| line == "linear DC analysis"),
        "{stdout}"
    );
    assert!(results(&stdout).is_empty(), "{stdout}");
}

#[test]
fn singular_circuits_fail_without_partial_results() {
    for text in [
        "Floating\nR1 a b 10\nR2 c 0 1\n.dc\n.end\n",
        "Conflict\nV1 p 0 DC 1\nV2 p 0 DC 2\n.dc\n.end\n",
    ] {
        let input = Input::new(text);
        let output = run(&[input.path.as_os_str()]);
        assert!(!output.status.success(), "{text:?}");
        assert!(output.stdout.is_empty(), "{text:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("ERROR"), "{stderr}");
        assert!(stderr.contains("input with spaces.cir"), "{stderr}");
        assert!(stderr.contains("linear DC analysis failed"), "{stderr}");
        assert!(stderr.contains("singular"), "{stderr}");
    }
}

const DIODE_CIRCUIT: &str = "* diode circuit\nVin n1 0 DC 1\nD1 n1 n2\nR n2 0 50\n.end\n";

#[test]
fn inspects_a_circuit_containing_a_diode() {
    let stdout = success(DIODE_CIRCUIT);
    assert!(stdout.starts_with("* diode circuit\n"), "{stdout}");
    assert!(
        stdout.contains("validated circuit: 2 non-ground nodes, 3 elements (1 diode), 3 unknowns"),
        "{stdout}"
    );
    assert!(stdout.contains("no analysis requested"), "{stdout}");
    assert!(results(&stdout).is_empty(), "{stdout}");

    let stdout = success("Two diodes\nD1 a 0\nD2 0 a\nR1 a 0 1\n.end\n");
    assert!(stdout.contains("3 elements (2 diodes)"), "{stdout}");
}

#[test]
fn malformed_diode_records_render_source_context() {
    for (text, record) in [
        ("Diode\nD1 n1\n.end\n", "D1 n1"),
        ("Diode\nD1 n1 0 model\n.end\n", "D1 n1 0 model"),
    ] {
        let input = Input::new(text);
        let output = run(&[input.path.as_os_str()]);
        assert!(!output.status.success(), "{text:?}");
        assert!(output.stdout.is_empty(), "{text:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("Dname anode cathode"), "{stderr}");
        assert!(stderr.contains(record), "{stderr}");
        assert!(stderr.contains("input with spaces.cir"), "{stderr}");
    }
}

#[test]
fn solves_a_circuit_containing_a_diode() {
    let stdout = success(&DIODE_CIRCUIT.replace(".end", ".dc\n.end"));
    assert!(
        stdout.lines().any(|line| line == "nonlinear DC analysis"),
        "{stdout}"
    );
    assert!(
        !stdout.lines().any(|line| line == "linear DC analysis"),
        "{stdout}"
    );
    for line in [
        "V(n1)  = 1 V",
        "V(n2)  = 0.267314 V",
        "I(Vin) = -0.00534629 A",
    ] {
        assert!(
            stdout.lines().any(|output| output == line),
            "{line:?}\n{stdout}"
        );
    }
    let summary = stdout
        .lines()
        .find(|line| line.starts_with("converged in "))
        .unwrap_or_else(|| panic!("{stdout}"));
    assert!(summary.ends_with(" Newton iterations"), "{summary}");
    assert!(!stdout.contains("source stepping"), "{stdout}");
}

#[test]
fn reports_source_stepping_when_direct_newton_fails() {
    let stdout = success("Hundred volts\nVin n1 0 DC 100\nD1 n1 n2\nR n2 0 50\n.dc\n.end\n");
    assert!(
        stdout.lines().any(|line| line == "nonlinear DC analysis"),
        "{stdout}"
    );
    assert!(
        stdout
            .lines()
            .any(|line| line.starts_with("converged with source stepping: ")
                && line.contains(" rejected, ")),
        "{stdout}"
    );
    assert_results(&stdout, &[("V(n1)", 100.0), ("V(n2)", 99.1194)]);
}

#[test]
fn unsolvable_nonlinear_circuits_fail_with_a_diagnostic() {
    let input = Input::new("Reverse\nI1 a 0 DC 1e-3\nD1 a 0\n.dc\n.end\n");
    let output = run(&[input.path.as_os_str()]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("ERROR"), "{stderr}");
    assert!(
        stderr.contains("input with spaces.cir: nonlinear DC analysis failed"),
        "{stderr}"
    );
    // miette wraps long causes and help across lines
    let words = stderr.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(words.contains("source stepping stopped"), "{stderr}");
    assert!(words.contains("Possible causes:"), "{stderr}");
    assert!(
        words.contains("current is forced backwards through a diode"),
        "{stderr}"
    );
    assert!(words.contains("not connected to the rest"), "{stderr}");
}

#[test]
fn singular_circuits_still_validate_without_dc() {
    let stdout = success("Floating\nR1 a b 10\nR2 c 0 1\n.end\n");
    assert!(stdout.contains("no analysis requested"), "{stdout}");
}

const DIVIDER: &str = "Divider\nVin n1 0 DC 6\nR1 n1 n2 100\nR2 n2 0 50\n.dc\n.end\n";

#[test]
fn results_have_aligned_labels_and_units() {
    let stdout = success(DIVIDER);
    for line in ["V(n1)  = 6 V", "V(n2)  = 2 V", "I(Vin) = -0.04 A"] {
        assert!(
            stdout.lines().any(|output| output == line),
            "{line:?}\n{stdout}"
        );
    }
}

/// Parse the rows of a printed matrix section into `(row label, values)`.
fn matrix(stdout: &str, name: &str, columns: usize) -> Vec<(String, Vec<f64>)> {
    let mut lines = stdout
        .lines()
        .skip_while(|line| *line != format!("{name}:"))
        .skip(2);
    let mut rows = Vec::new();
    for line in lines.by_ref() {
        let tokens: Vec<_> = line.split_whitespace().collect();
        if tokens.len() <= columns {
            break;
        }
        let (label, values) = tokens.split_at(tokens.len() - columns);
        rows.push((
            label.join(" "),
            values.iter().map(|value| value.parse().unwrap()).collect(),
        ));
    }
    rows
}

fn dump_mna(text: &str) -> Output {
    let input = Input::new(text);
    run(&[input.path.as_os_str(), OsStr::new("--dump-mna")])
}

#[test]
fn dumped_matrices_have_stable_labels_and_values() {
    let output = dump_mna(DIVIDER);
    assert!(output.status.success(), "{output:?}");
    let stdout = output_text(output.stdout);
    let positions: Vec<_> = ["[0] V(n1)", "[1] V(n2)", "[2] I(Vin)"]
        .iter()
        .map(|label| {
            stdout
                .find(label)
                .unwrap_or_else(|| panic!("{label}\n{stdout}"))
        })
        .collect();
    assert!(positions.is_sorted(), "{stdout}");
    assert_eq!(
        matrix(&stdout, "G", 3),
        [
            ("KCL n1".to_owned(), vec![0.01, -0.01, 1.0]),
            ("KCL n2".to_owned(), vec![-0.01, 0.03, 0.0]),
            ("branch Vin".to_owned(), vec![1.0, 0.0, 0.0]),
        ]
    );
    assert!(
        matrix(&stdout, "C", 3)
            .iter()
            .all(|(_, row)| row == &[0.0; 3])
    );
    let b: Vec<_> = matrix(&stdout, "b", 1)
        .into_iter()
        .map(|(_, row)| row[0])
        .collect();
    assert_eq!(b, [0.0, 0.0, 6.0]);
    assert!(stdout.find("G:").unwrap() < stdout.find("linear DC analysis").unwrap());
    assert_results(&stdout, &[("V(n2)", 2.0)]);
}

#[test]
fn dumped_storage_terms_and_branch_rows_are_labeled() {
    let output =
        dump_mna("Storage\nI1 0 a DC 1\nR1 a 0 1\nC1 a 0 2e-6\nL1 a b 3e-3\nR2 b 0 1\n.end\n");
    assert!(output.status.success(), "{output:?}");
    let stdout = output_text(output.stdout);
    let c = matrix(&stdout, "C", 3);
    assert_eq!(c[0], ("KCL a".to_owned(), vec![2e-6, 0.0, 0.0]));
    assert_eq!(c[2], ("branch L1".to_owned(), vec![0.0, 0.0, -3e-3]));
    assert!(stdout.contains("no analysis requested"), "{stdout}");
}

#[test]
fn matrices_remain_visible_when_the_solve_fails() {
    let output = dump_mna("Floating\nR1 a b 10\nR2 c 0 1\n.dc\n.end\n");
    assert!(!output.status.success());
    let stdout = output_text(output.stdout);
    assert_eq!(
        matrix(&stdout, "G", 3),
        [
            ("KCL a".to_owned(), vec![0.1, -0.1, 0.0]),
            ("KCL b".to_owned(), vec![-0.1, 0.1, 0.0]),
            ("KCL c".to_owned(), vec![0.0, 0.0, 1.0]),
        ]
    );
    assert!(
        !stdout.lines().any(|line| line == "linear DC analysis"),
        "{stdout}"
    );
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("singular")
    );
}

/// rows of the symbolic `f(x)` section as `(row label, terms)`
fn f_rows(stdout: &str) -> Vec<(String, String)> {
    stdout
        .lines()
        .skip_while(|line| *line != "f(x):")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .map(|line| {
            let (label, terms) = line.split_once("  ").unwrap();
            (label.trim().to_owned(), terms.trim().to_owned())
        })
        .collect()
}

#[test]
fn dumped_mna_shows_f_symbolically_and_at_the_solution() {
    let output =
        dump_mna("* diode circuit\nVin n1 0 DC 1\nD1 n1 n2\nR n2 0 50\nD2 0 n2\n.dc\n.end\n");
    assert!(output.status.success(), "{output:?}");
    let stdout = output_text(output.stdout);

    assert!(stdout.contains("G x + C dx/dt + f(x) = b"), "{stdout}");
    assert_eq!(
        f_rows(&stdout),
        [
            ("KCL n1".to_owned(), "+id(D1)".to_owned()),
            ("KCL n2".to_owned(), "-id(D1) -id(D2)".to_owned()),
            ("branch Vin".to_owned(), "0".to_owned()),
        ]
    );
    for line in [
        "id(D1) = 1e-15 * (exp((V(n1) - V(n2)) / 0.025) - 1)",
        "id(D2) = 1e-15 * (exp((-V(n2)) / 0.025) - 1)",
    ] {
        assert!(
            stdout.lines().any(|output| output == line),
            "{line}\n{stdout}"
        );
    }

    // G holds only the linear stamps; f appears between C and b
    assert_eq!(
        matrix(&stdout, "G", 3),
        [
            ("KCL n1".to_owned(), vec![0.0, 0.0, 1.0]),
            ("KCL n2".to_owned(), vec![0.0, 0.02, 0.0]),
            ("branch Vin".to_owned(), vec![1.0, 0.0, 0.0]),
        ]
    );
    let position = |text: &str| stdout.find(text).unwrap();
    assert!(position("C:") < position("f(x):") && position("f(x):") < position("b:"));

    // after solving, f is evaluated at the operating point before the results
    let f: Vec<_> = matrix(&stdout, "f(x*)", 1)
        .into_iter()
        .map(|(_, row)| row[0])
        .collect();
    assert_eq!(f.len(), 3, "{stdout}");
    assert!((f[0] - 0.00534629).abs() < 1e-8, "{stdout}");
    assert!((f[1] + 0.00534629).abs() < 1e-8, "{stdout}");
    assert_eq!(f[2], 0.0);
    assert!(position("f(x*):") < position("nonlinear DC analysis"));
}

#[test]
fn linear_circuits_dump_a_zero_f() {
    let output = dump_mna(DIVIDER);
    assert!(output.status.success(), "{output:?}");
    let stdout = output_text(output.stdout);
    assert!(stdout.lines().any(|line| line == "f(x) = 0"), "{stdout}");
    assert!(!stdout.contains("f(x*)"), "{stdout}");
}

#[test]
fn symbolic_f_remains_visible_when_the_solve_fails() {
    let output = dump_mna("Reverse\nI1 a 0 DC 1e-3\nD1 a 0\n.dc\n.end\n");
    assert!(!output.status.success());
    let stdout = output_text(output.stdout);
    assert_eq!(
        f_rows(&stdout),
        [("KCL a".to_owned(), "+id(D1)".to_owned())]
    );
    assert!(!stdout.contains("f(x*)"), "{stdout}");
    assert!(results(&stdout).is_empty(), "{stdout}");
}

fn trace(text: &str) -> Output {
    let input = Input::new(text);
    run(&[input.path.as_os_str(), OsStr::new("--trace-newton")])
}

const DIODE_DC: &str = "* diode circuit\nVin n1 0 DC 1\nD1 n1 n2\nR n2 0 50\n.dc\n.end\n";

/// one row of a single-diode Newton table
#[derive(Debug)]
struct TraceRow {
    iteration: usize,
    vd: f64,
    residual_in: f64,
    voltage_change: f64,
    node: String,
    residual_out: f64,
}

/// rows of the first Newton table after `heading`, for circuits with one diode
fn newton_rows(stdout: &str, heading: &str) -> Vec<TraceRow> {
    let mut lines = stdout
        .lines()
        .skip_while(|line| !line.starts_with(heading))
        .skip_while(|line| !line.trim_start().starts_with("iter"));
    let header: Vec<_> = lines.next().unwrap().split_whitespace().collect();
    assert_eq!(
        header,
        ["iter", "vd(D1)", "Φ", "in", "ΔV", "node", "Φ", "out"],
        "{stdout}"
    );

    lines
        .map_while(|line| {
            let tokens: Vec<_> = line.split_whitespace().collect();
            let [
                iteration,
                vd,
                residual_in,
                voltage_change,
                node,
                residual_out,
            ] = tokens[..]
            else {
                return None;
            };
            Some(TraceRow {
                iteration: iteration.parse().ok()?,
                vd: vd.parse().ok()?,
                residual_in: residual_in.parse().ok()?,
                voltage_change: voltage_change.parse().ok()?,
                node: node.to_owned(),
                residual_out: residual_out.parse().ok()?,
            })
        })
        .collect()
}

#[test]
fn traces_direct_newton_on_the_handout_circuit() {
    let output = trace(DIODE_DC);
    assert!(output.status.success(), "{output:?}");
    let stdout = output_text(output.stdout);

    assert!(
        stdout.lines().any(|line| line == "Newton trace"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Φ in: max |Φ(x_k)|, the residual put into the solve"),
        "{stdout}"
    );
    let rows = newton_rows(
        &stdout,
        "direct Newton on the full circuit, from x = 0: converged",
    );
    assert!(rows.len() > 2, "{stdout}");
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.iteration, index + 1);
        assert!(["V(n1)", "V(n2)"].contains(&row.node.as_str()), "{row:?}");
    }

    // iteration 1 starts from x = 0, where only the source constraint fails
    assert_eq!(rows[0].vd, 0.0);
    assert_eq!(rows[0].residual_in, 1.0);
    assert_eq!(
        (rows[0].voltage_change, rows[0].node.as_str()),
        (1.0, "V(n1)")
    );

    // iteration 2 linearizes at vd = 1 V and solves with the 235 A residual
    // that iteration 1 left, stepping V(n2) up by about Vt
    assert_eq!(rows[1].vd, 1.0);
    assert!((rows[1].residual_in - 235.4).abs() < 0.1, "{stdout}");
    assert_eq!(rows[1].node, "V(n2)");
    assert!((rows[1].voltage_change - 0.025).abs() < 1e-4, "{stdout}");
    assert!((rows[2].vd - 0.975).abs() < 1e-6, "{stdout}");

    // each residual left by one iteration is the next iteration's input
    for pair in rows.windows(2) {
        assert_eq!(pair[1].residual_in, pair[0].residual_out, "{stdout}");
    }

    let last = rows.last().unwrap();
    assert!((last.vd - 0.732686).abs() < 1e-6, "{stdout}");
    assert!(
        last.voltage_change.abs() < 1e-9 && last.residual_out < 1e-12,
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("converged in {} Newton iterations", rows.len())),
        "{stdout}"
    );
    assert!(!stdout.contains("source stepping"), "{stdout}");
    assert!(stdout.find("Newton trace").unwrap() < stdout.find("nonlinear DC analysis").unwrap());
    assert_eq!(results(&stdout), results(&success(DIODE_DC)));
}

#[test]
fn traces_one_voltage_column_per_diode() {
    let output = trace("Two diodes\nVin a 0 DC 1\nD1 a b\nR1 b 0 50\nD2 0 b\n.dc\n.end\n");
    assert!(output.status.success(), "{output:?}");
    let stdout = output_text(output.stdout);
    let header = stdout
        .lines()
        .find(|line| line.starts_with("iter"))
        .unwrap();
    assert_eq!(
        header.split_whitespace().collect::<Vec<_>>(),
        [
            "iter", "vd(D1)", "vd(D2)", "Φ", "in", "ΔV", "node", "Φ", "out"
        ]
    );
}

#[test]
fn traces_each_source_stepping_attempt() {
    let output = trace("Hundred volts\nVin n1 0 DC 100\nD1 n1 n2\nR n2 0 50\n.dc\n.end\n");
    assert!(output.status.success(), "{output:?}");
    let stdout = output_text(output.stdout);

    assert!(
        stdout.contains("direct Newton on the full circuit, from x = 0: failed"),
        "{stdout}"
    );
    let attempts: Vec<Vec<&str>> = stdout
        .lines()
        .skip_while(|line| !line.starts_with("source stepping:"))
        .skip(2)
        .take_while(|line| !line.is_empty())
        .map(|line| line.split_whitespace().collect())
        .collect();
    // attempt, from, ->, to, step, result, iterations
    assert!(attempts.iter().all(|row| row.len() == 7), "{attempts:?}");
    assert!(attempts.iter().any(|row| row[5] == "rejected"), "{stdout}");
    assert_eq!(attempts.last().unwrap()[3], "1");
    assert_eq!(attempts.last().unwrap()[5], "accepted");
    let headings = stdout
        .lines()
        .filter(|line| line.starts_with("attempt ") && line.contains(", λ = "))
        .count();
    assert_eq!(headings, attempts.len(), "{stdout}");
    assert_results(&stdout, &[("V(n2)", 99.1194)]);
}

#[test]
fn the_trace_remains_visible_when_the_nonlinear_solve_fails() {
    let output = trace("Reverse\nI1 a 0 DC 1e-3\nD1 a 0\n.dc\n.end\n");
    assert!(!output.status.success());
    let stdout = output_text(output.stdout);

    assert!(stdout.contains("Newton trace"), "{stdout}");
    assert!(stdout.contains("source stepping stopped"), "{stdout}");
    assert!(results(&stdout).is_empty(), "{stdout}");
    assert!(!stdout.contains("nonlinear DC analysis"), "{stdout}");
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("nonlinear DC analysis failed")
    );
}

#[test]
fn linear_circuits_note_that_newton_was_not_used() {
    let output = trace(DIVIDER);
    assert!(output.status.success(), "{output:?}");
    let stdout = output_text(output.stdout);
    assert!(stdout.contains("Newton-Raphson was not used"), "{stdout}");
    assert_eq!(results(&stdout), results(&success(DIVIDER)));
}

fn dump_lu(text: &str) -> Output {
    let input = Input::new(text);
    run(&[input.path.as_os_str(), OsStr::new("--dump-lu")])
}

fn dense_matrix(stdout: &str, name: &str, size: usize) -> DMatrix<f64> {
    let rows = matrix(stdout, name, size);
    assert_eq!(rows.len(), size, "{name}\n{stdout}");
    let values: Vec<_> = rows.into_iter().flat_map(|(_, row)| row).collect();
    DMatrix::from_row_slice(size, size, &values)
}

#[test]
fn dumped_lu_factors_reconstruct_the_pivoted_mna_system() {
    let input = Input::new("Pivot cycle\nV1 a b DC 1\nR1 a 0 2\nR2 b 0 4\n.dc\n.end\n");
    let output = run(&[
        input.path.as_os_str(),
        OsStr::new("--dump-mna"),
        OsStr::new("--dump-lu"),
    ]);
    assert!(output.status.success(), "{output:?}");
    let stdout = output_text(output.stdout);
    let g = dense_matrix(&stdout, "G", 3);
    let p = dense_matrix(&stdout, "P", 3);
    let l = dense_matrix(&stdout, "L", 3);
    let u = dense_matrix(&stdout, "U", 3);
    let identity = DMatrix::identity(3, 3);

    assert_ne!(p, identity, "the circuit must exercise row pivoting");
    assert_ne!(p, p.transpose(), "the row permutation must be a cycle");
    assert_eq!(p.transpose() * &p, identity);
    let expected_rows = ["[0 <- 2] branch V1", "[1 <- 0] KCL a", "[2 <- 1] KCL b"];
    for name in ["L", "U"] {
        let labels: Vec<_> = matrix(&stdout, name, 3)
            .into_iter()
            .map(|(label, _)| label)
            .collect();
        assert_eq!(labels, expected_rows, "{name}\n{stdout}");
    }
    let l_header = stdout
        .lines()
        .skip_while(|line| *line != "L:")
        .nth(1)
        .unwrap();
    assert_eq!(
        l_header.split_whitespace().collect::<Vec<_>>(),
        ["pivot", "0", "pivot", "1", "pivot", "2"]
    );
    // printed factors have six significant digits
    assert!((&p * &g - &l * &u).amax() < 1e-6, "{stdout}");
    assert!(stdout.find("U:").unwrap() < stdout.find("linear DC analysis").unwrap());
}

#[test]
fn lu_dump_preserves_the_solution_without_requesting_mna_output() {
    let normal = success(DIVIDER);
    let output = dump_lu(DIVIDER);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let stdout = output_text(output.stdout);

    assert_eq!(results(&stdout), results(&normal));
    for name in ["P:", "L:", "U:"] {
        assert!(stdout.lines().any(|line| line == name), "{stdout}");
    }
    assert!(!stdout.lines().any(|line| line == "G:"), "{stdout}");
}

#[test]
fn lu_factors_remain_visible_when_the_solve_fails() {
    let output = dump_lu("Floating\nR1 a b 10\nR2 c 0 1\n.dc\n.end\n");
    assert!(!output.status.success());
    let stdout = output_text(output.stdout);
    for name in ["P", "L", "U"] {
        assert_eq!(matrix(&stdout, name, 3).len(), 3, "{stdout}");
    }
    assert!(results(&stdout).is_empty(), "{stdout}");
    assert!(
        !stdout.lines().any(|line| line == "linear DC analysis"),
        "{stdout}"
    );
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("singular")
    );
}

#[test]
fn lu_dump_does_not_request_dc_analysis() {
    let output = dump_lu("No analysis\nV1 a 0 DC 2\nR1 a 0 1\n.end\n");
    assert!(output.status.success(), "{output:?}");
    let stdout = output_text(output.stdout);
    assert!(stdout.contains("no analysis requested"), "{stdout}");
    assert!(results(&stdout).is_empty(), "{stdout}");
    for name in ["P", "L", "U"] {
        assert_eq!(matrix(&stdout, name, 2).len(), 2, "{stdout}");
    }
}

#[test]
fn lu_dump_handles_an_empty_system() {
    let output = dump_lu("Empty\n.dc\n.end\n");
    assert!(output.status.success(), "{output:?}");
    let stdout = output_text(output.stdout);
    assert_eq!(stdout.matches("(empty)").count(), 3, "{stdout}");
    assert!(results(&stdout).is_empty(), "{stdout}");
    assert!(
        stdout.lines().any(|line| line == "linear DC analysis"),
        "{stdout}"
    );
}

#[test]
fn nonlinear_lu_dump_factors_the_jacobian_at_the_operating_point() {
    let input = Input::new(DIODE_DC);
    let output = run(&[
        input.path.as_os_str(),
        OsStr::new("--dump-mna"),
        OsStr::new("--dump-lu"),
    ]);
    assert!(output.status.success(), "{output:?}");
    let stdout = output_text(output.stdout);

    assert!(
        stdout.contains("LU decomposition of J = G + ∂f/∂x at the operating point"),
        "{stdout}"
    );
    assert!(
        stdout.lines().any(|line| line == "P * J = L * U"),
        "{stdout}"
    );
    assert!(
        !stdout.lines().any(|line| line == "P * G = L * U"),
        "{stdout}"
    );

    let g = dense_matrix(&stdout, "G", 3);
    let p = dense_matrix(&stdout, "P", 3);
    let l = dense_matrix(&stdout, "L", 3);
    let u = dense_matrix(&stdout, "U", 3);
    let jacobian = p.transpose() * &l * &u;

    // J - G is the diode conductance in the resistor pattern on n1 and n2
    let gd = (0.00534629 + 1e-15) / 0.025;
    let expected = DMatrix::from_row_slice(3, 3, &[gd, -gd, 0.0, -gd, gd, 0.0, 0.0, 0.0, 0.0]);
    assert!((&jacobian - &g - expected).amax() < 1e-5, "{stdout}");

    // the factorization follows the solve and precedes the results
    let position = |text: &str| stdout.find(text).unwrap();
    assert!(position("f(x*):") < position("P * J"));
    assert!(position("U:") < position("nonlinear DC analysis"));
    assert_eq!(results(&stdout), results(&success(DIODE_DC)));
}

#[test]
fn nonlinear_lu_dump_needs_an_operating_point() {
    for (text, reason, succeeds) in [
        (DIODE_CIRCUIT, "no analysis was requested", true),
        (
            "Reverse\nI1 a 0 DC 1e-3\nD1 a 0\n.dc\n.end\n",
            "the nonlinear solve did not converge",
            false,
        ),
    ] {
        let output = dump_lu(text);
        assert_eq!(output.status.success(), succeeds, "{output:?}");
        let stdout = output_text(output.stdout);
        let words = stdout.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(words.contains(reason), "{stdout}");
        assert!(words.contains("no Jacobian is available"), "{stdout}");
        assert!(!stdout.lines().any(|line| line == "P:"), "{stdout}");
    }
}

#[test]
fn input_errors_render_source_context_and_exit_without_success_output() {
    for (text, expected) in [
        ("Title\nR1 a 0 100", "finish the netlist with `.end`"),
        ("Τitle\r\nR1 α 0 invalid\r\n.end\r\n", "R1 α 0 invalid"),
        (
            "Title\nR1 a 0 100\nr1 b 0 50\n.end\n",
            "first declared here",
        ),
        (
            "Title\nCCCS1 out 0 missing 2\n.end\n",
            "unknown controlling element",
        ),
    ] {
        let input = Input::new(text);
        let output = run(&[input.path.as_os_str()]);
        assert!(!output.status.success(), "{text:?}");
        assert!(output.stdout.is_empty(), "{text:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("ERROR"), "{stderr}");
        assert!(!stderr.contains("Error:"), "{stderr}");
        assert!(!stderr.contains("ferrispice::netlist"), "{stderr}");
        assert!(!stderr.contains("\\x1b["), "{stderr}");
        let (header, report) = stderr.split_once('\n').expect("separate log header");
        assert!(header.contains("line "), "{stderr}");
        assert!(header.contains("column "), "{stderr}");
        assert!(!header.contains('×'), "{stderr}");
        assert!(report.lines().next().unwrap().contains('×'), "{stderr}");
        let (_, diagnostic) = stderr.split_once('×').expect("graphical diagnostic");
        assert!(diagnostic.contains("\u{1b}["), "{stderr}");
        assert!(stderr.contains("input with spaces.cir"), "{stderr}");
        assert!(stderr.contains(expected), "{stderr}");
    }
}

#[test]
fn missing_ground_names_the_input_without_blaming_a_record() {
    let input = Input::new("Ungrounded\nR1 a b 100\n.end\n");
    let output = run(&[input.path.as_os_str()]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let (header, report) = stderr.split_once('\n').expect("separate log header");
    assert!(header.contains("ERROR"), "{stderr}");
    assert!(header.contains("input with spaces.cir"), "{stderr}");
    assert!(header.contains("ground node 0"), "{stderr}");
    assert!(
        report.contains("connect at least one element terminal"),
        "{stderr}"
    );
    assert!(!report.contains("R1 a b 100"), "{stderr}");
}

#[test]
fn missing_files_report_the_input_path() {
    let input = Input::new("");
    let missing = input.directory.join("missing.cir");
    let output = run(&[missing.as_os_str()]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("could not read netlist"), "{stderr}");
    assert!(stderr.contains("missing.cir"), "{stderr}");
}

#[test]
fn help_succeeds_and_invalid_arguments_fail() {
    for help in ["--help", "-h"] {
        let output = run(&[OsStr::new(help)]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        assert_usage(&output.stdout);
    }

    for arguments in [
        vec![],
        vec![OsStr::new("one.cir"), OsStr::new("two.cir")],
        vec![OsStr::new("--unknown")],
    ] {
        let output = run(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_usage(&output.stderr);
    }
}
