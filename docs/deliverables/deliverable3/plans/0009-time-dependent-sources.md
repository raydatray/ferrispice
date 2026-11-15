# 0009: Time-Dependent Sources and Transient Syntax

## Objective

Accept the netlist in [Deliverable 3](<../Deliverable 3.html>) verbatim:

```text
* a half-wave recitifier circuit
Vin n1 0 COS 1 60
* Vin is a cosine with a 1V amplitude and 60Hz frequency
D1 n1 n2
R1 n2 0 50
C1 n2 0 100e-6
.tran 0.05s  0.1ms
* run transient simulation for 0.05s with step size of 0.1ms
.end
```

```text
netlist text with COS sources, .tran, and suffixed numbers
-> parsed waveforms and a transient request
-> validated circuit whose sources carry waveforms
-> MNA system with a time-dependent right-hand side b(t)
```

This plan carries time-dependent sources through every layer below the
solver, so the circuit equation becomes:

```text
G x + C dx/dt + f(x) = b(t)
```

This is the first of three implementation stacks:

1. **0009:** time-dependent sources from netlist text to `b(t)`.
2. [0010](0010-backward-euler-transient.md): backward-Euler transient analysis.
3. [0011](0011-cli-plot-and-delivery.md): command line, CSV, plot, delivery.

Each stack builds on the preceding stack, following the Deliverable 2
workflow.

## Scope

This plan covers:

- SPICE scale suffixes and trailing unit letters on numbers.
- `COS amplitude frequency` independent sources, voltage and current.
- The `.tran tstop tstep` directive.
- Netlists requesting `.dc`, `.tran`, both, or neither.
- A `Waveform` type in the circuit layer replacing the source `dc: f64`.
- `MnaSystem::b_at(t)` and a symbolic `b(t)` panel in `--dump-mna`.

`G`, `C`, and `f` are unchanged: only independent sources depend on time,
and they appear only in `b`.

The diode is unchanged: `Is = 1e-15 A` and `Vt = 0.025 V` remain hardcoded
constants (see [0005](../../deliverable2/plans/0005-diode-element.md)). The
handout's `Is = 1e-14 A` is the same typo as in Deliverable 2 and is not
followed. Consequently the simulated peaks of `V(n2)` are about `0.26 V`
rather than the `0.31 V` shown in `HwR2.png`; document this in the report.

## Numbers

The handout writes `0.05s` and `0.1ms`, which `str::parse::<f64>` rejects.
`parse_number` becomes:

```text
token = mantissa [scale] [unit letters]
mantissa: the longest prefix accepted by f64 parsing (sign, digits, '.', exponent)
scale (case-insensitive, longest match first):
    t 1e12   g 1e9   meg 1e6   k 1e3   mil 25.4e-6
    m 1e-3   u 1e-6  n 1e-9    p 1e-12  f 1e-15
unit letters: any remaining ASCII letters, ignored
```

| Token | Value |
| --- | --- |
| `0.05s` | `0.05` |
| `0.1ms` | `1e-4` |
| `100u`, `100uF` | `1e-4` |
| `1k`, `1K` | `1000` |
| `1meg`, `1MEG` | `1e6` |
| `1M` | `1e-3` (SPICE semantics: `M` is milli) |
| `100e-6` | `1e-4` |
| `60Hz` | `60` (`H` is not a scale; ignored as a unit) |

Rules:

- The mantissa must be a complete float. `1e` followed by letters is
  `InvalidNumber` (the exponent is malformed, not a suffix).
- After the mantissa only ASCII letters are allowed. `1_000`, `1k5`,
  `1.0.0`, `--1`, and `value` remain `InvalidNumber`.
- Non-finite results remain `NonFiniteNumber`, including overflow after
  scaling (`1e308k`).
- Suffixes apply everywhere `parse_number` is used, so `R1 a b 1k` now works.

The existing test `rejects_malformed_and_suffix_values_with_their_location`
asserts `1k` is rejected; rewrite it to cover the new accepted and rejected
sets.

## Sources

