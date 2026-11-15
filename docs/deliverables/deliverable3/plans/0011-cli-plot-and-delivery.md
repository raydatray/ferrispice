# 0011: CLI, Plot, and Deliverable Integration

## Objective

Run `.tran` from the `ferrispice` binary, write the waveforms to CSV, and
plot them in pure Rust, completing
[Deliverable 3](<../Deliverable 3.html>).

```text
netlist file
-> parse and compile
-> .dc requested: DC panel (unchanged)
-> .tran requested: transient analysis -> summary panel, CSV, SVG plot
```

## Command Contract

```sh
cargo run -- examples/half-wave-rectifier.cir
cargo run -- examples/half-wave-rectifier.cir --csv out/hwr.csv
cargo run -- examples/half-wave-rectifier.cir --plot out/hwr.svg
cargo run -- examples/half-wave-rectifier.cir --plot out/hwr.svg --plot-nodes n1,n2
```

- `--csv <path>`: write every unknown at every time point.
- `--plot <path>`: write an SVG of node voltages against time.
- `--plot-nodes a,b`: restrict the plot to named nodes (default: every
  non-ground node).
- `--csv`, `--plot`, and `--plot-nodes` without `.tran` are command errors.
- `.dc` and `.tran` in the same netlist run both, DC first.
- `--dump-mna` works as before and includes `b(t)` (0009). `--dump-lu`
  and `--trace-newton` keep their DC meaning and apply to the operating
  point.

`examples/half-wave-rectifier.cir` is the handout netlist verbatim.

## Summary Panel

Waveforms are not printed to the terminal. The panel summarizes them:

```text
╭ transient analysis (backward Euler) ─────────╮
│                                              │
│  * a half-wave recitifier circuit            │
│                                              │
│  0 s to 0.05 s, step 0.1 ms, 501 points      │
│  initial state: DC operating point           │
│  2,xxx Newton iterations (max x per step)    │
│                                              │
│             min          max        final    │
│  V(n1)     -1 V          1 V          1 V    │
│  V(n2)  0.0173 V     0.2673 V    0.2551 V    │
│  I(Vin)    ...          ...          ...     │
│                                              │
│  wrote out/hwr.csv                           │
│  wrote out/hwr.svg                           │
│                                              │
╰──────────────────────────────────────────────╯
```

Linear circuits say `linear: G + C/h factored once` instead of Newton
counts. Subdivided steps, if any, are reported.

## CSV

```text
time,V(n1),V(n2),I(Vin)
0,1,0.267314...,...
0.0001,...
```

Header uses the existing unknown labels; values use Rust's shortest
round-trip `f64` formatting so no precision is lost. Written with `std`
only; parent directories are created.

## Plot

Use the `plotters` crate with the SVG backend only:

```toml
plotters = { version = "0.3", default-features = false,
             features = ["svg_backend", "line_series"] }
```

SVG text is emitted as `<text>` elements, so no font files or system font
libraries are needed and the build stays pure Rust. PNG is not planned; an
SVG can be converted or embedded in the report directly.

The figure mirrors `HwR2.png`:

- title `Node voltages: <netlist title>`
- x axis `Time [s]` from 0 to `stop`; y axis `Voltage [V]` with a margin
  around the data range
- one line per node, distinct colors, legend in the lower-left
- light grid

Plotting lives in `bin/ferrispice/src/plot.rs` behind a single
`write_svg(path, solution, nodes)` function.

## Diagnostics

- Transient failures report the failing time and the Newton failure, using
  the existing `diagnostics::analysis` style.
- I/O errors on CSV/plot paths are reported with the path.
- Unknown `--plot-nodes` names are reported with the available node list.

## Examples and Checks

Add:

- `examples/half-wave-rectifier.cir` (handout)
- `examples/rc-step.cir` (linear RC, analytical check)
- `examples/rl-cos.cir` (linear RL driven by `COS`)

`bin/ferrispice/tests/examples.rs` gains a transient table: run each example
with `--csv` into a temp dir, parse the CSV, and check selected points
against the values in 0010. `tests/cli.rs` covers flag validation, the
summary panel, SVG creation (file exists, contains `<svg` and both legend
labels), and diagnostics.

## Report Notes

Document in the deliverable write-up:

- `Is = 1e-15 A` is used (handout `1e-14` treated as the same typo as
  Deliverable 2), so `V(n2)` peaks near `0.26 V` instead of the `0.31 V`
  in `HwR2.png`.
- The initial state is the DC operating point at `t = 0`.
- `.tran` uses the handout's `stop step` argument order.

## Diff Stack

| Subject | Deliverable |
| --- | --- |
| `[1/4][d3][cli] run transient analysis and print a summary` | `.tran` routing, summary panel, diagnostics |
| `[2/4][d3][cli] export transient waveforms to CSV` | `--csv` |
| `[3/4][d3][cli] plot node voltages to SVG with plotters` | `plotters` dependency, `--plot`, `--plot-nodes` |
| `[4/4][d3][cli] add transient examples and end-to-end checks` | Examples, example and CLI tests, help text |

## Test Plan

Run `cargo test --workspace` after each diff; CI (`cargo fmt --check`,
`cargo test --workspace --locked`) must pass with the updated `Cargo.lock`.
Manually compare the generated SVG for the handout circuit against
`HwR2.png`: same shape, the `n2` peak about 50 mV lower due to `Is`.

## Acceptance Criteria

- `cargo run -- examples/half-wave-rectifier.cir --plot hwr.svg` produces the
  deliverable figure with no non-Rust dependencies.
- CSV contains every unknown at every time point at full precision.
- All Deliverable 1 and 2 commands behave identically.
