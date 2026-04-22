# Audit — HHNK-leidraad invordering waterschapsbelastingen art 26

**Wet**: Leidraad invordering waterschapsbelastingen HHNK 2026 (vervangt 2023)
**`$id`**: `leidraad_invordering_waterschapsbelastingen_hhnk`
**Type**: beleidsregel College van Dijkgraaf en Heemraden HHNK (Awb 4:81)
**Wet-URL (CVDR)**: https://lokaleregelgeving.overheid.nl/CVDR756485/1#artikel_26
**Wet-URL (WSB)**: https://zoek.officielebekendmakingen.nl/wsb-2026-2845
**YAML-bestand 2026**: `corpus/regulation/nl/waterschaps_verordening/hhnk/leidraad_invordering_waterschapsbelastingen/2026-02-07.yaml`
**YAML-bestand 2023**: `corpus/regulation/nl/waterschaps_verordening/hhnk/leidraad_invordering_waterschapsbelastingen/2023-01-01.yaml` *(legacy parallel, 2023-01-01 t/m 2026-02-06)*
**Laatste review**: —
**Reviewer(s)**: —

---

## 2026-wijzigingen t.o.v. 2023 (bron: WSB-2026-2845)

**Authentieke bron**: Waterschapsblad wsb-2026-2845, publicatie 2026-02-06,
inwerkingtreding 2026-02-07 ([metadata.xml](https://zoek.officielebekendmakingen.nl/wsb-2026-2845/metadata.xml)).

**Inhoudelijke wijzigingen in art 26**:

| Subartikel | 2023-tekst | 2026-tekst | Impact op MR |
|---|---|---|---|
| 26.1.1 | Terugbetaling bij reeds betaald, **max 3 maanden** | Terugbetaling **onbeperkt** ("bedrag waarvoor kwijtschelding is verleend") | Geen — refund-logica niet in MR |
| 26.1.9 / 26.2.x (vermogen) | — | Schade-uitkering verzekeraar: **1 jaar buiten vermogen**; smartengeld: **5 jaar buiten vermogen** | Niet in MR (vermogenstoets via URI art 12) |
| 26.2.x (studenten) | HO forfait **€67**, MBO **€60** | HO **€80**, MBO **€70** | Niet in MR (bedragen niet geëncodeerd) |
| 26.2.19 (normpremie Zvw) | Alleenstaand **€3/mnd**, echtg. **€50/mnd** | Alleenstaand **€47/mnd**, echtg. **€106/mnd** | Niet in MR (caller-parameter) |
| 26.3.8 (saneringsakkoord) | Looptijd **10 maanden** | Looptijd **12 maanden** | Niet in MR (ondernemers-deel) |
| 26.4.2 (herhaald verzoek) | Bezwaar = beroepschrift | Herschreven: administratief beroep, met onderscheid nieuwe feiten | Niet in MR (procedureel) |

**Totaal-impact op machine_readable art 26**: **NIHIL**. De kern-orchestrator
(vermogenstoets, betalingscapaciteit, OR-over-7 uitsluitingsgronden,
hoogte-berekening) is tekst-identiek aan 2023. MR-logica is 1-op-1 geport
naar `2026-02-07.yaml`. BDD blijft 95/95 groen onder `calculation_date: 2026-06-01`.

**Keten-wijzigingen buiten art 26** (art 25 — uitstel van betaling):
- 25.5.7 VERVALLEN per 2025-01-01 (bijzondere uitgaven)
- 25.5.11 VERVALLEN per 2025-07-01 (betalingsregeling > 10 mnd)
- **25.5a NIEUW**: verlengde betalingsregeling illiquide vermogen, **max 60 mnd**
- 25.5.8 verruimd: aflossingen aan derden kunnen worden meegenomen

Valt buiten kwijtschelding-flow (art 25 = uitstel, art 26 = kwijtschelding).

---

## Werkwijze

Art 26 is geschreven als één omvangrijk artikel met subsecties `26.1.x`,
`26.2.x`, `26.3.x`. Voor de machine-readable vertaling is alles dat direct
invloed heeft op de beschikking (ja/nee + bedrag) samengebracht in één
`machine_readable` block. Hieronder elk van de zes outputs, gekoppeld aan
de wettekst waaruit ze voortkomen.

Symbolische kortschriften die in formules worden gebruikt:

- `G` = `verzoek_ingediend`
- `S` = `belastingsoort_in_scope` (via verordening art 1)
- `O` = `ondernemer_komt_in_aanmerking` (via verordening art 5)
- `U` = `uitgesloten_van_kwijtschelding` (OR over 7 weigergronden, inline)
- `R` = `raw_hoogte_kwijtschelding > 0`
- `V` = `vermogen_bedrag` (via URI art 12)
- `B` = `betalingscapaciteit` (via URI art 13)
- `g₁..g₇` = de zeven weigergronden (parameters)

---

## Output 1 — `beleidsregel_vereist_verzoek`

**Wettekst-excerpt** — uit HHNK-leidraad 26.1.2 *"Het indienen van een verzoek om kwijtschelding"*:

> "Het verzoek om kwijtschelding moet worden ingediend bij de ontvanger
> waaronder de belastingschuldige ressorteert, via digitale weg door
> middel van 'Mijn loket' op www.hhnk.nl (zie verder artikel 26.7), of
> op een daartoe ingesteld kwijtscheldingsformulier."

🔗 https://lokaleregelgeving.overheid.nl/CVDR756485/1#artikel_26 (subsectie 26.1.2)
🔗 Authentieke bron 2026: https://zoek.officielebekendmakingen.nl/wsb-2026-2845

| | |
|---|---|
| **Formule** | `true` (constant) |
| **Interpretatie** | De beleidsregel stelt zonder uitzondering dat een verzoek vereist is voor beoordeling. Ambtshalve terugbetaling (26.1.1) wordt apart afgehandeld en staat als `untranslatables`. |
| **YAML-locatie** | `articles[…].machine_readable.actions[0]` |

**Review**:

- ☐ Formule dekt 26.1.2 volledig: beoordeling start pas na indienen van een verzoek.
- ☐ Ambtshalve terugbetaling (26.1.1) is terecht als uitzondering (untranslatable) behandeld, niet als onderdeel van dit boolean.
- ☐ Geen andere bepalingen in art 26 die dit gate-karakter relativeren.
- ☐ **2026-wijziging**: tekst 26.1.2 inhoudelijk ongewijzigd t.o.v. 2023 — gate-karakter blijft identiek.

---

## Output 2 — `uitgesloten_van_kwijtschelding`

**Wettekst-excerpt** — uit HHNK-leidraad 26.1.9 *"Wanneer wordt geen kwijtschelding verleend"*:

> "Er wordt geen kwijtschelding verleend als:
> • de gevraagde gegevens voor de beoordeling van het verzoek niet, niet
>   volledig, onjuist, of niet op het door de ontvanger uitgereikte
>   formulier zijn verstrekt;
> • […onevenredige verhouding uitgaven/inkomen zonder opheldering …]
> • de belastingschuldige heeft nagelaten de vereiste aangifte in te dienen;
> • een bezwaarschrift tegen de hoogte van de belastingaanslag in
>   behandeling is bij de heffingsambtenaar, dan wel een beroepschrift
>   […] bij de rechtbank of (in hoger beroep) bij het gerechtshof/Hoge Raad;
> • voor de desbetreffende belastingaanslag zekerheid is gesteld;
> • er sprake is van meer dan één belastingschuldige;
> • een derde nog voor de belastingschuld aansprakelijk kan worden gesteld;
> • het aan de belastingschuldige kan worden toegerekend dat de
>   belastingaanslag niet kan worden voldaan […];
> • [overige gronden waaronder de schuldsaneringsregeling]."

🔗 https://lokaleregelgeving.overheid.nl/CVDR756485/1#artikel_26 (subsectie 26.1.9)

| | |
|---|---|
| **Formule** | `U = g₁ₐ ∨ g₁ᵦ ∨ g₁ᵧ ∨ g₂ ∨ g₃ ∨ g₄ ∨ g₅ ∨ g₆ ∨ g₇` |
| **Gronden** | g₁ₐ = *aanvraag_gegevens_onvolledig_of_onjuist* (26.1.9 bullet 1) · g₁ᵦ = *onevenredige_uitgaven_inkomen_onopgehelderd* (bullet 2) · g₁ᵧ = *aangifte_niet_ingediend* (bullet 3) · g₂ = *bezwaar_of_beroep_aanhangig* · g₃ = *zekerheid_gesteld* · g₄ = *meerdere_belastingschuldigen* · g₅ = *derde_aansprakelijk_gesteld* · g₆ = *verwijtbaarheid_belastingschuld* · g₇ = *in_faillissement_of_surseance_zonder_akkoord* |
| **Splitsing g₁ (refactor 2026-04-22)** | De 2023-MR had 1 parameter `gegevens_onvolledig_of_onjuist` voor bullets 1+2+3. Die bundelde drie gronden met verschillende herstelroutes en discretie-ruimte. Bij audit 2026-04-22 gesplitst in g₁ₐ/g₁ᵦ/g₁ᵧ zodat ontvanger in beschikking (26.1.6 motivering) per grond kan onderbouwen en herstelroute passend kan wijzen (2 weken herstel / opheldering / aangifte-spoor). |
| **Interpretatie** | OR — één grond is genoeg om kwijtschelding te weigeren. Gronden zijn cumulatief in de tekst, in formule identiek. |
| **YAML-locatie** | `articles[…].machine_readable.actions[1]` |

**Telling wettekst 26.1.9**: 13 bullets in hoofdlijst (waarvan bullet 11 placeholder "niet van toepassing") + 2 eindblok-gronden. Dus **12 echte gronden + 2 eindblok = 14 totaal**. MR dekt **9** via g₁ₐ/g₁ᵦ/g₁ᵧ/g₂..g₇.

**Niet gedekt** (5 echte gronden, blijven untranslatable): bullet 12 (nadere voorwaarden niet nagekomen), bullet 13 (gem. sociale dienst vergoedt), eindblok 1 (wisselende inkomens-verwachting), eindblok 2 (verbeteringsverwachting). Plus bullet 11 als placeholder.

**Review**:

- ☐ 9 van 12 echte gronden in formule (75% dekking hoofdlijst) + 0 van 2 eindblok-gronden (0%). Acceptabel, of moet coverage hoger?
- ☐ **Pre-workshop voorstel 2026-04-22 (nog te bekrachtigen)**: "Onevenredige verhouding uitgaven/inkomen zonder opheldering" (bullet 2) kreeg eigen parameter `g₁ᵦ = onevenredige_uitgaven_inkomen_onopgehelderd`. Voorgestelde juridische reden: bullet 2 bevat discretie ("naar het oordeel van de ontvanger") en andere herstelroute (opheldering in plaats van formulier-herstel). **Experts moeten bevestigen of terugdraaien.**
- ☐ **Pre-workshop voorstel 2026-04-22 (nog te bekrachtigen)**: "Nagelaten aangifte" (bullet 3) kreeg eigen parameter `g₁ᵧ = aangifte_niet_ingediend`. Voorgestelde reden: herstelroute via inspecteur (aanslag eerst correct laten vaststellen), niet via kwijtscheldings-formulier. **Experts moeten bevestigen of terugdraaien.**
- ☐ "Toerekenbaarheid" bullet 8: subpunten a, c, d, e horen bij g₆ (subpunten b, f, g, h staan als "niet van toepassing" in de wettekst). Geen eigen parameter per subpunt nodig?
- ☐ **2026-wijziging**: opsomming gronden in 26.1.9 tekst-identiek aan 2023 — MR blijft geldig zonder aanpassing.
- ☐ **2026-nieuw (26.2.x)**: vermogensvrijstelling schade-uitkering (1 jaar) en smartengeld (5 jaar) — niet in MR (vermogenstoets is via URI art 12). Accepteerbaar dat de vermogen-*inhoud* niet in HHNK-MR wordt herberekend?

**Open**:

- Akkoord-uitzonderingen op g₇ (surseance/faillissement/WSNP): bullets 9+10 van 26.1.9 noemen resp. art 138/252 FW en art 329 FW akkoorden. Huidige parameter `in_faillissement_of_surseance_zonder_akkoord` vangt dit met één boolean — caller bepaalt of er een akkoord is. Klopt deze modellering?

---

## Output 3 — `aanwendbare_betalingscapaciteit`

**Wettekst-excerpt** — uit HHNK-leidraad art 26 inleiding (paragraaf voor 26.1), beleidsregel-overname uit URI art 11:

> "Kwijtschelding wordt verleend voor:
> a. het gehele op de belastingaanslag openstaande bedrag indien geen
>    vermogen en geen betalingscapaciteit aanwezig is;
> b. het openstaande bedrag van de belastingaanslag dat resteert nadat:
>    1°. het aanwezige vermogen is aangewend ter voldoening van de
>        belastingaanslag;
>    2°. *ten minste 80 percent van de betalingscapaciteit is aangewend*"

🔗 Originele wetbron: https://wetten.overheid.nl/BWBR0004766/2026-01-01#Artikel11

| | |
|---|---|
| **Formule** | `aanwendbare_bc = 0.8 × B` |
| **Bron B** | `source:` → `uitvoeringsregeling_invorderingswet_1990` output `betalingscapaciteit` (URI art 13) |
| **Interpretatie** | HHNK-leidraad neemt de 80%-regel uit URI art 11 letterlijk over. Het exacte minimum is "ten minste 80%", in de praktijk exact 80%. |
| **YAML-locatie** | `articles[…].machine_readable.actions[2]` |

**Review**:

- ☐ Percentage 80% correct gemodelleerd (0.8).
- ☐ "Ten minste 80%" betekent in uitvoering = 80% (geen hogere aanwending tenzij apart afgesproken).
- ☐ Bron `B` via URI art 13 correct — HHNK-leidraad verwijst in bredere tekst (o.a. art 27) naar art 13.

---

## Output 4 — `raw_hoogte_kwijtschelding`

**Wettekst-excerpt** — zelfde passage als output 3:

> "Kwijtschelding wordt verleend voor:
> a. het gehele op de belastingaanslag openstaande bedrag indien geen
>    vermogen en geen betalingscapaciteit aanwezig is;
> b. het openstaande bedrag van de belastingaanslag dat resteert nadat:
>    1°. het aanwezige vermogen is aangewend ter voldoening van de
>        belastingaanslag;
>    2°. ten minste 80 percent van de betalingscapaciteit is aangewend;
> een en ander onverminderd het bepaalde in artikel 8, artikel 17 en
> artikel 18."

| | |
|---|---|
| **Formule** | `raw = max(0, aanslag − V − 0.8·B)` |
| **Bron V** | `source:` → URI art 12 (vermogen-bedrag met Leidraad 2008 26.2.3 auto-overrule) |
| **Bron B** | zie output 3 |
| **Interpretatie** | `max(0, …)` dekt punt a én b in één formule: als V + 0.8·B ≥ aanslag, is raw = 0 (niets resteert). Is V + 0.8·B = 0 en aanslag > 0, dan raw = aanslag (gehele bedrag, dekt punt a). |
| **YAML-locatie** | `articles[…].machine_readable.actions[3]` |

**Review**:

- ☐ Subtract-volgorde correct: *aanslag − vermogen − aanwendbare_betalingscap*, niet omgekeerd.
- ☐ MAX met 0 dekt zowel scenario a (niets aanwezig → heel bedrag kwijt) als b (gedeeltelijke kwijtschelding).
- ☐ Artikelen 8, 17, 18 URI — zijn deze voldoende afgedekt elders?
  - Art 8 URI: scope uitsluitingen, belastingsoort-specifiek → raakt `belastingsoort_in_scope` (output S)
  - Art 17 URI: €136-drempel — aparte URI machine_readable, nu niet gebruikt in deze keten; klopt dat HHNK dat niet meeweegt?
  - Art 18 URI: andere regeling; niet relevant voor kwijtschelding.

**Open**:

- URI art 17 (€136-drempel aflossingen op andere schulden) staat wel machine-readable maar is niet geketend in leidraad-26. Moet dit alsnog als extra AND-gate?

---

## Output 5 — `kan_kwijtschelding_worden_verleend`

**Wettekst-excerpt** — impliciete samenstelling van het hele artikel 26:

> Art 26 (inleiding): "De ontvanger verleent gehele of gedeeltelijke
> kwijtschelding als de belastingschuldige niet in staat is anders dan
> met buitengewoon bezwaar de belastingaanslag te betalen."

met expliciete gates uit:
- **26.1.2** — verzoek vereist
- **26.1.9** — geen uitsluitingsgrond
- **verordening art 1** — binnen scope (6 heffingen)
- **verordening art 5** — zakelijke aanslag van ondernemer valt buiten regeling

| | |
|---|---|
| **Formule** | `kan_kwijtschelding ≡ G ∧ S ∧ O ∧ ¬U ∧ R` |
| **Operanden** | G = verzoek (26.1.2) · S = scope (verord 1) · O = ondernemer-regel (verord 5) · U = uitsluitingsgrond (26.1.9) · R = raw_hoogte > 0 (art 26 materieel) |
| **Interpretatie** | AND van 5 onafhankelijke toetsen. Commutatief, dus volgorde in YAML is geen semantische keuze maar leesbaarheid. |
| **YAML-locatie** | `articles[…].machine_readable.actions[4]` |

**Review**:

- ☐ Alle vijf operanden aanwezig, niets meer, niets minder.
- ☐ Negatie bij U correct (`¬U`, niet `U`).
- ☐ `R` als boolean afgeleid uit `raw_hoogte > 0` — dekt scenario a en b uit art 26-tekst.
- ☐ Geen impliciete gate gemist (bv. bsn geldig, aanslag bestaat) — die zitten elders in de keten of zijn parameter-level.

---

## Output 6 — `hoogte_kwijtschelding`

**Wettekst-excerpt** — impliciet: na goedkeuring volgt het bedrag uit art 26 onderdelen a/b; bij afwijzing 0 (geen bepaling nodig, "er wordt geen kwijtschelding verleend" = 0).

| | |
|---|---|
| **Formule** | `hoogte = kan_kwijtschelding ? raw_hoogte : 0` |
| **Interpretatie** | IF-toewijzing. Separate van de kan-gate zodat je het ruwe bedrag én de gate-uitslag kan inspecteren in de trace. |
| **YAML-locatie** | `articles[…].machine_readable.actions[5]` |

**Review**:

- ☐ IF structuur correct: bij `kan = true` het raw bedrag, anders 0.
- ☐ Geen andere default (bv. -1 voor "onbepaald") nodig.
- ☐ Eurocent-unit consistent met aanslag en alle downstream systemen.

---

## Niet-getranslateerd (`untranslatables`, accepted)

Wat in art 26 staat maar bewust niet in de formule is vertaald:

| Subsectie | Wettekst-kern | Reden | Review |
|---|---|---|---|
| **26.1.1** Ambtshalve terugbetaling (2026) | "De ontvanger verleent ook kwijtschelding van belastingaanslagen die al zijn betaald, als (…) het verzoek [wordt] ingediend binnen drie maanden nadat de (laatste) betaling op de belastingaanslag heeft plaatsgevonden (…). Als de ontvanger het verzoek toewijst, betaalt hij de belastingschuldige het bedrag terug waarvoor kwijtschelding is verleend." | "Onder omstandigheden die aanleiding zouden hebben gegeven" = case-specifieke beoordeling; refund-bedrag volgt direct uit hoogte_kwijtschelding. **2026-wijziging**: de 2023-beperking "max 3 maanden voorafgaand aan verzoek" is in 2026 vervallen — refund is nu het volledige kwijtscheldingsbedrag. | ☐ Accepteerbaar als out-of-scope voor generieke engine (procedure-gate, indiening-drempel blijft 3 maanden) ☐ **2026**: verwijdering refund-cap niet in MR vertaald (geen refund-output) — acceptabel |
| **26.1.3** Herstelprocedure | Twee-weken-herstelmogelijkheid bij onvolledig formulier | Vereist proces-state (hersteltbrief verzonden? ontvangen?) die niet in parameters zit. | ☐ Accepteerbaar |
| **26.2.13** Bijzondere bijstand | "Uitkeringen in bijzondere bijstand bestemd voor specifieke kosten worden niet als inkomen in aanmerking genomen; bijzondere bijstand voor personen < 21 jr wél." | Case-analyse naar doel van de uitkering vereist; niet als generieke formule vast te leggen. | ☐ Accepteerbaar |
| **26.2.20** Onderhoud gezinsleden buitenland | Beoordeling van netto-besteedbaar inkomen bij buitenland-onderhoud | Individuele weging van bedragen, wettelijkheid, bewijslast. | ☐ Accepteerbaar |

---

## Externe override die wel doorwerkt

Niet onderdeel van art 26 zelf, maar aangehaald voor volledigheid:

| Override | Bron | Doel | Effect |
|---|---|---|---|
| `overrides:` op URI art 12 auto-drempel | Leidraad Invordering 2008 art 26.2.3 (rijks) | `auto_als_vermogen` | Auto-drempel wordt €3.350 i.p.v. URI's €2.269 — lex pro cive, werkt automatisch door op HHNK omdat leidraad-26 via URI art 12 sourcet |

**Review**:

- ☐ HHNK accepteert de rijks-lex-pro-cive op de auto-drempel (toepassen op HHNK-burgers).
- ☐ Of: HHNK wil een eigen auto-drempel en moet dan zelf een machine_readable artikel 26.2.3 krijgen dat de URI-override overschrijft — op dit moment niet.

---

## Open punten voor workshop

1. **URI art 17 €136-drempel** — niet gekoppeld aan leidraad-26. Moet wel of niet?
2. **Toerekenbaarheid-subpunten** (26.1.9) — alle drie (a) opzet, (b) vervallen, (c) niet-aanwenden teruggave — vangt `verwijtbaarheid_belastingschuld` parameter dit?
3. **Schuldsaneringsregeling-akkoord** — uitzondering op faillissement-grond — correct gemodelleerd als `in_faillissement_of_surseance_zonder_akkoord`?
4. **Vermogens- vs. betalingscap-volgorde** — art 26 zegt expliciet "nadat vermogen is aangewend én 80% betalingscap" — onze subtract doet beide tegelijk in één MAX. Semantisch equivalent, maar review-waardig.
5. **"Onverminderd art 8, 17, 18 URI"** — deze clause uit de art-26-tekst: hoe werken die 3 URI-artikelen door? Art 8 raakt scope, art 17 is niet gekoppeld, art 18 is niet relevant.
6. **2026: normpremie Zvw (26.2.19) sterk verhoogd** (€3→€47, €50→€106). Nu caller-parameter; moet dit niet een `source:` naar Zvw art 41 of een eigen bedrag-output worden in HHNK-leidraad 26?
7. **2026: student-forfaits** (26.2.x: HO €67→€80, MBO €60→€70). Niet in MR; impliciet via URI art 14 netto-besteedbaar. Moet HHNK zelf studentenforfait hardcoden of via Participatiewet source-en?
8. **2026: vermogensvrijstelling schade-uitkering en smartengeld** (1 jr / 5 jr buiten vermogen). Valt dit in vermogenstoets URI art 12 — en zo ja, moet URI art 12 een open_term "niet-tellende_vermogensbestanddelen" krijgen die HHNK vult?
9. **2026: art 25.5a nieuw (verlengde betalingsregeling illiquide vermogen, 60 mnd)** — uitstel, valt strict buiten kwijtschelding. Wel vermelden als keten-gerelateerd.
10. **2026: saneringsakkoord 10→12 maanden (26.3.8)** — raakt alleen ondernemers-saneringsflow, niet in MR. Acceptabel?
11. **2026: 26.4.2 herhaald verzoek herschreven** — procedureel, niet in MR. Moet er een `verzoek_herhaald_zonder_nieuwe_feiten` uitsluitingsgrond bij?

---

## Vervolg

Na workshop-review: vink ☐ om naar ☑ en schrijf eventuele afwijkingen
op in een nieuw "Bevindingen"-blok onderaan. Commit per review-ronde met
`docs(audit): review HHNK-leidraad art 26 ronde N`.
