# 0010: Backward-Euler Transient Analysis

## Objective

Integrate the circuit equation from [0009](0009-time-dependent-sources.md) in
time with the backward Euler method, for linear and nonlinear circuits, as
required by [Deliverable 3](<../Deliverable 3.html>).

```text
compiled circuit + .tran stop step
-> DC operating point at t = 0  (initial state x_0)
-> for each time point t_{n+1} = t_n + h:
       linear:    LU back-substitution with a pre-factored G + C/h
       nonlinear: Newton on (G + C/h) x + f(x) = b(t_{n+1}) + (C/h) x_n
-> waveform: every t_n with its unknown vector
```

## Scope

New module `crates/solver/src/transient.rs`. It reuses `solve_dc`,
`solve_linear`'s LU, `solve_newton`, and `advance` from Deliverable 2
without API changes.

## Discretization

Backward Euler replaces the derivative at the new point with a backward
difference:

```text
dx/dt |_{n+1} ≈ (x_{n+1} - x_n) / h

G x_{n+1} + C (x_{n+1} - x_n)/h + f(x_{n+1}) = b(t_{n+1})

(G + C/h) x_{n+1} + f(x_{n+1}) = b(t_{n+1}) + (C/h) x_n
 \_______/                        \___________________/
     A                                    r
```

This is exactly `NewtonProblem { a: A, r }` from 0007.

Element companion models fall out of the matrix form; no per-element
transient stamps are needed:

| Element | Companion model |
| --- | --- |
| Capacitor | conductance `C/h` in parallel with current source `(C/h) v_n` |
| Inductor | branch row `v - (L/h) i_{n+1} = -(L/h) i_n`, from the existing `-L` entry in `C` |

## Initial State

`x_0` is the DC operating point with every source at its `t = 0` value
(`Waveform::dc_value`), capacitors open and inductors shorted. This is
`solve_dc(circuit)`, unchanged. For the handout circuit it gives
`V(n1) = 1`, `V(n2) = 0.267314`.

The initial state is computed in one function, `initial_state`, so it can be
replaced if a different convention (for example `x_0 = 0`) is confirmed in
class. No command-line option is planned for it.

If the DC operating point fails (for example a node connected only through
capacitors, making `G` singular), transient returns
`TransientError::OperatingPoint(DcError)` preserving the cause.

## Time Grid

Fixed step from `.tran`: `t_n = n · h` for `n = 0..N`, `N = ceil(stop / h)`,
with the final point clamped to exactly `stop`. Points are computed as
`n * h`, not by repeated addition, to avoid drift. The handout gives
`N = 500` steps plus `t = 0`.

## Linear Circuits

When `system.is_linear()`:

1. Form `A = G + C/h` and factor it once (nalgebra `LU`), with the same
   singularity check as `solve_linear`.
2. Each step builds `r` and back-substitutes, then runs the existing
   residual check.
3. If the last step is shortened, factor its `A` separately.

This is the "linear transient analysis" part of the handout and is O(n²)
per step after one factorization.

## Nonlinear Circuits

For each step:

1. `A = G + C/h` (formed once per distinct `h`).
2. `r = b(t_{n+1}) + (C/h) x_n`.
3. `solve_newton` with `initial = x_n` and default `NewtonOptions`.

Using `x_n` as the guess keeps Newton near its answer; a 0.1 ms step on a
60 Hz source moves `Vin` by at most `2π·60·1e-4 ≈ 0.038 V`.

### Step Failure

If Newton fails for a step, subdivide that step with `advance` over
`[t_n, t_{n+1}]` using `StepOptions { initial_step: h/2, min_step: h·1e-9,
max_step: Some(h) }`. Each attempt solves backward Euler with its own
`h' = to - from`. Only `t_{n+1}` is recorded in the waveform; the
intermediate points are counted in statistics. If `advance` gives up,
return `TransientError::Step { time, failure }`.

This is a safety net. The handout circuit is expected to converge without
subdivision; a test asserts that.

## Results

```rust
pub struct TransientSolution {
    layout: UnknownLayout,
    times: Vec<f64>,
    states: Vec<DVector<f64>>,     // states[i] at times[i]; states[0] = x_0
    operating_point: DcSolution,
    stats: TransientStats,         // steps, subdivided steps, Newton iterations,
                                   // max iterations in one step
}

impl TransientSolution {
    pub fn times(&self) -> &[f64];
    pub fn node_voltage(&self, node: NodeId) -> Vec<f64>;
    pub fn element_current(&self, element: ElementId) -> Vec<f64>;
    pub fn columns(&self) -> impl Iterator<Item = (&Unknown, Vec<f64>)>;
}

pub fn solve_transient(circuit: &Circuit, stop: f64, step: f64)
    -> Result<TransientSolution, TransientError>;
```

Element currents reuse `dc::element_current` (made `pub(crate)`) for
resistive elements and branch unknowns. Capacitor current is
`C (v_{n+1} - v_n)/h`, and zero at `t = 0`.

## Errors

```rust
pub enum TransientError {
    OperatingPoint(DcError),
    Singular { time: f64, source: SolveError },       // linear path
    Step { time: f64, failure: SteppingError<..> },   // nonlinear path
}
```

All carry the failing time so the binary can report it.

## Diff Stack

| Subject | Deliverable |
| --- | --- |
| `[1/4][d3][solver] integrate linear circuits with backward Euler` | Time grid, DC initial state, pre-factored `G + C/h`, `TransientSolution` |
| `[2/4][d3][solver] integrate nonlinear circuits with Newton at each step` | Newton per step from `x_n` |
| `[3/4][d3][solver] subdivide transient steps when Newton fails` | `advance` fallback, statistics, `TransientError::Step` |
| `[4/4][d3][solver] validate transient circuits` | Analytical and handout tests below |

## Test Plan

Analytical linear circuits, compared with the exact backward-Euler recurrence
(exact to rounding) and with the continuous solution (error `O(h)`):

- **RC discharge:** a DC source charges `C` through `R` to a known operating
  point; set up so that the decay `v_n = v_0 / (1 + h/τ)^n` is checkable.
  Halving `h` roughly halves the error against `e^{-t/τ}`.
- **RC driven by `COS`:** after transients, amplitude and phase approach
  `1/sqrt(1 + (ωτ)²)` and `-atan(ωτ)` within `O(h)`.
- **RL** with an inductor branch current, checking the `-L` sign convention.
- **Purely resistive** `COS` circuit: every point equals the DC solution at
  that time.

Nonlinear, the handout circuit with `Is = 1e-15` (values from an independent
Python backward-Euler reference, DC initial state):

| `t` | `V(n2)` |
| --- | --- |
| 0 | 0.267314 |
| 0.1 ms | 0.267192 |
| 1 ms | 0.245161 |
| 5 ms | 0.111627 |
| 10 ms | 0.041472 |
| 25 ms | 0.057032 |
| 50 ms | 0.255129 |

Plus: `V(n1)` equals `cos(2π·60·t)` at every point; maximum after 10 ms
`≈ 0.2595`; minimum `≈ 0.0173`; 501 points; no step subdivision; KCL at
node `n2` holds at every point (`id = v/R + C dv/dt`).

Also: singular `G` reports an operating-point error; a forced Newton failure
(tiny iteration limit in a test hook) exercises subdivision; tests assert
behavior, not exact iteration counts.

## Acceptance Criteria

- Linear and nonlinear transient both run from a DC operating point.
- Linear transient factors `G + C/h` once per step size.
- The handout circuit reproduces the reference table.
- Failures are structured, carry the time, and never return partial
  waveforms as results.
