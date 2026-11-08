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
    let mut checked: Vec<_> = examples().map(|(file, _, _)| file.to_owned()).collect();
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
