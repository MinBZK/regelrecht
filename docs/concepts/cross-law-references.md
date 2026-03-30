# Cross-Law References

Dutch laws reference each other constantly. The Healthcare Allowance Act (*Zorgtoeslagwet*) needs your income, which is defined by the AWIR. It needs your insurance status, which comes from the Zorgverzekeringswet. It needs your age, which comes from the BRP.

Rather than duplicating these definitions, each law declares what it needs from other laws. The engine follows these references automatically.

## How it works

An article declares its inputs. When an input has a `source` block pointing to another law, the engine loads that law, executes it with the specified parameters, and feeds the result back.

```yaml
# Zorgtoeslagwet, article 2 - needs income from the AWIR
input:
  - name: toetsingsinkomen
    type: amount
    source:
      regulation: algemene_wet_inkomensafhankelijke_regelingen
      output: toetsingsinkomen
      parameters:
        bsn: $bsn
```

The engine loads `algemene_wet_inkomensafhankelijke_regelingen`, executes it for the given BSN, gets the `toetsingsinkomen` output, and uses that value in the healthcare allowance calculation.

## Chains of references

References can chain. The Zorgtoeslagwet references the AWIR, which might reference the Wet inkomstenbelasting, which references the BRP. The engine resolves the full chain, loading and executing each law as needed.

```mermaid
flowchart LR
    ZT[Zorgtoeslagwet] -->|toetsingsinkomen| AWIR
    ZT -->|is_verzekerd| ZVW[Zorgverzekeringswet]
    ZT -->|leeftijd| BRP[BRP]
    ZT -->|standaardpremie| RSP[Regeling standaardpremie]
    AWIR -->|inkomen| WIB[Wet inkomstenbelasting]
```

Results are cached: if two laws both need the same value from the BRP, it is computed once.

## A real example

The Zorgtoeslagwet article 2 declares these cross-law inputs:

```yaml
input:
  - name: leeftijd
    type: number
    source:
      regulation: wet_basisregistratie_personen
      output: leeftijd
      parameters:
        bsn: $bsn
        peildatum: $referencedate

  - name: is_verzekerde
    type: boolean
    source:
      regulation: zorgverzekeringswet
      output: is_verzekerd
      parameters:
        bsn: $bsn

  - name: toetsingsinkomen
    type: amount
    source:
      regulation: algemene_wet_inkomensafhankelijke_regelingen
      output: toetsingsinkomen
      parameters:
        bsn: $bsn
```

Each `source` block says: load this other law, pass it these parameters, and give me the named output.

## Same-law references

Articles within the same law can also reference each other. When `source` has an `output` but no `regulation`, the engine looks within the current law:

```yaml
# Zorgtoeslagwet, article 2 referencing article 4 (same law)
input:
  - name: standaardpremie
    type: amount
    source:
      output: standaardpremie
```

## Circular reference detection

The engine detects circular references (law A needs law B which needs law A) and raises an error. A `MAX_CROSS_LAW_DEPTH` limit of 20 prevents runaway chains.

## Further reading

- [Law Format](./law-format) - full structure of a law YAML file
- [Inversion of Control](./inversion-of-control) - a different pattern for cross-law values: delegation
- [RFC-007: Cross-Law Execution](/rfcs/rfc-007) - the full design specification
