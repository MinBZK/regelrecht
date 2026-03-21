# RFC-010: Federated Corpus — Decentralized Regulation Sources

**Status:** Proposed
**Date:** 2026-03-18
**Authors:** Anne Schuth

## Context

The corpus (`regulation/nl/`) lives in the regelrecht-mvp repo alongside the engine, pipeline, admin, and editor. That works for an MVP, but doesn't match the legislative reality: regulation is decentralized. Municipalities, provinces, water boards, and ministries each produce their own regulations, often filling in details delegated by higher-level laws.

RFC-003 (Inversion of Control) introduces IoC with `open_terms` and `implements`. This lets a municipality fill in a national law without modifying that law. Technically this already works cross-repo, as long as the engine loads all relevant laws. What's missing is the infrastructure for that:

- **Discovery**: how does the engine find municipal regulations?
- **Loading**: how does the engine fetch laws from multiple sources?
- **Scope validation**: how does the engine detect that a source claims to represent a jurisdiction it shouldn't?
- **Write-back**: how can a municipality maintain regulations in their own repo via the editor?

The old delegation approach (originally described in a previous version of RFC-003) has been superseded by RFC-003's IoC mechanism. This RFC builds on RFC-003 by adding the infrastructure for multiple sources, each with their own ownership.

## Decision

Four interconnected decisions form the federation model.

### 1. Corpus as a separate repo

The corpus (`regulation/nl/`) moves to its own repository, e.g. `MinBZK/regelrecht-corpus`. The regelrecht-mvp repo then contains only the engine, pipeline, admin, and editor.

External sources are any Git repository that follows the required directory structure. They don't need to be forks or clones of the central corpus. A municipality like Amsterdam creates a repo `gemeente-amsterdam/regelrecht-amsterdam` containing their municipal regulations that declare `implements` on laws from the central corpus.

#### Required source structure

A source repository must follow this layout:

```
regulation/nl/
  {law_type}/           # e.g. wet/, ministeriele_regeling/, verordening/
    {law_name}/
      {law_name}.yaml   # law file conforming to the regelrecht YAML schema
```

Each YAML file must:
- Conform to the regelrecht YAML schema (referenced via `$schema`)
- Have a `$id` that identifies the law (does not need to be globally unique - see Priority below for conflict resolution when multiple sources provide the same `$id`)
- Declare `implements` (per RFC-003) when filling in open terms from a higher-level law

The `path` field in the registry manifest points to the root of this structure within the repo. This allows repos to keep regulations in a subdirectory (e.g. `regulation/nl/`) or at the root.

### 2. Registry manifest

A YAML manifest describes which sources the engine should load:

```yaml
schema_version: "1.0"
sources:
  - id: minbzk-central
    name: "MinBZK Central Corpus"
    type: github
    github:
      owner: MinBZK
      repo: regelrecht-corpus
      branch: main
      path: regulation/nl
    scopes: []
    priority: 1

  - id: amsterdam
    name: "Gemeente Amsterdam"
    type: github
    github:
      owner: gemeente-amsterdam
      repo: regelrecht-amsterdam
      branch: main
      path: regulation/nl
    scopes:
      - type: gemeente_code
        value: "GM0363"
    priority: 10
    auth_ref: amsterdam

  - id: local-dev
    name: "Local development"
    type: local
    local:
      path: ./regulation/nl
    scopes: []
    priority: 100
```

**Manifest files:**

The manifest has two layers, similar to `.env` + `.env.local` or `docker-compose.yaml` + `docker-compose.override.yaml`:

- `corpus-registry.yaml` - checked into the repo, versioned. Contains the "official" source list.
- `corpus-registry.local.yaml` - gitignored, personal. Contains overrides for local development.

The engine merges both files: all sources from the central manifest plus all sources from the local file. If a local entry has the same `id` as a central entry, the local one **replaces it entirely** - there is no field-level inheritance. Any field not repeated in the local entry is dropped, including `auth_ref`. A local override must be a complete source definition.

Example: you want to point the Amsterdam source to your own fork and add a directory with test laws. Put this in `corpus-registry.local.yaml`:

