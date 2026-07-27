# arch-extract

Generates the **code-derived architecture model** (`model.json`) for the
regelrecht workspace. One language-agnostic file describes the workspace from
application → crate → module → type → method, plus how the parts depend on and
use each other. It is the data source for the local **architecture explorer** (a
separate tool), which renders the model interactively.

This is a build-time developer tool — a tooling-only workspace member. It is not
shipped or deployed; it only reads the workspace and writes one JSON file.

## On-demand, never committed

The model is **generated on-demand and not committed to git** — the output path
`docs/src/content/architecture/model.json` is gitignored. The explorer
regenerates the model from the working tree, so it is **always current by
construction**: there is no committed artifact to drift, no staleness gate, and
no CI check. Generation of the whole workspace takes ~2 seconds (release build),
which is why keeping a committed copy in sync is not worth the trouble.

## Usage

### Explore it (`just arch-explore`)

```bash
just arch-explore   # build the UI, then serve the explorer on 0.0.0.0:7180
```

This is the intended entry point: it builds the Vue Flow frontend
(`ui/`, see below) and starts the `arch-extract serve` server, then you open
<http://localhost:7180> and zoom crate → module → type → method. The model is
generated **on-demand from the working tree** on the first request and cached on
the newest `packages/**/src/**/*.rs` mtime, so a code change shows up on the next
refresh (a fresh generation, ~2 s) while unchanged reloads are served from cache.

**Port & binding (dev container).** The server binds `0.0.0.0` — not
`127.0.0.1` — so it is reachable from the host, and defaults to port **7180**,
inside the container's forwarded 7100–7300 range. Override with the `--port`
flag or the `ARCH_EXPLORE_PORT` env var (stay within 7100–7300):

```bash
ARCH_EXPLORE_PORT=7200 just arch-explore
```

Direct invocation (from `packages/`, so `cargo metadata` finds the workspace):

```bash
cargo run -p regelrecht-arch-extract -- serve [--port <n>] [--ui-dir <dir>] [--manifest-path <p>]
```

The server serves `GET /api/model` (the model as JSON) and the built UI at `/`.
`--ui-dir` overrides where the built assets are read from (default
`packages/arch-extract/ui/dist`; also `ARCH_EXPLORE_UI_DIR`).

### The frontend (`ui/`)

`ui/` is a standalone Vite + Vue 3 app (Vue Flow) — its own npm project, not part
of the root npm workspace. `just arch-explore` builds it for you; to iterate on
the UI with hot-reload, run the Rust server in one terminal and Vite's dev server
(which proxies `/api` to it) in another:

```bash
cargo run -p regelrecht-arch-extract -- serve      # terminal 1 (API on :7180)
npm --prefix packages/arch-extract/ui run dev        # terminal 2 (UI on :7181)
```

### Known limitations

- **Accessibility.** The explorer is an internal-only developer tool. Its Vue
  Flow canvas is not screen-reader navigable and there is no text alternative for
  the graph; this is deliberate, accepted debt for now. Because the explorer
  lives outside the docs site, it does not touch the docs a11y gates.
- **Thin edge coverage.** `syn` parses per file without name resolution, so the
  model has relatively few relationship edges (~270 for ~2200 nodes). The UI is
  built to absorb more edges later without changes; improving edge coverage is a
  separate ticket.

### Just inspect the model (`just arch-generate`)

```bash
just arch-generate   # write model.json to the gitignored path, to inspect it
```

Direct invocation:

```bash
cargo run -p regelrecht-arch-extract -- generate [--out <path>] [--stdout] [--deep a,b | --deep-all]
```

`--stdout` prints the model instead of writing it; `--out` overrides the output
path. Both are inspection conveniences.

## Prose sidecar (wat/waarom per node)

The model captures the **structure** of the workspace automatically; the
**narrative** — what a part is for and why it exists — is written by hand (or by
an agent) and kept *beside* the model in a **prose sidecar** rather than in the
generator, because narrative that is generated only rots.

- **Format.** One Markdown file per node under `prose/`, keyed by the model's
  stable **node id**. Each file has a small frontmatter block and a free-text
  body:

  ```markdown
  ---
  node: crate:engine
  fingerprint: 31375e4c860d7f37
  ---
  **Wat.** … **Waarom.** …
  ```

  The `node:` field is authoritative (the filename is only a readable slug). The
  `fingerprint` records the shape of the node the prose was written against
  (kind, level, name, path, doc); when the node changes, the fingerprint no
  longer matches and the entry is flagged **stale**.

- **Scope.** Only the **container** and **component** levels (crates/binaries and
  modules/types) are in scope — the `code` level (methods, free functions) is
  deliberately skipped: ~1600 of the ~2200 nodes, whose intent their
  doc-comments already carry. Container prose is seeded; the rest is filled in
  over time via the drift flow below.

- **In the explorer.** The server exposes the sidecar at `GET /api/prose`
  (`{ node-id: markdown }`), and the explorer overlays it in a node's detail
  panel under "Wat & waarom" when prose exists for that node.

### Drift flow (`just arch-prose-*`)

Because there is no committed `model.json`, the prose commands **regenerate the
model on-demand** and diff it against the sidecar:

