# Integration: Mijn Betaaloverzicht (FCID)

This page specifies how a cell that runs a RegelRecht engine integrates with [Mijn Betaaloverzicht (MBO)](https://www.eenoverheidsincasso.nl/onze-dienstverlening/vorderingenoverzicht-rijk) using the [Financial Claims Information Document (FCID)](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/) standard. It uses the chronolexogram types, the `extensions` mechanism, and the `source.kind: lexostatus_query` construct from [RFC-022](/rfcs/rfc-022). It uses the federation model and signing mechanics from [RFC-009](/rfcs/rfc-009).

The specification targets FCID v4.x (v4.2.0 as of mei 2026) and tracks upstream as the standard evolves. This document is intended to be updated when FCID minor versions are published, without requiring a new RFC.

For background on the Dutch claim-collection landscape this integration sits in, see [CJIB-uitvoeringslandschap](/concepts/cjib-uitvoeringslandschap).

## What this integration does

A cell that runs a RegelRecht engine and activates the `mbo_fcid` integration emits two streams of events to MBO endpoints:

- **Decretogram-derived FCID events**, when a regulation produces a `BESCHIKKING` with a financial-enforcement `decision_type` and its `extensions.mbo_fcid` block declares the FCID category.
- **Executogram-derived FCID events**, when an event in a chronicle-stream file (under `chronicles/`) declares an `extensions.mbo_fcid` block and the surrounding intake fires.

In the consumer direction, a regulation can ask MBO for a citizen's openstaande vorderingen via `source.kind: lexostatus_query`. The query reaches the CJIB cell through RFC-009's ACCEPT path; no wrapper-regulation needed.

## Activation

A cell decides whether it participates. Activation lives in cell-config, not in any regulation:

```yaml
# cell-config (sketch; full cell-config format is for a future RFC)
integrations:
  mbo_fcid:
    enabled: true
    endpoint: https://mbo.example.gov.nl/intake
    fcid_version: 4.2.0
```

A gemeente that runs the Wahv lexogram but does not connect to MBO simply omits the `mbo_fcid` block in its cell-config. The same regulation YAML works in both cells.

## FCID event types

FCID defines four event types. Each maps to exactly one chronolexogram type.

| FCID `event_type` | Chronolexogram type | Source in RegelRecht |
|---|---|---|
| `FinancieleVerplichtingOpgelegd` | decretogram | engine output, `decision_type: STRAFBESCHIKKING` (totaalbedrag) |
| `BetalingsverplichtingOpgelegd` | decretogram | engine output, `decision_type: BETALINGSVERPLICHTING` / `BESTUURLIJKE_BOETE` |
| `BetalingsverplichtingIngetrokken` | decretogram (intrekking) | engine output with `produces.modality.is_intrekking_van` set |
| `BetalingVerwerkt` | executogram | chronicle-stream event, triggered by intake from incasso system |

A primary BESCHIKKING that imposes a financial obligation and an intrekking of that same BESCHIKKING share `decision_type` (for example both `BETALINGSVERPLICHTING`); the intrekking carries `modality.is_intrekking_van: <id>`. The FCID mapping reads the modality and emits `BetalingsverplichtingIngetrokken` for the intrekking instance.

## Producer side: decretogram-derived FCID

A rule that should emit FCID on producing a decretogram declares its FCID intent inside the `extensions.mbo_fcid` block:

```yaml
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: BETALINGSVERPLICHTING
    bezwaarbaar:
      door: belanghebbende
      via: cell.cjib.bezwaar
      termijn_dagen: 42
      grondslag: "Awb 7:1"
    extensions:
      mbo_fcid:
        category: ALGEMEEN
```

The presence of the `extensions.mbo_fcid` block is a hint: "if your cell activates `mbo_fcid`, this rule's BESCHIKKING participates, with category X". The actual emission happens only when the cell activates the integration.

`category` is one of `ALGEMEEN`, `ADMINISTRATIEKOSTEN`, `VERHOGING`, `RENTE`. A regulation that produces multiple FCID lines from one beschikking (principal + administratiekosten + verhoging) declares those as separate articles or separate `produces` blocks, each with its own `extensions.mbo_fcid.category`.

### Field derivation

When the cell emits an FCID event from a decretogram:

| FCID field | Derivation |
|---|---|
| `event_type` | from `decision_type` plus `modality.is_intrekking_van` per the table above |
| `category` | from `extensions.mbo_fcid.category` |
| `juridische_grondslag_omschrijving` | first sentence of `article.text`, or `article.title` if shorter |
| `juridische_grondslag_bron` | `article.url` (canonical wetten.overheid.nl link) |
| `zaakkenmerk` | the cell's existing zaaknummer-systematiek; otherwise deterministic hash of `(cell.id, beschikking_id)` |
| `gebeurtenis_kenmerk` | UUID v7 generated at emission time |
| `bedrag` | currency-typed output × 100 (FCID requires centen as integer) |
| `bezwaar_route` | derived from `produces.bezwaarbaar` (this carries rechtsbescherming-information to the MBO surface; see [RFC-022 §7](/rfcs/rfc-022)) |
| `signature` | the cell's FSC signing key (RFC-009 §5) |
| `trace_id` | W3C Trace Context `trace_id` from the decretogram's execution trace |

The `trace_id` lets a downstream surface (citizen portal, oversight tool, another cell) follow back to the execution trace that produced the beschikking. The trace stays in the cell; only the `trace_id` reference travels with the event.

The `bezwaar_route` field is included so that a citizen-facing surface can show, alongside the obligation, how and within what term to object. This is the §7 of RFC-022 carried through: rechtsbescherming travels with the event, not as a separate channel that must be discovered.

## Producer side: executogram-derived FCID

A chronicle-stream entry that should emit FCID declares it the same way:

```yaml
# chronicles/cjib_payments.yaml
$id: cjib_payments
competent_authority: cjib
chronicle: cjib_main_chronicle
events:
  - name: payment_received
    intake: incasso_system_intake
    fields:
      case_reference: $external.zaakkenmerk
      amount_cents: $external.bedrag_centen
      received_at: $external.received_at
    bezwaarbaar: niet_van_toepassing
    bezwaarbaar_reden: "Geen besluit; feitelijke registratie van ontvangen betaling."
    extensions:
      mbo_fcid:
        event_type: BetalingVerwerkt
        category: ALGEMEEN
```

A chronicle-stream event that has no `extensions.mbo_fcid` block is still recorded in the cell's chronicle. It just does not surface in MBO.

### Field derivation for executograms

| FCID field | Derivation |
|---|---|
| `event_type` | from `extensions.mbo_fcid.event_type` |
| `category` | from `extensions.mbo_fcid.category` |
| `zaakkenmerk` | the same `zaakkenmerk` as the originating decretogram, linking the payment back to the obligation |
| `gebeurtenis_kenmerk` | UUID v7 generated at emission time |
| `bedrag` | from the event's `amount_cents` field |
| `gebeurtenis_datetime` | from the event's `received_at` field |
| `signature` | the cell's FSC signing key |

Whether `BetalingVerwerkt` carries a `bezwaar_route` is currently `niet_van_toepassing`: a received payment is a fact, not a besluit, and objecting to a fact is not what bezwaar is for. If a downstream besluit follows (such as "betaling toegerekend aan vordering X" being itself a decretogram), that decretogram carries its own `bezwaarbaar`.

## Consumer side: querying MBO

A regulation that needs the citizen's openstaande vorderingen declares a lexostatus-query source, not a fake regulation reference:

```yaml
input:
  - name: openstaande_vorderingen
    source:
      kind: lexostatus_query
      cell: cjib
      lexostatus: openstaande_vorderingen
      parameters:
        bsn: $bsn
```

The engine resolves through RFC-009's EXECUTE/ACCEPT decision tree on `source.kind: lexostatus_query`. A non-CJIB engine hits ACCEPT and calls the CJIB cell via FSC. A CJIB engine answers locally from its own chronicle. Either way the consumer sees a list of vordering records suitable as downstream input.

The CJIB cell internally performs a chronolexoreductie: it filters its chronicles for the BSN, combines outstanding decretograms with their executograms (payments, kwijtscheldingen), and returns a lexostatus. RegelRecht consumers do not see the chronicle entries; they see the result.

## Trust and signing

Trust mechanics are inherited from [RFC-009 §5](/rfcs/rfc-009) without modification. The cell signs both decretogram-derived and executogram-derived events with its FSC key. The receiver verifies against the FSC Directory's Trust Anchor. The `event_type` distinguishes the two on the wire; the signing key does not.

## Bezwaar and rechtsbescherming

The `bezwaar_route` field in emitted FCID events is the integration's contribution to Nieuwland §7.2.1. Concretely:

- For decretograms: every emitted event carries the `bezwaarbaar` block of the producing rule. MBO can render the objection-trigger inline with the obligation.
- For executograms: most events carry `bezwaarbaar: niet_van_toepassing` (a fact is not a besluit). Executograms that do have legal consequences (a kwijtschelding granted, a verrekening applied) carry their own `bezwaarbaar`.
- Citizens can therefore exercise bezwaar from the MBO surface against the rule that produced the obligation, not against MBO itself or against CJIB-as-collector.

The receiving cell at the bezwaar intake is identified by `bezwaar_route.via`; routing is the receiving cell's responsibility.

## Out of scope

- **Citizen authentication** for portal-side access to MBO data is at the API gateway layer.
- **Payment processing** (iDEAL, automatic incasso, reconciliation) is upstream of this integration.
- **The Financial Claim Request API and Session API** that surround FCID are not yet integrated. This document covers FCID-event emission and the lexostatus-query consumer only.
- **The legal basis** for each specific exchange (which cell may send which event to which receiver) is per-case and per the relevant statutory provisions.

## Implementation references

- Schema vendoring: vendor FCID v4.x JSON schemas under `schema/external/vorijk/` with a README documenting upstream URL and snapshot date.
- Engine module: `packages/engine/src/integrations/mbo_fcid.rs` (pure mapping from decretogram/executogram to FCID-shape JSON, including `bezwaar_route` derivation).
- Cell-config: a small `cells/<cell-id>/config.yaml` (sketch) that activates `mbo_fcid` with endpoint and version.
- Snapshot tests: a fixture beschikking and a fixture chronicle-event, validated against the vendored FCID schema.
- Service registry stub: a CJIB cell entry in the development `service-registry.yaml` for end-to-end testing of the lexostatus-query path.

## Pilot

The first concrete adoption is a Wahv pilot at CJIB. See [proposals/cjib-mbo-bridge](https://github.com/MinBZK/regelrecht/blob/main/proposals/cjib-mbo-bridge.md) for the casus and the proposed work plan.

## References

- [RFC-022: Chronolexogram types in the schema and the cell model](/rfcs/rfc-022)
- [RFC-009: Multi-Organisation Execution](/rfcs/rfc-009)
- [RFC-013: Execution Provenance](/rfcs/rfc-013)
- [CJIB-uitvoeringslandschap](/concepts/cjib-uitvoeringslandschap)
- [FCID specification](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/)
- [VORIJK GitLab (Blauwe Knop)](https://gitlab.com/blauwe-knop/vorderingenoverzicht)
- [Mijn Betaaloverzicht / Eén Overheidsincasso](https://www.eenoverheidsincasso.nl/onze-dienstverlening/vorderingenoverzicht-rijk)
