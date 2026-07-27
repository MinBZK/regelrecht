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

```bash
just arch-generate   # write model.json to the gitignored path, to inspect it
```

The recipe runs the `arch-extract` binary from `packages/` so `cargo metadata`
discovers the workspace. Direct invocation:

```bash
cargo run -p regelrecht-arch-extract -- generate [--out <path>] [--stdout] [--deep a,b | --deep-all]
```

`--stdout` prints the model instead of writing it; `--out` overrides the output
path. Both are inspection conveniences.

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
