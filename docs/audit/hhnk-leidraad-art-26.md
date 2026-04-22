# Audit — HHNK-leidraad invordering waterschapsbelastingen art 26

**Wet**: Leidraad invordering waterschapsbelastingen HHNK 2026
**`$id`**: `leidraad_invordering_waterschapsbelastingen_hhnk`
**Type**: beleidsregel College van Dijkgraaf en Heemraden HHNK (Awb 4:81)
**Wet-URL**: https://lokaleregelgeving.overheid.nl/CVDR756485/1#artikel_26
**YAML-bestand**: `corpus/regulation/nl/waterschaps_verordening/hhnk/leidraad_invordering_waterschapsbelastingen/2023-01-01.yaml`
**Laatste review**: —
**Reviewer(s)**: —

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

🔗 https://lokaleregelgeving.overheid.nl/CVDR756485/1#artikel_26

| | |
|---|---|
| **Formule** | `true` (constant) |
| **Interpretatie** | De beleidsregel stelt zonder uitzondering dat een verzoek vereist is voor beoordeling. Ambtshalve terugbetaling (26.1.1) wordt apart afgehandeld en staat als `untranslatables`. |
| **YAML-locatie** | `articles[…].machine_readable.actions[0]` |

**Review**:

- ☐ Formule dekt 26.1.2 volledig: beoordeling start pas na indienen van een verzoek.
- ☐ Ambtshalve terugbetaling (26.1.1) is terecht als uitzondering (untranslatable) behandeld, niet als onderdeel van dit boolean.
- ☐ Geen andere bepalingen in art 26 die dit gate-karakter relativeren.

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
| **Formule** | `U = g₁ ∨ g₂ ∨ g₃ ∨ g₄ ∨ g₅ ∨ g₆ ∨ g₇` |
| **Gronden** | g₁ = *gegevens_onvolledig_of_onjuist* · g₂ = *bezwaar_of_beroep_aanhangig* · g₃ = *zekerheid_gesteld* · g₄ = *meerdere_belastingschuldigen* · g₅ = *derde_aansprakelijk_gesteld* · g₆ = *verwijtbaarheid_belastingschuld* · g₇ = *in_faillissement_of_surseance_zonder_akkoord* |
| **Interpretatie** | OR — één grond is genoeg om kwijtschelding te weigeren. Gronden zijn cumulatief in de tekst, in formule identiek. |
| **YAML-locatie** | `articles[…].machine_readable.actions[1]` |

**Review**:

- ☐ Alle zeven gronden uit 26.1.9 gedekt.
- ☐ Grond "onevenredige verhouding uitgaven/inkomen zonder opheldering" valt onder `gegevens_onvolledig_of_onjuist` of heeft eigen parameter nodig?
- ☐ Grond "nagelaten aangifte" gedekt onder *gegevens_onvolledig_of_onjuist* of apart?
- ☐ "Toerekenbaarheid" (verwijtbaarheid): subpunten a-c uit de wettekst horen bij g₆; geen eigen parameter nodig?
- ☐ Geen achtste grond over het hoofd gezien.

**Open**:

- De wettekst noemt ook ondermeer "schuldsaneringsregeling met akkoord" als *uitzondering op* de faillissementsgrond — nu alleen *zonder* akkoord gemodelleerd. Klopt dit?

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
| **26.1.1** Ambtshalve terugbetaling | "De ontvanger verleent ook kwijtschelding van bedragen die op belastingaanslagen zijn betaald, als (…) het verzoek om kwijtschelding [wordt] ingediend binnen drie maanden nadat de (laatste) betaling op de belastingaanslag heeft plaatsgevonden." | Tijdsvenster-toetsing + "onder omstandigheden die aanleiding zouden hebben gegeven" = case-specifieke interpretatie. | ☐ Accepteerbaar als out-of-scope voor generieke engine |
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

---

## Vervolg

Na workshop-review: vink ☐ om naar ☑ en schrijf eventuele afwijkingen
op in een nieuw "Bevindingen"-blok onderaan. Commit per review-ronde met
`docs(audit): review HHNK-leidraad art 26 ronde N`.