```text
V<name> p n DC value
V<name> p n COS amplitude frequency
I<name> p n DC value
I<name> p n COS amplitude frequency
```

- The mode keyword is case-insensitive. `DC` takes 5 fields; `COS` takes 6.
- `COS` means `amplitude * cos(2π · frequency · t)`. No offset, phase, or
  damping; these are not required by the handout.
- `frequency` must be positive and finite; `amplitude` must be finite.
  Validation lives in `compile` with the other parameter checks and reports
  the value's span.
- `ExpectedDc` is renamed `UnknownSourceMode { found }`, listing `DC` and
  `COS` in its help text.

### Representation

```rust
// netlist
ParsedKind::VoltageSource { waveform: ParsedWaveform }
ParsedKind::CurrentSource { waveform: ParsedWaveform }
pub enum ParsedWaveform { Dc(f64), Cos { amplitude: f64, frequency: f64 } }

// circuit
ElementKind::VoltageSource { waveform: Waveform }
ElementKind::CurrentSource { waveform: Waveform }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Waveform { Dc(f64), Cos { amplitude: f64, frequency: f64 } }

impl Waveform {
    pub fn value_at(&self, t: f64) -> f64;
    pub fn dc_value(&self) -> f64 { self.value_at(0.0) }
    pub fn is_constant(&self) -> bool;
}
```

`ElementKind` stays `Copy`. DC analysis of a `COS` source uses its value at
`t = 0`, so `COS 1 60` is `1 V` at DC. This is the operating point that
starts the transient in 0010.

## The `.tran` Directive

```text
.tran tstop tstep
```

The handout order is **stop time first, then step**, which is the reverse of
standard SPICE (`.tran tstep tstop`). Follow the handout, and document the
difference in the command help.

- Exactly 3 fields; otherwise `WrongFieldCount`.
- Both values parse with suffixes.
- Compile-time validation: `tstep > 0`, `tstop > 0`, `tstep <= tstop`.
- `tstop / tstep` need not be an integer; the last step is shortened to land
  on `tstop` (0010).

```rust
pub enum Analysis {
    Dc,
    Tran { stop: f64, step: f64 },
}
```

`Analysis` loses `Eq` (it now holds floats). `requests_dc()` stays;
add `transient_request() -> Option<(f64, f64)>`. Multiple `.tran` records
are an error (`DuplicateTransient`) pointing at the second one, since unlike
repeated `.dc` they could disagree.

`RawRecord` gains `Tran(location, span, tokens)`. `.dc` keeps taking no
arguments.

## Time-Dependent Right-Hand Side

Replace the stored `b` vector with the list of source terms that produce
it. The source terms are the only record of `b`; there is no stored numeric
copy that could disagree with them:

```rust
pub struct MnaSystem {
    pub g: DMatrix<f64>,
    pub c: DMatrix<f64>,
    pub sources: Vec<SourceTerm>,     // replaces `b: DVector<f64>`
    pub layout: UnknownLayout,
    pub nonlinear: Vec<NonlinearDevice>,
}

/// one signed contribution of an independent source to `b`
pub struct SourceTerm {
    pub element: ElementId,
    pub row: usize,
    pub sign: f64,          // +1 or -1
    pub waveform: Waveform,
}

impl MnaSystem {
    /// `b(t)`: the right-hand side with every source evaluated at `t`
    pub fn b_at(&self, t: f64) -> DVector<f64>;
    /// `b(0)`, the right-hand side used by DC analysis
    pub fn b(&self) -> DVector<f64> { self.b_at(0.0) }
    /// whether `b` is constant in time
    pub fn is_time_invariant(&self) -> bool;
}
```

`b_at(t)` starts from zero and adds `sign * waveform.value_at(t)` for each
term. Ground rows are skipped when terms are recorded, as `add_b` does
today.
`b()` is `b_at(0)`, which for DC-only circuits equals the Deliverable 1
vector because `dc_value()` is `value_at(0)`.

| Source | Terms |
| --- | --- |
| `I p n` | `(p, -1)`, `(n, +1)` (current leaves `p` through the source) |
| `V p n` | `(branch, +1)` |

