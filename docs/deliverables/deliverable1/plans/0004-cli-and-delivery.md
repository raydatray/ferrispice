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
-> print named voltages and currents
```

The complete workflow should be reproducible from submitted source and easy
to run during the one-on-one demonstration.

## Scope

Stack 0001 already implements netlist arguments, file loading, parsing, circuit
validation, source-aware diagnostics, and summary output. This plan extends
that pipeline with `.dc` execution, stable result formatting, matrix
inspection, solve diagnostics, example circuits, end-to-end checks, and
submission documentation.

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
cargo run -- examples/voltage-divider.cir --dump-matrices
```

Accept one netlist path and optional `--dump-matrices`. Help explains the
invocation and that `.dc` in the netlist requests the linear DC analysis
defined by Deliverable 1.

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

Print the title and analysis type, followed by stable circuit-order results
with voltage/current labels and units:

```text
Voltage divider
Linear DC analysis

V(0)   = 0 V
V(n1)  = 6 V
V(n2)  = 2 V

I(Vin) = -0.04 A
I(R1)  = 0.04 A
I(R2)  = 0.04 A
```

Positive current flows from the first terminal to the second. Document this
so negative current through a delivering voltage source is understandable.
Use sufficient significant digits and scientific notation where appropriate.

## Matrix Inspection

`--dump-matrices` prints `G`, `C`, `b`, and ordered unknown labels.
Distinguish node/KCL rows from branch-constraint rows. Formatting belongs to
the binary; the MNA crate exposes data and layout.

Print matrices before solving so they remain available when a singular
system causes analysis to fail.

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
| `[1/3][d1][cli] execute requested linear DC analysis` | Extend the existing application pipeline with assembly, solving, `.dc` behavior, and solve diagnostics |
| `[2/3][d1][cli] display circuit results and MNA matrices` | Labels, units, orientation, matrix inspection |
| `[3/3][d1][docs] document and exercise deliverable one` | Examples, usage, submission instructions, end-to-end checks |

Each diff builds and carries its relevant checks. The final diff validates
the complete workflow rather than postponing all testing until then.

## Test Plan

Invoke the binary against fixture files and verify:

- Divider success and expected named values.
- `.dc` at different positions before `.end`.
- Repeated `.dc` records do not duplicate output.
- No solved-result section when `.dc` is absent.
- Matrix values and stable unknown ordering with `--dump-matrices`.
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
cargo run -- examples/voltage-divider.cir --dump-matrices
```

## Acceptance Criteria

The plan is complete when:

- A netlist file drives the complete pipeline.
- `.dc` controls whether linear DC analysis runs.
- Voltages and currents have understandable labels, units, and signs.
- All required element types have runnable examples.
- MNA matrices can be inspected during the demonstration.
- Failures have useful diagnostics and exit status.
- Documented build, test, and example commands pass.
- Submission instructions and the demonstration requirement are documented.
