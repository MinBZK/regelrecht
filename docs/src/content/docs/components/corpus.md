---
title: "Corpus Library"
description: "The shared Rust crate for loading, parsing, and validating the YAML regulation corpus."
---

The corpus library is a shared Rust crate for loading and managing the regulation corpus. It abstracts over multiple source types (local filesystem and GitHub repositories) and handles YAML parsing, registry management, and schema validation.

## Overview

- **Language**: Rust
- **Location**: `packages/corpus/`
- **Type**: Library crate (used by other packages)

## What it does

The corpus library provides a single API for loading law files regardless of where they are stored. It reads the `corpus-registry.yaml` manifest, authenticates with remote sources, fetches YAML files, and parses them into typed Rust structures.

Other packages use it:
- The **editor-api** uses it to serve law files to the frontend
- The **admin** uses it to proxy corpus data to the dashboard
- The **engine** uses the parsed output for execution

## Key modules

| Module | Purpose |
|--------|---------|
| `registry.rs` | `CorpusRegistry` - loads `corpus-registry.yaml`, merges local overrides |
| `source_map.rs` | `SourceMap` - maps law IDs to parsed regulation YAML |
| `models.rs` | `Source`, `SourceType` (Local/GitHub), `RegistryManifest` |
| `github.rs` | `GitHubFetcher` - fetches YAML via GitHub API (feature-gated) |
| `validation.rs` | Schema validation against the JSON schema |
| `auth.rs` | Token management for private repositories |
| `client.rs` | `CorpusClient` - higher-level read/write access used by editor-api |
| `backend.rs`, `pr_client.rs` | Git/GitHub write backends for editing through pull requests |
| `repo_access.rs` | Validates that a token can reach a private repository |
| `annotation_schema.rs`, `dto.rs`, `config.rs` | Annotation schema, transfer types, and deployment config |

## Usage

```rust
use regelrecht_corpus::CorpusRegistry;
use std::path::Path;

// load(manifest_path, local_override_path): the second argument merges a
// corpus-registry.local.yaml override when present.
let registry = CorpusRegistry::load(Path::new("corpus-registry.yaml"), None)?;

// load_all_sources_async includes GitHub sources. Pass the auth file when
// private repositories need a token, or None when all sources are public.
let source_map = registry
    .load_all_sources_async(Some(Path::new("corpus-auth.yaml")))
    .await?;

let law = source_map.get_law("zorgtoeslagwet");
```

The `github` feature flag enables remote fetching from GitHub repositories. Without it, only local filesystem sources are available.

## Further reading

- [Federated Corpus](/concepts/federated-corpus) - how the registry model works
- [Law Format](/concepts/law-format) - structure of the YAML files this library parses
