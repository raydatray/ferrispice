# 0007: Newton-Raphson DC Analysis

## Objective

Solve the nonlinear DC equation from [0006](0006-nonlinear-mna.md) with
Newton-Raphson and return the voltages and currents required by
[Deliverable 2](<../Deliverable 2.html>).

```text
validated circuit
-> assemble G, C, b, and nonlinear devices
-> Newton on the full circuit (fast path)
-> if that fails: ramp the sources up with adaptive steps
-> named circuit quantities
```

The Newton solver and the adaptive step controller are reusable library
components. Deliverable 3 calls Newton at each backward-Euler time point and
uses the same step controller to adapt the time step. DC analysis is their
first caller. Command-line integration follows in
[0008](0008-cli-and-delivery.md).

## Scope

This plan covers a generic Newton solver over `A x + f(x) - r`, step and
residual convergence, a generic adaptive step controller, source stepping
for DC, iteration and step history, DC analysis routing, diode currents, and
hand-validated circuits.

## Solver Organization

```text
crates/solver/src/
├── lib.rs
├── linear.rs       unchanged
├── newton.rs       Newton on A x + f(x) - r
├── stepping.rs     adaptive step controller
└── dc.rs           DC analysis: fast path, source stepping, results
```

`linear` is unchanged. `newton` evaluates devices through `MnaSystem` but does
not know which analysis is running. `stepping` knows nothing about circuits;
it advances a scalar parameter by calling a supplied solve. `dc` composes
them and interprets results.

## Newton-Raphson

### Problem

```rust
pub struct NewtonProblem<'a> {
    pub system: &'a MnaSystem,  // provides f and ∂f/∂x
    pub a: &'a DMatrix<f64>,    // DC: G        BE: G + C/h
    pub r: &'a DVector<f64>,    // DC: λ b      BE: b(t) + (C/h) x_n
}

pub struct NewtonOptions {
    pub max_iterations: usize,  // 50
    pub reltol: f64,            // 1e-9
    pub vntol: f64,             // 1e-9 V
    pub abstol: f64,            // 1e-12 A
}

pub fn solve_newton(
    problem: &NewtonProblem,
    initial: &DVector<f64>,
    options: &NewtonOptions,
) -> Result<NewtonSolution, NewtonError>;
```

The problem is defined by `A`, `r`, and the initial guess rather than by
`G` and `b`, so source stepping and backward Euler each supply their own
values without changing the solver.

### Mathematics

```text
Φ(x) = A x + f(x) - r
J(x) = A + ∂f/∂x
```

Each iteration linearizes `Φ` with a first-order Taylor expansion and solves
for the update:

```text
J(x_k) Δx = -Φ(x_k)
x_{k+1}   = x_k + Δx
```

`A` and `r` are constant during the solve. Only `f` and `∂f/∂x` are
re-evaluated, using the analytic derivatives from 0006. Each update is one
call to the existing `solve_linear` LU solver.

### Convergence

Converge only when both hold:

1. **Update:** each unknown satisfies
   `|Δx_i| <= reltol * max(|x_{k+1,i}|, |x_{k,i}|) + tol_i`, where `tol_i`
   is `vntol` for voltages and `abstol` for currents.
2. **Residual:** `Φ(x_{k+1})` passes a row-scaled bound. KCL rows use
   `abstol` plus `reltol` times the largest current magnitude contributing to
   the row; branch rows use `vntol`.

The relative tolerance is tighter than the six printed significant digits
so reported digits are reliable; validation showed that `1e-6` left KCL
errors of about `1e-7` relative at 20 A. `solve_linear`'s own residual check verifies
the linear update only; Newton convergence is checked separately.

The iteration limit is deliberately modest. An attempt that is not
converging fails quickly so the step controller can retry with a smaller
step.

### Failures

`NewtonError` distinguishes:

- **Nonconvergence:** iteration limit reached; worst unknown, final update,
  final residual.
- **Singular Jacobian:** iteration and the underlying `SolveError`.
- **Non-finite evaluation:** iteration and the `DeviceError` from 0006, or a
  non-finite iterate.

Each is a failed attempt. Newton never returns its last iterate as a
solution and never perturbs the matrix.

