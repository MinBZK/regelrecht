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

A full-screen terminal application for the local checkout: browse the corpus,
run the engine and the BDD suites, validate regulations, inspect execution
traces and follow log output. It links the engine crate directly, so it needs
no running service.

## Screens

There are ten tabs, in the order the tab bar shows them:

| Screen | Purpose |
|--------|---------|
| Dashboard | Corpus statistics: laws, articles, features and scenarios, laws per regulatory layer, the most referenced laws and the laws with most implementations |
| BDD | Run and view BDD test results |
| Engine | Execute laws with custom parameters |
| Corpus | Browse and search law files |
| Pipeline | Placeholder, see below |
| Validation | Run schema validation |
| Trace | Inspect execution trace trees |
| Dependencies | Which laws reference which, both ways, plus declared and implemented open terms |
| Logs | View log output |
| Actions | Run the `just` quality targets (format, lint, and the rest) and watch their output |

The Pipeline tab is a stub. `packages/tui/src/views/pipeline.rs` never connects
to a database: it always shows "Not connected" with a hint to set
`DATABASE_URL`, and setting that variable changes nothing. Job and law status
from the [pipeline](./pipeline) are visible in the harvester admin (the
Corpusinwinning section of the editor) and in [Grafana](./grafana), not here.

## Running

```bash
cd packages
cargo run -p regelrecht-tui
```

No configuration needed. The TUI walks up from the working directory to the
first directory with a `justfile` and treats that as the repository root, then
reads the corpus from `corpus/regulation/` below it.

## Further reading

- [Execution Engine](./engine) - the engine the TUI runs
- [Development Environment](/guide/dev-environment) - setting up the development environment
