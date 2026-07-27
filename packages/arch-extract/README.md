# arch-extract

Generates the **code-derived architecture model** (`model.json`) for the
regelrecht workspace **and the docs-site pages derived from it**. One
language-agnostic file describes the workspace from application → crate → module
→ type → method, plus how the parts depend on and use each other; from that same
model the tool renders the Mermaid C4 views and a page per crate, so the docs
site shows the high-level architecture without the diagrams drifting from the
code.

This is a build-time developer tool — an 11th, tooling-only workspace member. It
is not shipped or deployed; it only writes files that the docs site consumes.

## Usage

```bash
just arch-generate   # regenerate model.json + the docs/src/content/docs/architecture pages
just arch-check      # fail if the model or any generated page is stale (CI gate primitive)
```

Both recipes run the `arch-extract` binary from `packages/` so `cargo metadata`
discovers the workspace. Direct invocation:

```bash
cargo run -p regelrecht-arch-extract -- generate [--out <path>] [--stdout] [--deep a,b]
cargo run -p regelrecht-arch-extract -- check   [--out <path>]
```

`--stdout` and a custom `--out` are inspection-only: they write just the model
JSON and skip the page files (which only make sense at their fixed docs
location).

## Generated docs pages

`generate` also writes, under `docs/src/content/docs/architecture/` (a section
of the docs content collection, so each file is a `/architecture/...` route):

- `context.md` — **C4Context**: the platform as one system.
- `container.md` — **C4Container**: the ten crates and their `depends-on` graph.
- `component.md` — **C4Component**: the top-level modules inside each crate.
- `crates/<crate>.md` — one page per crate: doc, dependencies/dependents, a
  C4Component diagram of its modules, and a table of its types.
- `index.md` — a hub linking the above.

The pages are plain Markdown with fenced ```mermaid C4 blocks; the docs build
renders them through the existing `rehype-mermaid` (inline SVG) +
`rehype-mermaid-alt.ts` (accessible name) pipeline. Rendering is deterministic
(everything sorted, no timestamp), so `just arch-check` gates staleness with a
clean `git diff`. **Do not hand-edit these files** — change the code and
regenerate.

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

  **Scope:** the deep source pass runs for **all ten crates** by default. Pass
  `--deep <a,b,…>` to narrow it to a subset (e.g. a quick local run);
  `--deep-all` is the explicit form of the default.

### Committed model size

Deep extraction of all ten crates makes `model.json` roughly **755 KB**
(pretty-printed). That is above the 500 KB `check-added-large-files` pre-commit
threshold, so a note on the choice:

- We keep the file **pretty-printed and committed**. Byte-stable, line-oriented
  output is a core property of this tool — it is what lets `arch-check` /
  `git diff --exit-code` gate drift and lets a reviewer read the change.
  Minifying would collapse the file to a single line (destroying diff
  readability) and still leave it ≈587 KB — over the threshold — so it buys
  nothing.
- `check-added-large-files` (no `--enforce-all`) only inspects **newly-added**
  files; `model.json` is already tracked, so a size bump on an existing file is
  not blocked by the gate.
- If a hard cap is ever wanted, the model can move to CI-only generation. That
  belongs with the CI staleness gate (a later phase); for now the committed,
  readable model is the source of truth for the tests and the generated pages.

Nodes carry stable, path-shaped ids (`crate:engine`, `mod:engine::service`,
`type:engine::service::LawExecutionService`,
`fn:engine::service::LawExecutionService::execute`); containment is expressed via
`parent`, relationships via `edges`. The output is canonicalized (nodes sorted by
id, edges sorted and de-duplicated, **no timestamp**) so regeneration is a clean
`git diff` and CI can gate on drift.

## Toolchain decision: `cargo metadata` + `syn` (not rustdoc-JSON)

The ticket left the source-structure extraction open between two approaches. This
is the decision and its rationale.

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
  node (e.g. an external `JsValue`). These are rare and harmless for rendering.

If v1 later needs resolved cross-crate types or trait-object relationships, the
model shape is designed to absorb a **CI-only nightly rustdoc-JSON pass** as an
additional enrichment tier without changing consumers — the extraction method is
an implementation detail behind `model.json`.

## Tests

`cargo test -p regelrecht-arch-extract` (part of `just check`) validates the
committed `model.json` against the JSON schema and asserts the crate count and
the known dependency layer graph, so a stale or malformed model fails CI.
