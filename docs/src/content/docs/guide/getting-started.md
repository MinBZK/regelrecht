---
title: "Getting Started"
description: "From a fresh clone to a built engine and a passing test suite."
---

This is the shortest path from nothing to a working build. Running the editor and the rest of the local stack is on [Development Environment](./dev-environment).

## Prerequisites

- [Rust](https://rustup.rs/). The exact version is pinned in `rust-toolchain.toml`; rustup picks it up automatically.
- [just](https://github.com/casey/just), the command runner. Use the `just` recipes rather than calling `cargo` yourself: they are what CI and the pre-commit hooks run.
- [Node.js](https://nodejs.org/), for the frontends and for the script tests that `just check` runs.
- [Docker](https://www.docker.com/), for the tests that start a PostgreSQL container.
- [mold](https://github.com/rui314/mold), on x86_64 Linux (dev containers included). `packages/.cargo/config.toml` links with mold there, so a build fails without it. `just dev-setup` installs it; see [One-time setup](./dev-environment#one-time-setup-build-speed).

## Clone and build

```bash
git clone https://github.com/MinBZK/regelrecht.git
cd regelrecht
just dev-setup    # once per machine: mold, and one target dir shared by all worktrees
just build-check  # cargo check over the whole workspace
```

## Check that everything passes

```bash
just check
```

This runs what CI runs: formatting, clippy, a build check, schema and annotation validation, the script test suites, and the full Rust test suite. The last step needs Docker. On a machine without it, run `just test-no-docker` instead, which skips the container-backed crates.

`just` on its own lists every recipe. [Testing](./testing) explains which one to run when.

## Next steps

- [Development Environment](./dev-environment) - the editor and the rest of the local stack, and the pre-commit hooks
- [Law Format](/concepts/law-format) - how laws are structured
- [System Overview](./architecture) - the components and where their code lives
- [Contributing](/operations/contributing) - branches, commits and pull requests
