use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// examples solved directly by one LU factorization
const LINEAR: &[(&str, &[(&str, f64)])] = &[
    (
        "voltage-divider.cir",
        &[("V(n1)", 6.0), ("V(n2)", 2.0), ("I(Vin)", -0.04)],
    ),
    ("current-source.cir", &[("V(n1)", 2.0)]),
    (
        "storage-dc.cir",
        &[
            ("V(in)", 5.0),
            ("V(mid)", 4.0),
            ("V(out)", 4.0),
            ("I(Vin)", -1e-3),
            ("I(L1)", 1e-3),
        ],
    ),
    (
        "vcvs.cir",
        &[
            ("V(in)", 0.5),
            ("V(out)", 5.0),
            ("I(Vin)", -0.5e-3),
            ("I(VCVS1)", -2.5e-3),
        ],
    ),
    (
        "vccs.cir",
        &[("V(in)", 0.2), ("V(out)", 2.0), ("I(Vin)", 0.0)],
    ),
    (
        "ccvs.cir",
        &[
            ("V(in)", 5.0),
            ("V(sense)", 0.0),
            ("V(out)", 2.5),
            ("I(Vin)", -5e-3),
            ("I(Vsense)", 5e-3),
            ("I(CCVS1)", -2.5e-3),
        ],
    ),
    (
        "cccs.cir",
        &[
            ("V(in)", 5.0),
            ("V(sense)", 0.0),
            ("V(out)", 3.0),
            ("I(Vin)", -5e-3),
            ("I(Vsense)", 5e-3),
        ],
    ),
];

/// examples containing diodes, solved by Newton-Raphson; values are
/// independent bisection solutions rounded to the six printed digits
const NONLINEAR: &[(&str, &[(&str, f64)])] = &[
    (
        "diode.cir",
        &[("V(n1)", 1.0), ("V(n2)", 0.267314), ("I(Vin)", -0.00534629)],
    ),
    (
        "diode-reverse.cir",
        // V(n2) = -50 Is and I(Vin) = Is, both below the printed zero threshold
        &[("V(n1)", 5.0), ("V(n2)", 0.0), ("I(Vin)", 0.0)],
    ),
    (
        "diode-source-stepping.cir",
        &[("V(n1)", 100.0), ("V(n2)", 99.1194), ("I(Vin)", -1.98239)],
    ),
    (
        "diode-mixed.cir",
        &[
            ("V(in)", 1.0),
            ("V(b)", 2.0),
            ("V(c)", 1.24069),
            ("V(d)", 1.24069),
            ("I(Vin)", -1e-3),
            ("I(VCVS1)", -0.0155086),
            ("I(L1)", 0.00310172),
        ],
    ),
];

/// examples that request `.tran`; each is checked from its CSV by
/// `check_transient`
const TRANSIENT: &[&str] = &["half-wave-rectifier.cir", "rc-lowpass.cir", "rl-cos.cir"];

fn examples() -> impl Iterator<Item = (&'static str, &'static str, &'static [(&'static str, f64)])>
{
    LINEAR
        .iter()
        .map(|&(file, expected)| (file, "linear DC analysis", expected))
        .chain(
            NONLINEAR
                .iter()
                .map(|&(file, expected)| (file, "nonlinear DC analysis", expected)),
        )
}

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
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

#[test]
fn every_example_solves_to_its_hand_calculated_results() {
    for (file, panel, expected) in examples() {
        for extra in [
            &[][..],
            &["--dump-mna"][..],
            &["--dump-lu"][..],
            &["--trace-newton"][..],
            &["--dump-mna", "--trace-newton", "--dump-lu"][..],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_ferrispice"))
                .current_dir(workspace())
                .arg(format!("examples/{file}"))
                .args(extra)
                .env("RUST_LOG", "off")
                .output()
                .unwrap();
            assert!(output.status.success(), "{file}: {output:?}");
            let stdout = output_text(output.stdout);
            assert!(stdout.lines().any(|line| line == panel), "{file}\n{stdout}");
            let results = results(&stdout);
            assert_eq!(results.len(), expected.len(), "{file}\n{stdout}");
            for &(label, value) in expected {
                let actual = *results
                    .get(label)
                    .unwrap_or_else(|| panic!("{file}: missing {label}\n{stdout}"));
                assert!(
                    (actual - value).abs() <= 1e-9 * (1.0 + value.abs()),
                    "{file}: {label} expected {value}, got {actual}"
                );
            }
        }
    }
}

