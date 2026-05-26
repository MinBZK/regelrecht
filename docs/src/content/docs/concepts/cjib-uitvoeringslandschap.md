# CJIB-uitvoeringslandschap

This page surveys what the Centraal Justitieel Incassobureau (CJIB) actually does: which regulations it executes itself, which it executes on behalf of other organizations, and the policy framework that governs both. It is background material for [RFC-009 (Multi-Organisation Execution)](/rfcs/rfc-009), [RFC-019 (Chronolexogram types and the cell model)](/rfcs/rfc-019), and the concrete [MBO/FCID integration](/integrations/mbo-fcid). It is not normative.

The page uses the conceptual vocabulary from the [Chronolexografie](https://chronolexografie.nl/) position paper and the wider [Nieuwland](https://achterkantvandeoverheid.nl/) program.

## Why CJIB matters for RegelRecht

CJIB is a zelfstandig bestuursorgaan (ZBO) under the Ministry of Justice and Security. It is the central financial-enforcement hub of the Dutch government: the place where almost every administrative-law and criminal-law financial obligation eventually lands when a citizen does not pay voluntarily. As of 2026 it executes for at least 15 opdrachtgevers, ranging from the OM to a sectoral inspectorate like NEa.

RegelRecht's corpus today (23 regulations) covers fiscal and social domains exclusively. Nothing in it produces a `BETALINGSVERPLICHTING` or a `STRAFBESCHIKKING`. Bridging RegelRecht to the CJIB world is the precondition for two things: making incasso-domain laws executable, and connecting RegelRecht cells to [Mijn Betaaloverzicht (MBO, formerly Vorderingenoverzicht Rijk)](https://www.eenoverheidsincasso.nl/onze-dienstverlening/vorderingenoverzicht-rijk) so a citizen sees both the obligation and its juridical provenance.

## Conceptual framework: three kinds of recording, and the cell

Chronolexografie distinguishes three types of vastlegging. The distinction matters here because the CJIB landscape touches all three.

- **Lexogram**: vastlegging van een (mogelijke, toekomstige) wijziging in wet- of regelgeving. Example in this landscape: the Wahv itself (BWBR0004581), as a versioned text on wetten.overheid.nl. A RegelRecht corpus entry for the Wahv would be a lexogram.
- **Decretogram**: vastlegging van een concreet besluit. Example: a specific Wahv-sanctie of €X imposed on a specific kentekenhouder for a specific feit on a specific date.
- **Executogram**: vastlegging van daadwerkelijke levering of afhandeling. Example: a payment of €X arriving in CJIB's bank account against zaakkenmerk Y on date Z. Or a kwijtschelding granted because of payment incapacity.

In addition, Chronolexografie introduces the **chronolexocell**: the juridical and organisational domain that holds chronicles, owns signing keys, and is the competent authority that other cells contract with. CJIB is a cell. So are OM, NVWA, DUO, CAK, UWV, RVO, NEa, and each gemeente. A cell may contain one RegelRecht engine, several engines, an engine plus a legacy system, or none at all; the engine is a component within the cell, not the cell itself.

CJIB's daily work spans all three recording-types. It executes lexogrammen (the wetten and beleidsregels under which it works), produces decretogrammen (Wahv-sancties, OM-strafbeschikkingen-uitvoering), and records executogrammen (betalingen, kwijtscheldingen, deurwaardertrajecten). RFC-019 puts each of these in its proper place in the repository layout: lexograms in `corpus/regulation/`, chronicle-stream definitions (which declare which executograms a cell records) in `chronicles/`.

## CJIB's own statutory tasks

These are regulations where CJIB itself is the executing cell, either directly or by mandate of the Minister of Justice and Security.

| Regulation | Grondslag | BWB-ID | What it covers |
|---|---|---|---|
| Wahv (Wet Mulder) | Wet administratiefrechtelijke handhaving verkeersvoorschriften | [BWBR0004581](https://wetten.overheid.nl/BWBR0004581) | Administrative settlement of minor traffic offences |
| OM-strafbeschikking | Wetboek van Strafvordering art. 257a–257h | [BWBR0001903](https://wetten.overheid.nl/BWBR0001903) | Settlement act by the public prosecutor, executed by CJIB |
| Schadevergoedingsmaatregel | Wetboek van Strafrecht art. 36f | [BWBR0001854](https://wetten.overheid.nl/BWBR0001854) | Damages awarded to victim, collected by CJIB |
| Voorschotregeling slachtoffer | Sr art. 36f lid 7 | [BWBR0001854](https://wetten.overheid.nl/BWBR0001854) | State advances payment after 8 months; CJIB recovers from offender |
| Ontnemingsmaatregel ("Pluk-ze") | Wetboek van Strafrecht art. 36e | [BWBR0001854](https://wetten.overheid.nl/BWBR0001854) | Confiscation of unlawfully obtained gains |
| Wet DNA-V | Wet DNA-onderzoek bij veroordeelden | [BWBR0017212](https://wetten.overheid.nl/BWBR0017212) | DNA sampling of convicted persons. Cost recovery via beleidsregels `[onzeker]` |
| Tenuitvoerlegging strafrechtelijke beslissingen (Wet USB) | Wetboek van Strafvordering Boek 6, in werking 2020-01-01 | [BWBR0001903](https://wetten.overheid.nl/BWBR0001903/Boek6) | Responsibility for execution shifted from OM to Minister; CJIB/AICE is operational chain coordinator |
| EU wederzijdse erkenning geldelijke sancties | Wet wederzijdse erkenning en tenuitvoerlegging geldelijke sancties en beslissingen tot confiscatie | [BWBR0022604](https://wetten.overheid.nl/BWBR0022604) | Cross-border recognition and collection of fines/confiscation orders |

## CJIB as executor for other cells

CJIB collects for at least 15 opdrachtgevers. The legal basis is per-case: some are sectoral acts that designate the Minister of JenV or directly CJIB; others are mandate constructions under the Algemene wet bestuursrecht. The Clustering Rijksincasso (CRI) program, formalized via [eenoverheidsincasso.nl](https://www.eenoverheidsincasso.nl/onze-partners), structures this collaboration.

In Chronolexografie terms, the opdrachtgever's cell produces the primary decretogram (the inhoudelijke beschikking) and CJIB's cell records the executograms (betaling, kwijtschelding) on behalf of the opdrachtgever. Whether CJIB also produces a follow-on decretogram of its own (for example a dwangbevel under Awb 4:114) depends on the regulation and the convenant.

| Opdrachtgever | Type vordering | Grondslag (best available) | Decretogram-cel | Executogram-cel (collection) | Maps to RegelRecht schema? |
|---|---|---|---|---|---|
| OM | Strafbeschikking, schadevergoeding, ontneming | Sv 257a, Sr 36f, Sr 36e | OM | CJIB | New `decision_type: STRAFBESCHIKKING` |
| DUO | Studieschuld, lesgeld arrears | [Wet studiefinanciering 2000 (BWBR0011453)](https://wetten.overheid.nl/BWBR0011453), Les- en cursusgeldwet (BWBR0005407) `[BWB-IDs to verify]` | DUO | CJIB | New `decision_type: BETALINGSVERPLICHTING` |
| CAK | Eigen bijdrage Wmo/Wlz, wanbetalersregeling Zvw | [Zvw art. 18a–18d (BWBR0018450)](https://wetten.overheid.nl/BWBR0018450) | CAK | CJIB | Partial: `wet_langdurige_zorg` already in corpus |
| UWV | Terugvorderingen WW/WIA/Wajong | Sectoral employee-insurance acts + Awb titel 4.4 | UWV | CJIB | New `decision_type: BETALINGSVERPLICHTING` |
| RVO | Subsidie-terugvorderingen, agrarische boetes | Awb 4.4 + sectoral LNV/EZK acts | RVO | CJIB | New |
| NVWA | Bestuurlijke boetes voedselveiligheid, tabak, dier | [Warenwet](https://wetten.overheid.nl/BWBR0001969), Wet dieren, Tabaks- en rookwarenwet | NVWA | CJIB | New `decision_type: BESTUURLIJKE_BOETE` |
| RDW / e-Tol | Onverzekerd voertuig, MRB-schorsing | [WAM art. 30/34](https://wetten.overheid.nl/BWBR0002326), Wet MRB | RDW | CJIB | New |
| NEa | Emissiehandel-boetes | [Wet milieubeheer hoofdstuk 18 (BWBR0003245)](https://wetten.overheid.nl/BWBR0003245) | NEa | CJIB | New |
| Inspectie JenV | Bestuurlijke boetes | Per sectoral act `[onzeker]` | Inspectie JenV | CJIB | New |
| RDI | Telecom-boetes | [Telecommunicatiewet (BWBR0009950)](https://wetten.overheid.nl/BWBR0009950) | RDI | CJIB | New |
| ATKM | Boete tot 6e categorie of 4% jaaromzet | Uitvoeringswet Verordening terroristische online-inhoud art. 12–13 | ATKM | CJIB | New |
| DFEI | Diverse | `[onzeker — CRI lists "Dienst Financiële en Economische Integriteit"]` | unclear | CJIB | New |
| Gemeenten | Via mandaat | Gemeentewet + lokale verordeningen | gemeente | CJIB | Partial: `participatiewet` and `apv_erfgrens` exist |

The eight original CRI rijksorganisaties are: Belastingdienst, Dienst Toeslagen, CJIB, DUO, SVB, CAK, UWV, RVO. Since 2024 the Betalingsregeling Rijk has expanded to include NVWA, RDI, RDW e-Tol, Inspectie JenV, NEa, DFEI, and ATKM.

## What CJIB does not do

This is worth stating because lezers often misattribute. CJIB does **not** collect:

- **Gemeentelijke parkeerboetes**. These are municipal fiscal sanctions under the Wet Mulder regime; gemeenten collect via Cocensus, Belastingsamenwerkingen, or in-house.
- **Fiscale aanslagen** (income tax, BTW, etc.). The Belastingdienst executes its own invordering via the Invorderingswet 1990.
- **Gemeentelijke leges** and lokale heffingen, same reason as parkeerboetes.
- **Civielrechtelijke vorderingen** between private parties. Those go through gerechtsdeurwaarders.
- **Deurwaardersbeslag** in private-law disputes.

The line is roughly: CJIB does state-imposed financial obligations under public law (criminal, administrative, or specific civil-law victim measures), specifically when collection is centralized at Rijksniveau.

## Policy framework

| Instrument | Year | Source |
|---|---|---|
| Beleidsregels tenuitvoerlegging strafrechtelijke en administratiefrechtelijke beslissingen (USB 2021) | 2021 | [Stcrt 2021, 33851](https://zoek.officielebekendmakingen.nl/stcrt-2021-33851.html) |
| Wet USB + Invoeringswet USB | Stb 2017, 82; Stb 2019, 504; in werking 2020-01-01 | Boek 6 Sv |
| Aanwijzing OM-strafbeschikking | 2022A003 | [OM publication](https://www.om.nl/onderwerpen/beleidsregels/aanwijzingen/executie/aanwijzing-om-strafbeschikking-2022a003) |
| Algemene wet bestuursrecht titel 4.4 (Bestuursrechtelijke geldschulden) | In werking 2009-07-01 | [BWBR0005537 art. 4:85–4:125](https://wetten.overheid.nl/BWBR0005537) |
| Evaluatiewet bestuursrechtelijke geldschuldenregeling Awb (35.477) | In behandeling/aangenomen | [Eerste Kamer dossier](https://www.eerstekamer.nl/wetsvoorstel/35477_evaluatiewet) |
| CRI-programma (Clustering Rijksincasso) | Lopend | [eenoverheidsincasso.nl](https://www.eenoverheidsincasso.nl/) |

## Mapping onto the three chronolexogram types

### Lexogrammen (regulations themselves)

CJIB's own statutory base (Wahv, Sv 257a, Sr 36e/36f, Wet USB, Awb 4.4) and the sectoral acts behind every namens-uitvoering would each be a lexogram in `corpus/regulation/`. None of them are in the corpus today. RFC-019 does not deliver these; it makes them representable.

### Decretogrammen (beschikkingen)

The current `produces.decision_type` enum has nine values: TOEKENNING, AFWIJZING, GOEDKEURING, GEEN_BESLUIT, ALGEMEEN_VERBINDEND_VOORSCHRIFT, BELEIDSREGEL, VOORBEREIDINGSBESLUIT, ANDERE_HANDELING, AANSLAG. None describe the financial-enforcement domain.

[RFC-019 §2](/rfcs/rfc-019) adds three values, each a distinct type of besluit:

- `BETALINGSVERPLICHTING` — generic financial obligation imposed by a bestuursorgaan
- `STRAFBESCHIKKING` — criminal-law settlement under Sv 257a
- `BESTUURLIJKE_BOETE` — sectoral administrative fine

Intrekkingen of an earlier beschikking are not a separate type: they are the same `decision_type` with `modality.is_intrekking_van: <id>` (RFC-019 §2.1). This matches Awb practice: an intrekking is a handeling on an existing besluit, not a new besluit-type.

`legal_character: BESCHIKKING` already covers all of these.

### Executogrammen (factual events)

These do not belong in `produces` at all. A payment received, a kwijtschelding executed, a deurwaarder triggered: these are not regulation outputs. RFC-019 §3 introduces `chronicles/` as a separate top-level directory (parallel to `corpus/`, not inside it) for chronicle-stream files. The most natural CJIB chronicle-stream entries are:

- `payment_received` — maps to FCID `BetalingVerwerkt`
- `kwijtschelding_verleend` — maps to FCID `BetalingsverplichtingIngetrokken` with a reden veld
- `deurwaardertraject_gestart` — maps to FCID `BetalingsverplichtingOpgelegd` if it creates a separate verplichting for kosten; or is recorded internally without FCID emission if it is procedural

Each of these is a registratie, not an interpretatie. Putting them under `corpus/` (alongside regulations) would conflate norm and fact at the filesystem level; RFC-019 keeps them apart.

## Mapping onto MBO / FCID

The full mapping from decretograms and executograms to [FCID](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/) events lives in the [MBO/FCID integration document](/integrations/mbo-fcid). In short: each FCID `event_type` corresponds to either a decretogram (the three `*Opgelegd` and the `*Ingetrokken`) or an executogram (`BetalingVerwerkt`). Categories (Algemeen, Administratiekosten, Verhoging, Rente) are orthogonal to the chronolex-type and encoded via the `extensions.mbo_fcid` namespace that RFC-019 §5 adds for integration use. Activation of the integration lives in the cell-config, not in the lexogram: a gemeente that runs the same Wahv as CJIB but does not connect to MBO simply does not activate `mbo_fcid` in its own cell.

## Open questions and data gaps

The following items could not be verified from public sources and need input from CJIB or its opdrachtgevers:

1. **Complete CJIB portfolio**. Internal USB lists exist but are not publicly indexed.
2. **CJIB-side FCID adoption status**. Which regulations already emit FCID (in pilot or production), and which version?
3. **DFEI scope**. CRI lists "Dienst Financiële en Economische Integriteit", but its exact handover to CJIB is unclear.
4. **Cell topology at CJIB**. Does CJIB run one cell per opdrachtgever, one per regulation type, or one centrally? RFC-009 and RFC-019 support either; the choice affects chronicle ordering and signing keys.
5. **Bilateral convenants** that are not published in Staatscourant: there may be additional opdrachtgevers not surfaced through public sources.
6. **DNA-V cost recovery grondslag**. BWBR0017212 does not directly couple cost recovery to CJIB; this likely runs via beleidsregels that need verification.
7. **Per-opdrachtgever BWB-IDs** marked `[onzeker]` in the table.
8. **Bezwaar-routing**. Each CJIB-emitted FCID event carries a `bezwaar_route` derived from the producing rule's `bezwaarbaar` block ([RFC-019 §7](/rfcs/rfc-019)). For decretograms that CJIB itself produces (Wahv-sanctie), the bezwaar-route is CJIB's eigen bezwaar-intake. For decretograms that CJIB carries on behalf of another cell (a CAK-besluit, an OM-strafbeschikking), the bezwaar-route is that other cell's. The mechanics of this in the wettelijke routing per regulation need validation per case.
9. **Wet gegevensboekhouding interaction**. Nieuwland §7.3.2 sketches a Wet gegevensboekhouding that would put the executogram-side recording on a statutory footing. CJIB's current grondslag is implicit in Awb 4.4 + sectoral acts; an explicit statute would change the picture.
