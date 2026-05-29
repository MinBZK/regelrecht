---
title: "Schema Reference"
---

The law format is defined by a JSON Schema. All law YAML files in the corpus must conform to this schema.

## Current Version

The current schema version is **v0.5.2**.

Schema URLs use immutable git tags to guarantee reproducibility. The format is:

```
https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.2/schema/v0.5.2/schema.json
```

The tag `schema-vX.Y.Z` is created when a schema version is released. Using tags instead of `refs/heads/main` ensures that the schema a law file references can never change underneath it. See [RFC-013](/rfcs/rfc-013) for the rationale.

## Version History

This table is the single source of truth for which schema version introduced which construct. RFCs that add a construct reference this table rather than asserting a version independently.

| Version | Introduces | RFC |
|---------|-----------|-----|
| v0.5.2 | `annotation-schema.json` for stand-off notes | [RFC-005](/rfcs/rfc-005), [RFC-018](/rfcs/rfc-018) |
| v0.5.1 | Tag-based immutable schema URLs; refinements within the v0.5.x line | [RFC-013](/rfcs/rfc-013) |
| v0.5.0 | `hooks`, `overrides` (reactive execution); `procedure`, `procedure_id` (AWB lifecycle); WOO support | [RFC-007](/rfcs/rfc-007), [RFC-008](/rfcs/rfc-008) |
| v0.4.0 | `open_terms`, `implements` (IoC); `legal_character`; `date` and `array` value types | [RFC-003](/rfcs/rfc-003) |
| v0.3.2 | Minor fixes | — |
| v0.3.1 | Patch release | — |
| v0.3.0 | Cross-law references (`source`) | — |
| v0.2.0 | Initial public schema: `regulatory_layer`, `competent_authority`, `execution.produces` | [RFC-001](/rfcs/rfc-001), [RFC-002](/rfcs/rfc-002) |

Multi-organisation execution ([RFC-009](/rfcs/rfc-009)) reuses `competent_authority` (v0.2.0) and adds no schema construct of its own.

## Validation

Validate law files against the schema:

```bash
just validate                    # All files
just validate corpus/regulation/nl/wet/zorgtoeslag/2025-01-01.yaml  # Specific file
```

## Schema Structure

The schema defines:

- **Top-level metadata**: `$id`, `$schema`, `name`, `effective_date`
- **Service definition**: `input`, `output`, `articles`
- **Article structure**: `id`, `name`, `text`, `machine_readable`
- **Machine-readable**: `input`, `output`, `operations`
- **Operations**: Typed operations with `input` and `output` fields
- **Cross-references**: `source` blocks pointing to other regulations
- **Open terms**: `open_terms` and `implements` for delegation

See [Law Format](/concepts/law-format) for a guided walkthrough.
