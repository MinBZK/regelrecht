# CJIB-uitvoeringslandschap

This page surveys what the Centraal Justitieel Incassobureau (CJIB) actually does: which regulations it executes itself, which it executes on behalf of other organizations, and the policy framework that governs both. It is background material for [RFC-009 (Multi-Organisation Execution)](/rfcs/rfc-009) and [RFC-019 (RegelRecht × MBO)](/rfcs/rfc-019). It is not normative.

CJIB is a zelfstandig bestuursorgaan (ZBO) under the Ministry of Justice and Security. It is the central financial-enforcement hub of the Dutch government: the place where almost every administrative-law and criminal-law financial obligation eventually lands when a citizen does not pay voluntarily. As of 2026 it executes for at least 15 opdrachtgevers, ranging from the OM to a sectoral inspectorate like NEa.

This matters for RegelRecht because the corpus today (23 regulations) covers fiscal and social domains exclusively. Nothing in it produces a `BETALINGSVERPLICHTING` or a `STRAFBESCHIKKING`. Bridging RegelRecht to the CJIB world is the precondition for two things: making incasso-domain laws executable, and connecting RegelRecht to [Mijn Betaaloverzicht (MBO, formerly Vorderingenoverzicht Rijk)](https://www.eenoverheidsincasso.nl/onze-dienstverlening/vorderingenoverzicht-rijk) so a citizen sees both the obligation and its juridical provenance.

## CJIB's own statutory tasks

These are regulations where CJIB itself is the executing authority, either directly or by mandate of the Minister of Justice and Security.

| Regulation | Grondslag | BWB-ID | What it covers |
|---|---|---|---|
| Wahv (Wet Mulder) | Wet administratiefrechtelijke handhaving verkeersvoorschriften | [BWBR0004581](https://wetten.overheid.nl/BWBR0004581) | Administrative settlement of minor traffic offences |
| OM-strafbeschikking | Wetboek van Strafvordering art. 257a–257h | [BWBR0001903](https://wetten.overheid.nl/BWBR0001903) | Settlement act by the public prosecutor, executed by CJIB |
| Schadevergoedingsmaatregel | Wetboek van Strafrecht art. 36f | [BWBR0001854](https://wetten.overheid.nl/BWBR0001854) | Damages awarded to victim, collected by CJIB |
| Voorschotregeling slachtoffer | Sr art. 36f lid 7 | [BWBR0001854](https://wetten.overheid.nl/BWBR0001854) | State advances payment after 8 months; CJIB recovers from offender |
| Ontnemingsmaatregel ("Pluk-ze") | Wetboek van Strafrecht art. 36e | [BWBR0001854](https://wetten.overheid.nl/BWBR0001854) | Confiscation of unlawfully obtained gains |
| Wet DNA-V | Wet DNA-onderzoek bij veroordeelden | [BWBR0017212](https://wetten.overheid.nl/BWBR0017212) | DNA sampling of convicted persons. Connection to cost recovery via beleidsregels `[onzeker]` |
| Tenuitvoerlegging strafrechtelijke beslissingen (Wet USB) | Wetboek van Strafvordering Boek 6 (in werking 2020-01-01) | [BWBR0001903](https://wetten.overheid.nl/BWBR0001903/Boek6) | Responsibility for execution shifted from OM to Minister; CJIB/AICE is operational chain coordinator |
| EU wederzijdse erkenning geldelijke sancties | Wet wederzijdse erkenning en tenuitvoerlegging geldelijke sancties en beslissingen tot confiscatie | [BWBR0022604](https://wetten.overheid.nl/BWBR0022604) | Cross-border recognition and collection of fines/confiscation orders |

## CJIB as executor for other organizations

CJIB collects for at least 15 opdrachtgevers. The legal basis is per-case: some are sectoral acts that designate the Minister of JenV or directly CJIB; others are mandate constructions under the Algemene wet bestuursrecht. The Clustering Rijksincasso (CRI) program, formalized via [eenoverheidsincasso.nl](https://www.eenoverheidsincasso.nl/onze-partners), structures this collaboration.

| Opdrachtgever | Type of vordering | Grondslag (best available) | Maps to RegelRecht schema? | FCID category |
|---|---|---|---|---|
| OM | Strafbeschikking, schadevergoeding, ontneming | Sv 257a, Sr 36f, Sr 36e | New `decision_type: STRAFBESCHIKKING` needed | Algemeen + Verhoging + Administratiekosten |
| DUO | Studieschuld, lesgeld arrears | [Wet studiefinanciering 2000 (BWBR0011453)](https://wetten.overheid.nl/BWBR0011453), Les- en cursusgeldwet (BWBR0005407) `[BWB-IDs to verify]` | New `decision_type: INCASSO_OPGELEGD` needed | Algemeen + Rente |
| CAK | Eigen bijdrage Wmo/Wlz, wanbetalersregeling Zvw | [Zvw art. 18a–18d (BWBR0018450)](https://wetten.overheid.nl/BWBR0018450) | Partial — `wet_langdurige_zorg` already in corpus | Algemeen |
| UWV | Terugvorderingen WW/WIA/Wajong | Sectoral employee-insurance acts + Awb titel 4.4 | New `decision_type: BETALINGSVERPLICHTING` | Algemeen + Rente |
| RVO | Subsidie-terugvorderingen, agrarische boetes | Awb 4.4 + sectoral LNV/EZK acts | New | Algemeen + Verhoging |
| NVWA | Bestuurlijke boetes voedselveiligheid, tabak, dier | [Warenwet](https://wetten.overheid.nl/BWBR0001969), Wet dieren, Tabaks- en rookwarenwet | New `decision_type: BESTUURLIJKE_BOETE` | Algemeen + Verhoging |
| RDW / e-Tol | Onverzekerd voertuig, MRB-schorsing | [WAM art. 30/34](https://wetten.overheid.nl/BWBR0002326), Wet MRB | New | Algemeen + Verhoging |
| NEa | Emissiehandel-boetes | [Wet milieubeheer hoofdstuk 18 (BWBR0003245)](https://wetten.overheid.nl/BWBR0003245) | New | Algemeen + Verhoging |
| Inspectie JenV | Bestuurlijke boetes | Per sectoral act `[onzeker]` | New | Algemeen |
| RDI | Telecom-boetes | [Telecommunicatiewet (BWBR0009950)](https://wetten.overheid.nl/BWBR0009950) | New | Algemeen + Verhoging |
| ATKM | Boete tot 6e categorie of 4% jaaromzet | Uitvoeringswet Verordening terroristische online-inhoud art. 12–13 | New | Algemeen + Verhoging |
| DFEI | Diverse | `[onzeker — CRI lists "Dienst Financiële en Economische Integriteit"]` | New | Algemeen |
| Gemeenten | Via mandaat | Gemeentewet + lokale verordeningen | Partial — `participatiewet` and `apv_erfgrens` exist | Varies |

The eight original CRI rijksorganisaties are: Belastingdienst, Dienst Toeslagen, CJIB, DUO, SVB, CAK, UWV, RVO. Since 2024 the Betalingsregeling Rijk has expanded to include NVWA, RDI, RDW e-Tol, Inspectie JenV, NEa, DFEI, and ATKM.

## What CJIB does not do

This is worth stating because lezers often misattribute. CJIB does **not** collect:

- **Gemeentelijke parkeerboetes**. These are municipal fiscal sanctions under the Wet Mulder regime; gemeenten collect via Cocensus, Belastingsamenwerkingen, or in-house.
- **Fiscale aanslagen** (income tax, BTW, etc.). The Belastingdienst executes its own invordering via the Invorderingswet 1990.
- **Gemeentelijke leges** and lokale heffingen — same as parkeerboetes.
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

## Mapping onto the RegelRecht schema

The current `produces.decision_type` enum (`schema/v0.5.2/schema.json`) has nine values: TOEKENNING, AFWIJZING, GOEDKEURING, GEEN_BESLUIT, ALGEMEEN_VERBINDEND_VOORSCHRIFT, BELEIDSREGEL, VOORBEREIDINGSBESLUIT, ANDERE_HANDELING, AANSLAG. None of them describe the financial-enforcement domain.

For the CJIB landscape, the following values are missing and proposed in RFC-019:

- `BETALINGSVERPLICHTING` — generic financial obligation imposed by a bestuursorgaan
- `STRAFBESCHIKKING` — criminal-law settlement under Sv 257a
- `BESTUURLIJKE_BOETE` — sectoral administrative fines (NVWA, NEa, RDI, ATKM, etc.)
- `INCASSO_OPGELEGD` — explicit collection order, typically after non-payment of an underlying claim
- `INCASSO_INGETROKKEN` — withdrawal or cancellation of a collection order
- `BETALING_VERWERKT` — external event marking a payment as received

`legal_character: BESCHIKKING` already covers all of these — the distinction is in `decision_type`, not in legal character.

## Mapping onto MBO / FCID

[Mijn Betaaloverzicht](https://www.eenoverheidsincasso.nl/onze-dienstverlening/vorderingenoverzicht-rijk) uses the [Financial Claims Information Document (FCID)](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/) standard. The standard is event-sourced with four event types and four categories.

Event types:

- `FinancieleVerplichtingOpgelegd` — initial financial obligation
- `BetalingsverplichtingOpgelegd` — payment obligation imposed
- `BetalingsverplichtingIngetrokken` — payment obligation withdrawn
- `BetalingVerwerkt` — payment processed

Categories (each FCID line gets one):

- Algemeen (primary obligation)
- Administratiekosten
- Verhoging
- Rente

The mapping between RegelRecht `decision_type` and FCID `event_type` is straightforward and worked out in RFC-019.

## Open questions and data gaps

The following items could not be verified from public sources and need input from CJIB or its opdrachtgevers:

1. **Complete CJIB portfolio**. Internal USB lists exist but are not publicly indexed. Anne's CJIB contacts can confirm completeness.
2. **Status of CJIB-side FCID adoption**. Which CJIB regulations already emit FCID (in pilot or production), and which version? FCID v4.2.0 is the latest standard as of mei 2026, but it is unclear which version each opdrachtgever currently uses.
3. **DFEI scope**. CRI lists "Dienst Financiële en Economische Integriteit" — what specifically does it pass to CJIB?
4. **Bilateral convenants** that are not published in Staatscourant: there may be additional opdrachtgevers not surfaced through public sources.
5. **DNA-V cost recovery grondslag**. The base act (BWBR0017212) does not directly couple cost recovery to CJIB; this likely runs via beleidsregels that need verification.
6. **Per-opdrachtgever BWB-IDs** marked `[onzeker]` in the table above — to confirm against wetten.overheid.nl in detail.
