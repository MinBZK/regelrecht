# arch-extract

Generates the **code-derived architecture model** (`model.json`) for the
regelrecht workspace. One language-agnostic file describes the workspace from
application → crate → module → type → method, plus how the parts depend on and
use each other, so the docs site can render it without the diagrams drifting
from the code.

This is a build-time developer tool — an 11th, tooling-only workspace member. It
is not shipped or deployed; it only writes a file that the docs site consumes.

## Usage

```bash
just arch-generate   # regenerate docs/src/content/architecture/model.json
just arch-check      # fail if model.json is stale vs. the code (CI gate primitive)
```

Both recipes run the `arch-extract` binary from `packages/` so `cargo metadata`
discovers the workspace. Direct invocation:

```bash
cargo run -p regelrecht-arch-extract -- generate [--out <path>] [--stdout]
cargo run -p regelrecht-arch-extract -- check   [--out <path>]
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

  **Scope (v1):** the deep source pass runs for `engine` + `corpus` by default
  (`DEFAULT_DEEP_CRATES`). The crate graph above always covers all 10 crates;
  scaling the deep pass to the whole workspace is a config flip — `--deep-all`,
  or `--deep <a,b,…>` for a custom set — deferred to a follow-up so this first
  cut stays reviewable and the committed model small.

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
