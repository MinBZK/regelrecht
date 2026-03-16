# RFC-008: Reactive Execution

**Status:** Proposed
**Date:** 2026-03-16
**Authors:** Eelco Hotting

## Context

The engine currently supports **active execution**: someone requests a legal determination, the engine evaluates with specific parameters, and produces a result. This covers laws like the Zorgtoeslagwet, Participatiewet, and BW5.

But a large class of law operates differently. The Algemene wet bestuursrecht (AWB) reacts to events. When any government body makes a besluit (decision), AWB activates: article 6:7 sets a bezwaartermijn of six weeks, article 7:1 establishes the right to bezwaar. When a bezwaarschrift is subsequently filed, article 7:10 sets a beslistermijn for the beslissing op bezwaar. The trigger is always an event.

This is **reactive execution**: the law fires when a state transition occurs. Edge-triggered, not level-triggered.

### Execution modes

This is one of four identified execution modes:

1. **Active execution**: request-response (current engine, RFC-007 IoC)
2. **Reactive execution**: event-triggered (this RFC)
3. **Generative execution**: law that creates other law (git workflow, out of scope)
4. **Verificative execution**: continuous invariant checking (out of scope)

The specification (YAML) is the same across all modes. The modes differ in triggering mechanism and required infrastructure.

### Current state

The engine has no way to declare that a law reacts to events. The PoC (`poc-machine-law`) has reactive behaviour via event sourcing with a `Case` aggregate and `ProcessApplication`, but this is tightly coupled to infrastructure (aggregates, event types, update methods).

## Decision

Introduce `reacts_to` on article-level `machine_readable`:

```yaml
# AWB artikel 6:7
- number: '6:7'
  text: |-
    De termijn voor het indienen van een bezwaar- of
    beroepschrift bedraagt zes weken.
  machine_readable:
    reacts_to:
      event_type: besluit
    execution:
      output:
        - name: bezwaartermijn_weken
          type: number
      actions:
        - output: bezwaartermijn_weken
          value: 6
```

The `reacts_to` declaration is metadata. The engine does not subscribe to events or manage event routing. It declares which event type this article responds to; the orchestration layer does the actual wiring.

### Event types

An event type is a semantic label for a state transition. Initial types:

| Event type | Meaning | Example trigger |
|-----------|---------|----------------|
| `besluit` | An administrative decision is made | Zorgtoeslag application decided |
| `bezwaarschrift` | An objection is filed against a besluit | Citizen objects to zorgtoeslag decision |
| `aanvraag` | An application is submitted | Zorgtoeslag application submitted |

Event types are not an exhaustive taxonomy. They grow as new reactive laws are modelled.

### Event production

A `reacts_to` declaration says which event an article listens to. The other side is production: which article produces an event. This is declared with `produces`:

```yaml
# Zorgtoeslag artikel 2 (simplified)
- number: '2'
  text: |-
    Aanspraak op een zorgtoeslag heeft degene...
  machine_readable:
    execution:
      output:
        - name: zorgtoeslag_besluit
          type: boolean
      produces:
        event_type: besluit
      actions:
        - output: zorgtoeslag_besluit
          value:
            operation: LESS_THAN_OR_EQUAL
            subject: $toetsingsinkomen
            value: $drempelinkomen
```

When an article with `produces` is executed, the engine marks the result as a `besluit` event. The orchestration layer then triggers all articles with `reacts_to: besluit`.

### What the engine does

1. **At load time**: indexes all `reacts_to` declarations (keyed by `event_type`) and all `produces` declarations
2. **At execution time**: when an article with `produces` completes, the engine annotates the result with reactive metadata (which laws and articles react to this event type)
3. **The engine does not route events**. The orchestration layer uses the metadata to trigger reactive evaluations

### What the orchestration layer does

1. Detects that a result contains event metadata (e.g., output type `besluit`)
2. Looks up which articles react to this event type
3. Triggers those articles with the event data as input
4. Combines results (substantive decision + reactive obligations like bezwaartermijn)

### Full YAML example

```yaml
---
$id: algemene_wet_bestuursrecht
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '1994-01-01'
articles:
  - number: '6:7'
    text: |-
      De termijn voor het indienen van een bezwaar- of
      beroepschrift bedraagt zes weken.
    machine_readable:
      reacts_to:
        event_type: besluit
      execution:
        output:
          - name: bezwaartermijn_weken
            type: number
        actions:
          - output: bezwaartermijn_weken
            value: 6

  - number: '7:10'
    text: |-
      1. Het bestuursorgaan beslist binnen zes weken of – indien
         een commissie als bedoeld in artikel 7:13 is ingesteld –
         binnen twaalf weken, gerekend vanaf de dag na die waarop
         de termijn voor het indienen van het bezwaarschrift is
         verstreken.
      3. Het bestuursorgaan kan de beslissing voor ten hoogste
         zes weken verdagen.
    machine_readable:
      reacts_to:
        event_type: bezwaarschrift
      execution:
        parameters:
          - name: heeft_bezwaarcommissie
            type: boolean
            required: false
            description: Is een commissie als bedoeld in artikel 7:13 ingesteld
        output:
          - name: beslistermijn_weken
            type: number
          - name: maximale_verdagingstermijn_weken
            type: number
        actions:
          - output: beslistermijn_weken
            value:
              operation: IF
              condition:
                operation: EQUALS
                subject: $heeft_bezwaarcommissie
                value: true
              then: 12
              else: 6
          - output: maximale_verdagingstermijn_weken
            value: 6
```

## Why

### Benefits

AWB reacts to besluiten. The schema should capture that. With `reacts_to`, it does.

Because `reacts_to` only declares the event type (not the event bus, aggregate, or update method), the law specification stays the same whether executed actively or reactively. Only the trigger mechanism differs.

The engine can also answer "which articles react to a besluit?" by querying its index, without external configuration.

### Tradeoffs

The engine declares reactive relationships but does not implement event routing. A separate orchestration layer must exist to do the actual wiring.

Event types (`besluit`, `bezwaarschrift`, `aanvraag`) need to be defined and maintained, though the taxonomy can grow incrementally.

### Alternatives Considered

**Alternative 1: Infrastructure-coupled events (PoC approach)**
- `applies: { aggregate: "Case", events: [{ type: "Decided" }], update: [{ method: "determine_objection_status" }] }`
- Rejected: couples the law specification to a specific event sourcing implementation (aggregates, methods).

**Alternative 2: No schema support, purely orchestration**
- The orchestration layer hardcodes which laws to trigger on which events.
- Rejected: makes the reactive relationship invisible in the law YAML. The law text says the article reacts to besluiten; the machine readable version should too.

## References

- RFC-007: Inversion of Control for Delegated Legislation (PR #246)
- RFC-009: Lex Specialis Overrides (companion RFC, `overrides` mechanism, this PR)
- AWB article 6:7: https://wetten.overheid.nl/BWBR0005537/2024-01-01#Artikel6:7
- PoC implementation: `poc-machine-law/laws/awb/bezwaar/JenV-2024-01-01.yaml`
