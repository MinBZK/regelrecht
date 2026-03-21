# RFC-012: Lifecycle Stages for Administrative Decisions

**Status:** Proposed
**Date:** 2026-03-21
**Authors:** Eelco Hotting
**Depends on:** RFC-008 (Hooks), RFC-009 (Overrides)

## Context

RFC-008 introduced hooks: articles that fire when another article's `produces` annotation matches. This enables reactive execution — AWB 3:46 (motiveringsplicht) fires on any BESCHIKKING, AWB 6:7 (bezwaartermijn) fires on any BESCHIKKING, AWB 6:8 (start/einddatum) fires on any BESCHIKKING.

But a beschikking is not an instant computation. It is an administrative process that progresses through stages over time:

1. The belanghebbende submits an **aanvraag** (application, AWB 4:1).
2. The bestuursorgaan investigates during the **behandeling** (processing) phase, possibly requesting additional information (AWB 4:5), possibly extending the beslistermijn (AWB 4:14). This can take weeks, months, or in complex cases years.
3. The bestuursorgaan makes a **besluit** (decision, AWB 1:3).
4. The besluit is communicated to the belanghebbende: **bekendmaking** (notification, AWB 3:41).
5. The **bezwaartermijn** starts the day after bekendmaking (AWB 6:8 lid 1) and runs for six weeks (AWB 6:7).
6. The belanghebbende may file a **bezwaar** (objection, AWB 6:4).

If hooks fire without awareness of lifecycle stages, all AWB obligations trigger at once — including AWB 6:8 (bezwaartermijn date calculation), which needs `bekendmaking_datum`. But bekendmaking hasn't happened yet at decision time. Treating a missing `bekendmaking_datum` as "skip this hook gracefully" is a workaround, not a design.

The fundamental problem: **the decision and the bekendmaking are different moments in time, with different inputs, producing different outputs, governed by different articles of law.** They must not fire in the same execution step.

### Who defines the lifecycle?

The lifecycle of a beschikking is not defined by the Vreemdelingenwet or the Zorgtoeslagwet. It is defined by the **Algemene wet bestuursrecht (AWB)**. That is the purpose of the AWB: to define the general administrative procedure that all bestuursorganen follow, regardless of which specific law they are executing.

The AWB defines stages. Specific laws fill in the content at each stage. Other laws (Termijnenwet, KB gelijkgestelde dagen) hook into specific stages. This relationship already exists in law — it needs to be expressed in the schema.

### What goes wrong without lifecycle stages

1. **Semantic confusion**: AWB 6:8 hooks on BESCHIKKING but should only fire when bekendmaking has occurred. Without stages, parameter absence becomes implicit control flow.
2. **Incorrect composition**: hooks that belong to different stages fire in the same execution, mixing outputs from different moments in time.
3. **No way to model waiting**: an engine without stages cannot express "the besluit is taken, now waiting for bekendmaking."
4. **Manual actions invisible**: a bestuursorgaan's investigation, decision-making, and notification are real-world actions that the law regulates (beslistermijn per AWB 4:13) but cannot be represented without stages.

## Decision

### The lifecycle is law

The AWB defines a **lifecycle** for administrative processes. This lifecycle is expressed in the YAML specification as a first-class construct. Laws do not define their own lifecycles — they declare which AWB-defined lifecycle they participate in through `produces`.

A lifecycle is a sequence of **stages**. Each stage:
- Has a **name** (e.g., `AANVRAAG`, `BESLUIT`, `BEKENDMAKING`)
- May produce **outputs** (computed automatically by the engine)
- May require **inputs** that come from external events (human decisions, real-world actions)
- May have **hooks** from other laws that fire when the stage is reached

### Lifecycle definition in AWB

The AWB defines the lifecycle for BESCHIKKING as a machine-readable construct:

