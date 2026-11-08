# 0006: Nonlinear MNA

## Objective

Extend the assembly from [0002](../../deliverable1/plans/0002-mna-assembly.md)
with the nonlinear vector `f(x)` and its analytic Jacobian, completing the
course equation form required by [Deliverable 2](<../Deliverable 2.html>).

```text
validated circuit
-> linear G, C, b (unchanged)
-> nonlinear device list representing f(x)
-> f(x) and ∂f/∂x evaluated at any x
```

The result feeds [0007](0007-newton-dc-analysis.md).

## Scope

This plan covers the MNA equation with a nonlinear term, the device-list
representation of `f`, the diode equations, analytic `f` and `∂f/∂x`
evaluation, symbolic description of `f` for inspection, and mathematical
validation.

## Equation Form

Deliverable 1 represented the circuit as `G x + C x' = b`. With nonlinear
devices, the MNA equation becomes:

```text
G x + C x' + f(x) = b
```

`f(x)` holds the nonlinear device currents leaving each node. Rows for
branch constraints receive no diode contributions. Diodes add no unknowns,
so the unknown layout is unchanged.

`G`, `C`, and `b` are assembled once, exactly as in Deliverable 1. `f` cannot
be stored as numbers because it depends on `x`; it is stored as the list of
devices that produce it.

Each analysis builds its own residual from these parts:

```text
DC (0007):              Φ(x) = G x + f(x) - λ b
Backward Euler (d3):    Φ(x) = (G + C/h) x + f(x) - b(t_{n+1}) - (C/h) x_n
```

Both are `A x + f(x) - r` with a constant `A` and `r` during a Newton solve.
This plan therefore exposes `f` and `∂f/∂x` independently of `G` and `b`, and
does not bake DC assumptions into the MNA crate.

## Representation of f

```rust
pub enum NonlinearDevice {
    Diode(DiodeStamp),
}

pub struct DiodeStamp {
    pub element: ElementId,     // the circuit owns its source location
    pub name: String,           // labels and error messages
    pub anode: Option<usize>,   // matrix index; None for ground
    pub cathode: Option<usize>,
    pub saturation_current: f64,
    pub thermal_voltage: f64,
}

pub struct MnaSystem {
    pub g: DMatrix<f64>,
    pub c: DMatrix<f64>,
    pub b: DVector<f64>,
    pub layout: UnknownLayout,
    pub nonlinear: Vec<NonlinearDevice>,
}
```

Assembly resolves terminal indices once, alongside linear stamping, in
circuit element order. Use enum dispatch, matching existing stamps. Later
nonlinear elements add variants with their own evaluation arms.

```text
crates/mna/src/
├── lib.rs              MnaSystem, NonlinearDevice, f, df_dx
├── layout.rs
├── builder.rs
└── stamps/
    ├── passive.rs
    ├── independent.rs
    ├── controlled.rs
    └── diode.rs        record at assembly; id, gd; f and ∂f/∂x stamps
```

The diode is a stamp like every other element: it writes a fixed pattern
into the system. The difference is only when it runs. Linear stamps run once
during assembly; the diode stamp runs again for each `x`. One file per
element family keeps the existing convention. A separate device-model module
is not justified for a single two-line model.

## Diode Equations

For junction voltage `vd = v[anode] - v[cathode]`:

```text
id(vd) = Is * (exp(vd / Vt) - 1)     computed with f64::exp_m1
gd(vd) = Is / Vt * exp(vd / Vt)      gd = d id / d vd
```

`stamps/diode.rs` provides these as pure functions. `exp_m1` preserves
accuracy near zero bias.

`f64::exp` overflows to infinity above an argument of about 709, which is
`vd ≈ 17.7 V` at `Vt = 0.025 V`. An early Newton iterate can reach such
voltages. Evaluation does not panic or clamp; it reports a structured
non-finite error identifying the device and junction voltage. 0007 treats
that as a failed Newton attempt and reduces its step.

## Evaluation API

```rust
impl MnaSystem {
    pub fn is_linear(&self) -> bool;

    /// f(x)
    pub fn f(&self, x: &DVector<f64>) -> Result<DVector<f64>, DeviceError>;

    /// ∂f/∂x at x
    pub fn df_dx(&self, x: &DVector<f64>) -> Result<DMatrix<f64>, DeviceError>;

    /// row-wise symbolic description of f for inspection
    pub fn describe_f(&self) -> Vec<Vec<FTerm>>;
}
```

The MNA crate knows nothing about Newton, source stepping, or time steps.
It evaluates `f` and `∂f/∂x` at whatever `x` it is given.

### Stamping f

```text
f[p] += id
f[n] -= id
```

### Stamping ∂f/∂x

Same four-entry pattern as a resistor:

```text
J[p,p] += gd      J[p,n] -= gd
J[n,p] -= gd      J[n,n] += gd
```

Omit ground entries and accumulate shared entries, using the existing
ground-aware helpers.

## Symbolic f

`describe_f` returns, per unknown row, the signed device terms contributing
to that row. For the handout circuit, ordered `V(n1), V(n2), I(Vin)`:

```text
KCL n1   [ +id(D1) ]
KCL n2   [ -id(D1) ]
Vin      [ 0       ]
```

Formatting belongs to the binary in 0008.

## Errors

Assembly errors identify the responsible diode. `DeviceError` identifies the
device and the junction voltage that produced a non-finite value. Singular
systems are diagnosed by the solver.

## Diff Stack

| Subject | Deliverable |
| --- | --- |
| `[1/3][d2][mna] evaluate diode current and conductance` | Diode stamp module, `exp_m1` current, conductance, non-finite errors |
| `[2/3][d2][mna] record nonlinear devices during assembly` | `NonlinearDevice`, terminal resolution, `is_linear`, unchanged linear assembly |
| `[3/3][d2][mna] evaluate f and its Jacobian` | `f`, `df_dx`, `describe_f`, accumulation |

Each diff builds and validates the behavior it introduces.

## Test Plan

Check `id` and `gd` against closed forms at reverse, zero, and forward bias.
Check `gd` against a central finite difference of `id`. Confirm `exp_m1`
accuracy near zero bias. Confirm a junction voltage beyond the overflow
threshold returns a `DeviceError` rather than infinity or a panic.

Check `f` and `∂f/∂x` by hand for a grounded diode, a floating diode,
reversed orientation, and two diodes sharing a node. Use a central finite
difference of `f` as a test-only oracle for `∂f/∂x` at several operating
points to catch sign and index errors.

Confirm all Deliverable 1 circuits assemble to identical `G`, `C`, and `b`
with an empty device list.

For the handout circuit, ordered `V(n1), V(n2), I(Vin)`:

```text
G = [ 0   0     1 ]      b = [ 0, 0, 1 ]      C = all zeros
    [ 0   0.02  0 ]
    [ 1   0     0 ]

f(x)  = [ +id, -id, 0 ]

∂f/∂x = [  gd  -gd  0 ]
        [ -gd   gd  0 ]
        [  0    0   0 ]
```

Run `cargo test --workspace` after each diff.

## Acceptance Criteria

The plan is complete when:

- The MNA system represents `G x + C x' + f(x) = b`.
- Linear assembly is unchanged for circuits without diodes.
- `f` and `∂f/∂x` are analytically correct and checked against finite
  differences.
- Overflowing device evaluation returns a structured error.
- `f` can be described symbolically for inspection.
- 0007 can build residuals and Jacobians without diode internals.
