### ferrispice
rusty spice

## Development

The Rust 2024 workspace grows with each implementation stack. Stack 0001
provides the `ferrispice-netlist` parser and `ferrispice-circuit` compiler.
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
`cargo run -- --help` for usage. MNA assembly and DC solving are implemented by
the following stacks.

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
