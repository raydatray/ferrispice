//! Run every shipped example exactly as documented and check its MNA unknown vector.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

const EXAMPLES: &[(&str, &[(&str, f64)])] = &[
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
            let value = value.split_whitespace().next()?.parse().ok()?;
            Some((label.trim().to_owned(), value))
        })
        .collect()
}

#[test]
fn every_example_solves_to_its_hand_calculated_results() {
    for &(file, expected) in EXAMPLES {
        for extra in [
            &[][..],
            &["--dump-mna"][..],
            &["--dump-lu"][..],
            &["--dump-mna", "--dump-lu"][..],
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
            assert!(
                stdout.lines().any(|line| line == "linear DC analysis"),
                "{file}\n{stdout}"
            );
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
    let mut checked: Vec<_> = EXAMPLES.iter().map(|&(file, _)| file.to_owned()).collect();
    checked.sort();
    assert_eq!(shipped, checked);
    for file in shipped {
        let command = format!("cargo run -- examples/{file}");
        assert!(readme.contains(&command), "README is missing `{command}`");
    }
}
