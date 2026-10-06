# 0001: Netlist and Circuit Representation

## Objective

Build the first stage of [Deliverable 1](<../Deliverable 1.html>): turn netlist
text into a validated, typed circuit that matrix assembly can consume.

```text
netlist text
-> parsed elements and analysis requests
-> resolved nodes and element references
-> validated circuit
```

This is the first of four implementation stacks:

1. **0001:** netlist parsing and circuit representation.
2. [0002](0002-mna-assembly.md): MNA unknown allocation and element stamps.
3. [0003](0003-linear-dc-analysis.md): linear DC solving and circuit results.
4. [0004](0004-cli-and-delivery.md): command-line integration and delivery.

Each stack builds on the preceding stack. While earlier work is in review,
the next stack can start from its tip. After it lands, rebase the remaining
work onto the updated `remote/main`.

## Scope

This plan covers:

- A Rust workspace with an executable, netlist parser, and circuit model,
  introducing each crate when its implementation first needs it.
- Source-aware parsing of the title, elements, `.dc`, and `.end`.
- All nine element types required by the handout.
- Stable node and element IDs, ground, and reference resolution.
- Semantic validation and structured errors.
- Exact byte spans and source-highlighted diagnostics in the inspection example.
- Parser and circuit-compilation tests.

## Workspace

Create crates incrementally. Diff 1 creates the workspace and binary; diff 2
adds `netlist`; diff 8 adds `circuit`. Stack 0002 introduces `mna`, and stack
0003 introduces `solver`. Workspace members are added explicitly alongside
their implementation rather than creating empty future crates.

The eventual layout is:

```text
ferrispice/
├── Cargo.toml
├── bin/
│   └── ferrispice/
│       ├── Cargo.toml
│       └── src/main.rs
├── crates/
│   ├── netlist/
│   ├── circuit/
│   ├── mna/
│   └── solver/
├── examples/
└── docs/
```

The root is a workspace. Its default member is `bin/ferrispice`, so
`cargo run -- <netlist>` works from the repository root. Library packages are
named `ferrispice-netlist`, `ferrispice-circuit`, `ferrispice-mna`, and
`ferrispice-solver`.

| Crate | Responsibility |
| --- | --- |
| `netlist` | Text syntax, parsed values, source locations, analysis requests |
| `circuit` | Typed elements, node identity, reference resolution, validation |
| `mna` | Unknown layout, matrix allocation, element stamps |
| `solver` | Linear algebra, DC analysis, interpreted results |
| `bin/ferrispice` | Arguments, files, orchestration, presentation |

`netlist` is independent of `circuit`; `circuit` consumes parsed syntax.
Matrix and solver dependencies are introduced in their respective stacks.
Use `nalgebra` for dense linear algebra in Deliverable 1.

## Parsing Contract

Use [Deliverable 1](<../Deliverable 1.html>) as the syntax contract.

The first physical line is always the title, even if it resembles an element.
Retain it as metadata without interpreting it as a circuit statement.

After the title:

- Parse whitespace-separated element records.
- Treat `0` as ground.
- Accept `.dc` anywhere before `.end` as a linear DC analysis request.
- Require `.end` as the final meaningful record.
- Ignore blank lines and full-line `*` comments.
- Reject unsupported directives and records after `.end`.
- Match `VCVS`, `VCCS`, `CCVS`, and `CCCS` before single-letter prefixes.

Recognize prefixes, directives, and `DC` case-insensitively. Resolve node and
element names case-insensitively while retaining original spelling for output.
Detect duplicates after normalization.

The required element prefixes are `V`, `I`, `R`, `C`, `L`, `VCVS`, `CCVS`,
`VCCS`, and `CCCS`. Passive records and independent DC sources follow the
forms demonstrated by the handout:

```text
R1 n1 n2 100
C1 n2 0 1e-6
L1 n2 n3 1e-3
Vin n1 0 DC 6
Iin n2 0 DC 0.01
```

The runnable divider adds `.dc` before `.end`:

```text
Name of circuit
Vin n1 0 DC 6
R1 n1 n2 100
R2 n2 0 50
.dc
.end
```

The initial numeric grammar supports signed decimals and scientific notation.
Add engineering suffixes if course netlists require them, using a documented
suffix table and tests.

## Representation and Resolution

Illustrative boundaries:

```rust
pub fn parse(text: &str) -> Result<ParsedNetlist, ParseError>;
pub fn compile(parsed: &ParsedNetlist) -> Result<Circuit, CircuitError>;
```

Parsed records retain names and source locations. Use an enum for element
forms rather than an untyped token map. Parsing does not allocate matrix rows.

`NetlistPosition` represents a one-based line and character column in the
netlist text. `ByteSpan` represents a zero-based byte offset and length in the
original input. Tokens, parsed elements, and analysis requests retain spans;
compiled elements and errors preserve them. A zero-length span at the input
length points to EOF. Neither type represents a circuit source or matrix index.

