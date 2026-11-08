# 0008: CLI and Deliverable Integration

## Objective

Extend the `ferrispice` binary from
[0004](../../deliverable1/plans/0004-cli-and-delivery.md) with
[0005](0005-diode-element.md), [0006](0006-nonlinear-mna.md), and
[0007](0007-newton-dc-analysis.md), completing
[Deliverable 2](<../Deliverable 2.html>).

```text
netlist file
-> parse and compile
-> assemble G, C, b, and f
-> run requested DC analysis (linear or Newton)
-> print the labelled entries of x
```

The workflow should be reproducible from submitted source and easy to run
during the demonstration.

## Scope

Deliverable 1 already implements arguments, file loading, diagnostics, `.dc`
execution, result panels, `--dump-mna`, and `--dump-lu`. This plan adds
nonlinear result output, `f` in MNA inspection, Newton iteration tracing,
LU inspection of the final Jacobian, nonlinear solve diagnostics, examples,
end-to-end checks, and documentation.

## Command Contract

From the workspace root:

```sh
cargo run -- examples/diode.cir
cargo run -- examples/diode.cir --dump-mna
cargo run -- examples/diode.cir --trace-newton
cargo run -- examples/diode.cir --dump-lu
cargo run -- examples/diode.cir --dump-mna --trace-newton --dump-lu
```

`.dc` requests DC analysis. The binary does not choose the algorithm; the
solver selects linear or Newton from the assembled system, and the output
states which ran. All flags are independent, combinable, and never change
solution entries.

`examples/diode.cir` is the handout netlist verbatim:

```text
* diode circuit
Vin n1 0 DC 1
D1 n1 n2
R n2 0 50
.dc
.end
```

## Result Output

Nonlinear circuits use a titled `nonlinear DC analysis` panel with the
Deliverable 1 layout and a convergence summary:

```text
╭ nonlinear DC analysis ─────────────╮
│                                    │
│  * diode circuit                   │
│                                    │
│  V(n1)  = 1 V                      │
│  V(n2)  = 0.267314 V               │
│  I(Vin) = -0.00534629 A            │
│                                    │
│  converged in 17 Newton iterations │
│                                    │
╰────────────────────────────────────╯
```

When source stepping was needed, the summary states it, for example
`converged with source stepping: 7 steps accepted, 3 rejected,
64 Newton iterations`.

Values use the default `Is = 1e-15 A`. Linear circuits keep the
`linear DC analysis` panel unchanged.

Document that the handout's `IE = 0.0053` is the source-current magnitude;
under the established convention, a source delivering power reports negative
current.

## MNA Inspection

`--dump-mna` prints the course equation `G x + C x' + f(x) = b` as `G`, `C`,
`f`, and `b`, with ordered unknown labels and KCL/branch row labels.

`f` depends on `x`, so it is printed symbolically from `describe_f` before
solving. This preserves the Deliverable 1 rule that matrices remain visible
when a solve fails:

```text
f(x) =
  KCL n1   [ +id(D1) ]
  KCL n2   [ -id(D1) ]
  Vin      [ 0       ]

id(D1) = 1e-15 * (exp((V(n1) - V(n2)) / 0.025) - 1)
```

After successful analysis, the panel adds `f(x*)` evaluated at the solution.
Circuits without nonlinear devices print `f(x) = 0`.

## Newton Tracing

`--trace-newton` prints the solve history from 0007 as nested tables.

Each Newton attempt prints its iterations so that the plug-back is visible:
the residual one iteration leaves is the residual the next one solves with.

```text
iter  vd(D1)  Φ in      ΔV        node   Φ out
   1  0       1.000e0   1.000e0   V(n1)  2.354e2
   2  1       2.354e2   2.500e-2  V(n2)  8.659e1
   3  0.975   8.659e1   2.500e-2  V(n2)  3.186e1
```

`vd` is each diode's voltage at the linearization point (up to four diodes),
`Φ in` is `max |Φ(x_k)|`, `ΔV` is the largest node-voltage change with its
node, and `Φ out` is `max |Φ(x_k + Δx)|`. A legend above the tables explains
the columns. The first attempt is the fast path on the full circuit. When it fails, the trace states why, then prints each
source-stepping attempt as a header followed by its Newton iterations:

