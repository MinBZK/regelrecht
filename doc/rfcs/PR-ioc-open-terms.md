# PR: Inversion of Control for Delegated Legislation

## Problem

Dutch legislation follows a bottom-up delegation pattern: a formal law (_wet_)
declares that certain values will be determined by a lower regulatory layer, and
a ministerial regulation (_ministeriële regeling_) opens with "Gelet op artikel
4 van de Wet op de zorgtoeslag" — it registers itself as filling that delegated
value.

The engine previously only supported **top-down** resolution. The higher law had
to explicitly search for matching lower regulations using `resolve` actions,
`legal_basis` indexes, and `select_on` criteria. This forced the higher law to
know _how_ to find its implementations, and adding a new implementing regulation
required modifying the higher law's YAML. That inverts the actual legal
relationship.

## Solution

This PR implements **Inversion of Control (IoC)** via two new schema constructs,
as described in [RFC-007](RFC-007-inversion-of-control.md):

### `open_terms` — declared by the higher law

An article can declare abstract values that must (or may) be filled by
implementing regulations:

```yaml
# Wet op de zorgtoeslag, artikel 4
machine_readable:
  open_terms:
    - id: standaardpremie
      type: amount
      required: true
      delegated_to: minister
      delegation_type: MINISTERIELE_REGELING
```

### `implements` — declared by the implementing regulation

A lower regulation registers itself as filling an open term:

```yaml
# Regeling standaardpremie, artikel 1
machine_readable:
  implements:
    - law: zorgtoeslagwet
      article: '4'
      open_term: standaardpremie
      gelet_op: "Gelet op artikel 4 van de Wet op de zorgtoeslag"
  actions:
    - output: standaardpremie
      value: 211200
```

### How resolution works

1. The engine indexes all `implements` declarations at law load time
2. When executing an article with `open_terms`, the engine looks up
   implementations via the index
3. Conflicts are resolved using **lex superior** (higher regulatory layer wins)
   then **lex posterior** (newer `valid_from` date wins)
4. The winning implementation's article is executed to obtain the value
5. Open terms support an optional `default` block for standalone execution
6. Missing required implementations produce a clear `DelegationError`

## Why this matters

- **Matches legislative reality**: lower regulations register themselves, just as
  in real Dutch law
- **Decoupled**: adding a new implementing regulation does not require changes to
  the higher law
- **Discoverable**: the engine builds an index automatically
- **Traceable**: each resolution produces trace output showing which
  implementations were found, which one won priority, and why
- **Backward compatible**: existing `resolve` actions and `source.delegation`
  continue to work; IoC is an additional resolution path

## What changed

| Area | Files | What |
|------|-------|------|
| **Schema** | `schema/v0.4.0/schema.json`, `schema/latest/schema.json` | New `open_terms` and `implements` definitions on `machine_readable` |
| **Data model** | `article.rs` | `OpenTerm`, `OpenTermDefault`, `ImplementsDeclaration` structs; new fields on `MachineReadable` |
| **Index** | `resolver.rs` | `implements_index` keyed by `(law_id, article, open_term_id)`, maintained on load/unload |
| **Priority** | `priority.rs` (new) | Lex superior / lex posterior candidate resolution |
| **Execution** | `service.rs` | `resolve_open_terms()` method, integrated into `evaluate_article_with_service()` |
| **Tracing** | `trace.rs`, `types.rs` | `PathNodeType::OpenTermResolution`, `ResolveType::OpenTerm` |
| **Validation** | `validate.rs` | v0.4.0 schema support |
| **Proof** | `regulation/` | Zorgtoeslag wet + Regeling standaardpremie updated with IoC pattern |

## Proof of concept

The zorgtoeslag / standaardpremie pair demonstrates the full cycle:

1. `wet_op_de_zorgtoeslag` article 4 declares `open_terms: [standaardpremie]`
2. `regeling_standaardpremie` article 1 declares `implements: [{law: zorgtoeslagwet, article: '4', open_term: standaardpremie}]`
3. When the engine executes article 4 of the zorgtoeslag wet, it finds the
   regeling via the implements index, executes its article 1, and uses the
   result (€ 2.112,00) as the standaardpremie value

## When to use which delegation pattern

| Pattern | Use when |
|---------|----------|
| **IoC** (`open_terms` + `implements`) | Simple delegation: higher law delegates a value to a lower regulation |
| **Delegation** (`source.delegation` + `select_on`) | Selection-based: pick a regulation based on runtime criteria (e.g., gemeente) |
| **External reference** (`source.regulation`) | Direct reference: one law needs a specific value from another |