`MatrixBuilder::add_b` is replaced by `add_source(row, sign, element,
waveform)`, which only appends a term. Independent sources are the only
writers of `b`, so no other stamp is affected.

`b()` returns an owned vector, so callers that borrowed the field bind it
first and borrow the binding:

| Caller | Before | After |
| --- | --- | --- |
| Linear DC (`solver/src/dc.rs`) | `solve_linear(&system.g, &system.b)` | `let b = system.b();` then `&b` |
| Source stepping (`solver/src/dc.rs`) | `&system.b * scale` | `system.b() * scale`, with `b()` computed once outside the step closure |
| `--dump-mna` | `system.b` | `system.b()` |
| MNA and solver tests | `system.b` / `r: &system.b` | `system.b()` / `let b = system.b(); r: &b` |
| Builder test (`mna/tests/layout_and_builder.rs`) | `builder.add_b(row, value)` | `builder.add_source(row, sign, element, Waveform::Dc(value))` |

This is a mechanical rename with no change in values, so all D1/D2 results
are unchanged. DC source stepping scales `b()`, which stays correct because
DC uses `b(0)` only.

### Inspection

`--dump-mna` keeps printing numeric `b()` (labelled `b(0)` when any source is
time-varying) and adds a symbolic panel when `!is_time_invariant()`, using
the same row labels as the existing `f(x)` panel:

```text
b(t)
  I(Vin)   cos(2π·60·t)
```

## Errors

All new errors reuse span-carrying `ParseError`/`CompileError`, so the
binary highlights the offending field:

- invalid suffix or mantissa: `InvalidNumber` on the token
- unknown source mode: `UnknownSourceMode` on the mode token
- `.tran` field count, non-positive or inverted times
- duplicate `.tran`
- non-positive `COS` frequency

## Diff Stack

| Subject | Deliverable |
| --- | --- |
| `[1/6][d3][netlist] accept SPICE scale suffixes and units on numbers` | New `parse_number`, rewritten unit tests |
| `[2/6][d3][netlist] parse COS independent sources` | `ParsedWaveform`, `COS` records, `UnknownSourceMode` |
| `[3/6][d3][netlist] parse .tran requests` | `Analysis::Tran`, field-count and duplicate errors |
| `[4/6][d3][circuit] model source waveforms and transient requests` | `Waveform`, `value_at`, compile validation; MNA and solver use `dc_value()` so all D1/D2 results are unchanged |
| `[5/6][d3][mna] record independent sources as time-dependent terms` | `SourceTerm` replaces the `b` field, `add_source` replaces `add_b`, `b_at`, `b()`, `is_time_invariant`, callers moved to `b()` |
| `[6/6][d3][cli] show b(t) in MNA dumps` | Symbolic `b(t)` panel |

## Test Plan

- Number table above, plus every rejected form, spans unchanged.
- Parse the handout netlist verbatim: title, three elements plus the source,
  `Tran { stop: 0.05, step: 1e-4 }`, comment lines skipped.
- `COS` and `DC` in mixed case, wrong field counts, `SIN` rejected as an
  unknown mode.
- `Waveform::value_at`: `COS 1 60` is `1` at `t = 0`, `-1` at `1/120 s`,
  `≈0` at `1/240 s`.
- Compile errors for bad `.tran` times and frequency.
- `b()` matches the existing expected `b` vectors in the MNA tests for every
  D1/D2 example; a `COS 1 60` voltage source gives
  `b_at(1/120)[branch] == -1`; a `COS` current source signs both nodes and
  skips ground; with mixed DC and `COS` sources only the `COS` rows vary.
- Dump snapshot of the handout circuit includes the `b(t)` panel.
- All existing tests and examples pass unchanged.

## Acceptance Criteria

- The handout netlist parses and compiles without edits.
- Sources carry a `Waveform`; DC analysis behaves exactly as before.
- Invalid transient syntax produces highlighted diagnostics.
- `b_at(t)` evaluates every independent source at `t`, so 0010 needs
  nothing beyond `MnaSystem`.