```yaml
# algemene_wet_bestuursrecht.yaml
$id: algemene_wet_bestuursrecht

lifecycle:
  - id: beschikking
    applies_to:
      legal_character: BESCHIKKING
    stages:
      - name: AANVRAAG
        description: Belanghebbende dient aanvraag in (AWB 4:1)
        requires:
          - name: aanvraag_datum
            type: date

      - name: BEHANDELING
        description: Bestuursorgaan onderzoekt de aanvraag (AWB 3:2)
        requires:
          - name: beslistermijn_start
            type: date
        # AWB 4:13: beslistermijn is "redelijke termijn", typically 8 weeks
        # AWB 4:14: extension possible with notification

      - name: BESLUIT
        description: Bestuursorgaan neemt besluit (AWB 1:3)
        requires:
          - name: besluit_datum
            type: date

      - name: BEKENDMAKING
        description: Besluit wordt bekendgemaakt (AWB 3:41)
        requires:
          - name: bekendmaking_datum
            type: date

      - name: BEZWAAR
        description: Bezwaarperiode (AWB 6:4 e.v.)
        # This stage is entered automatically after BEKENDMAKING
        # and runs for the duration of the bezwaartermijn
```

### Hooks bind to stages, not just legal_character

The `applies_to` in hooks (RFC-008) gains a `stage` field:

```yaml
# AWB 3:46 — motiveringsplicht
# Must be satisfied AT decision time
- number: '3:46'
  machine_readable:
    hooks:
      - hook_point: pre_actions
        applies_to:
          legal_character: BESCHIKKING
          stage: BESLUIT

# AWB 6:7 — bezwaartermijn
# Property of the decision, determined at BESLUIT
- number: '6:7'
  machine_readable:
    hooks:
      - hook_point: post_actions
        applies_to:
          legal_character: BESCHIKKING
          stage: BESLUIT

# AWB 6:8 — start/einddatum berekening
# Only meaningful AFTER bekendmaking
- number: '6:8'
  machine_readable:
    hooks:
      - hook_point: post_actions
        applies_to:
          legal_character: BESCHIKKING
          stage: BEKENDMAKING
```

This is the key insight: **hooks apply to the AWB's lifecycle, not to the specific law's decision.** The Vreemdelingenwet produces a BESCHIKKING and thereby enters the AWB lifecycle. The Vreemdelingenwet does not know about AWB 6:8. AWB 6:8 does not know about the Vreemdelingenwet. They are connected through the lifecycle defined by the AWB.

### The besluit as state container

A **besluit** progresses through the AWB lifecycle and accumulates outputs at each stage. The besluit itself is the state container — there is no separate "case" or "zaak" abstraction. This follows the AWB, which defines everything in terms of the besluit.

```
Besluit {
    lifecycle: "beschikking"          -- which AWB lifecycle
    contextual_law: "vreemdelingenwet_2000"  -- lex specialis context
    current_stage: BEKENDMAKING       -- where we are
    outputs: {                        -- accumulated from all stages
        // from BESLUIT stage:
        verblijfsvergunning_verleend: true,
        motivering_vereist: true,
        bezwaartermijn_weken: 4,      // overridden by Vw art 69
        // from BEKENDMAKING stage:
        bekendmaking_datum: "2026-03-23",
        bezwaartermijn_startdatum: "2026-03-24",
        bezwaartermijn_einddatum: "2026-04-20",
    }
    pending: {                        -- what's needed to advance
        // nothing — all stages completed
    }
}
```

### Execution becomes multi-step

When the engine executes a law that produces a BESCHIKKING:

**Step 1: BESLUIT stage**
```
Input:  { heeft_geldige_mvv: true, heeft_geldig_document: true }
Engine: executes Vw art 14, fires BESLUIT-stage hooks (AWB 3:46, 6:7)
Output: { verblijfsvergunning_verleend: true, motivering_vereist: true,
          bezwaartermijn_weken: 4 }
Yields: "Waiting for BEKENDMAKING — need: bekendmaking_datum"
```

**Step 2: BEKENDMAKING stage** (days/weeks later)
```
Input:  { bekendmaking_datum: "2026-03-23", jaar: 2026, pasen_datum: "2026-04-05" }
Engine: fires BEKENDMAKING-stage hooks (AWB 6:8 → Termijnenwet art 1)
Output: { bezwaartermijn_startdatum: "2026-03-24",
          bezwaartermijn_einddatum: "2026-04-20" }
Yields: "BEZWAAR stage — bezwaartermijn running until 2026-04-20"
```

