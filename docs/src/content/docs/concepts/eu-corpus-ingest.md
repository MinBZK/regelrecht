---
title: "EU / LU corpus ingest"
description: "How European and Luxembourgish instruments land in the RegelRecht corpus, and what remains for EUR-Lex harvest."
---

RegelRecht’s working corpus has historically been Dutch (`regulation/nl/…`). Schema v0.5.9 adds an optional `jurisdictie` field (ISO 3166-1 alpha-2) so non-NL instruments can validate without a `bwb_id`.

## Layout

```
corpus/regulation/
  nl/…          # BWB / CVDR (existing)
  lu/…          # Luxembourg (manual / Legilux — demo present)
  eu/…          # EUR-Lex / CELLAR (planned)
```

File paths and URIs use `regulation/{cc}/{layer}/{law_id}/{valid_from}.yaml` (`cc` = `nl`, `lu`, `eu`, …). The engine URI parser accepts that shape; create-paths in the editor derive `cc` from `jurisdictie` (default `nl`).

## Phase 1 (done)

- Schema + law-model: `jurisdictie`; `bwb_id` only required for NL national layers
- Luxembourg flight-tax example under `corpus/regulation/lu/` with BDD scenarios
- `POST /api/v1/execute` and `GET /api/v1/regulations/{id}/nrml` on editor-api
- Minimal NRML bridge crate (`regelrecht-nrml-bridge`) for 4LM interop

## Phase 2 (stubbed)

The harvester recognises CELEX ids (`32016R0679`, `CELEX:…`) via `LawSourceType::EurLex` / `EurLexSource`. Detection and public URLs work; **download is not implemented** and returns `NotImplemented`.

Planned work:

1. CELLAR / EUR-Lex Notice API (or SPARQL) adapter → structured text
2. Map EU layers to `EU_VERORDENING` / `EU_RICHTLIJN` with required `celex_nummer`
3. Write under `regulation/eu/…` (and optionally federate via `corpus-registry.yaml`)
4. Legilux adapter for additional LU instruments → `regulation/lu/…`

Until then, add EU/LU YAML by hand (or convert from the 4LM NRML bridge) and run them through validate + BDD like any other law.

## Related

- [Federated Corpus](./federated-corpus) — multi-source registry
- [Schema Reference](../reference/schema) — `jurisdictie` in v0.5.9
- `4llm/4lm-×-regelrecht-concreet-integratievoorbeeld.md` — 4LM integration example
