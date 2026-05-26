# Integration: Mijn Betaaloverzicht (FCID)

This page specifies how a RegelRecht engine integrates with [Mijn Betaaloverzicht (MBO)](https://www.eenoverheidsincasso.nl/onze-dienstverlening/vorderingenoverzicht-rijk) using the [Financial Claims Information Document (FCID)](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/) standard. It uses the chronolexogram types and integration hooks from [RFC-022](/rfcs/rfc-022), the federation model from [RFC-009](/rfcs/rfc-009), and the trust mechanics from RFC-009 §5.

The specification targets FCID v4.x (v4.2.0 as of mei 2026) and tracks upstream as the standard evolves. This document is intended to be updated when FCID minor versions are published, without requiring a new RFC.

For background on the Dutch claim-collection landscape this integration sits in, see [CJIB-uitvoeringslandschap](/concepts/cjib-uitvoeringslandschap).

## What this integration does

A RegelRecht engine that opts into the MBO/FCID integration emits two streams of events to MBO endpoints:

- **Decretogram-derived FCID events**, when a regulation produces a `BESCHIKKING` with a financial-enforcement `decision_type` (see RFC-022 §2) and the producing rule sets `outbound_emit: true`.
- **Executogram-derived FCID events**, when an entry in `corpus/executogram/` fires (typically on an external trigger such as a payment received in the surrounding incasso system).

In the consumer direction, a RegelRecht regulation can query MBO for a citizen's openstaande vorderingen through a wrapper regulation that declares CJIB as `competent_authority`. The query path is the standard [RFC-009](/rfcs/rfc-009) ACCEPT path.

## FCID event types

FCID defines four event types. Each maps to one chronolexogram type as defined in RFC-022.

| FCID `event_type` | Chronolexogram type | RegelRecht source |
|---|---|---|
| `FinancieleVerplichtingOpgelegd` | decretogram | engine output, `decision_type: STRAFBESCHIKKING` (totaalbedrag) |
| `BetalingsverplichtingOpgelegd` | decretogram | engine output, `decision_type: BETALINGSVERPLICHTING` / `BESTUURLIJKE_BOETE` / `INCASSO_BESCHIKKING` |
| `BetalingsverplichtingIngetrokken` | decretogram (intrekking) | engine output, `decision_type: INTREKKING_BESCHIKKING`, often via [RFC-007](/rfcs/rfc-007) override |
| `BetalingVerwerkt` | executogram | executogram-stream entry triggered by external incasso system |

## Producer side: emitting decretogram-derived FCID

A regulation rule that should emit FCID on producing a decretogram sets two flags on its `produces` block:

```yaml
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: BETALINGSVERPLICHTING
    outbound_emit: true
    outbound_category: ALGEMEEN
```

`outbound_category` for FCID is one of: `ALGEMEEN`, `ADMINISTRATIEKOSTEN`, `VERHOGING`, `RENTE`. These are the four FCID categories. Each FCID line carries exactly one category. A regulation that produces multiple FCID lines from one beschikking (for example: principal + administrative cost + verhoging) declares separate articles or separate `produces` blocks, each with its own category.

### Field derivation

When the engine emits an FCID event from a decretogram, fields are derived as follows.

| FCID field | RegelRecht source |
|---|---|
| `event_type` | from `decision_type` per the table above |
| `category` | from `outbound_category` |
| `juridische_grondslag_omschrijving` | first sentence of `article.text`, or `article.title` if shorter |
| `juridische_grondslag_bron` | `article.url` (canonical wetten.overheid.nl link) |
| `zaakkenmerk` | deterministic hash of `(engine.organisation_id, beschikking_id)`; engines that have their own zaaknummer-systematiek substitute it here |
| `gebeurtenis_kenmerk` | UUID v7 generated at emission time |
| `bedrag` | currency-typed output × 100 (FCID requires centen as integer) |
| `signature` | RFC-009 FSC signing key for this cell |
| `trace_id` | W3C Trace Context `trace_id` from the decretogram's execution trace |

The `trace_id` is the link that lets a downstream consumer (citizen portal, oversight system, other cell) follow back to the execution trace that produced the beschikking. The engine's RFC-013 trace stays in the cell; only the `trace_id` reference travels with the FCID event.

## Producer side: emitting executogram-derived FCID

A cell that receives a payment, processes a kwijtschelding, or completes a deurwaardertraject records this as an executogram. The executogram-stream YAML declares the FCID mapping:

```yaml
$id: payments_received_cjib
competent_authority: CJIB
events:
  - name: payment_received
    source: incasso_system_intake
    fcid:
      event_type: BetalingVerwerkt
      category: ALGEMEEN
    fields:
      case_reference: $external.zaakkenmerk
      amount_cents: $external.bedrag_centen
      received_at: $external.received_at
```

The `fcid` block under each event is read by the integration layer when this executogram-stream is wired to MBO. Executogram-stream files that do not declare an `fcid` block are not emitted to MBO; they are still recorded in the cell's chronicle.

### Field derivation for executograms

| FCID field | Executogram source |
|---|---|
| `event_type` | from `events[].fcid.event_type` |
| `category` | from `events[].fcid.category` |
| `zaakkenmerk` | the same `zaakkenmerk` as the originating decretogram, linking the payment back to its obligation |
| `gebeurtenis_kenmerk` | UUID v7 generated at emission time |
| `bedrag` | from the executogram's `amount_cents` field |
| `gebeurtenis_datetime` | from the executogram's `received_at` field |
| `signature` | RFC-009 FSC signing key for the executogram cell (may or may not be the same as the decretogram cell's key) |

## Consumer side: querying MBO

A regulation that needs the citizen's openstaande vorderingen references a wrapper regulation in `corpus/regulation/`:

```yaml
# In a downstream regulation that needs openstaande vorderingen as input
input:
  - name: openstaande_vorderingen
    source:
      regulation: "procedureregeling_vorderingenoverzicht_rijk"
      output: "openstaande_vorderingen"
      parameters:
        bsn: $bsn
```

The wrapper regulation `procedureregeling_vorderingenoverzicht_rijk` declares CJIB as `competent_authority` for the output `openstaande_vorderingen[]`. The engine resolves through the RFC-009 EXECUTE/ACCEPT decision tree. A non-CJIB engine hits ACCEPT and calls CJIB's cell via FSC. A CJIB engine executes locally, which delegates to its internal MBO query system. Either way the consumer sees a list of vordering records suitable as downstream input.

The wrapper regulation does not duplicate FCID semantics. It presents the result as RegelRecht-native data so that downstream regulations can use it without knowing about FCID.

## Trust and signing

Trust mechanics are inherited from [RFC-009 §5](/rfcs/rfc-009) without modification. Each emitting cell signs with its FSC key; the receiver verifies against the FSC Directory's Trust Anchor. The same key signs both decretogram-derived and executogram-derived events; the receiver can tell them apart from the `event_type` only.

## Out of scope

- **Citizen authentication** for portal-side access to MBO data is a separate concern at the API gateway layer.
- **Payment processing** (iDEAL, automatic incasso, reconciliation) is upstream of this integration.
- **The Financial Claim Request API and Session API** that surround FCID are not yet integrated. This document covers FCID-event emission and the consumer wrapper only. Adding the request/session APIs is a follow-up.
- **The legal basis** for each specific data exchange (which cell may send which event to which receiver) is per-case and per the relevant statutory provisions.

## Implementation references

- Schema vendoring: vendor FCID v4.x JSON schemas under `schema/external/vorijk/` with a README documenting the upstream URL and snapshot date.
- Engine module: `packages/engine/src/integrations/mbo_fcid.rs` (pure mapping from decretogram/executogram to FCID-shape JSON).
- Snapshot tests: a fixture beschikking validated against the vendored FCID schema.
- Service registry stub: a CJIB peer entry in the development `service-registry.yaml` for end-to-end testing of the consumer path.

## Pilot

The first concrete adoption is a Wahv pilot at CJIB. See [proposals/cjib-mbo-bridge](https://github.com/MinBZK/regelrecht/blob/main/proposals/cjib-mbo-bridge.md) for the casus and the proposed work plan.

## References

- [RFC-022: Chronolexogram types](/rfcs/rfc-022) — the conceptual ground for this integration
- [RFC-009: Multi-Organisation Execution](/rfcs/rfc-009) — cell identity, federation, trust
- [RFC-013: Execution Provenance](/rfcs/rfc-013) — trace propagation as W3C Trace Context
- [CJIB-uitvoeringslandschap](/concepts/cjib-uitvoeringslandschap) — landscape of what CJIB executes and for whom
- [FCID specification](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/) — upstream standard, currently v4.2.0
- [VORIJK GitLab (Blauwe Knop)](https://gitlab.com/blauwe-knop/vorderingenoverzicht) — reference implementation
- [Mijn Betaaloverzicht / Eén Overheidsincasso](https://www.eenoverheidsincasso.nl/onze-dienstverlening/vorderingenoverzicht-rijk)