```yaml
schema_version: "1.0"
sources:
  # Replaces the central amsterdam entry (same id)
  - id: amsterdam
    name: "Amsterdam (my fork)"
    type: github
    github:
      owner: my-github-user
      repo: regelrecht-amsterdam
      branch: feature/new-ordinance
      path: regulation/nl
    scopes:
      - type: gemeente_code
        value: "GM0363"
    priority: 10

  # New entry, added to the list
  - id: local-tests
    name: "Local test laws"
    type: local
    local:
      path: ./test-regulation
    scopes: []
    priority: 100
```

**Scopes:**

Scopes are **claims**: a source declares which jurisdiction(s) it provides regulations for. A scope is not a routing mechanism ("use this source when someone asks for GM0363") but an ownership declaration ("this source contains regulations from the municipality of Amsterdam").

The engine uses scopes for two things:

1. **Validation** - if a law from a source with scope `gemeente_code: GM0363` declares `gemeente_code: GM0518` in the law itself, the engine generates a warning. The source claims to be Amsterdam but delivers a law for a different municipality.
2. **Filtering** - when running the engine for a specific municipality, scopes determine which sources are relevant and which can be skipped.

A source without scopes (like the central corpus) delivers laws without jurisdictional restrictions. A source can have multiple scopes, for example a province that delivers regulations for multiple scope types:

```yaml
scopes:
  - type: provincie_code
    value: "PV27"
  - type: waterschap_code
    value: "WS0155"
```

**Priority:**

Multiple sources may provide a law with the same `$id`. This is allowed and expected: a local development source may override a central law for testing, or a municipality may provide a patched version of a national law during a transition period. **Lower priority value = higher priority** — the law from the source with the lowest priority number is the one the engine uses. Think of it as rank: priority 1 outranks priority 10.

Where this matters in practice:
- **Development**: your local source (priority 100) contains a modified version of a central law (priority 1). Lower the local priority to 0 to make it win, so you can test without modifying the central manifest.
- **Migration**: when moving laws between sources, temporary overlap is normal.
- **Patches**: a municipality may temporarily override a central law with a corrected version by using a lower priority number.

When two sources have equal priority and the same `$id`, the engine raises an error at load time. This is detected when sources are fetched and indexed, not deferred to per-request execution. A misconfigured source fails clearly at startup, not when a citizen's request hits it.

All `$id` collisions (including resolved ones) are logged and surfaced via the `/api/sources` admin endpoint, so operators have visibility into which laws are being shadowed and by which source.

**Temporal consistency (reference_date):**

The engine already supports multi-version laws via `valid_from` and `reference_date` (see `resolver.rs`). In a federated model this becomes more important: when executing a law as it was on 2025-01-15, you need all sources at that date, not just the law versions.

Each source in the manifest can optionally pin a Git ref:

```yaml
sources:
  - id: minbzk-central
    name: "MinBZK Central Corpus"
    type: github
    github:
      owner: MinBZK
      repo: regelrecht-corpus
      branch: main
      path: regulation/nl
      ref: "v2025.1"  # optional: tag, branch, or commit SHA
    scopes: []
    priority: 1
```

When `ref` is absent, the engine uses the `branch` head (latest). For reproducible historical execution, sources should use tags following a date-based convention (e.g. `v2025.1`, `v2025-01-15`). Tags are explicit, immutable, and don't depend on commit timestamp semantics.

The engine resolves temporal consistency as follows:

1. **Pinned ref**: if `ref` is set, use that exact tag or commit SHA. This is the recommended approach for production.
2. **Branch head**: if only `branch` is set and no `ref`, use the latest commit on that branch. Suitable for development but not reproducible.
3. **Local sources**: always at their current filesystem state.

Within each source, the existing `valid_from` filtering still applies: the engine selects the law version valid at the `reference_date`. The two mechanisms work together: Git tags give you the right file versions, `valid_from` gives you the right law versions within those files.

### 3. Authentication

Credentials are **completely separate** from the registry manifest. The manifest contains no tokens, passwords, or secrets. The only auth-related field in the manifest is an optional `auth_ref`: a string that refers to an entry in a separate auth file. Without `auth_ref`, the engine assumes the source is public.

**Auth types** (enum):

| Type | Description |
|------|-------------|
| `none` | Public repo, no auth needed (default when `auth_ref` is absent) |
| `github_pat` | GitHub Personal Access Token |
| `github_app` | GitHub App installation token (for organizations) |

Two mechanisms for configuring credentials, both valid:

