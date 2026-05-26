# CJIB-uitvoeringslandschap

This page surveys what the Centraal Justitieel Incassobureau (CJIB) actually does: which regulations it executes itself, which it executes on behalf of other organizations, and the policy framework that governs both. It is background material for [RFC-009 (Multi-Organisation Execution)](/rfcs/rfc-009), [RFC-022 (Chronolexogram types)](/rfcs/rfc-022), and the concrete [MBO/FCID integration](/integrations/mbo-fcid). It is not normative.

The page uses the conceptual vocabulary from the [Chronolexografie](https://chronolexografie.nl/) position paper and the wider [Nieuwland](https://achterkantvandeoverheid.nl/) program. Where relevant, regulations are tagged with the chronolexogram types they involve: **lexogram** for the regulation itself, **decretogram** for a beschikking imposing an obligation, **executogram** for the factual handling (payment received, levering, kwijtschelding).

## Why CJIB matters for RegelRecht

CJIB is a zelfstandig bestuursorgaan (ZBO) under the Ministry of Justice and Security. It is the central financial-enforcement hub of the Dutch government: the place where almost every administrative-law and criminal-law financial obligation eventually lands when a citizen does not pay voluntarily. As of 2026 it executes for at least 15 opdrachtgevers, ranging from the OM to a sectoral inspectorate like NEa.

RegelRecht's corpus today (23 regulations) covers fiscal and social domains exclusively. Nothing in it produces a `BETALINGSVERPLICHTING` or a `STRAFBESCHIKKING`. Bridging RegelRecht to the CJIB world is the precondition for two things: making incasso-domain laws executable, and connecting RegelRecht to [Mijn Betaaloverzicht (MBO, formerly Vorderingenoverzicht Rijk)](https://www.eenoverheidsincasso.nl/onze-dienstverlening/vorderingenoverzicht-rijk) so a citizen sees both the obligation and its juridical provenance.

## Conceptual framework: three kinds of recording

Chronolexografie distinguishes three types of vastlegging. The distinction matters here because the CJIB landscape touches all three.

- **Lexogram** — vastlegging van een (mogelijke, toekomstige) wijziging in wet- of regelgeving. Example in this landscape: the Wahv itself (BWBR0004581), as a versioned text on wetten.overheid.nl. A RegelRecht corpus entry for the Wahv would be a lexogram.
- **Decretogram** — vastlegging van een concreet besluit bij toepassing van wetgeving. Example: a specific Wahv-sanctie of €X imposed on a specific kentekenhouder for a specific feit on a specific date. This is the BESCHIKKING that a RegelRecht engine would produce.
- **Executogram** — vastlegging van daadwerkelijke levering of afhandeling. Example: a payment of €X arriving in CJIB's bank account against zaakkenmerk Y on date Z. Or a kwijtschelding granted because of payment incapacity. Executograms are not regulation outputs; they are recordings of factual events that the cel learns from its surrounding systems.

CJIB is in all three categories. It executes lexogrammen (the wetten and beleidsregels under which it works), produces decretogrammen (Wahv-sancties, OM-strafbeschikkingen-uitvoering), and records executogrammen (betalingen, kwijtscheldingen, deurwaardertrajecten). A future RegelRecht setup at CJIB would similarly span all three. See [RFC-022 §2](/rfcs/rfc-022) for how the schema accommodates this.

## CJIB's own statutory tasks

These are regulations where CJIB itself is the executing authority, either directly or by mandate of the Minister of Justice and Security. All entries here are lexogrammen; CJIB's daily work on them produces decretogrammen and executogrammen.

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

## CJIB as executor for other organizations

CJIB collects for at least 15 opdrachtgevers. The legal basis is per-case: some are sectoral acts that designate the Minister of JenV or directly CJIB; others are mandate constructions under the Algemene wet bestuursrecht. The Clustering Rijksincasso (CRI) program, formalized via [eenoverheidsincasso.nl](https://www.eenoverheidsincasso.nl/onze-partners), structures this collaboration.

In Chronolexografie terms, every row in this table is a triple: the opdrachtgever's cel produces the decretogram (the inhoudelijke beschikking), and CJIB's cel produces the executograms (oplegging-event, betaling-event, intrekking-event) on behalf of the opdrachtgever. Today these flows are not federated in the chronolexosfeer sense; that is exactly what RFC-022 enables.

| Opdrachtgever | Type vordering | Grondslag (best available) | Chronolex-rol | Maps to RegelRecht schema? | FCID category |
|---|---|---|---|---|---|
| OM | Strafbeschikking, schadevergoeding, ontneming | Sv 257a, Sr 36f, Sr 36e | OM = decretogram-cel; CJIB = executogram-cel | New `decision_type: STRAFBESCHIKKING` | Algemeen + Verhoging + Administratiekosten |
| DUO | Studieschuld, lesgeld arrears | [Wet studiefinanciering 2000 (BWBR0011453)](https://wetten.overheid.nl/BWBR0011453), Les- en cursusgeldwet (BWBR0005407) `[BWB-IDs to verify]` | DUO = decretogram-cel; CJIB = executogram-cel | New `decision_type: INCASSO_BESCHIKKING` | Algemeen + Rente |
| CAK | Eigen bijdrage Wmo/Wlz, wanbetalersregeling Zvw | [Zvw art. 18a–18d (BWBR0018450)](https://wetten.overheid.nl/BWBR0018450) | CAK = decretogram-cel; CJIB = executogram-cel | Partial: `wet_langdurige_zorg` already in corpus | Algemeen |
| UWV | Terugvorderingen WW/WIA/Wajong | Sectoral employee-insurance acts + Awb titel 4.4 | UWV = decretogram-cel; CJIB = executogram-cel | New `decision_type: BETALINGSVERPLICHTING` | Algemeen + Rente |
| RVO | Subsidie-terugvorderingen, agrarische boetes | Awb 4.4 + sectoral LNV/EZK acts | RVO = decretogram-cel; CJIB = executogram-cel | New | Algemeen + Verhoging |
| NVWA | Bestuurlijke boetes voedselveiligheid, tabak, dier | [Warenwet](https://wetten.overheid.nl/BWBR0001969), Wet dieren, Tabaks- en rookwarenwet | NVWA = decretogram-cel; CJIB = executogram-cel | New `decision_type: BESTUURLIJKE_BOETE` | Algemeen + Verhoging |
| RDW / e-Tol | Onverzekerd voertuig, MRB-schorsing | [WAM art. 30/34](https://wetten.overheid.nl/BWBR0002326), Wet MRB | RDW = decretogram-cel; CJIB = executogram-cel | New | Algemeen + Verhoging |
| NEa | Emissiehandel-boetes | [Wet milieubeheer hoofdstuk 18 (BWBR0003245)](https://wetten.overheid.nl/BWBR0003245) | NEa = decretogram-cel; CJIB = executogram-cel | New | Algemeen + Verhoging |
| Inspectie JenV | Bestuurlijke boetes | Per sectoral act `[onzeker]` | Inspectie = decretogram-cel; CJIB = executogram-cel | New | Algemeen |
| RDI | Telecom-boetes | [Telecommunicatiewet (BWBR0009950)](https://wetten.overheid.nl/BWBR0009950) | RDI = decretogram-cel; CJIB = executogram-cel | New | Algemeen + Verhoging |
| ATKM | Boete tot 6e categorie of 4% jaaromzet | Uitvoeringswet Verordening terroristische online-inhoud art. 12–13 | ATKM = decretogram-cel; CJIB = executogram-cel | New | Algemeen + Verhoging |
| DFEI | Diverse | `[onzeker — CRI lists "Dienst Financiële en Economische Integriteit"]` | unclear | New | Algemeen |
| Gemeenten | Via mandaat | Gemeentewet + lokale verordeningen | Gemeente = decretogram-cel; CJIB = executogram-cel | Partial: `participatiewet` and `apv_erfgrens` exist | Varies |

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

CJIB's own statutory base (Wahv, Sv 257a, Sr 36e/36f, Wet USB, Awb 4.4) and the sectoral acts behind every namens-uitvoering would each be a lexogram in a RegelRecht corpus. None of them are in the corpus today. RFC-022 does not deliver these; it makes them representable.

### Decretogrammen (beschikkingen)

The current `produces.decision_type` enum has nine values: TOEKENNING, AFWIJZING, GOEDKEURING, GEEN_BESLUIT, ALGEMEEN_VERBINDEND_VOORSCHRIFT, BELEIDSREGEL, VOORBEREIDINGSBESLUIT, ANDERE_HANDELING, AANSLAG. None describe the financial-enforcement domain.

[RFC-022 §2](/rfcs/rfc-022) proposes the following additions, which cover the CJIB landscape:

- `BETALINGSVERPLICHTING` — generic financial obligation imposed by a bestuursorgaan
- `STRAFBESCHIKKING` — criminal-law settlement under Sv 257a
- `BESTUURLIJKE_BOETE` — sectoral administrative fines (NVWA, NEa, RDI, ATKM, etc.)
- `INCASSO_BESCHIKKING` — explicit collection order, typically after non-payment of an underlying claim
- `INTREKKING_BESCHIKKING` — withdrawal or annulment of an earlier decision

`legal_character: BESCHIKKING` already covers all of these.

### Executogrammen (factual events)

These do not belong in `produces` at all. A payment received, a kwijtschelding executed, a deurwaarder triggered: these are not regulation outputs. RFC-022 §1 introduces `corpus/executogram/` as a separate top-level corpus directory for executogram-stream definitions. The most natural CJIB executograms are:

- `betaling_ontvangen` — maps to FCID `BetalingVerwerkt`
- `kwijtschelding_verleend` — maps to FCID `BetalingsverplichtingIngetrokken` with a kwijtschelding-reden
- `deurwaardertraject_gestart` — maps to FCID `BetalingsverplichtingOpgelegd` if it creates a separate verplichting for kosten; or is recorded internally without FCID emission if it is procedural

Each of these is a registratie, not an interpretatie. Putting them under `decision_type` would conflate registratie with besluit and erase a juridically meaningful distinction.

## Mapping onto MBO / FCID

The full mapping from RegelRecht decretograms and executograms to [FCID](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/) events lives in the [MBO/FCID integration document](/integrations/mbo-fcid). In short: each FCID `event_type` corresponds to either a decretogram (the three `*Opgelegd` and `*Ingetrokken` types) or an executogram (`BetalingVerwerkt`). Categories (Algemeen, Administratiekosten, Verhoging, Rente) are orthogonal to the chronolex-type and encoded via the `outbound_category` field that RFC-022 §3 adds for integration use.

## Open questions and data gaps

The following items could not be verified from public sources and need input from CJIB or its opdrachtgevers:

1. **Complete CJIB portfolio**. Internal USB lists exist but are not publicly indexed. Anne's CJIB contacts can confirm completeness.
2. **CJIB-side FCID adoption status**. Which CJIB regulations already emit FCID (in pilot or production), and which version? FCID v4.2.0 is the latest standard as of mei 2026.
3. **DFEI scope**. CRI lists "Dienst Financiële en Economische Integriteit", but its exact handover to CJIB is unclear.
4. **Cel topology at CJIB**. Does CJIB run one cel per opdrachtgever, one per regulation type, or one centrally? RFC-009 supports either, but the choice has consequences for chronicle ordering and signing keys.
5. **Bilateral convenants** that are not published in Staatscourant: there may be additional opdrachtgevers not surfaced through public sources.
6. **DNA-V cost recovery grondslag**. BWBR0017212 does not directly couple cost recovery to CJIB; this likely runs via beleidsregels that need verification.
7. **Per-opdrachtgever BWB-IDs** marked `[onzeker]` in the table — to confirm against wetten.overheid.nl in detail.
8. **Wet gegevensboekhouding interaction**. Nieuwland §7.3.2 sketches a Wet gegevensboekhouding that would put the executogram-side recording on a statutory footing. CJIB's current grondslag for keeping its own chronicle is implicit in Awb 4.4 + sectoral acts; an explicit statute would change the picture.
