---
title: "Terminal UI (TUI)"
description: "An interactive terminal dashboard for browsing the corpus and running the engine."
---

The TUI is an interactive terminal dashboard for developers working with the regulation corpus and engine.

## Overview

- **Language**: Rust (Ratatui + Crossterm)
- **Location**: `packages/tui/`
- **Binary**: `rrtui`

## What it does

A full-screen terminal application that lets you browse the corpus, run the execution engine, execute BDD tests, validate regulations, inspect execution traces, view pipeline status, and monitor logs. Uses the engine crate directly for execution.

## Screens

There are ten tabs, in the order the tab bar shows them:

| Screen | Purpose |
|--------|---------|
| Dashboard | Overview of corpus and pipeline status |
| BDD | Run and view BDD test results |
| Engine | Execute laws with custom parameters |
| Corpus | Browse and search law files |
| Pipeline | Monitor harvest/enrich job status |
| Validation | Run schema validation |
| Trace | Inspect execution trace trees |
| Dependencies | Which laws reference which, both ways, plus declared and implemented open terms |
| Logs | View log output |
| Actions | Run the `just` quality targets (format, lint, and the rest) and watch their output |

## Running

```bash
cargo run -p regelrecht-tui
```

No configuration needed. Reads the corpus from local filesystem paths.

## Further reading

- [Execution Engine](./engine) - the engine the TUI runs
- [Getting Started](/guide/getting-started) - setting up the development environment