The engine **yields** between stages, returning:
- What it computed so far (accumulated outputs)
- What stage it's at
- What inputs are needed to advance to the next stage

The orchestration layer persists the besluit state and feeds new inputs when they become available.

### What is state, precisely?

The besluit state consists of:

| Component | What it is | Where it lives |
|-----------|-----------|---------------|
| **Accumulated outputs** | All outputs from completed stages | Besluit record |
| **Current stage** | Which lifecycle stage the besluit is at | Besluit record |
| **Pending inputs** | What external data is needed to advance | Derived from lifecycle definition |
| **Contextual law** | The lex specialis context for overrides | Set at creation, immutable |
| **Parameters** | Original parameters from the initial execution | Besluit record |

The engine itself remains **stateless** in the sense that it does not maintain besluit state internally. The besluit state is an external record (database row, event store, file) managed by the orchestration layer. The engine receives the besluit state as input and returns the updated state as output.

This is important: the engine is still a pure function per stage. But the **composition** of stages into a besluit lifecycle is now explicit, governed by the AWB lifecycle definition, and persisted externally.

### Automatic vs. manual stage transitions

Some stage transitions are automatic (engine computes the next stage's outputs immediately). Others require external events:

| Transition | Type | Trigger |
|-----------|------|---------|
| AANVRAAG → BEHANDELING | Automatic | Application received |
| BEHANDELING → BESLUIT | Manual | Bestuursorgaan decides |
| BESLUIT → BEKENDMAKING | Manual | Notification sent |
| BEKENDMAKING → BEZWAAR | Automatic | Bezwaartermijn starts |

The lifecycle definition distinguishes these: stages with `requires` fields that are not computable from previous outputs need manual input. The engine signals this by yielding with a description of what's needed.

## Why

### Benefits

**Conceptual correctness.** The model matches the legal reality: a besluit is a process with stages, not an instant computation. The AWB defines the process, specific laws fill in the content.

**Separation of concerns.** Decision logic (Vreemdelingenwet) is separate from procedural logic (AWB lifecycle). Each law does what it's supposed to do. The lifecycle connects them.

**Temporal accuracy.** Outputs are computed at the right moment. The bezwaartermijn einddatum is calculated when the bekendmaking happens, not when the decision is made. The feestdagenkalender uses the correct year for the bekendmaking date, not the decision date.

**Auditability.** The besluit record shows exactly what happened at each stage, when, and with what inputs. This supports the motiveringsplicht (AWB 3:46) and provides a complete administrative trail.

**Extensibility.** New lifecycle stages can be added by the AWB without changing specific laws. New hooks can bind to any stage. The lifecycle is data (YAML), not code.

**Real-world fidelity.** The model naturally handles long-running processes (asylum decisions that take months), manual steps (bestuursorgaan investigation), and asynchronous events (bekendmaking by post).

### Tradeoffs

**Complexity.** The engine moves from "pure function" to "state machine executor." The orchestration layer must now manage besluit state persistence. This is significant implementation effort.

**Backwards compatibility.** Existing laws that produce BESCHIKKING without a lifecycle still work — they complete in a single stage. But new laws should use the lifecycle model. RFC-008 hooks without `stage` default to BESLUIT for backward compatibility.

**State management.** Besluit state must be persisted somewhere. The engine doesn't dictate where (database, event store, file system), but the orchestration layer must handle it.

### Alternatives Considered

**Alternative 1: Implicit stages via parameter presence**
- AWB 6:8 hooks on BESCHIKKING and skips when `bekendmaking_datum` is absent.
- Rejected: uses parameter absence as control flow. Semantically wrong — the hook doesn't "fail to fire," it fires at the wrong time. Silently skipping hooks hides the lifecycle.

**Alternative 2: Multiple explicit executions (no lifecycle)**
- Caller invokes the decision law first, then separately invokes AWB 6:8 with the results plus `bekendmaking_datum`.
- Rejected: makes the lifecycle invisible. The caller must know which AWB articles to invoke and in what order. The machine-readable specification should capture this.

**Alternative 3: Event sourcing / CQRS**
- The original poc-machine-law approach: a `Case` aggregate with event sourcing.
- Rejected: couples the law specification to infrastructure (aggregates, event types, projections). The lifecycle should be expressed in law YAML, not in infrastructure code. However, an event-sourced persistence layer is a valid *implementation* of besluit state management.

### Implementation Notes

The lifecycle is a new top-level construct in the schema, defined at the law level (not article level). It references stages, and hooks reference stages.

The engine needs:
- **Lifecycle index**: maps `(legal_character) → lifecycle_definition`, loaded from AWB YAML.
- **Stage-aware hook resolution**: `find_hooks` gains a `stage` parameter. Hooks without `stage` default to BESLUIT.
- **Besluit state**: a struct carrying accumulated outputs, current stage, and context. Passed in and returned by the engine.
- **Yield mechanism**: the engine returns either a completed result or a "waiting for input" signal with the next stage's requirements.

The besluit state is *not* stored in the engine. It is passed in by the caller and returned with updates. The engine remains a library, not a service.

## Open Questions

1. ~~**Do all besluiten share the same lifecycle?**~~ **Resolved:** No. Each legal_character has its own lifecycle, defined by the relevant AWB chapters. A BESCHIKKING has aanvraag → behandeling → besluit → bekendmaking → bezwaar. A BESLUIT_VAN_ALGEMENE_STREKKING has a different procedure (AWB afdeling 3.4, Staatscourant publication, no bezwaar, direct beroep). The AWB defines these different procedures — the lifecycle definition in YAML follows the AWB structure per type.

2. ~~**Nested lifecycles.**~~ **Resolved:** A bezwaar is itself a besluit (AWB 7:12), which starts its own lifecycle (with its own bekendmaking, and possibility of beroep at the rechter). The engine applies the same lifecycle pattern recursively — a besluit op bezwaar enters the AWB lifecycle just like the original beschikking. If a law inadvertently creates infinite recursion, that is a defect in the law, not in the engine. The engine's existing cycle detection (RFC-008) will catch and report it.

3. **Parallel stages.** Some processes have parallel tracks (e.g., horen per AWB 7:2 while investigating, voorlopige voorziening parallel to bezwaar). The lifecycle is not strictly sequential — it is a state machine with concurrent states. *Under investigation — see RFC-012-research-parallel-stages.md.*

4. ~~**Beslistermijn enforcement.**~~ **Resolved:** The beslistermijn is calculated by a hook at the AANVRAAG stage — AWB 4:13 provides the default ("redelijke termijn"), specific laws override via lex specialis (same pattern as bezwaartermijn_weken). The engine does **not** enforce the deadline: if besluit_datum exceeds the beslistermijn, the engine continues normally but annotates the besluit with a warning. Exceeding the beslistermijn does not invalidate the besluit — it **expands the lifecycle** with new available paths for the belanghebbende: ingebrekestelling (AWB 4:17), dwangsom (AWB 4:18), and beroep tegen niet tijdig beslissen (AWB 6:2 lid 1 sub b). These are modeled as conditional branches in the lifecycle state machine.

5. ~~**Intrekking and herroeping.**~~ **Resolved:** Intrekking (AWB 10:4-10:5) is a state transition in the original besluit's lifecycle, not a separate lifecycle. A beschikking continues to exist after bekendmaking — it can be onherroepelijk, ingetrokken, gewijzigd, or verlopen. The intrekking itself is a nested besluit (same pattern as question 2): it requires motivering, bekendmaking, and can be challenged via bezwaar. The original beschikking's state changes as a consequence of the intrekkingsbesluit completing its own lifecycle.

## References

- RFC-008: Execution Lifecycle Hooks (hooks mechanism)
- RFC-009: Lex Specialis Overrides (contextual law overrides)
- AWB Hoofdstuk 3: Algemene bepalingen over besluiten (bekendmaking, motivering)
- AWB Hoofdstuk 4: Bijzondere bepalingen over besluiten (beslistermijn, aanvraag)
- AWB Hoofdstuk 6: Algemene bepalingen over bezwaar en beroep (termijnen)
- AWB Hoofdstuk 7: Bijzondere bepalingen over bezwaar en administratief beroep
