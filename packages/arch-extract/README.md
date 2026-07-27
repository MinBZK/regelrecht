# arch-extract

Generates the **code-derived architecture model** (`model.json`) for the
regelrecht workspace. One language-agnostic file describes the whole codebase —
the Rust workspace (crate → module → type → method) **and** the JS/TS/Vue
frontends (app → directory → component/composable/module) — plus how the parts
depend on and use each other. It is the data source for the local
**architecture explorer** (a separate tool), which renders the model
interactively.

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
- **Best-effort edge coverage on the Rust tier.** `syn` parses per file without
  a full name-resolver or macro expansion, so edges are resolved with a
  workspace-wide symbol table plus per-file `use` resolution (see "Edge
  resolution" below) rather than by compiling the code. This captures
  cross-crate references, `use`-aliases and multi-segment paths, but anything
  introduced purely by a macro, reached through a glob import, or renamed by a
  re-export at a path we can't see is still missed. The stance is deliberate:
  a missing edge is preferred over a wrong one, because the explorer presents an
  edge as a fact.

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

Three tiers feed one model (see `../../docs/src/content/architecture/model.schema.json`):

- **Crate graph** — workspace members and their internal path dependencies, from
  `cargo metadata`. Only *normal* (non-dev, non-build) dependencies become
  `depends-on` edges, which yields the documented production layer graph
  (`shared` → `law-model`/`auth` → `engine`/`harvester`/`corpus` → `pipeline` →
  `admin`/`editor-api`/`tui`).
- **Rust source structure** — modules, structs, enums, traits, methods and free
  functions (with the first line of each doc-comment), from a `syn` parse of a
  crate's `src/**.rs`. Plus best-effort `impl` (type → trait) and `uses` edges;
  the `uses` edges come from struct fields, enum-variant fields, and the
  parameter/return types of methods (attributed to the owning type) and free
  functions (attributed to the function). Cross-crate references are resolved —
  see "Edge resolution" below. Test-only code (`#[cfg(test)]`, `#[test]`) is
  skipped.

  **Scope:** the deep source pass runs for **every crate** by default. Pass
  `--deep <a,b,…>` to narrow it to a subset (e.g. a quick local run);
  `--deep-all` is the explicit form of the default.
- **Frontends (JS/TS/Vue)** — the npm-workspace frontends (`frontend/`,
  `frontend-lawmaking/`, `packages/frontend-shared/`, discovered from the root
  `package.json` `workspaces`). Each app's `src/` tree (plus a top-level
  `main`/`index` entry) becomes nodes: a `.vue` file is a **component**, a
  `useXxx.{js,ts}` file a **composable**, any other JS/TS file a **module**, all
  nested under **directory** grouping nodes. ES `import`s become edges — a
  relative import is a file→file `uses` edge (so you see which component uses
  which composable), and importing one app's package from another is a cross-app
  `depends-on` edge (both Vue apps → `frontend-shared`). Test/spec, `*.config.*`
  and `*.d.ts` files are skipped (the JS analogue of skipping `#[cfg(test)]`).
  See the toolchain decision below for why this is a regex scan, not an AST
  parser or a Node script.

Nodes carry stable, path-shaped ids; containment is expressed via `parent`,
relationships via `edges`. The prefixes are per-language but share the same
`prefix:path::…` shape:

| tier | prefixes | example |
|------|----------|---------|
| Rust | `crate:` `bin:` `mod:` `type:` `fn:` | `type:engine::service::LawExecutionService` |
| JS/TS/Vue | `app:` `dir:` `component:` `composable:` `module:` | `component:frontend::src::components::LawTable` |

A JS node's id is derived from its **path only** (never its contents), so it is
stable: the `component:`/`composable:`/`module:` split follows the file
extension and the `useXxx` naming convention, not a parse of what the file
exports. The output is canonicalized (nodes sorted by id, edges sorted and
de-duplicated, **no timestamp**) so a node keeps a stable identity between runs —
the explorer relies on that.

For reference, a full run currently yields roughly 2400 nodes (about 2200 Rust +
180 frontend) and ~1500 edges, and generates in ~2.2 s (release build) — well
inside the on-demand budget. Collecting the signature-level `uses` references
added ~0.3 s over the earlier structure-only pass. Per-crate node counts range
from `shared` at the low end to `engine` at the high end.

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

### Edge resolution (the "middle road")

`syn` sees one file at a time with no name-resolver and no macro expansion, so a
type reference is just an identifier or a path — there is no compiler to tell us
which declaration it points at. The first versions of this tool therefore
resolved `impl`/`uses` edges by matching a type's *leaf* identifier against the
same file's crate only, which missed everything that arrived through a `use`
alias, a multi-segment path or another crate — the Rust tier had ~270 edges for
~2200 nodes, almost all of them same-crate struct fields.

The pass now takes a middle road between "one file at a time" and full type
resolution, without building a type-checker (`syn_pass.rs`):

1. **A workspace-wide symbol table.** Every deep-parsed crate contributes its
   declared type/trait leaf names (`crate → leaf → node id`). A reference can
   now resolve to a type in *another* crate, not only its own. A leaf that is
   declared twice in a crate is recorded as ambiguous and never resolved.