```bash
just arch-prose-status     # report drift (coverage + missing/stale/orphaned)
just arch-prose-check      # same, but exit non-zero on any drift
just arch-prose-sync       # scaffold stubs for undocumented in-scope nodes
just arch-prose-bless <id> # refresh a fingerprint after rewriting its prose (--all for every entry)
```

Three drift categories: **missing** (a new/undocumented node), **stale** (a node
whose shape changed since its prose was written), **orphaned** (prose whose node
is gone). Stubs are seeded with the node's existing doc-comment as a starting
point; they read as "still undocumented" until real prose is written.

### Scheduled proposal PR

`scripts/prose-drift-pr.sh` (also `just arch-prose-drift-pr`) is the scheduled
flow: it runs the drift check, and **only when there is drift** scaffolds the
stubs, commits them, and opens a **draft PR** with the drift report as proposals.
`DRY_RUN=true` previews without committing. It is meant to run on a schedule
(e.g. nightly). Wiring the cron trigger is a `.github/workflows/*.yml` change and
is intentionally left to a maintainer; a minimal workflow that invokes it:

```yaml
# .github/workflows/arch-prose-drift.yml (to be added by a maintainer)
name: Arch prose drift
on:
  schedule:
    - cron: '0 4 * * 1'   # Monday 04:00 UTC
  workflow_dispatch: {}
permissions:
  contents: write
  pull-requests: write
jobs:
  drift:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@stable
      - run: sudo apt-get update && sudo apt-get install -y mold just
      - run: just arch-prose-drift-pr
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## What it extracts

Two tiers feed one model (see `../../docs/src/content/architecture/model.schema.json`):

- **Crate graph** — workspace members and their internal path dependencies, from
  `cargo metadata`. Only *normal* (non-dev, non-build) dependencies become
  `depends-on` edges, which yields the documented production layer graph
  (`shared` → `law-model`/`auth` → `engine`/`harvester`/`corpus` → `pipeline` →
  `admin`/`editor-api`/`tui`).
- **Source structure** — modules, structs, enums, traits, methods and free
  functions (with the first line of each doc-comment), from a `syn` parse of a
  crate's `src/**.rs`. Plus best-effort `impl` (type → trait) and `uses`
  (type → type) edges. Test-only code (`#[cfg(test)]`, `#[test]`) is skipped.

  **Scope:** the deep source pass runs for **every crate** by default. Pass
  `--deep <a,b,…>` to narrow it to a subset (e.g. a quick local run);
  `--deep-all` is the explicit form of the default.

Nodes carry stable, path-shaped ids (`crate:engine`, `mod:engine::service`,
`type:engine::service::LawExecutionService`,
`fn:engine::service::LawExecutionService::execute`); containment is expressed via
`parent`, relationships via `edges`. The output is canonicalized (nodes sorted by
id, edges sorted and de-duplicated, **no timestamp**) so a node keeps a stable
identity between runs — the explorer relies on that.

For reference, a full run currently yields roughly 2200 nodes and a ~755 KB
pretty-printed model (per-crate node counts range from `shared` at the low end to
`engine` at the high end).

## Toolchain decision: `cargo metadata` + `syn` (not rustdoc-JSON)

The source-structure extraction was left open between two approaches. This is the
decision and its rationale.

**Chosen: `cargo metadata` + `syn` on the pinned stable toolchain.**

- **No nightly.** rustdoc-JSON (`cargo rustdoc -- -Z unstable-options
  --output-format json`) requires a **nightly** toolchain. The workspace is
  pinned to stable (`rust-toolchain.toml` → 1.96.0). A `syn` parse runs on the
  pinned stable toolchain with no extra toolchain to install, pin, or keep in
  sync — so the same command works locally and in CI.
- **Format stability.** The rustdoc-JSON format is explicitly unstable and its
  `FORMAT_VERSION` changes between nightlies; a consumer (the `rustdoc-types`
  crate) has to be upgraded in lockstep, and a nightly bump can silently break
  extraction. `syn`'s AST is stable across the supported edition.
- **No build required.** `syn` parses source text; it does not compile the
  workspace, so generation is fast and cannot be broken by an unrelated build
  failure. rustdoc-JSON must actually build each crate.
- **Right granularity.** For an architecture map at crate/module/type/method
  level plus doc-comments, source-level parsing is sufficient. We do not need
  rustdoc's fully type-resolved cross-references for v1.

**Accepted trade-offs (documented, not hidden).** `syn` sees one file at a time
with no name resolution or macro expansion, so:

- `impl`/`uses` edges are resolved best-effort by matching a type's leaf
  identifier against the *same crate's* own type nodes; cross-crate and
  macro-generated relationships are not captured.
- Types introduced purely by macros, and type aliases, are not emitted as nodes,
  so a handful of `impl` methods can reference a `parent` type id that has no
  node (e.g. an external `JsValue`). These are rare and harmless.

If a later phase needs resolved cross-crate types or trait-object relationships,
the model shape is designed to absorb an additional enrichment tier (e.g. a
nightly rustdoc-JSON pass) without changing consumers — the extraction method is
an implementation detail behind `model.json`.

## Tests

`cargo test -p regelrecht-arch-extract` (part of `just check`) runs the extractor
on-demand and validates the **generated** model against the JSON schema, asserts
the crate set and the known dependency layer graph, checks that the deep pass
covered every crate, and confirms generation is deterministic — so a malformed
model or a lost dependency edge fails CI.
