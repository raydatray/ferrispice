# 0004: CLI and Deliverable Integration

## Objective

Extend the `ferrispice` binary from [0001](0001-netlist-and-circuit.md) with
[0002](0002-mna-assembly.md) and [0003](0003-linear-dc-analysis.md), completing
[Deliverable 1](<../Deliverable 1.html>).

```text
netlist file
-> parse and compile
-> assemble matrices
-> run requested DC analysis
-> print the labelled entries of the MNA unknown vector x
```

The complete workflow should be reproducible from submitted source and easy
to run during the one-on-one demonstration.

## Scope

Stack 0001 already implements netlist arguments, file loading, parsing, circuit
validation, source-aware diagnostics, and summary output. This plan extends
that pipeline with `.dc` execution, stable result formatting, framed output,
MNA and LU inspection, solve diagnostics, example circuits, end-to-end checks,
and submission documentation.

## Binary Layout

```text
bin/ferrispice/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── args.rs
    ├── diagnostics.rs
    ├── logger.rs
    └── output.rs
```

`main` owns orchestration, `args` owns arguments, `diagnostics` owns application
error presentation, and `output` owns result and matrix formatting. Libraries
do not read files, print, or exit the process.

## Command Contract

From the workspace root:

```sh
cargo run -- examples/voltage-divider.cir
cargo run -- examples/voltage-divider.cir --dump-mna
cargo run -- examples/voltage-divider.cir --dump-lu
cargo run -- examples/voltage-divider.cir --dump-mna --dump-lu
```

Accept one netlist path and independent `--dump-mna` and `--dump-lu` flags.
Help explains the invocation and that `.dc` in the netlist requests the linear
DC analysis defined by Deliverable 1.

Load, parse, compile, and assemble. Run DC analysis when `.dc` is present.
Repeated `.dc` records request one analysis rather than duplicate output.
Without `.dc`, validate and assemble, print a concise no-analysis summary,
and show matrices if requested. Missing `.end` remains an input error.

The handout example omits `.dc`; add it to `examples/voltage-divider.cir`:

```text
Voltage divider
Vin n1 0 DC 6
R1 n1 n2 100
R2 n2 0 50
.dc
.end
```

## Result Output

Print a titled `linear DC analysis` panel containing the circuit title and
exactly the entries of `x` in MNA layout order, with voltage/current labels
and units:

```text
╭ linear DC analysis ────╮
│                        │
│  Voltage divider       │
│                        │
│  V(n1)  = 6 V          │
│  V(n2)  = 2 V          │
│  I(Vin) = -0.04 A      │
│                        │
╰────────────────────────╯
```

Positive current flows from the first terminal to the second. Document this
so negative current through a delivering voltage source is understandable.
Use sufficient significant digits and scientific notation where appropriate.

Use Boxen for rounded, titled MNA, LU, and result panels. Keep matrix text
aligned inside each panel. Sections wider than the terminal use plain output
so the numeric columns are preserved.

## Matrix Inspection

`--dump-mna` prints `G`, `C`, `b`, and ordered unknown labels.
Distinguish node/KCL rows from branch-constraint rows. Formatting belongs to
the binary; the MNA crate exposes data and layout.

Print matrices before solving so they remain available when a singular
system causes analysis to fail.

`--dump-lu` prints `P`, `L`, and `U` with `P * G = L * U`. `P` maps original
MNA row positions to factor positions. Rows of `L` and `U` show both positions
as `[factor position <- original MNA row]`, with the original equation label.
These identify provenance through row swaps; elimination changes the equations.
Columns of `L` are pivot steps, and columns of `U` retain the unknown labels.
The binary computes this diagnostic factorization separately from the solver.
Both flags work without `.dc` and can be combined without changing solution entries.

## Errors and Exit Behavior

Successful validation or analysis exits successfully. Argument, file,
parsing, circuit, and solve errors exit with nonzero status.

Write diagnostics to stderr and results to stdout. Include file and line
context where available, the affected element or directive, and useful causes.
Recoverable errors must not panic or print a partial successful solution.

## Examples and Documentation

Provide runnable examples for the divider, independent current-source
excitation, capacitor/inductor DC behavior, and all four controlled-source
types using the confirmed course syntax.

Document grammar, numeric formats, `.dc` behavior, current signs, matrix form,
required Rust toolchain, and reproducible commands in the root README or a
linked usage document.

Document the workspace manifests, crate sources, lockfile, examples, and usage
instructions needed in the source submission. The handout also requires
scheduling a one-on-one demonstration during the week of Monday October 19.
Keep that requirement visible; use the actual course deadline rather than
infer one from the demonstration week.

## Diff Stack

| Subject | Deliverable |
| --- | --- |
| `[1/5][d1][cli] execute requested linear DC analysis` | Extend the existing application pipeline with assembly, solving, `.dc` behavior, and solve diagnostics |
| `[2/5][d1][cli] display circuit results and MNA matrices` | Labels, units, orientation, `--dump-mna` inspection |
| `[3/5][d1][cli] dump LU factors and row permutations` | `--dump-lu`, permutation/factor inspection, and unchanged solution output |
| `[4/5][d1][cli] frame terminal output with Boxen` | Rounded, titled output panels, aligned content, and plain output for wide sections |
| `[5/5][d1][docs] document and exercise deliverable one` | Examples, usage, submission instructions, end-to-end checks |

Each diff builds and carries its relevant checks. The final diff validates
the complete workflow rather than postponing all testing until then.

## Test Plan

Invoke the binary against fixture files and verify:

- Divider success and expected named values.
- `.dc` at different positions before `.end`.
- Repeated `.dc` records do not duplicate output.
- No solved-result section when `.dc` is absent.
- Matrix values and stable unknown ordering with `--dump-mna`.
- Printed LU factors reconstruct the pivoted MNA matrix within display precision.
- Factor-row labels track original equations through multiple row swaps, and
  columns of `L` are labeled as pivot steps.
- `--dump-lu` preserves solution entries and works alone or with `--dump-mna`.
- Numeric and factor checks accept panel framing while preserving labels and values.
- Requested matrices remain visible on a singular solve.
- Paths containing spaces work.
- Missing files, missing `.end`, unknown controllers, and singular circuits
  produce useful stderr diagnostics and nonzero status.
- Every documented example runs with the documented command.

Compare numbers with tolerances rather than freezing incidental whitespace.

Final verification from the workspace root:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -- examples/voltage-divider.cir --dump-mna --dump-lu
```

## Acceptance Criteria

The plan is complete when:

- A netlist file drives the complete pipeline.
- `.dc` controls whether linear DC analysis runs.
- Voltages and currents have understandable labels, units, and signs.
- All required element types have runnable examples.
- MNA matrices can be inspected during the demonstration.
- LU factors and row permutations can be inspected during the demonstration.
- Failures have useful diagnostics and exit status.
- Documented build, test, and example commands pass.
- Submission instructions and the demonstration requirement are documented.
