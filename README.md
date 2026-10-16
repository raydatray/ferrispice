### ferrispice
rusty spice

## Development

The Rust 2024 workspace grows with each implementation stack. Stack 0001
provides the `ferrispice-netlist` parser and `ferrispice-circuit` compiler.
Stack 0002 adds dense MNA assembly in `ferrispice-mna`.
The application binary initializes tracing; libraries emit events without
installing a subscriber.

```sh
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -- examples/voltage-divider.cir
```

The application reads a netlist, parses and validates the circuit, and prints
its title, node/element counts, and whether DC analysis was requested. Use
`cargo run -- --help` for usage. DC solving and matrix display are implemented
by the following stacks.

For a detailed dump of resolved nodes and elements, run the library example:

```sh
cargo run -p ferrispice-circuit --example inspect -- examples/voltage-divider.cir
```

Use the libraries directly:

```rust
let parsed = ferrispice_netlist::parse(text)?;
let circuit = ferrispice_circuit::compile(&parsed)?;
```

Circuit IDs are scoped to their circuit and are separate from future MNA row
and column indices. Nodes are ordered by first appearance, with ground first;
elements remain in netlist order. Name lookup is ASCII case-insensitive and
retains the original spelling for output. Positive current runs from the
first terminal to the second.

## MNA assembly

```rust
let system = ferrispice_mna::assemble(&circuit)?;
```

The assembled system is `G x + C dx/dt = b`, exposed as dense `nalgebra`
matrices `system.g`, `system.c`, and vector `system.b`. The layout retains
ordered labels and maps circuit IDs to matrix indices. Non-ground node
voltages come first, followed by currents through voltage sources, inductors,
VCVS, and CCVS in element order. Ground has no row or column.

All nine element kinds are stamped additively, including forward current
controls. Capacitors contribute only to `C`; inductors retain a zero-voltage
constraint at DC. Assembly accepts singular topologies; the solver handles
their diagnosis in stack 0003.

Run the MNA equation checks with `cargo test -p ferrispice-mna`.

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
requests linear DC analysis. Blank lines and full-line `*` comments are
ignored. Prefixes, directives, and `DC` are case-insensitive. Values use decimal
or scientific notation.

`NetlistPosition` identifies a line and character column in the input.
`ByteSpan` identifies an exact UTF-8 byte range. These are input metadata,
separate from circuit IDs and matrix indices.

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

The controlled-source argument order is the proposed convention from
[plan 0001](docs/deliverables/deliverable1/plans/0001-netlist-and-circuit.md);
confirm it against class examples. A current controller must have an explicit
branch-current unknown: a voltage source, inductor, VCVS, or CCVS. Forward
references are supported.

## Diagnostics

The application uses `miette` to render parser and compiler errors with
the input name, source snippets, and highlighted ranges. Duplicate-name reports
show both declarations. Errors are logged at `ERROR` level, followed by a
separate colored source report on stderr. Failed validation exits with a
nonzero status. The core libraries continue to return typed errors.

The application reads a path, including `/dev/stdin`, until EOF. The library
inspection example returns ordinary typed errors; source-highlighted reports
are rendered by the application.

## Tracing

The application uses `RUST_LOG`, defaulting to `info`, and writes logs to stderr:

```sh
RUST_LOG=trace cargo run -- examples/voltage-divider.cir
```

Parser and compiler events use the `ferrispice_netlist` and
`ferrispice_circuit` targets. The application initializes their subscriber;
library callers can install their own `tracing-subscriber`.
