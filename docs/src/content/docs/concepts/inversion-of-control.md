# Inversion of Control

Dutch law has a hierarchy. Parliament passes a *wet* (formal law), which often delegates specifics to a minister or municipality. The Healthcare Allowance Act says the minister sets the standard premium. The Participation Act says municipalities set sanctions policy.

In RegelRecht, this delegation is modeled through two constructs: the higher law declares an **open term** (a value it needs but does not define), and the lower regulation declares that it **implements** that term.

## The legal pattern

A Dutch ministerial regulation typically opens with a preamble: *"Gelet op artikel 4 van de Wet op de zorgtoeslag"* ("In consideration of article 4 of the Healthcare Allowance Act"). This is the lower regulation registering itself as the authority that fills in a delegated value.

RegelRecht mirrors this exactly. The higher law does not need to know which lower regulation exists. The lower regulation registers itself.

## How it works

### The higher law declares an open term

```yaml
# Zorgtoeslagwet, article 4
machine_readable:
  open_terms:
    - id: standaardpremie
      type: amount
      required: true
      delegated_to: minister
      delegation_type: MINISTERIELE_REGELING
      legal_basis: artikel 4 Wet op de zorgtoeslag
      default:
        actions:
          - output: standaardpremie
            value: 211200
```

This says: "I need a value called `standaardpremie`. The minister should set it via a ministerial regulation. If nobody has set it, use 211200 (EUR 2,112.00)."

### The lower regulation implements it

```yaml
# Regeling standaardpremie, article 1
# regulatory_layer: MINISTERIELE_REGELING
# valid_from: 2025-01-01
machine_readable:
  implements:
    - law: zorgtoeslagwet
      article: '4'
      open_term: standaardpremie
      gelet_op: Gelet op artikel 4 van de Wet op de zorgtoeslag
  execution:
    output:
      - name: standaardpremie
        type: amount
        type_spec:
          unit: eurocent
    actions:
      - output: standaardpremie
        value: 211200
```

The `gelet_op` field matches the real legal preamble text. The `implements` block tells the engine: "I fill in the `standaardpremie` open term from Zorgtoeslagwet article 4."

### The engine connects them at load time

When the engine loads all law files, it builds an index of all `implements` declarations. When it encounters an `open_term` during execution, it looks up the index, finds the implementing regulation, and executes it to get the value.

## Municipal delegation

The Participation Act delegates sanctions policy to municipalities. Each municipality can set different reduction percentages. The engine uses `gemeente_code` in the execution parameters to select the right municipal ordinance.

### The national law declares open terms with defaults

```yaml
# Participatiewet, article 8
machine_readable:
  open_terms:
    - id: verlaging_percentage
      type: number
      required: true
      delegated_to: gemeenteraad
      delegation_type: GEMEENTELIJKE_VERORDENING
      legal_basis: artikel 8 lid 1 onderdeel a Participatiewet
      default:
        actions:
          - output: verlaging_percentage
            value: 0
```

The default of 0 follows the legal logic: article 18(2) says "reduces in accordance with the ordinance." No ordinance means no reduction.

### A municipality implements the open term

```yaml
# Afstemmingsverordening Participatiewet Diemen, article 9
# regulatory_layer: GEMEENTELIJKE_VERORDENING
# gemeente_code: GM0384
machine_readable:
  implements:
    - law: participatiewet
      article: '8'
      open_term: verlaging_percentage
      gelet_op: Gelet op artikel 8, eerste lid, onderdeel a van de Participatiewet
```

When executing for a person in Diemen (parameters include `gemeente_code: GM0384`), the engine uses Diemen's percentages. For a municipality without an ordinance, the Participation Act's default applies.

## Conflict resolution

When multiple regulations implement the same open term, the engine resolves conflicts using two rules from legal theory:

1. **Lex superior** (higher regulatory layer wins): a *wet* takes precedence over a *ministerieel regeling*
2. **Lex posterior** (newer wins): between two regulations at the same level, the one with the later `valid_from` date takes precedence

Temporal filtering ensures the right version applies: only regulations where `valid_from <= calculation_date` are considered.

## Comparison with cross-law references

Cross-law references and IoC both let laws use values from other laws, but they serve different purposes:

| | Cross-law reference | Inversion of Control |
|---|---|---|
| Who knows whom? | The referencing law names the target law | The higher law does not know which lower regulation exists |
| Direction | Top-down: "give me this value from that law" | Bottom-up: "I fill in this value for that law" |
| Use case | A law needs a specific value from a specific other law | A law delegates a value to whichever lower regulation fills it |
| YAML construct | `source: { regulation: ..., output: ... }` | `open_terms` + `implements` |

## Further reading

- [Cross-Law References](./cross-law-references) - the other pattern for inter-law values
- [Hooks and Reactive Execution](./hooks-and-reactive-execution) - yet another pattern: laws that fire automatically
- [RFC-003: Inversion of Control](/rfcs/rfc-003) - the full design specification
