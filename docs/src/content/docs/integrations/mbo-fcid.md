# Integration: Mijn Betaaloverzicht (FCID)

This page specifies how a cell that runs a RegelRecht engine integrates with [Mijn Betaaloverzicht (MBO)](https://www.eenoverheidsincasso.nl/onze-dienstverlening/vorderingenoverzicht-rijk) using the [Financial Claims Information Document (FCID)](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/) standard. It uses the chronolexogram types, the `extensions` mechanism, and the `source.kind: lexostatus_query` construct from [RFC-022](/rfcs/rfc-022), the AWB lifecycle from [RFC-008](/rfcs/rfc-008), and the federation/signing mechanics from [RFC-009](/rfcs/rfc-009).

The specification targets FCID v4.x (v4.2.0 as of mei 2026) and tracks upstream as the standard evolves. This document is intended to be updated when FCID minor versions are published, without requiring a new RFC.

For background on the Dutch claim-collection landscape this integration sits in, see [CJIB-uitvoeringslandschap](/concepts/cjib-uitvoeringslandschap).

## What this integration does

A cell that runs a RegelRecht engine and activates the `mbo_fcid` integration emits two streams of events to MBO endpoints:

- **Decretogram-derived FCID events**, when a regulation produces a `BESCHIKKING` with a financial-enforcement `decision_type` and its `extensions.mbo_fcid` block declares the FCID category. Emission happens at a specific AWB lifecycle stage (RFC-008), typically `BEKENDMAKING`.
- **Executogram-derived FCID events**, when an event in a chronicle-stream file (under `chronicles/`) declares an `extensions.mbo_fcid` block and the surrounding intake fires.

In the consumer direction, a regulation can ask MBO for a citizen's openstaande vorderingen via `source.kind: lexostatus_query`. The query reaches the CJIB cell through RFC-009's ACCEPT path; no wrapper-regulation needed.

## Activation

A cell decides whether it participates. Activation lives in cell-config, not in any regulation:

```yaml
# cell-config (sketch; full cell-config format is defined in a future RFC,
# see RFC-022 §1.3 note)
integrations:
  mbo_fcid:
    enabled: true
    endpoint: https://mbo.example.gov.nl/intake
    fcid_version: 4.2.0
```

The shape above is provisional and may evolve. What is fixed is the principle: activation of the `mbo_fcid` integration is a cell-side decision, not part of any regulation. A gemeente that runs the Wahv lexogram but does not connect to MBO simply omits the `mbo_fcid` block in its cell-config. The same regulation YAML works in both cells.

## FCID event types

FCID defines four event types. Each maps to exactly one chronolexogram type.

| FCID `event_type` | Chronolexogram type | Source in RegelRecht |
|---|---|---|
| `FinancieleVerplichtingOpgelegd` | decretogram | engine output, `decision_type: STRAFBESCHIKKING` (totaalbedrag) |
| `BetalingsverplichtingOpgelegd` | decretogram | engine output, `decision_type: BETALINGSVERPLICHTING` / `BESTUURLIJKE_BOETE` |
| `BetalingsverplichtingIngetrokken` | decretogram (intrekking-modaliteit) | engine output, same `decision_type` as the original, with `produces.modality.is_intrekking_van` set |
| `BetalingVerwerkt` | executogram | chronicle-stream event, triggered by intake from incasso system |

An intrekking is itself a fresh BESCHIKKING with its own AWB lifecycle (per RFC-008's resolved Open Question 5). The integration recognises it as an intrekking through `produces.modality.is_intrekking_van: <original-id>` and maps it to `BetalingsverplichtingIngetrokken`. The intrekking and the original share neither a single decretogram nor a single lifecycle; they share a `zaakkenmerk` so a downstream consumer can tie them together.

## Producer side: decretogram-derived FCID

A rule that should emit FCID on producing a decretogram declares its FCID intent inside the `extensions.mbo_fcid` block:

```yaml
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: BETALINGSVERPLICHTING
    procedure_id: beschikking         # RFC-008 procedure selection
    extensions:
      mbo_fcid:
        category: ALGEMEEN
        emit_at_stage: BEKENDMAKING   # default; can be overridden
```

The presence of the `extensions.mbo_fcid` block is a hint: "if your cell activates `mbo_fcid`, this rule's BESCHIKKING participates, with category X". The actual emission happens only when the cell activates the integration.

`category` is one of `ALGEMEEN`, `ADMINISTRATIEKOSTEN`, `VERHOGING`, `RENTE`. A regulation that produces multiple FCID lines from one beschikking (principal + administratiekosten + verhoging) declares those as separate articles or separate `produces` blocks, each with its own `extensions.mbo_fcid.category`.

`emit_at_stage` selects the RFC-008 lifecycle stage at which emission fires. Default is `BEKENDMAKING`: an obligation that has not been bekendgemaakt has no juridical existence to display in MBO. Other stages are allowed (an integration may want a placeholder at BESLUIT for internal-overview surfaces) but for citizen-facing MBO the default is correct.

### Stage-binding: why BEKENDMAKING and not BESLUIT

RFC-008 models the AWB lifecycle as discrete stages: AANVRAAG, BEHANDELING, BESLUIT, BEKENDMAKING, BEZWAARTERMIJN, BEZWAAR. The bezwaartermijn (AWB 6:7, six weeks) starts the day after bekendmaking (AWB 6:8 lid 1). The `bezwaartermijn_einddatum` is therefore a value that does not exist at BESLUIT-time and is computed by RFC-008 hooks (AWB 6:7, AWB 6:8, Termijnenwet) at the BEKENDMAKING stage.

FCID emission at BESLUIT would therefore carry an incomplete `bezwaar_route`: no einddatum, only a hint. Emission at BEKENDMAKING carries the actual einddatum, computed correctly per the AWB lifecycle. The integration accepts the RFC-008 timing rather than reinventing it.

For UOV procedures (RFC-008 §A.8) the regular bezwaar is excluded (AWB 7:1 lid 1 sub d). The integration emits at the UOV's BEKENDMAKING stage with `beroep_route` instead of `bezwaar_route`. For BESLUIT_VAN_ALGEMENE_STREKKING (RFC-008 §A.9) neither route applies except for "concretiserend BAS"; the integration omits both fields when neither applies and records a `geen_rechtsbescherming_reden` for transparency.

### Field derivation

When the cell emits an FCID event from a decretogram at its configured stage:

| FCID field | Derivation |
|---|---|
| `event_type` | from `decision_type` plus `modality.is_intrekking_van` per the table above |
| `category` | from `extensions.mbo_fcid.category` |
| `juridische_grondslag_omschrijving` | first sentence of `article.text`, or `article.title` if shorter |
| `juridische_grondslag_bron` | `article.url` (canonical wetten.overheid.nl link) |
| `zaakkenmerk` | the cell's existing zaaknummer-systematiek; otherwise deterministic hash of `(cell.id, beschikking_id)` |
| `gebeurtenis_kenmerk` | UUID v7 generated at emission time |
| `bedrag` | currency-typed output × 100 (FCID requires centen as integer) |
| `bezwaar_route` | derived from the decretogram's RFC-008 procedure-stage outputs at the emission stage; see below |
| `signature` | the cell's FSC signing key (RFC-009 §5) |
| `trace_id` | W3C Trace Context `trace_id` from the decretogram's execution trace |

The `trace_id` lets a downstream surface (citizen portal, oversight tool, another cell) follow back to the execution trace that produced the beschikking. The trace stays in the cell; only the `trace_id` reference travels with the event.

### `bezwaar_route` derivation from RFC-008

The integration does not read a `bezwaarbaar` field from `produces`. Instead, at emission time, it queries the decretogram's RFC-008 procedure state for the bezwaar-stage outputs:

| `bezwaar_route` field | Derived from |
|---|---|
| `intake` | the cell's bezwaar-intake URL for the rule's `procedure_id` (cell-config) |
| `termijn_grondslag` | the AWB article (or lex specialis override) that determined the termijn, e.g. `"Awb 6:7"` or `"Vw 2000 art. 69"` |
| `termijn_einddatum` | `bezwaartermijn_einddatum` output of the BEKENDMAKING-stage hooks (AWB 6:8 + Termijnenwet) |
| `direct_beroep_mogelijk` | true when AWB 7:1a applies; absent otherwise |

If the procedure has no bezwaar-stage (UOV, AVV-without-direct-beroep), `bezwaar_route` is absent and either `beroep_route` (UOV, concretiserend BAS) or `geen_rechtsbescherming_reden` (AVV, beleidsregel) is present.

This is the operationalisation of [RFC-022 §3.3](/rfcs/rfc-022): rechtsbescherming travels with the event, derived from RFC-008's lifecycle outputs, not declared per-rule. A citizen-facing surface (MBO portal, MijnOverheid) can render the objection-trigger inline with the obligation, with the correct einddatum that the AWB-hooks computed at the right moment.

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

### Rechtsbescherming on executograms

Most executograms carry no `bezwaar_route`. A received payment is a fact, not a besluit; objecting to a fact is not what bezwaar is for.

A small class of executograms does carry one: events that *implicitly reference a nested besluit*. A `kwijtschelding_verleend` event is the outward face of a kwijtschelding-decretogram (which has its own AWB lifecycle). The chronicle-stream declares the link:

```yaml
- name: kwijtschelding_verleend
  references_decision: $external.kwijtschelding_decision_id
  fields:
    case_reference: $external.zaakkenmerk
    reden: $external.reden
  extensions:
    mbo_fcid:
      event_type: BetalingsverplichtingIngetrokken
      category: ALGEMEEN
```

When `references_decision` is present, the integration looks up that decision's RFC-008 procedure state and derives `bezwaar_route` from it (same mechanism as for decretograms). The chronicle-stream itself does not duplicate the route.

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

The CJIB cell internally performs a chronolexoreductie: it filters its chronicles for the BSN, combines outstanding decretograms with their executograms (payments, kwijtscheldingen), and returns a lexostatus. RegelRecht consumers do not see the chronicle entries; they see the result. Each vordering in the returned lexostatus carries the same `bezwaar_route` as the corresponding FCID event would.

## Trust and signing

Trust mechanics are inherited from [RFC-009 §5](/rfcs/rfc-009) without modification. The cell signs both decretogram-derived and executogram-derived events with its FSC key. The receiver verifies against the FSC Directory's Trust Anchor. The `event_type` distinguishes the two on the wire; the signing key does not.

## Out of scope

- **Citizen authentication** for portal-side access to MBO data is at the API gateway layer.
- **Payment processing** (iDEAL, automatic incasso, reconciliation) is upstream of this integration.
- **The Financial Claim Request API and Session API** that surround FCID are not yet integrated. This document covers FCID-event emission and the lexostatus-query consumer only.
- **The legal basis** for each specific exchange (which cell may send which event to which receiver) is per-case and per the relevant statutory provisions.
- **The AWB lifecycle internals**. All bezwaar-mechanics, nested besluit op bezwaar, termijn-berekening live in RFC-008. This integration reads RFC-008's outputs.

## Implementation references

- Schema vendoring: vendor FCID v4.x JSON schemas under `schema/external/vorijk/` with a README documenting upstream URL and snapshot date.
- Engine module: `packages/engine/src/integrations/mbo_fcid.rs` (pure mapping from decretogram/executogram to FCID-shape JSON, including `bezwaar_route` derivation from RFC-008 procedure state).
- Stage-hook wiring: the integration registers itself as a RFC-008 stage hook for `BEKENDMAKING` (or whichever stage `emit_at_stage` selects).
- Cell-config: a small `cells/<cell-id>/config.yaml` (sketch) that activates `mbo_fcid` with endpoint and version.
- Snapshot tests: a fixture beschikking driven through its AWB lifecycle to BEKENDMAKING and a fixture chronicle-event, both validated against the vendored FCID schema.
- Service registry stub: a CJIB cell entry in the development `service-registry.yaml` for end-to-end testing of the lexostatus-query path.

## Pilot

The first concrete adoption is a Wahv pilot at CJIB. See [proposals/cjib-mbo-bridge](https://github.com/MinBZK/regelrecht/blob/main/proposals/cjib-mbo-bridge.md) for the casus and the proposed work plan.

## References

- [RFC-022: Chronolexogram types in the schema and the cell model](/rfcs/rfc-022)
- [RFC-008: AWB Administrative Procedures](/rfcs/rfc-008)
- [RFC-009: Multi-Organisation Execution](/rfcs/rfc-009)
- [RFC-013: Execution Provenance](/rfcs/rfc-013)
- [CJIB-uitvoeringslandschap](/concepts/cjib-uitvoeringslandschap)
- [FCID specification](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/)
- [VORIJK GitLab (Blauwe Knop)](https://gitlab.com/blauwe-knop/vorderingenoverzicht)
- [Mijn Betaaloverzicht / Eén Overheidsincasso](https://www.eenoverheidsincasso.nl/onze-dienstverlening/vorderingenoverzicht-rijk)
