# Review Guidelines

## Project context

regelrecht-mvp makes Dutch law machine-readable and executable. The engine evaluates
YAML-encoded law to produce legally binding decisions (beschikkingen) that directly
affect citizens — benefit entitlements, tax calculations, allowances. Errors are not
just bugs: a wrong operator, misplaced decimal, or broken cross-law reference produces
incorrect legal decisions at scale.

## Always check

### Legal faithfulness (regulation YAML changes)

When `machine_readable` sections are added or changed:

- Does the execution logic faithfully implement what the legal `text` says?
- Are percentages, thresholds, and amounts correct? (e.g., text says "4,273 procent", YAML should have `0.04273`)
- All monetary amounts MUST be in eurocent (integers). Flag any euro values or floats.
- Are conditions complete? If the law says "A AND B AND C", does the logic check all three?
- Are edge cases from the legal text handled (e.g., "tenzij" / "unless" clauses)?
- Do `definitions` values match exact numbers from the legal text?

### Cross-law reference integrity (regulation YAML changes)

- Do `source.regulation` values reference valid law `$id` slugs that exist in `regulation/`?
- Do `source.output` values match actual `output.name` fields in the referenced law?
- Are `source.parameters` passed correctly (matching the referenced law's parameter names)?
- Are `regelrecht://` URIs well-formed: `regelrecht://{law_id}/{output_name}#{field}`?

### Schema and format compliance (regulation YAML changes)

- Does the YAML structure conform to the schema (v0.3.0)?
- Are required fields present (`$schema`, `$id`, `regulatory_layer`, `valid_from`, etc.)?
- Are operation names valid (`IF`, `AND`, `OR`, `ADD`, `SUBTRACT`, `MULTIPLY`, etc.)?
- Are type declarations correct (`string`, `number`, `boolean`, `amount` with `type_spec.unit`)?

### Engine correctness (engine Rust changes)

- No `unwrap()` or `panic!()` on paths reachable during law execution — these crash the engine mid-decision. Use `Result`/`Option` propagation.
- Operation implementations must be mathematically correct (especially integer arithmetic for eurocent amounts — watch for overflow and rounding).
- Cross-law resolution must handle missing laws and circular references gracefully.
- New operations or types must be deterministic — same input must always produce same output.

### BDD scenario correctness (feature file changes)

- Do expected values match what the law actually prescribes?
- Are test data tables realistic and internally consistent?
- Do scenarios cover the important paths from the law (eligibility, calculation, exclusions)?

### Harvester and pipeline (harvester/pipeline/corpus changes)

- Does XML-to-YAML conversion preserve legal text faithfully (no dropped articles, no mangled Unicode)?
- Job queue operations: correct use of `FOR UPDATE SKIP LOCKED`, proper state transitions, retry logic with backoff, no lost jobs on worker crash.
- Law status transitions must be valid (unknown → queued → harvesting → harvested → enriching → enriched).
- Database migrations must be backwards-compatible and idempotent.

### Admin and frontend (admin/frontend changes)

- OIDC authentication: no session fixation, proper token validation, secure cookie settings.
- SQL injection: all queries must use parameterized statements, sort columns must be allowlisted.
- No XSS vectors in rendered content (law text may contain special characters).
- API pagination: no unbounded queries that could OOM on large datasets.

## Severity scale

- **Critical** — wrong legal outcome, data loss, runtime crash, security vulnerability
- **Significant** — likely bug, broken reference, missing edge case, lost jobs
- **Minor** — code quality, style, non-blocking improvement

## Skip

- Generated files under `packages/harvester/test-output/`
- Lock files (`Cargo.lock`, `package-lock.json`) unless dependencies were intentionally changed
- Formatting-only changes caught by `cargo fmt` or pre-commit hooks
