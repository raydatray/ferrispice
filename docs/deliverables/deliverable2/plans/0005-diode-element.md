# 0005: Diode Element

## Objective

Add the diode required by [Deliverable 2](<../Deliverable 2.html>) to the
netlist and circuit layers built in
[0001](../../deliverable1/plans/0001-netlist-and-circuit.md).

```text
netlist text with D records
-> parsed diode elements
-> resolved anode and cathode nodes
-> validated circuit with diode parameters
```

This is the first of four implementation stacks:

1. **0005:** diode parsing and circuit representation.
2. [0006](0006-nonlinear-mna.md): the nonlinear vector `f(x)` and its Jacobian.
3. [0007](0007-newton-dc-analysis.md): Newton-Raphson DC analysis.
4. [0008](0008-cli-and-delivery.md): command-line integration and delivery.

Each stack builds on the preceding stack. While earlier work is in review,
the next stack can start from its tip. After it lands, rebase the remaining
work onto the updated `remote/main`.

Deliverable 3 adds transient analysis using backward Euler with Newton-Raphson
at each time point. These plans keep the nonlinear machinery independent of
DC so that transient analysis can reuse it without restructuring.

## Scope

This plan covers:

- The `D` prefix and its two-terminal record.
- A typed diode element carrying its `Is` and `Vt`.
- Default parameters from the handout, defined in one place.
- Source-span preservation.
- Inspection summary and diagnostic support for diodes.

No crates are introduced. Diode equations belong to 0006.

## Parsing Contract

Use the handout netlist as the syntax contract:

```text
* diode circuit
Vin n1 0 DC 1
D1 n1 n2
R n2 0 50
.dc
.end
```

- `D<name> anode cathode` has exactly two node fields and no value.
- The prefix is case-insensitive. No existing prefix starts with `D`, so
  long-prefix classification is unchanged.
- Extra fields are a `WrongFieldCount` error spanning the record. Inline
  model parameters and `.model` cards remain unsupported until a handout
  requires them.
- The first line, `* diode circuit`, is the title even though it resembles a
  comment. The existing title rule handles this; add a regression test.
- The bare element name `R` is already accepted; keep it under test.

## Representation

```rust
pub const DEFAULT_SATURATION_CURRENT: f64 = 1e-15; // Is, amperes
pub const DEFAULT_THERMAL_VOLTAGE: f64 = 0.025;    // Vt, volts

ElementKind::Diode { saturation_current: f64, thermal_voltage: f64 }
```

The parameters are plain fields, like every other element's. Compilation
fills them from the two constants, which are the single source of the
handout values; every diode uses them in this deliverable.

The handout states `Is = 1e-14 A`, but its expected results (`V2 = 0.2673`,
`IE = 0.0053`) are produced by `Is = 1e-15 A`; `1e-14 A` gives
`V2 = 0.3204` and `I = 0.006407`. Use `1e-15 A` so the handout results are
reproduced, and document the discrepancy.

The anode and cathode are the element's existing `positive` and `negative`
terminals, so terminal resolution is shared with every other element and
positive current flows from anode to cathode.

A diode has no branch-current unknown. `has_branch_current()` remains false,
and naming a diode as a CCVS or CCCS controller is rejected by the existing
controller validation.

The diode holds the two handout parameters only. Per-diode parameters,
`.model` cards, and junction capacitance are not planned.

## Validation

`Is` and `Vt` come from fixed positive constants rather than the netlist, so
there is nothing to validate. Parameter checks belong in `compile` alongside
the other elements' if diode parameters ever become netlist values. A diode
whose anode and cathode resolve to the same node is valid and contributes
nothing, consistent with repeated-terminal handling elsewhere.

## Errors

Diode record errors reuse the existing parser error kinds and spans, so the
application highlights the offending record. Diodes need no error types of
their own.

## Diff Stack

| Subject | Deliverable |
| --- | --- |
| `[1/3][d2][netlist] parse diode records` | `D` prefix, two-node record shape, field-count errors, precise spans |
| `[2/3][d2][circuit] model diodes with default parameters` | `ElementKind::Diode` with `Is` and `Vt` fields, default constants, terminal resolution, controller rejection |
| `[3/3][d2][cli] inspect circuits containing diodes` | Summary output, diagnostics, handout netlist inspection test |

Every diff builds. Until 0006 lands, assembly returns an explicit
unsupported-element error for diodes rather than silently contributing
nothing.

## Test Plan

Parse the handout netlist verbatim and confirm the title `* diode circuit`.
Cover case-insensitive prefixes, a missing node, extra fields, and span
accuracy including CRLF input.

Check stable IDs, forward node references, reversed orientation, and
same-node terminals. Reject a diode used as a current controller. Confirm
compiled diodes carry the default `Is` and `Vt`.

Invoke the application on the handout netlist without `--dump` flags and
confirm successful inspection, then on malformed diode records and confirm
highlighted diagnostics with nonzero exit status.

Run `cargo test --workspace --all-targets` after each behavioral diff.

## Acceptance Criteria

The plan is complete when:

- The handout netlist parses and compiles.
- Diodes carry resolved terminals and the documented default `Is` and `Vt`.
- Invalid diode records produce structured, source-highlighted errors.
- 0006 can consume diodes without reparsing text or resolving names.
