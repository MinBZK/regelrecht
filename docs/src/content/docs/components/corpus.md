---
title: "Corpus Library"
description: "The shared Rust crate for loading, parsing, and validating the YAML regulation corpus."
---

The corpus library is a shared Rust crate for loading and managing the regulation corpus. It abstracts over multiple source types (local filesystem and GitHub repositories) and handles YAML parsing, registry management, and scope checks on what each source provides.

## Overview

- **Language**: Rust
- **Location**: `packages/corpus/`
- **Type**: Library crate (used by other packages)

## What it does

The corpus library provides a single API for loading ruleworks regardless of where they are stored. It reads the `corpus-registry.yaml` manifest, authenticates with remote sources, fetches YAML files, and parses them into typed Rust structures.

Other packages use it:
- The **editor-api** uses it to serve ruleworks to the frontend
- The **admin** uses it to proxy corpus data to the dashboard
- The **pipeline** and the **poc-portal** use it as well

The engine does not depend on it. The engine reads ruleworks through `regelrecht-law-model` and gets them handed in by its caller.

## Key modules

| Module | Purpose |
|--------|---------|
| `registry.rs` | `CorpusRegistry` - loads `corpus-registry.yaml`, merges local overrides |
| `source_map.rs` | `SourceMap` - maps law IDs to parsed ruleworks |
| `models.rs` | `Source`, `SourceType` (Local/GitHub), `RegistryManifest` |
| `github.rs` | `GitHubFetcher` - fetches YAML via GitHub API (feature-gated) |
| `validation.rs` | Scope check: a `ScopeWarning` for each law that falls outside the jurisdictional scope its source declares |
| `auth.rs` | Token management for private repositories |
| `client.rs` | `CorpusClient` - higher-level read/write access used by editor-api |
| `backend.rs` | The `RepoBackend` trait and the factory that picks an implementation per source |
| `github_api_backend.rs` | API-only backend: no local clone, reads through the Contents API, writes buffered in memory and flushed as one PUT or DELETE per file |
| `implements_index.rs` | Reads the committed implements-index, the precomputed artefact that replaces a full-corpus scan for IoC resolution |
| `timing.rs` | Request-scoped phase timings, surfaced as a `Server-Timing` header and as tracing spans |
| `annotation_schema.rs`, `dto.rs`, `config.rs`, `error.rs` | Annotation schema validation (JSON schema, behind the optional `annotation-validation` feature), transfer types, deployment config, and error types |
| `bin/implements_indexer.rs` | The `implements-indexer` binary that generates the committed implements-index from a corpus checkout |

## Usage

```rust
use regelrecht_corpus::CorpusRegistry;
use std::path::Path;

// load(manifest_path, local_override_path): the second argument merges a
// corpus-registry.local.yaml override when present.
let registry = CorpusRegistry::load(Path::new("corpus-registry.yaml"), None)?;

// Local sources only; GitHub sources are skipped. `today` is the date
// each law's versions are collapsed against.
let local = registry.load_local_sources("2026-01-01")?;

// index_all_sources_async includes GitHub sources. For those it builds a
// metadata-only index; law bodies are fetched lazily on first read. Pass the auth file when
// private repositories need a token, or None when all sources are public.
// Sources that fail to enumerate are returned next to the map, not as an error.
let (source_map, failures) = registry
    .index_all_sources_async(Some(Path::new("corpus-auth.yaml")), "2026-01-01")
    .await?;

let law = source_map.get_law("wet_op_de_zorgtoeslag");
```

The `github` feature flag (on by default) enables remote fetching from GitHub repositories, including `load_favorites_async`, which fetches only a given set of law IDs. Without it, only local filesystem sources are available.

The HTTP calls themselves are not made here. Every GitHub REST request goes through `packages/github/` (crate `regelrecht-github`), a small hand-written client covering only the endpoints in use (trees, contents, refs, compare, pulls and archive downloads), with ETag and rate-limit state kept in one place. The editor API calls it directly as well; the corpus library keeps the domain layer on top.

## Further reading

- [Federated Corpus](/concepts/federated-corpus) - how the registry model works
- [Law Format](/concepts/law-format) - structure of the YAML files this library parses