**Convention-based environment variables:**
```
CORPUS_AUTH_AMSTERDAM_TOKEN=ghp_abc123...
CORPUS_AUTH_MINBZK_CENTRAL_TOKEN=ghp_def456...
```

The naming convention is `CORPUS_AUTH_{SOURCE_ID_UPPERCASE}_TOKEN`, where hyphens in the source ID become underscores. With env vars, the type is always `github_pat`.

**Auth config file** (`corpus-auth.yaml`, gitignored):
```yaml
amsterdam:
  type: github_pat
  token: ghp_abc123...

minbzk-central:
  type: github_app
  app_id: 12345
  private_key_path: /etc/regelrecht/keys/minbzk.pem
  installation_id: 67890
```

Lookup order: env var first, then auth config file. Env vars work well in CI/CD and containers; the config file is more convenient when developing locally with multiple sources.

**Editor auth:** the editor stores a GitHub token in the browser using `sessionStorage` (not `localStorage`, to avoid persisting tokens across sessions and reducing XSS exposure). This token is not part of the registry manifest or auth config - it's per-user and only used client-side for the write path.

### 4. Discovery, loading, and writing

#### Read path (engine + admin)

```
corpus-registry.yaml
        |
        v
+-------------------+
| CorpusRegistry    |  parses manifest, merges with local overrides
+--------+----------+
         |
    +----+----+
    v         v
+--------+ +----------+
|GitHub  | |Filesystem|
|Fetcher | |Fetcher   |
+---+----+ +----+-----+
    |           |
    v           v
+-------------------+
|  RuleResolver     |  +source_map, +load_sourced_law()
+-------------------+
```

- `CorpusRegistry` parses the manifest and dispatches to the appropriate fetcher per source
- `GitHubFetcher` uses the GitHub Trees API (1 call per repo for directory structure) and the Contents API (per file, for YAML content). Note: the Trees API with `recursive=1` truncates responses exceeding 100,000 entries or 7 MB (`truncated: true`). The fetcher must detect this and fall back to per-directory traversal.
- ETag caching: the fetcher stores ETags and sends `If-None-Match` headers, so a 304 costs no bandwidth
- Rate limit tracking: the fetcher reads `X-RateLimit-Remaining` headers and warns when limits are low
- `RuleResolver` gets a `source_map: HashMap<String, SourceId>` tracking which source each loaded law came from
- `load_sourced_law(law_id, source_id)`: loads a specific law from a specific source
- On conflicts (same `$id` from multiple sources), the source with the highest priority wins

**Scope validation:** scopes are claims, not hard-enforced boundaries. A law from a source with scope `gemeente_code: GM0363` that declares `gemeente_code: GM0518` generates a warning, not an error. The engine trusts the source but signals deviations.

#### Write path (editor)

The editor can write directly to a GitHub repo via the GitHub Contents API, without a backend intermediary.

Flow:
1. User edits YAML in the editor
2. Preview and validation against the schema
3. Commit via `PUT /repos/{owner}/{repo}/contents/{path}` using the GitHub token from the browser
4. Branch management: the editor works on a feature branch and can create a PR via the GitHub API (`POST /repos/{owner}/{repo}/pulls`)

This makes the editor a full YAML editor for municipalities: edit, validate, commit, create PR, without needing a local development environment.

#### Admin API

