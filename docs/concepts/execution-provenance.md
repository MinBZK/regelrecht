# Execution Provenance

Government agencies must be able to reproduce a specific decision months or years later, with the exact same result. Dutch administrative law requires this (Awb Art. 3:46, the AERIUS rulings), and the EU AI Act makes it mandatory for high-risk systems from August 2026.

Determinism within a single execution is necessary but not sufficient. Reproducibility requires pinning three things: the regulation YAML, the schema version it conforms to, and the engine version that executed it.

## The Execution Receipt

Every execution produces an **Execution Receipt**: an output envelope that contains everything needed to reproduce the result.

```json
{
  "provenance": {
    "engine": "regelrecht",
    "engine_version": "0.6.0",
    "schema_version": "v0.5.1",
    "regulation_id": "wet_op_de_zorgtoeslag",
    "regulation_valid_from": "2025-01-01",
    "regulation_hash": "sha256:a1b2c3..."
  },
  "scope": {
    "loaded_regulations": [
      { "id": "wet_op_de_zorgtoeslag", "valid_from": "2025-01-01", "hash": "sha256:a1b2c3..." },
      { "id": "regeling_zorgverzekering", "valid_from": "2025-01-01", "hash": "sha256:b2c3d4..." }
    ]
  },
  "execution": {
    "calculation_date": "2025-01-01",
    "parameters": { "bsn": "999993653" }
  },
  "results": {
    "outputs": { "heeft_recht_op_zorgtoeslag": true, "hoogte_zorgtoeslag": 209692 },
    "trace": { }
  }
}
```

The receipt records which engine version and schema produced the result, which regulations were loaded (with content hashes), the input parameters, and the full output with trace.

## Schema and engine versioning

The schema defines the regulation format. The engine interprets and executes regulations that conform to the schema. These are versioned independently:

- **Schema versions** are immutable directories under `schema/` (e.g., `schema/v0.5.1/schema.json`). A published version is never modified.
- **Engine versions** correspond to GitHub Release tags. Each release declares which schema versions it supports.

This distinction matters because third-party organisations may build their own engine implementations. The schema is the specification; the engine is one implementation of it.

## Cross-organisation reproducibility

When a decision depends on values from other organisations (via [Multi-Org Execution](./multi-org-execution)), the receipt captures the provenance of each accepted value:

```json
{
  "accepted_values": [
    {
      "output": "toetsingsinkomen",
      "value": 3200000,
      "authority": "inspecteur",
      "engine_version": "0.5.1",
      "regulation_hash": "sha256:d4e5f6...",
      "signed": true
    }
  ]
}
```

When reproducing the decision, the engine uses these **sealed accepted values** rather than re-calling the other organisation. The other organisation may now be running a different engine version. A *beschikking* stands once issued — the accepted value at the time is a legal fact.

## Why this matters

A flagged, reproducible decision is fundamentally different from an opaque one. Citizens can request their trace. Auditors can verify the computation. Courts can reconstruct the reasoning. And when a bug is found, every affected decision can be identified by querying receipts for the engine version and regulation hash.

## Further reading

- [Hooks and Reactive Execution](./hooks-and-reactive-execution) - AWB procedure hooks
- [Multi-Org Execution](./multi-org-execution) - cross-organisation value exchange
- [RFC-013: Execution Provenance](/rfcs/rfc-013) - full specification
- [RFC-014: Engine Conformance](/rfcs/rfc-014) - conformance test suite
