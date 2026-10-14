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

#[test]
fn validates_the_divider_from_a_path_containing_spaces() {
    let input = Input::new(include_str!("../../../examples/voltage-divider.cir"));
    let output = run(&[input.path.as_os_str()]);
    assert!(output.status.success(), "{:?}", output);
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Name of circuit"));
    assert!(stdout.contains("2 non-ground nodes, 3 elements"));
    assert!(stdout.contains("DC requested: true"));
}

#[test]
fn validates_a_circuit_without_an_analysis_request() {
    let input = Input::new("No analysis\nR1 a 0 100\n.end\n");
    let output = run(&[input.path.as_os_str()]);
    assert!(output.status.success(), "{:?}", output);
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("DC requested: false")
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
