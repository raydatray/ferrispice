# ferrispice
rusty spice - for ecse 472 

a linear DC circuit simulator that reads a SPICE-style
netlist, builds the modified nodal analysis (MNA) equations, and prints the
solved unknown vector `x` with voltage/current labels

## usage

from the workspace root:

```sh
cargo run -- examples/voltage-divider.cir
cargo run -- examples/voltage-divider.cir --dump-mna
cargo run -- examples/voltage-divider.cir --dump-lu
cargo run -- examples/voltage-divider.cir --dump-mna --dump-lu
cargo run -- --help
```

for a release build:

```sh
cargo build --release
./target/release/ferrispice examples/voltage-divider.cir
```

- the program loads, parses, validates, and assembles the netlist
- a`.dc` record requests linear DC analysis; repeated `.dc` records still run one analysis 
- without `.dc`, the circuit is validated and assembled, and a short
summary is printed instead of results

### `--dump-mna` 
- prints `G`, `C`, `b`, and the unknown ordering before solving
- rows are labeled `KCL <node>` for node equations and `branch <element>` for branch constraints 

### `--dump-lu` 
- prints the row-permutation matrix `P` and triangular factors `L`
and `U` of `G`, with `P * G = L * U`
- `P` maps original MNA rows to factor positions
- rows of `L` and `U` show `[position <- original MNA row]` and the
original equation label: `[0 <- 2] branch Vin` means factor row 0 came from
original row 2 

## netlist syntax

- the first line is the circuit title
- `.end` terminates the input, and `.dc` may appear anywhere before it
- blank lines and full-line `*` comments are ignored
- prefixes, directives, and `DC` are case-insensitive
- node `0` is ground, and at least one element terminal must connect to it; a control port sensing `0` does not count
- values use decimal or scientific notation: `100`, `-2.5`, `1e-6`, `4.7E3`.
engineering suffixes such as `1k` are not accepted

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

## equations

the circuit becomes `G x + C dx/dt = b`. the unknowns `x` are non-ground node
voltages followed by currents through voltage sources, inductors, VCVS, and
CCVS, in element order. ground has no row or column.

| element | stamp |
| --- | --- |
| resistor | conductance `1/R` in `G` |
| capacitor | `c` in `C` only; open at DC |
| inductor | branch current; zero-voltage constraint in `G`, `-L` in `C` |
| voltage source | branch current; `v[p] - v[n] = V` |
| current source | `-I` at `p`, `+I` at `n` in `b` |
| VCVS | branch current; `v[p] - v[n] = gain * (v[cp] - v[cn])` |
| VCCS | `gm * (v[cp] - v[cn])` flowing from `p` to `n` |
| CCVS | branch current; `v[p] - v[n] = r * i[control]` |
| CCCS | `gain * i[control]` flowing from `p` to `n` |

- DC analysis solves `G x = b` with partially pivoted LU
- a pivot smaller than `n * EPSILON * max|G|` is reported as singular rather
than perturbed
- each equation's residual must satisfy `|r_i| <= 1e-12 + 1e-9 * (sum_j |G_ij x_j| + |b_i|)`. 
- floating subcircuits and conflicting ideal sources produce a singular-system error

### libraries

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
| `crates/netlist` | netlist parsing, positions, byte spans |
| `crates/circuit` | typed circuit model, name resolution, validation |
| `crates/mna` | unknown layout and element stamps |
| `crates/solver` | dense LU solve and DC results |
| `bin/ferrispice` | arguments, files, output, diagnostics |
