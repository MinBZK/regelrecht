---
title: "Traceability"
description: "How to read an execution trace: the node types, the box-drawing tree, and a real zorgtoeslag trace with a cross-law chain, IoC delegation, and Awb hooks."
---

When the engine computes an output, it can record every step it took to get there: which articles applied, which inputs it fetched and from where, which operations ran, and what each one produced. That record is the **trace**. It is the legal reasoning behind a number, in a form you can read top to bottom.

A trace is opt-in. The plain evaluation path builds no trace at all; you ask for one explicitly through a separate entry point (`evaluate_law_output_with_trace`, not `evaluate_law_output`). This page explains how to read a trace, then walks through a real one. For how a trace fits into a reproducible, signed [Execution Receipt](./execution-provenance), see that page.

## Anatomy of a node

A trace is a tree of nodes. Each node has a type (what kind of step it was), a name, an optional result, and children. The type is one of:

| Node type | What it marks |
|-----------|---------------|
| `Article` | An article being evaluated |
| `Action` | An action within an article |
| `Resolve` | Resolving a variable to a value |
| `Operation` | An operation running (`ADD`, `EQUALS`, `MAX`, …) |
| `Requirement` | A requirement (eligibility) check |
| `CrossLawReference` | A `source` lookup into another law |
| `Cached` | A cross-law result reused from earlier in the same execution |
| `OpenTermResolution` | An open term filled by an `implements` regulation (IoC) |
| `HookResolution` | A hook firing on another article's output (RFC-007) |
| `OverrideResolution` | A value replaced by lex specialis (RFC-007) |

A `Resolve` node also carries a **resolve type** saying where the value came from: `Parameter` (caller input), `Definition` (an article constant), `Output` (a value computed earlier), `DataSource` (an external register), `ResolvedInput` (a cached cross-law result), `OpenTerm`, `Hook`, `Override`, `Context` (the `referencedate`), `Local` (a loop variable), `Input`, or `Uri`. The resolve type is the difference between "this number is a hard-coded constant in the law" and "this number came from the Tax Authority". The full set lives in `PathNodeType` and `ResolveType` in `packages/engine/src/types.rs`.

## How to read the tree

The default rendering uses box-drawing characters, and the connector tells you whether a step crossed a law boundary:

- **Double lines** (`║`, `╟──`, `╙──`) wrap a cross-law scope. Everything indented under a double line is being computed inside a different law than its parent.
- **Single lines** (`│`, `├──`, `└──`) are steps within one law: operations, variable resolutions, nested calculations.

So the shape of the left margin is a map of which law you are in at any depth, without reading a single label.

## A real trace

Here is the engine computing `hoogte_zorgtoeslag` (the healthcare allowance amount) for one person on 2025-01-01. The full rendering is checked in at `packages/engine/tests/expected_zorgtoeslag_trace.txt` and pinned by a snapshot test, so it stays in step with the engine. The interesting parts:

### A cross-law chain

```text
║   ╟──Reference: algemene_wet_inkomensafhankelijke_regelingen#toetsingsinkomen
║   ║   ╟──Resolving from PARAMETERS: $BSN = '999993653'
║   ║   ╟──Reference: wet_inkomstenbelasting_2001#toetsingsinkomen
║   ║   ║   ╟──Resolving from PARAMETERS: $BSN = '999993653'
║   ║   ║   ╟──Reference: wet_inkomstenbelasting_2001#box1_inkomen
║   ║   ║   ║   ╟──Resolving from DATA_SOURCE: $LOON_UIT_DIENSTBETREKKING = 79547
```

The zorgtoeslag law needs `toetsingsinkomen`. The Awir provides it, but to do so the Awir itself calls the Wet inkomstenbelasting 2001, which in turn reads `loon_uit_dienstbetrekking` from a data source. Each new `║` column is one law deeper. The zorgtoeslag YAML asks only the Awir for this value; the step into the income-tax law is the Awir's own reference, which the engine follows transitively.

### IoC delegation

