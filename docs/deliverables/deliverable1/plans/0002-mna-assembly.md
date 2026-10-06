# 0002: MNA Assembly

## Objective

Convert the validated circuit from [0001](0001-netlist-and-circuit.md) into the
MNA matrices required by [Deliverable 1](<../Deliverable 1.html>).

```text
validated circuit
-> allocate all unknowns
-> stamp every element
-> matrices with labeled rows and columns
```

The completed system feeds [0003](0003-linear-dc-analysis.md).

## Scope

This plan covers deterministic unknown allocation, ground handling, a dense
`nalgebra` matrix builder, all nine required stamps, static and storage terms,
right-hand-side contributions, and mathematical validation.

## Equation Form

Represent the linear circuit as:

```text
G x + C dx/dt = b
```

`G` contains static conductance and constraints. `C` is the storage matrix,
including capacitor and inductor terms. `b` contains independent-source terms.
Storage stamps express the required reactive elements; the deliverable solves
only the DC equation `G x = b`.

```rust
pub struct MnaSystem {
    pub g: DMatrix<f64>,
    pub c: DMatrix<f64>,
    pub b: DVector<f64>,
    pub layout: UnknownLayout,
}

pub fn assemble(circuit: &Circuit) -> Result<MnaSystem, AssemblyError>;
```

## Unknown Layout

Order unknowns as non-ground node voltages in circuit node order, followed by
branch currents in circuit element order.

Allocate branch currents for independent voltage sources, inductors, VCVS,
and CCVS. Other required elements do not receive their own current unknowns
under the agreed representation.

The layout maps node and element IDs to indices and retains labels. Ground
has no equation or unknown; node lookup returns no matrix index for ground.
Allocate the entire layout before stamping so forward current-control
references work.

## Matrix Builder

Start `G`, `C`, and `b` at zero. Stamps add rather than overwrite entries.
Provide small ground-aware helpers for node/branch entries, RHS additions,
the two-terminal four-entry pattern, and voltage-defined branch incidence.

```text
crates/mna/src/
├── lib.rs
├── layout.rs
├── builder.rs
└── stamps/
    ├── passive.rs
    ├── independent.rs
    └── controlled.rs
```

Dispatch on the circuit element enum into focused stamp functions. Parsing,
printing, and solving remain responsibilities of their respective crates.

## Sign Convention

For terminals `p`, `n`, voltage is `v[p] - v[n]` and positive current flows
from `p` to `n`. Nodal equations express outgoing currents. Omit entries
involving grounded nodes, and accumulate repeated-terminal contributions so
cancelling terms cancel.

## Passive and Independent Stamps

### Resistor

For `g = 1 / R`:

```text
G[p,p] += g       G[p,n] -= g
G[n,p] -= g       G[n,n] += g
```

### Independent Current Source

For current `I` from `p` to `n`:

```text
b[p] -= I
b[n] += I
```

### Independent Voltage Source

For branch-current index `k` and voltage `V`:

```text
G[p,k] += 1       G[n,k] -= 1
G[k,p] += 1       G[k,n] -= 1
b[k]   += V
```

The branch row enforces `v[p] - v[n] = V`.

### Capacitor

For capacitance `c`:

```text
C[p,p] += c       C[p,n] -= c
C[n,p] -= c       C[n,n] += c
```

No static conductance is added; a capacitor is open at DC.

### Inductor

Allocate branch index `k`, add voltage-source incidence entries to `G`,
leave the branch RHS at zero, and add `C[k,k] -= L`.

The equation is `v[p] - v[n] - L di/dt = 0`. At DC this imposes zero terminal
voltage while retaining an explicit current unknown.

## Controlled-Source Stamps

Use the confirmed sensing contract from 0001. Voltage controls use `cp`, `cn`;
current controls use the preallocated branch index `j`.

### VCVS

Allocate output branch `k`, add voltage-source incidence entries, then:

```text
G[k,cp] -= gain
G[k,cn] += gain
```

This enforces `v[p] - v[n] = gain * (v[cp] - v[cn])`.

### VCCS

For transconductance `gm`:

```text
G[p,cp] += gm     G[p,cn] -= gm
G[n,cp] -= gm     G[n,cn] += gm
```

### CCVS

Allocate output branch `k`, add voltage-source incidence entries, then:

```text
G[k,j] -= transresistance
```

This enforces `v[p] - v[n] = transresistance * x[j]`.

### CCCS

```text
G[p,j] += gain
G[n,j] -= gain
```

## Errors

Assembly errors identify the responsible element. Missing controlling-current
indices produce errors rather than accessing unrelated columns. Singular
systems are still assembled; their diagnosis belongs to 0003.

## Diff Stack

| Subject | Deliverable |
| --- | --- |
| `[1/10][d1][mna] allocate unknowns and matrix storage` | Introduce mna crate; layout, labels, ground-aware builder |
| `[2/10][d1][mna] stamp resistors` | Conductance accumulation |
| `[3/10][d1][mna] stamp independent current sources` | Signed RHS injection |
| `[4/10][d1][mna] stamp independent voltage sources` | Branch incidence and constraints |
| `[5/10][d1][mna] stamp capacitors` | Nodal storage terms |
| `[6/10][d1][mna] stamp inductors` | Branch incidence and inductive storage |
| `[7/10][d1][mna] stamp voltage-controlled voltage sources` | Controlled voltage constraint |
| `[8/10][d1][mna] stamp voltage-controlled current sources` | Controlled KCL terms |
| `[9/10][d1][mna] stamp current-controlled voltage sources` | Current-controlled voltage constraint |
| `[10/10][d1][mna] stamp current-controlled current sources` | Current-controlled KCL terms |

Each diff builds and validates its equations. During the stack, unimplemented
element kinds return explicit errors rather than silently contributing nothing.

## Test Plan

Check expected equations and matrices for every element, grounded terminals,
repeated terminals, shared-entry accumulation, reversed source polarity,
controlled-source signs, and forward controller references. Verify that
capacitors affect `C` rather than `G` and inductors retain a DC short constraint.

For the handout divider, order unknowns as `V(n1), V(n2), I(Vin)`:

```text
G = [  0.01  -0.01   1 ]
    [ -0.01   0.03   0 ]
    [  1      0      0 ]

b = [ 0, 0, 6 ]
C = all zeros
```

Use appropriate floating-point tolerances. Run `cargo test --workspace` after
each stamping diff.

## Acceptance Criteria

The plan is complete when:

- All nine elements produce mathematically correct stamps.
- Unknown allocation is complete before stamping and labels are stable.
- Ground adds no extra row or column.
- Contributions accumulate and current controls use the correct columns.
- The divider produces the stated matrices.
- `G`, `C`, `b`, and the layout are ready for 0003.