#[test]
fn every_example_is_documented_and_every_file_is_checked() {
    let readme = std::fs::read_to_string(workspace().join("README.md")).unwrap();
    let mut shipped: Vec<_> = std::fs::read_dir(workspace().join("examples"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".cir"))
        .collect();
    shipped.sort();
    let mut checked: Vec<_> = examples()
        .map(|(file, _, _)| file)
        .chain(TRANSIENT.iter().copied())
        .map(str::to_owned)
        .collect();
    checked.sort();
    assert_eq!(shipped, checked);
    for file in shipped {
        let command = format!("cargo run -- examples/{file}");
        assert!(readme.contains(&command), "README is missing `{command}`");
    }
}

#[test]
fn the_diode_example_is_the_handout_netlist() {
    let text = std::fs::read_to_string(workspace().join("examples/diode.cir")).unwrap();
    assert_eq!(
        text,
        "* diode circuit\nVin n1 0 DC 1\nD1 n1 n2\nR n2 0 50\n.dc\n.end\n"
    );
}

#[test]
fn the_rectifier_example_is_the_handout_netlist() {
    let text =
        std::fs::read_to_string(workspace().join("examples/half-wave-rectifier.cir")).unwrap();
    assert_eq!(
        text,
        "* a half-wave recitifier circuit\n\
         Vin n1 0 COS 1 60\n\
         * Vin is a cosine with a 1V amplitude and 60Hz frequency\n\
         D1 n1 n2\n\
         R1 n2 0 50\n\
         C1 n2 0 100e-6\n\
         .tran 0.05s  0.1ms\n\
         * run transient simulation for 0.05s with step size of 0.1ms\n\
         .end\n"
    );
}