Track physical line endings when computing spans so Unicode and CRLF input
remain correctly highlightable. Numeric errors identify the value token,
keyword errors identify the keyword, and record-shape errors identify the record.

The circuit owns typed node and element IDs, original names, terminal IDs,
and parameters for all nine element types. Use one element enum with focused
modules; each element does not need its own crate.

Resolve in two passes:

```text
register nodes and element names
-> resolve terminals and controlling elements
-> validate the complete circuit
```

This supports forward references. Node order follows first appearance and
element order follows the netlist, independently of hash-map iteration.
Ground has a distinguished node ID; matrix indices are allocated in 0002.

Reject duplicate names, non-finite parameters, zero resistance, negative
capacitance or inductance, and unresolved references. Source values and
controlled-source gains may be signed. Zero capacitance and zero inductance
have valid stamps. Singular topology is diagnosed by the solver.

## Controlled-Source Contract

The handout specifies prefixes but not argument syntax. Confirm the grammar
against class notes or instructor examples before finalizing its parser diff.
The proposed convention is:

```text
VCVS1 p n cp cn gain
VCCS1 p n cp cn transconductance
CCVS1 p n controlling_element transresistance
CCCS1 p n controlling_element gain
```

A current controller names an element with an explicit branch-current unknown:
an independent voltage source, inductor, VCVS, or CCVS. Positive current flows
from its positive terminal to its negative terminal. A zero-volt voltage source
can act as a sensor.

If the course uses sensing-terminal pairs or another convention, update this
contract and downstream plans together. Arbitrary resistor references must not
be silently treated as allocated current unknowns.

## Errors

Parser errors identify the line and expected record shape. Circuit errors
identify the element and preserve its source location. Libraries return errors;
the binary prints diagnostics and controls process exit.

The inspection example attaches the input name and original text to typed
parser/compiler errors and renders them with `miette`. Keep `miette` as a dev
dependency for this example; the libraries return concrete `thiserror` errors
without depending on terminal formatting. Duplicate-name errors retain the
first declaration span as well as the offending declaration span. The example
support module can later supply the application CLI diagnostics.

## Diff Stack

| Subject | Deliverable |
| --- | --- |
| `[1/9][d1][workspace] establish crate boundaries` | Workspace and binary with tracing initialization |
| `[2/9][d1][netlist] introduce netlist positions and byte spans` | Introduce netlist crate; NetlistPosition and ByteSpan |
| `[3/9][d1][netlist] parse finite numbers with structured errors` | Typed parser errors, decimal/scientific values, non-finite rejection |
| `[4/9][d1][netlist] tokenize records with positions and spans` | Token extraction, character columns, absolute byte spans, record ranges |
| `[5/9][d1][netlist] scan netlist structure and directives` | Raw records, title, comments, physical line offsets, `.dc`, `.end`, framing errors |
| `[6/9][d1][netlist] parse passive and independent elements` | R, C, L, V, I records and precise syntax-error spans |
| `[7/9][d1][netlist] parse controlled-source records` | Confirmed VCVS, VCCS, CCVS, and CCCS grammar with span-preserving parsing |
| `[8/9][d1][circuit] resolve and validate circuit elements` | Introduce circuit crate; typed model, IDs, ground, references, validation, semantic-error spans |
| `[9/9][d1][diagnostics] render source-aware netlist errors` | Miette reports in inspect; input names, source snippets, labels, and help |

Every diff builds. Behavioral diffs include their relevant tests.

## Test Plan

Verify the handout divider and every element form, title handling, `.dc` at
different positions, missing `.end`, trailing records, malformed values,
unsupported directives, and long-prefix classification.

Check stable IDs, shared ground, case-normalized duplicate detection, forward
controller references, invalid current controllers, and parameter validation
that permits signed source values and gains. Errors must identify the input
line or affected element.

Check token/record spans against the original text, including Unicode, CRLF,
and EOF. Diagnostic tests render malformed values, missing `.end`, duplicate
declarations, and unknown controllers with named source snippets. Verify both
duplicate declarations are labeled and successful inspection still works.

Run `cargo test --workspace --all-targets` after each behavioral diff and
`cargo test --workspace --doc` for the public API examples.

## Acceptance Criteria

The plan is complete when:

- The workspace builds with the agreed binary/library layout.
- All nine required element types have documented, tested syntax representations.
- `.dc` is parsed and retained as the linear DC analysis request for later stacks.
- Netlists compile into circuits with stable, resolved identities.
- Ground and current-control direction have explicit contracts.
- Invalid input produces useful structured errors.
- Inspection errors include input names, highlighted source text, and valid
  byte ranges; duplicate-name reports show both declarations.
- 0002 can consume the circuit without reparsing text or resolving names.

## Open Questions

- What controlled-source argument syntax and sensing convention does the
  course require?
- Do course netlists require engineering-value suffixes?
