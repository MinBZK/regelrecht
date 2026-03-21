# RFC-003: Inversion of Control for Delegated Legislation

**Status:** Accepted
**Date:** 2026-03-15
**Authors:** Eelco Hotting

## Context

Dutch legislation follows a hierarchical delegation pattern: a formal law (wet) delegates authority to lower regulatory layers. For example, the Wet op de zorgtoeslag, article 4, delegates the determination of the standaardpremie to the minister via a ministerial regulation.

The engine previously supported this via a **top-down** `resolve` action: the higher law explicitly searches for matching lower regulations using `legal_basis` indexes and `select_on` criteria. This inverts the real legal relationship. In practice, a ministerial regulation opens with "Gelet op artikel 4 van de Wet op de zorgtoeslag" — it *registers itself* as filling in a delegated term.

The top-down approach has limitations:
- The higher law must know how to find its implementations
- Adding new implementations requires modifying the higher law's YAML
- The pattern doesn't match how legislation actually works

## Decision

Implement **Inversion of Control (IoC)** via two new constructs in the schema:

### `open_terms` (on article-level `machine_readable`)

Declares abstract values that can or must be filled by implementing regulations:

```yaml
machine_readable:
  open_terms:
    - id: standaardpremie
      type: amount
      required: true
      delegated_to: minister
      delegation_type: MINISTERIELE_REGELING
```

### `implements` (on article-level `machine_readable`)

Declares that an article fills an open term from a higher-level law:

```yaml
machine_readable:
  implements:
    - law: zorgtoeslagwet
      article: '4'
      open_term: standaardpremie
      gelet_op: "Gelet op artikel 4 van de Wet op de zorgtoeslag"
```

### Resolution model

1. Engine indexes all `implements` declarations at law load time
2. When executing an article with `open_terms`, the engine looks up implementations
3. **Temporal filtering**: for each candidate, the engine selects the version valid for the calculation date (`valid_from <= calculation_date`). Candidates with no valid version for the requested date are excluded. This ensures that e.g. the 2025 standaardpremie is used for a 2025 calculation, even if a 2026 version is also loaded
4. **Scope filtering**: candidates are filtered against the execution scope (e.g., `gemeente_code`). A scoped regulation only matches when the execution parameters contain the same value. Unscoped (national) regulations always match
5. **Priority resolution**: among remaining candidates, **lex superior** (higher regulatory layer wins) then **lex posterior** (newer `valid_from` wins). When candidates have the same layer and date, this is ambiguous — the engine returns an error rather than silently picking one. This is a law authoring error that needs fixing
6. If found: execute the implementing article to get the value
7. If not found + has `default`: execute the default actions block
8. If not found + `required: true` + no default: error
9. If not found + `required: false` + no default: skip (traced)
10. **Cycle detection**: if an open term is already being resolved (via `ResolutionContext.visited`), a `CircularReference` error is raised — circular dependencies are a law authoring problem, not something the engine should fix. The cycle detection key uses `\0` (null byte) as separator to prevent key collisions when law IDs or article numbers contain `#`
11. **Delegation type validation**: the engine validates that an implementing regulation's `regulatory_layer` matches the open term's `delegation_type`. If a gemeente verordening tries to implement a term delegated to a minister, the engine rejects it with a clear error
12. **Array size validation**: `open_terms` and `implements` arrays are validated against `MAX_ARRAY_SIZE` at law load time, preventing resource exhaustion

### Temporal model for open term resolution

Temporal filtering is critical for correctness when multiple versions of an implementing regulation are loaded (e.g., the 2024 and 2025 standaardpremie). The mechanism works at two levels:

**Same `$id`, multiple versions.** The resolver stores multiple versions of the same law (keyed by `$id`), sorted newest-first. `get_law_for_date` filters these by `valid_from <= calculation_date` and returns the most recent valid version. So for a 2025-01-15 calculation with both `regeling_standaardpremie` versions loaded, the 2025 version is selected. A 2026 version with `valid_from: 2026-01-01` would be excluded because `2026-01-01 <= 2025-01-15` is false.

**Different `$id`s implementing the same term.** If two separate law IDs both declare `implements` for the same open term, the `implements_index` contains entries for both. At resolution time, `find_implementations` calls `get_law_for_date` for each candidate independently — so a future-dated law is excluded before it ever reaches priority resolution.

**Invariant: `valid_from` must be present on implementing regulations.** The temporal filter uses `is_none_or`, meaning a regulation *without* `valid_from` passes the date check unconditionally — it matches every calculation date. This is by design for laws where the effective date is unknown, but for implementing regulations it undermines temporal correctness. Law authors must ensure that every regulation with an `implements` declaration has an explicit `valid_from` date. The schema does not enforce this (since `valid_from` is optional for other use cases), so this is a convention that must be maintained through review.

**Why not filter in the index?** The `implements_index` is built at load time and is date-independent — it records *all* implementing relationships across all loaded versions. Temporal filtering happens at query time in `find_implementations`, which is the correct place because the calculation date is only known at execution time.

### Same-law routing via `source.output`

When multiple articles in the same law need an open term value, only one article should declare the `open_terms` and serve as the single point of delegation. Other articles reference it via `source.output` (without `source.regulation`):

```yaml
# Article 2 gets standaardpremie from article 4 (same law)
input:
  - name: standaardpremie
    type: amount
    source:
      output: standaardpremie  # resolved from article 4
```

This ensures the flow is: **article 2 → article 4 → IoC → regeling**, rather than article 2 bypassing article 4 and reaching into the regeling directly.

### Default pattern

