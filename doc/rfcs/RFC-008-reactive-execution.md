# RFC-008: Reactive Execution

**Status:** Proposed
**Date:** 2026-03-16
**Authors:** Eelco Hotting

## Context

The engine currently supports **active execution**: someone requests a legal determination, the engine evaluates with specific parameters, and produces a result. This covers laws like the Zorgtoeslagwet, Participatiewet, and BW5.

But a significant class of law operates differently. The Algemene wet bestuursrecht (AWB) doesn't wait to be asked — it *reacts* to events. When any government body makes a besluit (decision), AWB activates: article 6:7 sets a bezwaartermijn of six weeks, article 7:1 establishes the right to bezwaar. When a bezwaarschrift is subsequently filed, article 7:10 sets a beslistermijn for the beslissing op bezwaar. The trigger is an event, not a citizen's request.

This is **reactive execution**: the law fires when a specific state transition occurs. It is edge-triggered (fires on the transition, not the resulting state).

### Execution modes

This is one of four identified execution modes:

1. **Active execution** — request-response (current engine, RFC-007 IoC)
2. **Reactive execution** — event-triggered (this RFC)
3. **Generative execution** — law that creates other law (git workflow, out of scope)
4. **Verificative execution** — continuous invariant checking (out of scope)

The specification (YAML) is the same across all modes. The modes differ in triggering mechanism and required infrastructure.

### Current state

The engine has no way to declare that a law reacts to events. Reactive behaviour exists in the PoC (`poc-machine-law`) via event sourcing with a `Case` aggregate and `ProcessApplication` that listens to domain events, but this is tightly coupled to infrastructure (aggregates, event types, update methods).

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

The `reacts_to` declaration is metadata — the engine does not subscribe to events or manage event routing. It declares the *kind* of event this article responds to, enabling the orchestration layer to set up the reactive wiring.

### Event types

An event type is a semantic label for a state transition. Initial types:

| Event type | Meaning | Example trigger |
|-----------|---------|----------------|
| `besluit` | An administrative decision is made | Zorgtoeslag application decided |
| `bezwaarschrift` | An objection is filed against a besluit | Citizen objects to zorgtoeslag decision |
| `aanvraag` | An application is submitted | Zorgtoeslag application submitted |

Event types are not an exhaustive taxonomy — they grow as new reactive laws are modelled.

### What the engine does

1. **At load time**: indexes all `reacts_to` declarations, keyed by `event_type`
2. **At execution time**: when an article produces an output whose type matches an `event_type`, the engine includes reactive metadata in the result — which laws and articles react to this event
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
          - name: verdagingstermijn_weken
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
          - output: verdagingstermijn_weken
            value: 6
```

## Why

### Benefits

- **Matches legal reality**: AWB reacts to besluiten — the schema now captures this
- **No coupling to infrastructure**: `reacts_to` declares the event type, not the event bus, aggregate, or update method
- **Same YAML, different runtime**: the law specification is identical whether executed actively or reactively — only the trigger mechanism differs
- **Discoverable**: the engine can answer "which articles react to a besluit?" without external configuration

### Tradeoffs

- **Orchestration layer required**: the engine declares reactive relationships but doesn't implement event routing — a separate orchestration layer must exist
- **Event type taxonomy**: needs to be defined and maintained, though it can grow incrementally

### Alternatives Considered

**Alternative 1: Infrastructure-coupled events (PoC approach)**
- `applies: { aggregate: "Case", events: [{ type: "Decided" }], update: [{ method: "determine_objection_status" }] }`
- Rejected: couples the law specification to a specific event sourcing implementation (aggregates, methods). The law should declare *what* it reacts to, not *how* the wiring works

**Alternative 2: No schema support — purely orchestration**
- The orchestration layer hardcodes which laws to trigger on which events
- Rejected: makes the reactive relationship invisible in the law YAML. The law text says the article reacts to besluiten — the machine readable version should too

## References

- RFC-007: Inversion of Control for Delegated Legislation
- RFC-009: Lex Specialis Overrides (companion RFC — `overrides` mechanism)
- AWB article 6:7: https://wetten.overheid.nl/BWBR0005537/2024-01-01#Artikel6:7
- PoC implementation: `poc-machine-law/laws/awb/bezwaar/JenV-2024-01-01.yaml`
