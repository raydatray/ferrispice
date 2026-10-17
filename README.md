### ferrispice
rusty spice

A linear DC circuit simulator for Deliverable 1. It reads a SPICE-style
netlist, builds the modified nodal analysis (MNA) equations, and prints the
solved unknown vector `x` with voltage/current labels.

## Requirements

- Rust 1.98.1 or newer (`rust-version` in `Cargo.toml`), edition 2024.
- `Cargo.lock` is committed; builds use its exact dependency versions.

## Usage

From the workspace root:

```sh
cargo run -- examples/voltage-divider.cir
cargo run -- examples/voltage-divider.cir --dump-mna
cargo run -- examples/voltage-divider.cir --dump-lu
cargo run -- examples/voltage-divider.cir --dump-mna --dump-lu
cargo run -- --help
```

For a release build:

```sh
cargo build --release
./target/release/ferrispice examples/voltage-divider.cir
```

The program loads, parses, validates, and assembles the netlist. A `.dc`
record requests linear DC analysis; repeated `.dc` records still run one
analysis. Without `.dc`, the circuit is validated and assembled, and a short
summary is printed instead of results.

```text
╭ linear DC analysis ────╮
│                        │
│  Name of circuit       │
│                        │
│  V(n1)  = 6 V          │
│  V(n2)  = 2 V          │
│  I(Vin) = -0.04 A      │
│                        │
╰────────────────────────╯
```

MNA, LU, and result sections use titled, rounded Boxen panels. Sections wider
than the terminal use plain output so matrix columns remain intact.

Results contain exactly the entries of `x`, in MNA layout order: non-ground
node voltages followed by currents through voltage sources, inductors, VCVS,
and CCVS in element order. Values use six significant digits, switching to
scientific notation outside `[1e-3, 1e6)`. Magnitudes below `1e-12` print as `0`.

**Current sign convention:** positive current flows through an element from its
first terminal to its second. A source delivering power therefore reports a
negative current: `Vin n1 0` pushes current out of `n1`, so `I(Vin) = -0.04 A`.

`--dump-mna` prints `G`, `C`, `b`, and the unknown ordering before
solving, so they remain visible even if the solve fails. Rows are labeled
`KCL <node>` for node equations and `branch <element>` for branch constraints.

`--dump-lu` prints the row-permutation matrix `P` and triangular factors `L`
and `U` of `G`, with `P * G = L * U`. `P` maps original MNA rows to factor
positions. Rows of `L` and `U` show `[position <- original MNA row]` and the
original equation label: `[0 <- 2] branch Vin` means factor row 0 came from
original row 2. These labels record row provenance; elimination changes the
equations. Columns of `L` are pivot steps, while columns of `U` retain the
unknown labels because row pivoting does not reorder variables.

This is a separate diagnostic factorization; DC analysis computes its own
factors. The two flags can be used independently or together, including without
`.dc`, and dumps remain visible if solving fails.

Results go to stdout. Diagnostics go to stderr as an `ERROR` log line followed
by a source-highlighted report. Any input, validation, or solve failure exits
with a nonzero status and prints no partial results.

## Examples

Each example notes its hand-calculated answer in a comment:

| Command | Demonstrates |
| --- | --- |
| `cargo run -- examples/voltage-divider.cir` | Handout divider with an independent voltage source |
| `cargo run -- examples/current-source.cir` | Independent current-source excitation |
| `cargo run -- examples/storage-dc.cir` | Inductor short and capacitor open at DC |
| `cargo run -- examples/vcvs.cir` | Voltage-controlled voltage source |
| `cargo run -- examples/vccs.cir` | Voltage-controlled current source |
| `cargo run -- examples/ccvs.cir` | Current-controlled voltage source |
| `cargo run -- examples/cccs.cir` | Current-controlled current source with a forward reference |

`cargo test -p ferrispice --test examples` runs each command and checks its results.

## Linear DC analysis

```rust
let solution = ferrispice_solver::solve_dc(&circuit)?;
let v = solution.node_voltage(circuit.find_node("n2").unwrap());
let i = solution.element_current(circuit.find_element("R1").unwrap());
```

`ferrispice-solver` sets time derivatives to zero and solves `G x = b` with
partially pivoted LU. Inputs must be finite. A pivot no larger than
`n * EPSILON * max|G|` is reported as singular rather than perturbed. Each
equation's residual must satisfy `|r_i| <= 1e-12 + 1e-9 * (sum_j |G_ij x_j| + |b_i|)`.

Results are queried by circuit ID. Ground is exactly zero. Every element has a
current oriented from its first terminal to its second: resistors use
`(v[p] - v[n]) / R`, capacitors are open (zero), current sources report their
signed value, VCCS and CCCS currents are derived from their controls, and
voltage sources, inductors, VCVS, and CCVS report solved branch currents.
`solve_dc` assembles the system internally, keeping the circuit and its matrix
layout paired.