Open terms can have an optional `default` block containing actions. This makes the article executable standalone while allowing refinement by lower regulations. The implementing regulation replaces the default entirely and must handle all cases.

```yaml
open_terms:
  - id: redelijk_percentage
    type: number
    required: true
    default:
      actions:
        - output: redelijk_percentage
          value: 6
```

This pattern is more common at lower regulatory layers (a policy rule with a reasonable default that can be overridden by implementation policy) but the mechanism works on all layers.

Defaults also serve a legal correctness role. For example, Participatiewet article 8's open terms have `default` blocks with `verlaging_percentage: 0` and `duur_maanden: 0`. Legal basis: art. 18 lid 2 says "verlaagt ... overeenkomstig de verordening" — no verordening means no verlaging. A missing verordening now results in full bijstand (no reduction) rather than a `DelegationError`.

## Why

### Benefits

- **Matches legislative reality**: Lower regulations register themselves, just like in real law
- **Decoupled**: Adding a new implementing regulation doesn't require changes to the higher law
- **Discoverable**: The engine builds an index; implementations are found automatically
- **Traceable**: Each resolution produces trace output showing which implementations were found, which won, and why
- **One unified delegation model**: IoC replaces the old top-down `source.delegation` + `select_on` + `legal_basis_for` mechanism with a single, cleaner pattern

### Convergence: replacing `source.delegation`

The old delegation mechanism (`source.delegation` + `select_on`) forced the higher law to encode *how* to find its implementations:

```yaml
# Old pattern: higher law must specify selection logic
source:
  delegation:
    law_id: participatiewet
    article: '8'
    select_on:
      - name: gemeente_code
        value: $gemeente_code
  output: verlaging_percentage
```

This is backwards. The Participatiewet doesn't know which gemeenten have verordeningen — it just delegates. The gemeente verordening knows which wet it implements. IoC corrects this by letting the implementing regulation declare the relationship:

```yaml
# New pattern: lower regulation registers itself
implements:
  - law: participatiewet
    article: '8'
    open_term: verlaging_percentage
    gelet_op: Gelet op artikel 8 van de Participatiewet
```

The scoping question (which gemeente's verordening applies?) is an engine concern, not a law-encoding concern. The engine already knows the execution scope (e.g., `gemeente_code: GM0384`) from its parameters. The `find_implementations` method uses a `matches_scope` helper that checks all scope fields on the candidate law against execution parameters. Currently supports `gemeente_code`; designed for easy extension to `provincie_code` etc. This eliminates `select_on`, `legal_basis_for`, and `source.delegation` entirely — all delegation flows through `open_terms` + `implements`.

When resolving open terms, the engine does not forward all execution parameters to implementing articles. It uses `filter_parameters_for_article` to only pass parameters declared in the implementing article's `execution.parameters` section (principle of least privilege).

### Tradeoffs

- **Index maintenance**: The implements index must be kept in sync when laws are loaded/unloaded

### Alternatives Considered

**Alternative 1: Extend `enables` field**
- The `enables` field was added to the schema in v0.3.1 but never implemented in the engine
- It represents authority metadata (who is allowed to implement) rather than execution semantics
- Rejected: mixing authority and execution concerns; `open_terms` is a cleaner separation

**Alternative 2: `implements` as top-level metadata**
- Place `implements` at the law level, alongside `legal_basis`
- Rejected: one regulation can have multiple articles each implementing different open terms from different laws, so `implements` belongs at the article level

**Alternative 3: Default as separate construct**
- Have a separate `fallback` or `default_implementation` concept
- Rejected: simpler to put `default` directly on the open term, keeping the declaration and its fallback together

### Implementation Notes

- Schema version: v0.4.0 (minor bump due to conceptual shift)
- New Rust module: `packages/engine/src/priority.rs` for lex superior/lex posterior resolution
- `implements_index` in `RuleResolver` keyed by `(law_id, article, open_term_id)`
- Open term resolution runs in `evaluate_article_with_service()` before `pre_resolve_actions()`
- New trace types: `PathNodeType::OpenTermResolution`, `ResolveType::OpenTerm`

### Resolution patterns (target state)

| Pattern | Use when |
|---------|----------|
| **IoC** (`open_terms` + `implements`) | Any delegation: a higher law delegates a value to a lower regulation (with or without scope) |
| **Same-law reference** (`source.output`) | Internal: one article needs a value produced by another article in the same law |
| **External reference** (`source.regulation`) | Direct reference: one law needs a specific value from another law |

The old `source.delegation` + `select_on` + `legal_basis_for` pattern is superseded by IoC and will be phased out.

### Migration path

1. **This PR**: IoC for parameter-free delegation (zorgtoeslag → standaardpremie) ✅
2. **This PR**: scope filtering, parameter forwarding, delegation type validation, Participatiewet defaults ✅
3. **Follow-up**: migrate BW5 erfgrens from `source.delegation` to `open_terms`
4. **Follow-up**: remove `source.delegation`, `select_on`, and `legal_basis_for` from the schema

### History

This RFC replaces the original RFC-003 (Delegation Pattern), which described a top-down delegation model using `source.delegation` + `select_on` + `legal_basis_for`. The IoC model described here inverts that relationship.

## References

- Schema v0.4.0: `schema/v0.4.0/schema.json`
- Zorgtoeslag proof: `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml` and `corpus/regulation/nl/ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml`
- Gemeente implements: `corpus/regulation/nl/gemeentelijke_verordening/amsterdam/apv_erfgrens/2024-01-01.yaml` and `corpus/regulation/nl/gemeentelijke_verordening/diemen/afstemmingsverordening_participatiewet/2015-01-01.yaml`
