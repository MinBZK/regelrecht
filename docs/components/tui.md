# Terminal UI (TUI)

The TUI is an interactive terminal dashboard for developers working with the regulation corpus and engine.

## Overview

- **Language**: Rust (Ratatui + Crossterm)
- **Location**: `packages/tui/`
- **Binary**: `rrtui`

## What it does

A full-screen terminal application that lets you browse the corpus, run the execution engine, execute BDD tests, validate regulations, inspect execution traces, view pipeline status, and monitor logs. Uses the engine crate directly for execution.

## Screens

| Screen | Purpose |
|--------|---------|
| Dashboard | Overview of corpus and pipeline status |
| Corpus | Browse and search law files |
| Engine | Execute laws with custom parameters |
| Trace | Inspect execution trace trees |
| BDD | Run and view BDD test results |
| Validation | Run schema validation |
| Pipeline | Monitor harvest/enrich job status |
| Logs | View log output |

## Running

```bash
cargo run -p regelrecht-tui
```

No configuration needed. Reads the corpus from local filesystem paths.

## Further reading

- [Execution Engine](./engine) - the engine the TUI runs
- [Getting Started](/guide/getting-started) - setting up the development environment