## Netlist syntax

The first line is the circuit title. `.end` terminates the input, and `.dc`
may appear anywhere before it. Blank lines and full-line `*` comments are
ignored. Prefixes, directives, and `DC` are case-insensitive. Node `0` is
ground, and at least one element terminal must connect to it; a control port
sensing `0` does not count.

Values use decimal or scientific notation: `100`, `-2.5`, `1e-6`, `4.7E3`.
Engineering suffixes such as `1k` are not accepted.

```text
R1 p n resistance
C1 p n capacitance
L1 p n inductance
V1 p n DC voltage
I1 p n DC current
VCVS1 p n cp cn gain
VCCS1 p n cp cn transconductance
CCVS1 p n controlling_element transresistance
CCCS1 p n controlling_element gain
```

`VCVS`, `VCCS`, `CCVS`, and `CCCS` prefixes take precedence over the
single-letter prefixes. The controlled-source argument order is the proposed
convention from
[plan 0001](docs/deliverables/deliverable1/plans/0001-netlist-and-circuit.md);
confirm it against class examples. A current controller must have an explicit
branch-current unknown: a voltage source, inductor, VCVS, or CCVS. A zero-volt
voltage source works as a current sensor. Forward references are supported.

Resistance must be nonzero. Capacitance and inductance must be nonnegative.
Source values and gains may be negative. Names are matched case-insensitively,
and duplicate element names are rejected.

## Equations

The circuit becomes `G x + C dx/dt = b`. The unknowns `x` are non-ground node
voltages followed by currents through voltage sources, inductors, VCVS, and
CCVS, in element order. Ground has no row or column.

| Element | Stamp |
| --- | --- |
| Resistor | Conductance `1/R` in `G` |
| Capacitor | `c` in `C` only; open at DC |
| Inductor | Branch current; zero-voltage constraint in `G`, `-L` in `C` |
| Voltage source | Branch current; `v[p] - v[n] = V` |
| Current source | `-I` at `p`, `+I` at `n` in `b` |
| VCVS | Branch current; `v[p] - v[n] = gain * (v[cp] - v[cn])` |
| VCCS | `gm * (v[cp] - v[cn])` flowing from `p` to `n` |
| CCVS | Branch current; `v[p] - v[n] = r * i[control]` |
| CCCS | `gain * i[control]` flowing from `p` to `n` |

DC analysis solves `G x = b` with partially pivoted LU. Inputs must be finite.
A pivot no larger than `n * EPSILON * max|G|` is reported as singular rather
than perturbed. Each equation's residual must satisfy
`|r_i| <= 1e-12 + 1e-9 * (sum_j |G_ij x_j| + |b_i|)`. Floating subcircuits
and conflicting ideal sources produce a singular-system error.

## Libraries

```rust
let parsed = ferrispice_netlist::parse(text)?;      // syntax and source spans
let circuit = ferrispice_circuit::compile(&parsed)?; // names, IDs, validation
let system = ferrispice_mna::assemble(&circuit)?;    // G, C, b, and layout
let solution = ferrispice_solver::solve_dc(&circuit)?;
let v = solution.node_voltage(circuit.find_node("n2").unwrap());
let i = solution.element_current(circuit.find_element("R1").unwrap());
```

| Path | Responsibility |
| --- | --- |
| `crates/netlist` | Netlist parsing, positions, byte spans |
| `crates/circuit` | Typed circuit model, name resolution, validation |
| `crates/mna` | Unknown layout and element stamps |
| `crates/solver` | Dense LU solve and DC results |
| `bin/ferrispice` | Arguments, files, output, diagnostics |

Results are queried by circuit ID, not matrix index. `solve_dc` assembles the
system internally, keeping the circuit and its matrix layout paired. Callers
can assemble a system separately to inspect matrices before solving. Libraries
return typed errors and never print or exit.
`cargo run -p ferrispice-circuit --example inspect -- examples/voltage-divider.cir`
dumps the resolved circuit model.

## Development

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -- examples/voltage-divider.cir --dump-mna --dump-lu
```

The application uses `RUST_LOG`, defaulting to `info`, and writes logs to
stderr. Parser, compiler, assembly, and solver events use their crate targets:

```sh
RUST_LOG=trace cargo run -- examples/voltage-divider.cir
```

## Submission

Submit the complete workspace: `Cargo.toml`, `Cargo.lock`, `bin/`, `crates/`,
`examples/`, and this README. Do not include `target/`. Upload the code to
myCourses by the deadline posted there.

The handout also requires scheduling a one-on-one demonstration during the
week of Monday, October 19 to run the submitted code. For the demo:

```sh
cargo run --release -- examples/voltage-divider.cir --dump-mna --dump-lu
```