### History

`NewtonSolution` contains the solution, the final `J` so the binary can
factor it for inspection, and one `NewtonIteration` per iteration:

- `linearized_at`: `x_k`, where `f` and `J` were evaluated
- `residual_in`: `max |Φ(x_k)|`, the residual put into the solve
- `delta`: `Δx`
- `worst_unknown` and `update`: the unknown whose update is largest relative
  to its tolerance, and its `|Δx|`
- `residual`: `max |Φ(x_k + Δx)|`

Each iteration's `linearized_at` is the previous `linearized_at + delta`,
and its `residual_in` is the previous `residual`. Keeping the iterates lets
the binary show diode voltages and node-voltage steps without the solver
knowing how they are presented.

## Adaptive Step Controller

The controller advances a scalar parameter from `start` to `end`. Each step
attempts a solve from the last accepted state. A converged attempt is
accepted and the step doubles; a failed attempt is rejected and the step
halves, retrying from the same accepted state. It stops with an error when
the step falls below a minimum.

```rust
pub struct StepOptions {
    pub initial_step: f64,      // DC: 0.1
    pub min_step: f64,          // DC: 1e-15
    pub max_step: Option<f64>,  // DC: none
}

pub fn advance<S, E>(
    start: f64,
    end: f64,
    initial: S,
    options: &StepOptions,
    attempt: impl FnMut(f64, f64, &S) -> Result<S, E>,  // (from, to, state)
) -> Result<SteppingOutcome<S, E>, SteppingError<E>>;
```

```text
p = start, step = initial_step, state = initial
while p < end:
    to = min(p + step, end), capped by max_step
    attempt(p, to, state)
        success → accept: p = to, state = result, step × 2
        failure → reject: keep p and state,       step ÷ 2
    step < min_step → stop with SteppingError
```

The outcome lists every accepted point in order and every attempt with its
parameter values, step, result, and Newton iteration count. The error
carries the parameter reached, the final step, and the last attempt's
failure.

The controller is generic over the solved state and error, so Deliverable 3
reuses it with time as the parameter: `from` and `to` give the backward-Euler
step `h = to - from`, and the accepted points form the transient waveform.

## DC Analysis

`solve_dc` assembles the system and chooses:

- **Linear circuits:** the Deliverable 1 path, a single `solve_linear` of
  `G x = b`, with unchanged results and errors.
- **Nonlinear circuits:**
  1. **Fast path:** Newton on the full circuit, `A = G`, `r = b`, from
     `x = 0`. If it converges, return the result.
  2. **Source stepping:** otherwise, scale every independent source by
     `λ` and ramp `λ` from 0 to 1 with the step controller.

### Source Stepping

Independent sources appear only in `b`, so scaling the sources scales the
right-hand side:

```text
G x + f(x) = λ b
```

At `λ = 0`, all sources are off and `x = 0` is the exact solution, so the
ramp starts from a known state. At `λ = 1`, the equation is the original
circuit. Each attempt solves Newton with `A = G`, `r = λ_to b`, starting from
the solution at the last accepted `λ`. Because consecutive `λ` values are
close, each attempt begins near its answer.

With `initial_step = 0.1`, an uninterrupted ramp visits
`λ = 0.1, 0.3, 0.7, 1.0`. A failure at `0.7` retries at `0.5`.

The minimum step is `1e-15` rather than a more typical `1e-6`. A diode at
`x = 0` conducts only `Is / Vt`, so the first Newton update for a current
source driving it is roughly `I / (Is / Vt)` volts. The source must be
scaled down to about `Is` before that update stays within `exp`'s range;
the doubling steps then reach full strength in a few dozen attempts.

Large overshoots usually end an attempt as a singular linear solve rather
than at the iteration limit: once `gd` is enormous, the other pivots fall
below `solve_linear`'s relative singularity threshold. Either way the attempt
fails and the step halves.

If the controller gives up, `solve_dc` returns a source-stepping error that
preserves the fast-path failure, the `λ` reached, and the last attempt's
`NewtonError`.

### Results

`DcSolution` additionally records whether Newton ran, whether source
stepping was needed, and the full attempt and iteration history. Results
remain queryable by circuit IDs.

