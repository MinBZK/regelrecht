# regelrecht

[![CI](https://github.com/MinBZK/regelrecht/actions/workflows/ci.yml/badge.svg)](https://github.com/MinBZK/regelrecht/actions/workflows/ci.yml)
[![License: EUPL-1.2](https://img.shields.io/badge/License-EUPL--1.2-blue.svg)](https://opensource.org/licenses/EUPL-1.2)
[![Rust 2021](https://img.shields.io/badge/Rust-2021_edition-orange.svg)](https://doc.rust-lang.org/edition-guide/rust-2021/)
[![Mutation Testing](https://github.com/MinBZK/regelrecht/actions/workflows/mutation-testing.yml/badge.svg)](https://github.com/MinBZK/regelrecht/actions/workflows/mutation-testing.yml)
[![Docs](https://img.shields.io/badge/docs-regelrecht-green.svg)](https://docs.regelrecht.rijks.app)

RegelRecht MVP - Machine-readable Dutch law execution engine with a web-based editor.

## Components

- **[packages/engine/](packages/engine/)** - Rust law execution engine
- **[packages/harvester/](packages/harvester/)** - Rust harvester for downloading Dutch legislation from BWB
- **[packages/pipeline/](packages/pipeline/)** - PostgreSQL-backed job queue and law status tracking for the processing pipeline
- **corpus/regulation/** - Dutch legal regulations in machine-readable YAML format
- **frontend/** - Static HTML/CSS law editor prototype

## Deployment

The frontend is automatically deployed to RIG (Quattro/rijksapps):

| Environment | URL | Trigger |
|-------------|-----|---------|
| Production | https://editor-regelrecht-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl | Push to `main` |
| PR Preview | https://editor-prN-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl | PR opened/updated |

PR preview environments are automatically cleaned up when the PR is closed.

## Development

See the [docs site](https://docs.regelrecht.rijks.app) for detailed development instructions