```text
attempt   λ from → to    step    result      iterations
1         0.0 → 0.1      0.1     accepted    12
2         0.1 → 0.3      0.2     accepted    9
3         0.3 → 0.7      0.4     rejected    50 (no convergence)
4         0.3 → 0.5      0.2     accepted    11
...
```

The trace is printed when the solve fails too, so failures can be inspected.
For linear circuits it prints a one-line note that Newton was not used.

`J` and `Φ` belong to the iteration rather than the circuit equations and are
not part of `--dump-mna`.

## LU Inspection

`--dump-lu` factors the matrix solved last: the final Jacobian
`J = G + ∂f/∂x` at the converged point for nonlinear circuits, or `G` for
linear circuits. The panel title states which. Layout, permutation, and row
provenance labels follow Deliverable 1. Without `.dc`, nonlinear circuits
have no operating point, so `--dump-lu` reports that no Jacobian is
available rather than factoring `G` silently.

## Errors and Exit Behavior

Nonconvergence, singular Jacobians, non-finite evaluation, and source
stepping that reaches its minimum step exit nonzero. Diagnostics name the
iteration, the worst unknown by label, the final update and residual, the
affected diode with its source span, and for source stepping the `λ` reached
and the last attempt's cause. Requested dumps
and traces up to the failure are still printed. No partial solution is shown
as successful.

## Examples and Documentation

Provide runnable examples for:

- The handout diode circuit (`examples/diode.cir`).
- A reverse-biased diode (`examples/diode-reverse.cir`).
- The handout topology driven at 100 V, which requires source stepping
  (`examples/diode-source-stepping.cir`).
- A diode mixed with Deliverable 1 elements (`examples/diode-mixed.cir`).

Document in the README or linked usage document: diode syntax, the model
equation, default parameters and why `Is = 1e-15 A` is used instead of the
handout's stated `1e-14 A`, current signs, the
`G x + C x' + f(x) = b` form, the Newton algorithm, tolerances, the fast path
and source stepping, and the new flags.

Keep the demonstration requirement visible; the handout lists demonstration
days as TBA.

## Diff Stack

| Subject | Deliverable |
| --- | --- |
| `[1/5][d2][cli] execute nonlinear DC analysis` | Nonlinear panel, iteration count, Newton diagnostics, unchanged linear output |
| `[2/5][d2][cli] display f in MNA inspection` | Symbolic `f`, evaluated `f(x*)`, row labels |
| `[3/5][d2][cli] trace Newton iterations and source steps` | `--trace-newton` attempt and iteration tables, output on failure, linear note |
| `[4/5][d2][cli] dump LU factors of the final Jacobian` | `--dump-lu` on `J`, panel labelling, no-operating-point handling |
| `[5/5][d2][docs] document and exercise deliverable two` | Examples, usage, end-to-end checks |

Each diff builds and carries its relevant checks.

## Test Plan

Invoke the binary against fixture files and verify:

- The handout netlist prints the expected values within tolerance.
- Deliverable 1 examples produce unchanged results and panels.
- `--dump-mna` shows `G`, `C`, symbolic `f`, `b`, and `f(x*)` after success.
- `--dump-mna` output remains visible on nonconvergence.
- `--trace-newton` shows the handout circuit converging on the fast path to
  update and residual tolerances, with no source stepping.
- Printed LU factors reconstruct `P * J = L * U` within display precision.
- The 100 V example converges, and its trace shows the failed fast path and
  the source-stepping attempts, including at least one rejection.
- A fixture with no solution, such as a current source forcing more than
  `Is` in reverse through a lone diode, exits nonzero with a useful
  diagnostic and visible trace.
- All flags combine without changing solution entries.
- Every documented example runs with its documented command.

Compare numbers with tolerances rather than freezing incidental whitespace.

Final verification from the workspace root:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -- examples/diode.cir --dump-mna --trace-newton --dump-lu
```

## Acceptance Criteria

The plan is complete when:

- The handout netlist runs unmodified and prints `V1`, `V2`, and the source
  current.
- Output states whether linear or Newton analysis ran.
- `G`, `C`, `f`, and `b` can be inspected during the demonstration.
- Newton progress, source-stepping attempts, and the final Jacobian's LU
  factors can be inspected.
- Failures have useful diagnostics and exit status.
- Documented build, test, and example commands pass.
- The `Is` choice and demonstration requirement are documented.