| Element | DC current |
| --- | --- |
| Diode | `Is * (exp((v[anode] - v[cathode]) / Vt) - 1)` at the converged `x` |

All other element currents are unchanged from Deliverable 1.

## Errors

`DcError` gains a boxed `SourceSteppingFailure` variant holding the
fast-path `NewtonError` and the `SteppingError`, preserving every cause.
`NewtonError` carries its failure kind and the iterations completed before
it failed, so traces remain available.
Failed solves never return fabricated results.

## Deliverable 3 Compatibility

Backward Euler with step `h` solves, at each time point:

```text
(G + C/h) x_{n+1} + f(x_{n+1}) = b(t_{n+1}) + (C/h) x_n
```

This is a `NewtonProblem` with `A = G + C/h`, `r = b(t_{n+1}) + (C/h) x_n`,
and `initial = x_n`, driven by `advance` with time as the parameter. The
following must hold for Deliverable 3 to reuse this stack without
restructuring:

- Newton takes `A`, `r`, and the initial guess as inputs.
- The step controller is independent of DC, `λ`, and circuits.
- The controller supplies both ends of each step and returns every accepted
  point.
- Options are passed per call, so transient can use its own step limits.
- Errors are structured so transient can attach the failing time point.

Time-dependent sources, `C/h` assembly, and transient output are
Deliverable 3 work.

## Diff Stack

| Subject | Deliverable |
| --- | --- |
| `[1/5][d2][solver] solve nonlinear problems with Newton-Raphson` | `newton` module, `NewtonProblem`, options, `J Δx = -Φ` loop, update and residual convergence, errors, history |
| `[2/5][d2][solver] add an adaptive step controller` | `stepping` module, accept/double, reject/halve, minimum step, attempt history |
| `[3/5][d2][solver] route DC analysis through Newton for nonlinear circuits` | Linear/nonlinear selection, fast path, diode currents, unchanged linear results |
| `[4/5][d2][solver] ramp sources when direct Newton fails` | Source stepping with `λ b`, combined errors, recorded history |
| `[5/5][d2][solver] validate nonlinear DC circuits` | Handout circuit and independently solved cases |

Every diff builds and verifies the behavior it introduces. Newton and the
step controller are usable before DC routing is complete.

## Test Plan

Newton tests on small synthetic problems cover a known root, quadratic
convergence near the root, a supplied initial guess, nonconvergence at the
iteration limit, a singular Jacobian, and non-finite evaluation.

Step-controller tests use a synthetic attempt function that fails above a
chosen step size. Verify the accepted sequence `0.1, 0.3, 0.7, 1.0` without
failures, halving after a rejection, retrying from the last accepted state,
never overshooting `end`, the minimum-step error, and a complete attempt
history.

Circuit tests compare against independent bisection solutions and check KCL
at the converged point:

- The handout circuit, `V(n1) = 1`, `V(n2) = 0.267314`,
  `I(Vin) = -0.00534629`, solved by the fast path without source stepping.
- Reverse bias, with diode current approximately `-Is`.
- Series diodes, anti-parallel diodes, and a diode driven by a current source.
- The handout topology driven at 5 V, 20 V, 100 V, and 1000 V. Direct Newton
  fails within the iteration limit or overflows; source stepping must
  converge, and at least one case must include a rejected step.
- A diode combined with a controlled source.
- A circuit that cannot be solved returns a source-stepping error preserving
  the underlying cause.
- All Deliverable 1 circuits produce identical results through the linear path.

Tests assert convergence, accuracy, and whether stepping occurred, not exact
iteration counts.

Run `cargo test --workspace` after each diff.

## Acceptance Criteria

The plan is complete when:

- Newton solves `A x + f(x) - r = 0` for supplied `A`, `r`, and initial guess.
- The handout circuit reproduces the stated results through the fast path.
- Convergence requires both the update and residual criteria.
- Circuits where direct Newton fails converge through source stepping.
- The step controller doubles on success, halves on failure, and stops at
  its minimum step with a structured error.
- Nonconvergence and singularity produce structured errors, never results.
- Linear circuits behave exactly as in Deliverable 1.
- Deliverable 3 can reuse Newton and the step controller without API changes.
