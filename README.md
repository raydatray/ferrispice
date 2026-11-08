# ferrispice
rusty spice - for ecse 472 

a DC circuit simulator that reads a SPICE-style netlist, builds the modified
nodal analysis (MNA) equations, and prints the solved unknown vector `x` with
voltage/current labels. linear circuits are solved directly; circuits with
diodes are solved by Newton-Raphson

## usage

from the workspace root:

```sh
cargo run -- examples/voltage-divider.cir
cargo run -- examples/voltage-divider.cir --dump-mna
cargo run -- examples/voltage-divider.cir --dump-lu
cargo run -- examples/voltage-divider.cir --dump-mna --dump-lu
cargo run -- examples/diode.cir --trace-newton
cargo run -- examples/diode.cir --dump-mna --trace-newton --dump-lu
cargo run -- --help
```

for a release build:

```sh
cargo build --release
./target/release/ferrispice examples/voltage-divider.cir
```

- the program loads, parses, validates, and assembles the netlist
- a`.dc` record requests DC analysis; repeated `.dc` records still run one analysis 
- without `.dc`, the circuit is validated, and a short summary is printed
instead of results
- linear circuits print a `linear DC analysis` panel; circuits with diodes
print a `nonlinear DC analysis` panel that also says how Newton-Raphson
converged

### examples

```sh
cargo run -- examples/voltage-divider.cir
cargo run -- examples/current-source.cir
cargo run -- examples/storage-dc.cir
cargo run -- examples/vcvs.cir
cargo run -- examples/vccs.cir
cargo run -- examples/ccvs.cir
cargo run -- examples/cccs.cir
cargo run -- examples/diode.cir
cargo run -- examples/diode-reverse.cir
cargo run -- examples/diode-source-stepping.cir
cargo run -- examples/diode-mixed.cir
```

### `--dump-mna` 
- prints `G`, `C`, `f(x)`, `b`, and the unknown ordering before solving
- rows are labeled `KCL <node>` for node equations and `branch <element>` for branch constraints 
- `f(x)` is printed symbolically: the signed diode currents in each row,
then each diode's equation in terms of the unknowns
- after a successful nonlinear solve, `f(x*)` is printed at the operating point

### `--dump-lu` 
- prints the row-permutation matrix `P` and triangular factors `L`
and `U`, with `P * G = L * U`
- for circuits with diodes, it factors the matrix Newton-Raphson solved last
instead: the Jacobian `J = G + ∂f/∂x` at the operating point, with
`P * J = L * U`. without a converged operating point, it says no Jacobian is
available
- `P` maps original MNA rows to factor positions
- rows of `L` and `U` show `[position <- original MNA row]` and the
original equation label: `[0 <- 2] branch Vin` means factor row 0 came from
original row 2 

### `--trace-newton`
- prints every Newton iteration, so the plug-back is visible row to row:

```text
iter  vd(D1)  Φ in      ΔV        node   Φ out
   1  0       1.000e0   1.000e0   V(n1)  2.354e2
   2  1       2.354e2   2.500e-2  V(n2)  8.659e1
   3  0.975   8.659e1   2.500e-2  V(n2)  3.186e1
```

- `vd` is each diode's voltage at `x_k`, where `f` and `J` are evaluated (up
to four diodes)
- `Φ in` is `max |Φ(x_k)|`, the residual put into `J(x_k) Δx = -Φ(x_k)`
- `ΔV` is the largest node-voltage change in `Δx`, and `node` is where it
happened
- `Φ out` is `max |Φ(x_k + Δx)|`, which becomes the next row's `Φ in`
- in the handout circuit, row 1 lands on `vd = 1 V`, leaving a 235 A KCL
error; row 2 solves with it and steps `V(n2)` up by `Vt = 25 mV`, because
`id / gd = Vt` for an exponential
- when Newton on the full circuit fails, it prints why, then a table of
source-stepping attempts (`λ` from/to, step, accepted or rejected,
iterations), followed by each attempt's iterations
- the trace is printed even when the solve fails
- for linear circuits it notes that Newton-Raphson was not used

## netlist syntax