```text
║   ╟──Resolving $WET_OP_DE_ZORGTOESLAG#STANDAARDPREMIE
║   ║   ├──Resolving from RESOLVED_INPUT: 211200
║   ║   ├──Delegation: Open term 'standaardpremie' implemented by regeling_standaardpremie article 1: 211200
```

The zorgtoeslag law leaves `standaardpremie` open; it does not state the number. A ministerial regulation, `regeling_standaardpremie`, declares that it implements that open term, and the trace records which regulation and article filled the blank. This is the [Inversion of Control](./inversion-of-control) pattern made visible.

### A reused result

```text
║   ║   ├──Reference: algemene_wet_inkomensafhankelijke_regelingen#heeft_toeslagpartner
║   ║   │   ╟──Resolving from PARAMETERS: $BSN = '999993653'
║   ║   │   ╙──Cached: algemene_wet_inkomensafhankelijke_regelingen#heeft_toeslagpartner: False
```

`heeft_toeslagpartner` was already computed earlier in this execution. The second time it is needed, the engine reuses the result instead of recomputing the whole subtree. The `Cached` node is how you spot memoization: same law, same output, same parameters, computed once.

### Awb hooks firing

```text
║   ╟──HOOK: Hook PreActions on BESCHIKKING stage BESLUIT → algemene_wet_bestuursrecht:3:46
║   ║   ╙──Computing motivering_vereist
...
║   ╙──HOOK: Hook PostActions on BESCHIKKING stage BESLUIT → algemene_wet_bestuursrecht:6:7
║       ╙──Computing bezwaartermijn_weken
║           └──Result: bezwaartermijn_weken = 6
```

Neither the zorgtoeslag law nor the Awb references the other. Because the zorgtoeslag decision is a *beschikking*, two Awb articles fire on it: 3:46 adds the duty to give reasons (`motivering_vereist`), and 6:7 adds the six-week objection period (`bezwaartermijn_weken`). These are [reactive](./hooks-and-reactive-execution) outputs, and in the receipt they carry a `Reactive` provenance tag rather than `Direct`. (Where a more specific law shortens that period, an `Override` node appears instead; the Vreemdelingenwet's four-week term is the worked example on the hooks page.)

### Operation branches

Inside a single law the tree is just the calculation. An `IF` records which branch it took:

```text
║   ║       ├──Compute LESS_THAN_OR_EQUAL(...) = True
║   ║       │   ├──Resolving from PARAMETERS: $VERMOGEN = 0
║   ║       │   └──IF(took default) = 14189600
║   ║       │       ├──CASE 0: False
║   ║       │       │   └──Compute EQUALS(...) = False
║   ║       │       │       └──Resolving from PARAMETERS: $HEEFT_TOESLAGPARTNER = False
║   ║       │       └──DEFAULT: 14189600
```

`CASE 0: False` means the first case condition did not hold, so the operation `took default`. The asset limit applied here (14189600 eurocent) is the single-person limit, because the partner check returned `False`.

## Generating a trace

The quickest way to see one is the bundled example, which loads the local corpus and prints the rendered tree:

```bash
cargo run --example trace -- wet_op_de_zorgtoeslag hoogte_zorgtoeslag 2025-01-01 bsn=999993653
```

(See `packages/engine/examples/trace.rs`.) For a simpler starting point, `packages/engine/tests/expected_standaardpremie_trace.txt` is a seven-line trace of a single law with no cross-law calls.

In Rust, call `evaluate_law_output_with_trace(...)` and render the `trace` field with `render_box_drawing()`. In the browser, the WASM engine exposes `executeWithTrace(...)` (and `executeMultipleWithTrace(...)` for several outputs at once); both return the trace as a structured tree you can render in the UI. The editor's execution view and the [TUI](../components/tui)'s trace screen both build on this.

## Further reading

- [Hooks and Reactive Execution](./hooks-and-reactive-execution) - where the hook and override nodes come from
- [Inversion of Control](./inversion-of-control) - the open-term delegation a trace shows
- [Execution Provenance](./execution-provenance) - the receipt that wraps a trace for reproducibility
