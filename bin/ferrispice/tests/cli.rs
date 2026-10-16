use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

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

fn success(text: &str) -> String {
    let input = Input::new(text);
    let output = run(&[input.path.as_os_str()]);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// Parse `NAME(...) = value` result lines, ignoring units and alignment.
fn results(stdout: &str) -> HashMap<String, f64> {
    stdout
        .lines()
        .filter_map(|line| {
            let (label, value) = line.split_once('=')?;
            let value = value.split_whitespace().next()?.parse().ok()?;
            Some((label.trim().to_owned(), value))
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
    let stdout = String::from_utf8(output.stdout).unwrap();
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
    let stdout = String::from_utf8(output.stdout).unwrap();
    let c = matrix(&stdout, "C", 3);
    assert_eq!(c[0], ("KCL a".to_owned(), vec![2e-6, 0.0, 0.0]));
    assert_eq!(c[2], ("branch L1".to_owned(), vec![0.0, 0.0, -3e-3]));
    assert!(stdout.contains("no analysis requested"), "{stdout}");
}

#[test]
fn matrices_remain_visible_when_the_solve_fails() {
    let output = dump_mna("Floating\nR1 a b 10\nR2 c 0 1\n.dc\n.end\n");
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
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