2. **Per-file `use` resolution.** Each file's `use` statements are read into an
   in-scope-name → target map. `use regelrecht_law_model::money::Cents as Bedrag`
   makes `Bedrag` resolve to `(law-model, Cents)`; `use crate::service::Foo`
   pins `Foo` to this crate. The crate of a path is identified from its leading
   segment (`crate`/`self`/`super`, or an extern-crate ident like
   `regelrecht_corpus` matched against `cargo metadata`).
3. **Drop, don't guess.** A name explicitly imported from a non-workspace crate
   (`use std::io::Error`) is recorded as *external* and never leaf-matched
   locally; a multi-segment path whose head is an unknown crate (`std::fmt::…`)
   is dropped rather than reduced to its leaf. Only a reference whose crate is
   known **and** whose leaf is unambiguous inside that crate becomes an edge.

**What this now covers.** Cross-crate `uses` and `impl` edges (e.g. `editor-api`
→ `corpus`, `corpus` → `github`, and the `law-model` `ArticleBasedLaw` → engine
`LawLoad` trait impl); `use`-aliases and multi-segment/`crate::`-qualified
paths; and more reference *sites* — enum-variant fields and the parameter/return
types of methods and free functions, on top of the original struct fields. In
this workspace that took the Rust tier from ~270 edges to ~1150 (of which ~110
are cross-crate), with no committed false edges (spot-checked, and guarded by
unit tests in `syn_pass.rs` plus the integration tests in
`tests/model_validation.rs`).

**Accepted trade-offs (documented, not hidden).** This is still best-effort, not
name resolution, so it does not capture:

- **Macros.** Types, impls or fields introduced purely by a macro are invisible
  to a source parse — `syn` never sees them. A handful of `impl` methods can
  still reference a `parent` type id that has no node (e.g. an external
  `JsValue`); dangling edges are dropped by the canonicalizer.
- **Glob imports.** `use foo::*` brings in names we cannot enumerate, so a bare
  reference resolved only through a glob is missed (never guessed).
- **Re-exports.** A type used through a `pub use` at a path different from its
  declaration resolves by crate + leaf, which is usually enough; but a re-export
  that also *renames* across a crate boundary we can't see is missed.
- **Type aliases** are not emitted as nodes, so an alias used as a field type
  resolves to the alias name (dropped) rather than the aliased type.
- **No `calls` edges.** A full call-graph remains deliberately out of scope; the
  `uses` edges from a function's signature are as deep as this pass goes.

If a later phase needs resolved cross-crate types or trait-object relationships,
the model shape is designed to absorb an additional enrichment tier (e.g. a
nightly rustdoc-JSON pass) without changing consumers — the extraction method is
an implementation detail behind `model.json`.

## Toolchain decision: a self-contained regex import scan (not swc/oxc, not Node)

The frontend tier had the same open question as the Rust one. This is the
decision and its rationale.

**Chosen: a self-contained regex scan of the ES-module `import`/`export … from`
statements, inside this Rust binary** (`js_pass.rs`). For `.vue` single-file
components the `<script>` / `<script setup>` block is cut out first, so template
text can't masquerade as an import.

Two heavier routes were rejected:

- **A JS/TS AST parser in-crate (`swc_ecma_parser`, `oxc`).** These are large
  compile-time dependencies, and the model only needs the import *specifiers*,
  not a resolved syntax tree — the same best-effort, structural stance the `syn`
  tier already takes (no name resolution). The cost/benefit does not justify the
  dependency weight for reading strings after `from`.
- **A separate Node script writing a partial model.** It would give the best Vue
  SFC fidelity, but it breaks the property the whole tool is built on: the
  explorer regenerates the model **on-demand from the working tree**, so a Node
  step would require `node_modules` installed and Node on `PATH` on every
  request. That is exactly the "no extra toolchain" argument that picked `syn`
  over nightly rustdoc-JSON above. Keeping everything in one stable-toolchain
  binary keeps `just arch-explore` and CI identical and dependency-free.

**Accepted trade-offs (documented, not hidden).** Like the `syn` tier, this is
best-effort:

- No name resolution: only relative imports that resolve to a file we actually
  emitted become `uses` edges; bare imports of third-party packages are ignored,
  and a subpath import of another workspace app still yields the app-level
  `depends-on` but not a file-level edge.
- The scanner is textual: a specifier inside a comment could be over-counted,
  and an import assembled dynamically (a computed specifier) is missed. These
  are rare and harmless for an architecture map.
- Node ids are path-derived, so two files that differ only by a non-`.vue`
  extension in the same directory (e.g. `Foo.js` and `Foo.ts`) would collide on
  one `module:` id; the canonicalizer keeps the first. This does not occur in
  the current tree and is the JS analogue of `syn`'s "rare and harmless" alias
  gaps.

## Tests

`cargo test -p regelrecht-arch-extract` (part of `just check`) runs the extractor
on-demand and validates the **generated** model against the JSON schema, asserts
the crate set and the known dependency layer graph, checks that the deep pass
covered every crate, asserts the three frontends appear as `app`s with their
component/composable nodes and import edges (app→app `depends-on` and a
component→composable `uses`), asserts concrete cross-crate `uses`/`impl` edges
now resolve (and that no edge points outside the workspace — the "no false edge"
guard), and confirms generation is deterministic — so a malformed model or a lost
edge fails CI. The edge-resolution logic itself (path/`use` resolution, the
drop-don't-guess rules) is unit-tested in `src/syn_pass.rs`.
