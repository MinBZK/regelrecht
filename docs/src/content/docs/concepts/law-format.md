# Law Format

Laws in RegelRecht are stored as YAML files conforming to the [law schema](/reference/schema). Each file represents one law at a specific point in time.

## File Organization

```
corpus/regulation/nl/
├── wet/                              # Formal laws (wetten)
│   ├── wet_op_de_zorgtoeslag/
│   │   └── 2025-01-01.yaml
│   ├── participatiewet/
│   │   └── 2022-03-15.yaml
│   └── burgerlijk_wetboek_boek_5/
│       └── 2024-01-01.yaml
├── ministeriele_regeling/            # Ministerial regulations
│   └── regeling_standaardpremie/
│       ├── 2024-01-01.yaml
│       └── 2025-01-01.yaml
└── gemeentelijke_verordening/        # Municipal ordinances
    ├── amsterdam/
    │   └── apv_erfgrens/
    │       └── 2024-01-01.yaml
    └── diemen/
        └── afstemmingsverordening_participatiewet/
            └── 2015-01-01.yaml
```

## Structure of a Law File

### Header Metadata

```yaml
$schema: https://raw.githubusercontent.com/.../refs/tags/schema-v0.5.1/schema/v0.5.1/schema.json
$id: zorgtoeslagwet
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
bwb_id: BWBR0018451
url: https://wetten.overheid.nl/BWBR0018451/2025-01-01
name: Wet op de zorgtoeslag
```

| Field | Required | Description |
|-------|----------|-------------|
| `$id` | Yes | Machine identifier (snake_case) |
| `regulatory_layer` | Yes | WET, AMVB, MINISTERIELE_REGELING, GEMEENTELIJKE_VERORDENING, etc. |
| `publication_date` | Yes | Official publication date (ISO 8601) |
| `valid_from` | No | Effective date |
| `bwb_id` | Conditional | Required for national laws (format: `BWBR` + 7 digits) |
| `gemeente_code` | Conditional | Required for municipal ordinances (format: `GM` + 4 digits) |

### Articles

Each article mirrors a real article in Dutch law:

```yaml
articles:
  - number: '2'
    text: |
      1. De verzekerde heeft aanspraak op een zorgtoeslag...
      2. De hoogte van de zorgtoeslag is het verschil...
    url: https://wetten.overheid.nl/.../artikel/2
    machine_readable:
      definitions:
        DREMPELINKOMEN:
          value: 2500000   # €25,000 in eurocent
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: toetsingsinkomen
            type: amount
            source:
              regulation: algemene_wet_inkomensafhankelijke_regelingen
              output: toetsingsinkomen
              parameters:
                bsn: $bsn
        output:
          - name: hoogte_zorgtoeslag
            type: amount
            type_spec:
              unit: eurocent
        actions:
          - output: hoogte_zorgtoeslag
            value:
              operation: MAX
              values:
                - 0
                - operation: SUBTRACT
                  values:
                    - $standaardpremie
                    - $normpremie
```

## Key Concepts

### Definitions

Constants used in calculations:

```yaml
definitions:
  norm_alleenstaande:
    value: 109171        # €1,091.71 in eurocent
  PERCENTAGE_LID_5:
    value: 100
```

### Operations

Operations are the building blocks of law logic:

| Category | Operations | Syntax |
|----------|-----------|--------|
| **Arithmetic** | `ADD`, `SUBTRACT`, `MULTIPLY`, `DIVIDE` | `values: [...]` |
| **Aggregate** | `MIN`, `MAX` | `values: [...]` |
| **Comparison** | `EQUALS`, `NOT_EQUALS`, `GREATER_THAN`, `LESS_THAN`, `GREATER_THAN_OR_EQUAL`, `LESS_THAN_OR_EQUAL` | `subject:`, `value:` |
| **Logical** | `AND`, `OR` | `conditions: [...]` |
| **Membership** | `IN`, `NOT_IN` | `subject:`, `values: [...]` |
| **Conditional** | `IF` | `when:`, `then:`, `else:` |
| **Multi-branch** | `SWITCH` | `cases: [{when:, then:}]`, `default:` |
| **Date** | `SUBTRACT_DATE` | `subject:`, `value:`, `unit:` |
| **Null** | `IS_NULL`, `NOT_NULL` | `subject:` |

See [RFC-004: Uniform Operation Syntax](/rfcs/rfc-004) for the full specification.

### Variable References

- `$variableName` - reference inputs, outputs, definitions, or parameters
- `$referencedate.year` - dot notation for property access
- `#output_name` - internal reference (same law)

### Cross-Law References

Laws reference outputs from other laws via `source`:

```yaml
input:
  - name: toetsingsinkomen
    type: amount
    source:
      regulation: algemene_wet_inkomensafhankelijke_regelingen
      output: toetsingsinkomen
      parameters:
        bsn: $bsn
```

See [RFC-007: Cross-Law Execution](/rfcs/rfc-007) for details.

### Open Terms and Delegation (IoC)

Higher laws declare `open_terms` that lower regulations implement:

```yaml
# In the wet (higher law)
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

# In the ministerial regulation (lower law)
implements:
  - law: zorgtoeslagwet
    article: '4'
    open_term: standaardpremie
    gelet_op: Gelet op artikel 4 van de Wet op de zorgtoeslag
```

The engine resolves implementations at runtime using lex superior / lex posterior rules.

See [RFC-003: Inversion of Control](/rfcs/rfc-003) for the full pattern.

### Legal Character

Articles can declare what kind of legal product they produce:

```yaml
execution:
  produces:
    legal_character: BESCHIKKING   # Administrative decision
    decision_type: TOEKENNING      # Grant
```

This enables the AWB procedure hooks ([RFC-008](/rfcs/rfc-008)).

### Type Specifications

Fields can have detailed type information:

```yaml
output:
  - name: hoogte_zorgtoeslag
    type: amount
    type_spec:
      unit: eurocent
      precision: 0
```

## Corpus Contents

The current corpus contains laws across three regulatory layers:

| Layer | Count | Examples |
|-------|-------|---------|
| WET | 11 | Participatiewet, Zorgtoeslag, Zorgverzekeringswet, BW Boek 5 |
| MINISTERIELE_REGELING | 1 | Regeling standaardpremie (2 versions) |
| GEMEENTELIJKE_VERORDENING | 2 | Amsterdam APV erfgrens, Diemen afstemmingsverordening |

## Next Steps

- [Testing](/guide/testing) - writing BDD scenarios for laws
- [Schema Reference](/reference/schema) - full schema specification
- [Engine](/components/engine) - how the engine executes laws
