# Schema ↔ law-model conformance suite

The canonical, hand-authored `schema/*/schema.json` is the **public, language-agnostic
contract** for the law-YAML format — what a third-party engine author reads to know what a
valid law is, and what `just validate` enforces. The Rust `law-model` (`packages/law-model/`)
is *one* implementation that must provably **conform** to that contract. This suite proves it.

It is the structural twin of the BDD bucket-B engine-conformance suite (`bdd/conformance/`):
bucket-B proves an engine speaks the whole language *behaviourally*; this proves the model
accepts exactly the whole language *structurally*.

Run it with `just conformance` (it needs the engine's `validate` feature, which pulls in
`jsonschema`). The test lives in `packages/engine/tests/conformance.rs`.

## The contract

For every candidate law document `d`, with schema-valid set `S` and model-parseable set `M`:

- **Soundness** — `d ∈ M ⇒ d ∈ S`. If the model parses it, the schema accepts it
  (the model is not *more permissive* than the spec).
- **Completeness** — `d ∈ S ⇒ d ∈ M`, losslessly. If the schema accepts it, the model parses
  it and a re-serialize round-trips to a schema-valid, value-equal document (the model is not
  *more restrictive*, and does not silently drop data).

## Two tiers

- **Tier A — corpus differential** (`tier_a_corpus_differential`). Walks every
  `corpus/regulation/**/*.yaml` with a recognised `$schema`. Hard assertions (also guaranteed
  by the `just validate` CI gate, made explicit here): the schema accepts it and the model
  parses it. **Reported, non-fatal**: whether the re-serialized model is still schema-valid and
  value-stable — these quantify lossy serialization (the model emits `None` as `null`, etc.).
- **Tier B — synthetic fixtures** (`tier_b_fixtures`), under `valid/` and `invalid/`. Each
  fixture is a single, isolated construct.
  - `valid/`: schema accepts ∧ model parses ∧ re-serialized (null-normalized) still schema-valid.
  - `invalid/`: schema **rejects** (asserted). The model verdict is *measured* against `KNOWN_GAPS`.

## `KNOWN_GAPS` (the measurement)

The model has no `#[serde(deny_unknown_fields)]` and uses `#[serde(untagged)]` enums, so it is
currently **more permissive** than the schema. `KNOWN_GAPS` in `conformance.rs` lists the
`invalid/` fixtures the model *accepts* anyway — i.e. the soundness gap. The list is kept honest:
an **undocumented** gap fails the suite, and a **stale** entry (the model now rejects it) also
fails. Resolving these gaps (tighten the model vs. consciously declare it lenient) is a Phase-2
decision driven by this measurement.

## Adding a fixture

1. Drop a `*.yaml` law document into `valid/` or `invalid/`. Give it a `$schema` line pinned to
   the latest schema version and otherwise isolate the *one* construct under test (so a rejection
   is attributable to it — mind conditional `required` rules, e.g. `WET` requires `bwb_id`).
2. Run `just conformance`. For an `invalid/` fixture the model also accepts, add its filename to
   `KNOWN_GAPS` with a one-line note on *why* the model is lenient.