/// a directory removed when the test ends
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("ferrispice-examples-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// a CSV column by header label
fn column(header: &[String], rows: &[Vec<f64>], label: &str) -> Vec<f64> {
    let index = header
        .iter()
        .position(|name| name == label)
        .unwrap_or_else(|| panic!("no {label} column in {header:?}"));
    rows.iter().map(|row| row[index]).collect()
}

/// a first-order response to `cos(ω t)` from `initial`:
/// `A cos(ω t - φ) + (initial - A cos φ) e^{-t/τ}`
fn first_order(omega: f64, tau: f64, initial: f64, t: f64) -> f64 {
    let amplitude = 1.0 / (1.0 + (omega * tau).powi(2)).sqrt();
    let phase = (omega * tau).atan();
    amplitude * (omega * t - phase).cos() + (initial - amplitude * phase.cos()) * (-t / tau).exp()
}

fn check_transient(file: &str, header: &[String], rows: &[Vec<f64>]) {
    let time = column(header, rows, "time");
    let omega = |frequency: f64| std::f64::consts::TAU * frequency;

    match file {
        "half-wave-rectifier.cir" => {
            assert_eq!(header, ["time", "V(n1)", "V(n2)", "I(Vin)"]);
            assert_eq!(time.len(), 501);
            // independent Python backward-Euler reference, DC initial state
            let n2 = column(header, rows, "V(n2)");
            for (index, expected) in [
                (0, 0.267314),
                (1, 0.267192),
                (10, 0.245161),
                (50, 0.111627),
                (100, 0.041472),
                (250, 0.057032),
                (500, 0.255129),
            ] {
                assert!(
                    (n2[index] - expected).abs() < 5e-7,
                    "t = {}: {} vs {expected}",
                    time[index],
                    n2[index]
                );
            }
            let n1 = column(header, rows, "V(n1)");
            for (&t, &v) in time.iter().zip(&n1) {
                assert!((v - (omega(60.0) * t).cos()).abs() < 1e-12, "{t}");
            }
        }
        "rc-lowpass.cir" => {
            let (r, c, h) = (1e3, 1e-6, 1e-4);
            let out = column(header, rows, "V(out)");
            assert_eq!(out.len(), 201);
            // the backward-Euler recurrence, exact to rounding
            let mut expected = 1.0;
            for (index, &t) in time.iter().enumerate().skip(1) {
                let source = (omega(100.0) * t).cos();
                expected = (source / r + c / h * expected) / (1.0 / r + c / h);
                assert!((out[index] - expected).abs() < 1e-12, "{t}");
            }
            // and within O(h) of the continuous response
            for (&t, &v) in time.iter().zip(&out) {
                let exact = first_order(omega(100.0), r * c, 1.0, t);
                assert!((v - exact).abs() < 0.05, "t = {t}: {v} vs {exact}");
            }
        }
        "rl-cos.cir" => {
            let (r, l, h) = (10.0, 10e-3, 1e-4);
            let current = column(header, rows, "I(L1)");
            assert_eq!(current[0], 0.1);
            let mut expected = 0.1;
            for (index, &t) in time.iter().enumerate().skip(1) {
                let source = (omega(100.0) * t).cos();
                expected = (source + l / h * expected) / (r + l / h);
                assert!((current[index] - expected).abs() < 1e-12, "{t}");
            }
            for (&t, &i) in time.iter().zip(&current) {
                let exact = first_order(omega(100.0), l / r, 1.0, t) / r;
                assert!((i - exact).abs() < 5e-3, "t = {t}: {i} vs {exact}");
            }
        }
        other => panic!("no transient check for {other}"),
    }
}

#[test]
fn every_transient_example_reproduces_its_reference_waveforms() {
    for file in TRANSIENT {
        let scratch = Scratch::new(file);
        let csv = scratch.0.join("waveforms.csv");
        let svg = scratch.0.join("plot.svg");
        let output = Command::new(env!("CARGO_BIN_EXE_ferrispice"))
            .current_dir(workspace())
            .arg(format!("examples/{file}"))
            .arg("--csv")
            .arg(&csv)
            .arg("--plot")
            .arg(&svg)
            .env("RUST_LOG", "off")
            .output()
            .unwrap();
        assert!(output.status.success(), "{file}: {output:?}");
        let stdout = output_text(output.stdout);
        assert!(
            stdout
                .lines()
                .any(|line| line == "transient analysis (backward Euler)"),
            "{file}\n{stdout}"
        );
        assert!(std::fs::read_to_string(&svg).unwrap().starts_with("<svg"));

        let text = std::fs::read_to_string(&csv).unwrap();
        let mut lines = text.lines();
        let header: Vec<String> = lines
            .next()
            .unwrap()
            .split(',')
            .map(str::to_owned)
            .collect();
        let rows: Vec<Vec<f64>> = lines
            .map(|line| {
                line.split(',')
                    .map(|value| value.parse().unwrap())
                    .collect()
            })
            .collect();
        check_transient(file, &header, &rows);
    }
}

#[test]
fn transient_examples_accept_every_inspection_flag() {
    for file in TRANSIENT {
        for extra in [
            &["--dump-mna"][..],
            &["--dump-lu"][..],
            &["--trace-newton"][..],
            &["--dump-mna", "--trace-newton", "--dump-lu"][..],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_ferrispice"))
                .current_dir(workspace())
                .arg(format!("examples/{file}"))
                .args(extra)
                .env("RUST_LOG", "off")
                .output()
                .unwrap();
            assert!(output.status.success(), "{file} {extra:?}: {output:?}");
            let stdout = output_text(output.stdout);
            assert!(
                stdout
                    .lines()
                    .any(|line| line == "transient analysis (backward Euler)"),
                "{file}\n{stdout}"
            );
            if extra.contains(&"--dump-mna") {
                assert!(
                    stdout.lines().any(|line| line == "b(t):"),
                    "{file}\n{stdout}"
                );
            }
        }
    }
}
