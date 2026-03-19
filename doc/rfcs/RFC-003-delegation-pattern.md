# RFC-003: Delegation Pattern for Multi-Level Regulations

**Status:** Superseded by [RFC-007](RFC-007-inversion-of-control.md)
**Date:** 2025-12-09
**Authors:** Regelrecht Team

## Context

In gelaagde rechtssystemen bepaalt een hoger niveau vaak het "wat", maar delegeert het "hoe" naar een lager niveau. Voorbeelden:

- **EU → Nationaal**: EU-richtlijnen die lidstaten ruimte geven voor nationale invulling
- **Nationaal → Gemeente**: Participatiewet art. 8 delegeert verlagingspercentages aan gemeenten
- **Wet → Ministeriële regeling**: Wetten die details delegeren aan ministeriële regelingen

De vraag is: hoe vindt de engine automatisch de juiste lagere regelgeving?

## Decision

### YAML-structuur

**Delegerende wet** (hoger niveau):
```yaml
machine_readable:
  legal_basis_for:
    - regulatory_layer: GEMEENTELIJKE_VERORDENING  # of andere laag
      contract:
        parameters:
          - name: input_param
            type: number
        output:
          - name: result_value
            type: number
      defaults:  # OPTIONEEL - alleen bij optionele delegatie
        actions:
          - output: result_value
            value: 100
```

**Lagere regelgeving**:
```yaml
regulatory_layer: GEMEENTELIJKE_VERORDENING
gemeente_code: GM0384  # identificatie van jurisdictie

legal_basis:
  - law_id: delegerende_wet
    article: '8'

articles:
  - machine_readable:
      execution:
        output:
          - name: result_value  # matcht interface
```

**Aanroep via delegation source** (met `select_on`):
```yaml
input:
  - name: data
    source:
      delegation:
        law_id: delegerende_wet
        article: '8'
        # Generic select_on mechanism - niet gebonden aan specifieke property
        select_on:
          - name: gemeente_code
            value: $gemeente_code
      output: result_value
      parameters:
        input_param: $value
```

### select_on Mechanisme

Het `select_on` mechanisme biedt een generieke manier om gedelegeerde regelgeving te
vinden op basis van arbitraire criteria:

```yaml
delegation:
  law_id: delegerende_wet
  article: '8'
  select_on:
    - name: gemeente_code
      value: $gemeente_code
    - name: jaar            # Optioneel: meerdere criteria
      value: 2024
```

**Voordelen:**
1. **Generiek** - Niet gebonden aan specifieke property naam
2. **Uitbreidbaar** - Meerdere selectiecriteria mogelijk
3. **Toekomstbestendig** - Klaar voor andere jurisdicties (provincies, waterschappen)

### Twee patronen

| Patroon | `defaults` aanwezig? | Zonder lagere regelgeving |
|---------|---------------------|---------------------------|
| **Verplicht** | Nee | `ValueError` (no legal basis) |
| **Optioneel** | Ja | Defaults uit hogere wet |

## Why

### Benefits

- Automatische lookup van lagere regelgeving op basis van jurisdictie
- Expliciete foutmelding bij ontbrekende verplichte regelgeving
- Fallback naar defaults bij optionele delegatie
- Juridische traceerbaarheid via `legal_basis` ↔ `legal_basis_for`

### Tradeoffs

- `legal_basis` moet exact matchen met `legal_basis_for`
- Lagere regelgeving moet juiste `regulatory_layer` en identificatie hebben

## References

- `engine/context.py`: `_resolve_from_delegation()` *(original Python prototype, removed)*
- `engine/rule_resolver.py`: `find_delegated_regulation()` *(original Python prototype, removed)*
- `packages/engine/src/engine.rs`: `resolve_input_sources()`, `evaluate_select_on_criteria()` *(Rust implementation)*
- `packages/engine/src/resolver.rs`: `find_delegated_regulation()` *(Rust implementation)*