New endpoints on the admin service:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/sources` | GET | Source list with status (last synced, law count, errors) |
| `/api/corpus/laws` | GET | All loaded laws with source metadata (source ID, priority, scopes) |
| `/api/sources/{id}/sync` | POST | Force re-fetch of a specific source |

#### Editor UI

- **Source badge**: each law shows a badge with the source name (e.g. "MinBZK Central" or "Gemeente Amsterdam")
- **Source picker**: dropdown or sidebar to switch between sources
- **Add source**: dialog to enter a GitHub owner/repo and add it as a source
- **Cross-source `implements` visualization**: show which municipal law fills in which national law
- **Save to repo**: button that opens a commit dialog (choose branch, commit message, create PR)

## Why

### Benefits

- **Fits IoC**: the `implements` mechanism from RFC-003 works cross-repo as long as the laws are loaded. The registry makes that loading explicit and configurable.
- **Decentralized ownership**: municipalities, provinces, and water boards manage their regulations in their own repo. No PR to a central repo needed to change a municipal ordinance.
- **Transparent**: the registry manifest is a YAML file in Git. Anyone who wants to see which sources the engine loads can look at the manifest.
- **Scope validation as trust boundary**: without complex PKI infrastructure, the engine can validate via scopes that a source doesn't deliver laws outside its jurisdiction.
- **Incrementally adoptable**: start with the central corpus and add sources as municipalities are ready. The `type: local` option makes local development easy.

### Tradeoffs

- **GitHub API rate limits**: 60 requests per hour without auth, 5,000 with a token. With many sources or large repos this can become a bottleneck. Mitigation: ETag caching, Trees API (1 call per repo), and tokens per source.
- **Write path complexity**: branch management, merge conflicts, and PR handling in the editor is non-trivial. This is deliberately planned for a later phase.
- **Schema version compatibility**: when the YAML schema changes, all sources need to follow. The engine must handle sources on an older schema version.
- **GitHub dependency**: the entire model relies on GitHub as hosting platform. The `type: local` fallback and the fact that the manifest is a simple YAML file make it extensible to other platforms (GitLab, Gitea), but that's not a priority now.

### Alternatives Considered

**Alternative 1: Git clone instead of API**

Clone the entire repo to the filesystem and read files locally. Conceptually simpler, but requires a git binary in the container, more disk space, and makes it harder for the editor to write directly to a repo without a backend. The GitHub API approach is lighter and works both from the engine (server-side) and the editor (client-side).

**Alternative 2: Central discovery service**

A separate service that tracks which repos exist and caches their metadata. Introduces an extra component to manage and deploy. The manifest-in-Git pattern is more transparent, reviewable, and has no runtime dependency on an additional service.

**Alternative 3: Git submodules**

Use submodules to include external repos in the corpus. Too rigid: every addition or update of a source requires a commit in the parent repo. With dozens of municipalities this becomes unmanageable. The registry manifest decouples registering sources from loading them.

### Implementation Notes

Implementation is planned in seven phases, each delivering a working whole.

**Phase 1 - Decouple corpus**
Move `regulation/nl/` to its own repo (`MinBZK/regelrecht-corpus`). Update CI, tests, and engine configuration to load from the new repo.

**Phase 2 - Registry + local multi-source**
Implement `CorpusRegistry` and `corpus-registry.yaml` parsing. Extend `RuleResolver` with `source_map` and priority-based conflict resolution. Support `type: local` so the current workflow keeps working.

**Phase 3 - GitHub fetcher**
Implement `GitHubFetcher` with Trees API, Contents API, ETag caching, and rate limit tracking. Auth via environment variables and `corpus-auth.yaml`.

**Phase 4 - Admin API**
Add `/api/sources`, `/api/corpus/laws`, and `/api/sources/{id}/sync` endpoints to the admin service.

**Phase 5 - Editor multi-source reading**
Source picker, source badge per law, and direct GitHub access from the editor (via the GitHub API, not via the admin backend).

**Phase 6 - Editor writing**
Commit via GitHub Contents API, branch management, and PR creation from the editor.

**Phase 7 - Validation and polish**
Scope validation, schema version compatibility checks, collision reporting, and edge case handling.

### Affected components

| File | Role | Change |
|------|------|--------|
| `packages/engine/src/resolver.rs` | Law registry + indexes | +`source_map`, +`load_sourced_law()` |
| `packages/engine/src/article.rs` | `ArticleBasedLaw` struct | Unchanged (source is metadata on resolver, not on law) |
| `packages/engine/src/service.rs` | `LawExecutionService` | +registry loading, +source in trace output |
| `packages/engine/src/uri.rs` | `regelrecht://` URI parsing | Unchanged (URIs refer to law_id, not source) |
| `packages/corpus/src/config.rs` | Existing corpus config | Reuse pattern for auth config |
| `packages/admin/src/handlers.rs` | Admin API endpoints | +`/api/sources`, +`/api/corpus/laws` |
| `frontend/src/composables/useLaw.js` | Law loading | +multi-source, +GitHub direct access |
| `frontend/src/EditorApp.vue` | Editor UI | +source picker, +badge, +write-back |

## References

- RFC-003: Inversion of Control with `open_terms` and `implements`
- GitHub Trees API: `GET /repos/{owner}/{repo}/git/trees/{tree_sha}?recursive=1`
- GitHub Contents API: `GET /repos/{owner}/{repo}/contents/{path}`, `PUT /repos/{owner}/{repo}/contents/{path}`