- the first line is the circuit title, even if it starts with `*`
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
D1 anode cathode
```

- diodes have no parameters in the netlist; every diode uses
`Is = 1e-15 A` and `Vt = 0.025 V`

## equations

the circuit becomes `G x + C dx/dt + f(x) = b`. the unknowns `x` are non-ground
node voltages followed by currents through voltage sources, inductors, VCVS,
and CCVS, in element order. ground has no row or column. `f(x)` holds the
nonlinear diode currents leaving each node; diodes add no unknowns

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
| diode | `+id` at the anode, `-id` at the cathode in `f(x)`, with `id = Is * (exp(vd / Vt) - 1)` and `vd = v[anode] - v[cathode]` |

### linear DC analysis

- circuits without diodes solve `G x = b` with partially pivoted LU
- a pivot smaller than `n * EPSILON * max|G|` is reported as singular rather
than perturbed
- each equation's residual must satisfy `|r_i| <= 1e-12 + 1e-9 * (sum_j |G_ij x_j| + |b_i|)`. 
- floating subcircuits and conflicting ideal sources produce a singular-system error

### nonlinear DC analysis

circuits with diodes solve `Φ(x) = G x + f(x) - b = 0` by Newton-Raphson.
each iteration solves the linearized equation with the same LU solver:

```text
J(x_k) Δx = -Φ(x_k),   J = G + ∂f/∂x,   x_{k+1} = x_k + Δx
```

- `∂f/∂x` is analytic: each diode adds `gd = Is / Vt * exp(vd / Vt)` in the
same four-entry pattern as a resistor
- an iteration converges only when every update satisfies
`|Δx_i| <= 1e-9 * |x_i| + tol_i` (`1e-9 V` for voltages, `1e-12 A` for
currents) and every residual of `Φ` is within `1e-12 A` (KCL rows) or
`1e-9 V` (branch rows) plus `1e-9` times the row's largest term
- each attempt is limited to 50 iterations
- `exp` overflows above `vd ≈ 17.7 V`; that, a singular Jacobian, or the
iteration limit fails the attempt rather than producing a result

the solve first tries Newton on the full circuit from `x = 0`. the handout
circuit converges this way in 17 iterations. if that fails, every
independent source is scaled by `λ`, giving `G x + f(x) = λ b`, and `λ` is
ramped from 0 to 1:

- at `λ = 0` every source is off and `x = 0` is the exact solution
- each attempt starts from the last accepted solution, beginning with a step
of 0.1
- an accepted attempt doubles the step; a rejected attempt halves it and
retries from the same point
- the analysis fails once the step falls below `1e-15`. the minimum is tiny
because a diode at `x = 0` conducts only `Is / Vt`, so a current source
driving it must start near `Is`

a diode cannot carry more than `Is` in reverse, so a current source forcing
more than that backwards through a lone diode has no solution and fails with
a diagnostic

### libraries

```rust
let parsed = ferrispice_netlist::parse(text)?;      // syntax and source spans
let circuit = ferrispice_circuit::compile(&parsed)?; // names, IDs, validation
let system = ferrispice_mna::assemble(&circuit)?;    // G, C, b, f(x), and layout
let solution = ferrispice_solver::solve_dc(&circuit)?;
let v = solution.node_voltage(circuit.find_node("n2").unwrap());
let i = solution.element_current(circuit.find_element("D1").unwrap());
```

`ferrispice_solver::solve_newton` solves any `A x + f(x) - r = 0` from a
supplied initial guess, and `ferrispice_solver::advance` steps a parameter
adaptively, doubling on success and halving on failure; DC analysis uses
them for source stepping, and transient analysis can reuse them per time
step

| Path | Responsibility |
| --- | --- |
| `crates/netlist` | netlist parsing, positions, byte spans |
| `crates/circuit` | typed circuit model, diode defaults, name resolution, validation |
| `crates/mna` | unknown layout, element stamps, `f(x)` and its Jacobian |
| `crates/solver` | dense LU, Newton-Raphson, adaptive stepping, DC results |
| `bin/ferrispice` | arguments, files, output, diagnostics |
