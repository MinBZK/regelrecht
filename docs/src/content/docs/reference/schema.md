# Schema Reference

The law format is defined by a JSON Schema. All law YAML files in the corpus must conform to this schema.

## Current Version

The current schema version is **v0.5.1**.

Schema URLs use immutable git tags to guarantee reproducibility. The format is:

```
https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.1/schema/v0.5.1/schema.json
```

The tag `schema-vX.Y.Z` is created when a schema version is released. Using tags instead of `refs/heads/main` ensures that the schema a law file references can never change underneath it. See [RFC-013](/rfcs/rfc-013) for the rationale.

## Version History

| Version | Description |
|---------|-------------|
| v0.5.1 | Current - tag-based immutable schema URLs |
| v0.5.0 | Operation set with engine, corpus migration, and WOO support |
| v0.4.0 | Open terms, implements, legal character, type specifications |
| v0.3.2 | Minor fixes |
| v0.3.1 | Patch release |
| v0.3.0 | Added cross-law references |
| v0.2.0 | Initial public schema |

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
