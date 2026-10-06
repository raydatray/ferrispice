# 0003: Linear DC Analysis

## Objective

Solve the MNA system from [0002](0002-mna-assembly.md) and return the voltages
and currents required by [Deliverable 1](<../Deliverable 1.html>).

```text
validated circuit
-> assemble G, C, b, and unknown layout
-> solve G x = b
-> named circuit quantities
```

This is a library-level analysis API. Command-line integration follows in
[0004](0004-cli-and-delivery.md).

## Scope

This plan covers dense LU solving, input and numerical checks, DC analysis,
typed voltage/current results, derived element currents, and hand-calculated
circuit validation.

## Solver Organization

```text
crates/solver/src/
├── lib.rs
├── linear.rs
└── dc.rs
```

`linear` works with matrices and vectors without knowing netlist syntax or
component types. `dc` consumes circuit and MNA APIs and interprets results.

```rust
pub fn solve_linear(
    a: &DMatrix<f64>,
    b: &DVector<f64>,
) -> Result<DVector<f64>, SolveError>;

pub fn solve_dc(circuit: &Circuit) -> Result<DcSolution, DcError>;
```

Checking whether `.dc` was requested is a binary orchestration decision.
The library can solve a circuit directly without a fabricated directive.

## Linear Solve

Set time derivatives to zero and solve `G x = b` using LU with pivoting.
MNA matrices are not generally positive-definite, and controlled sources can
make them nonsymmetric. Do not rely on Cholesky or explicitly compute an inverse.

Check that the matrix is square, the RHS dimension matches, and all entries
are finite. Check that the solution is finite and satisfies a scale-aware
residual bound:

```text
r = G x - b
```

Use documented absolute and relative tolerances with row scaling. A large
coefficient in one equation must not hide a poor result in another.

Factorization failure is a singular-system error. A non-finite solution or
unacceptable residual is a numerical error. Do not silently perturb the
matrix to manufacture a solution.

## Circuit Results

Map the solution through the unknown layout rather than exposing raw indices:

```rust
impl DcSolution {
    pub fn node_voltage(&self, node: NodeId) -> Option<f64>;
    pub fn element_current(&self, element: ElementId) -> Option<f64>;
}
```

Ground voltage is exactly zero. Missing IDs are distinct from valid zero
results. Result iteration follows stable circuit order.

Use the positive-to-negative current convention from 0002:

| Element | DC current |
| --- | --- |
| Resistor | `(v[p] - v[n]) / R` |
| Capacitor | `0` |
| Inductor | Solved branch-current unknown |
| Independent voltage source | Solved branch-current unknown |
| Independent current source | Specified signed DC value |
| VCVS | Solved branch-current unknown |
| VCCS | `gm * (v[cp] - v[cn])` |
| CCVS | Solved branch-current unknown |
| CCCS | `gain * controlling_branch_current` |

Derived currents are post-processing and do not add unknowns. Retain layout
metadata needed by matrix inspection in 0004.

## Errors

Distinguish invalid numerical inputs, assembly failure, singular systems,
and unacceptable numerical results. Preserve causes across error layers.

Floating nodes and ideal-source constraints can produce singular systems.
Report what the solver establishes without guessing a specific topology defect.
The binary adds file context and formats diagnostics.

## Diff Stack

| Subject | Deliverable |
| --- | --- |
| `[1/3][d1][solver] solve dense linear systems with LU` | Introduce solver crate; linear API, input checks, factorization, residual checks, errors |
| `[2/3][d1][solver] expose linear DC circuit analysis` | Assembly, node voltages, explicit branch currents |
| `[3/3][d1][solver] derive element currents and validate circuits` | All element currents and mixed-circuit validation |

Every diff builds and verifies the behavior it introduces. The numerical API
is usable before the circuit-facing API is complete.

## Test Plan

Linear tests cover a known solution, pivoting, dimension mismatch, singularity,
non-finite input, and residual quality.

Circuit tests use independently calculated answers for:

- The handout divider.
- A resistor driven by a current source, including reversed source direction.
- A capacitor in parallel with a resistor, confirming zero capacitor DC current.
- A resistor/inductor circuit, confirming zero inductor voltage and its current.
- Each controlled-source type, with nonzero controls and signed-gain coverage.
- A current controller whose sensing source occurs later in the input.
- A floating circuit and incompatible ideal voltage-source constraints.

Divider results:

```text
V(n1)  = 6 V
V(n2)  = 2 V
I(Vin) = -0.04 A
I(R1)  = 0.04 A
I(R2)  = 0.04 A
```

Check KCL and the assembled-system residual as well as expected values.
Compare floating-point results with tolerances. Failed solves must not return
fabricated successful results.

Run `cargo test --workspace` after each diff.

## Acceptance Criteria

The plan is complete when:

- Valid linear DC circuits solve through the library API.
- The divider produces the stated voltages and currents.
- Capacitors and inductors have correct DC behavior.
- All controlled-source types pass hand-calculated checks.
- Callers query results by circuit IDs rather than matrix indices.
- Every required element has a correctly oriented current result.
- Invalid or singular systems produce structured errors.
- Successful results pass residual and circuit-consistency checks.
