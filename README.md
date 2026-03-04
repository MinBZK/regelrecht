# regelrecht-mvp

RegelRecht MVP - Machine-readable Dutch law execution engine with a web-based editor.

## Components

- **[packages/engine/](packages/engine/)** - Rust law execution engine
- **[packages/harvester/](packages/harvester/)** - Rust harvester for downloading Dutch legislation from BWB
- **[packages/pipeline/](packages/pipeline/)** - PostgreSQL-backed job queue and law status tracking for the processing pipeline
- **regulation/** - Dutch legal regulations in machine-readable YAML format
- **frontend/** - Static HTML/CSS law editor prototype

## Deployment

The frontend is automatically deployed to RIG (Quattro/rijksapps):

| Environment | URL | Trigger |
|-------------|-----|---------|
| Production | https://editor-regelrecht-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl | Push to `main` |
| PR Preview | https://editor-prN-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl | PR opened/updated |

PR preview environments are automatically cleaned up when the PR is closed.

## Development

See [CLAUDE.md](CLAUDE.md) for detailed development instructions
