# HHNK-expert workshop — art 26 kwijtschelding — 2026-04-23 (3 uur)

**Doel**: HHNK-leidraad art 26 (kwijtschelding van belastingen) item-voor-item doornemen met experts, machine-uitvoerbare logica valideren en gaps bepalen.

**Deelnemers**: —

**Te auditen**:
- YAML: `corpus/regulation/nl/waterschaps_verordening/hhnk/leidraad_invordering_waterschapsbelastingen/2026-02-07.yaml` — artikel 26
- Wettekst 2026: https://lokaleregelgeving.overheid.nl/CVDR756485/1#artikel_26
- Formules: `docs/audit/corpus/nl/waterschaps_verordening/hhnk/leidraad_invordering_waterschapsbelastingen/2026-02-07.formulas.md`

**Afbakening**: alleen HHNK-leidraad art 26. URI 1990 (vermogen/bc-berekening), HHNK-verordening (scope) en Leidraad 2008 (auto-drempel-override) raken we *alleen* aan waar art 26 er direct naar delegeert. Diepgaande audit van URI-keten = aparte sessie.

---

## Agenda

| Tijd      | Blok                       |
| --------- | -------------------------- |
| 0:00–0:15 | Kennismaking               |
| 0:15–0:45 | Kennis-oogst               |
| 0:45–1:15 | Scope + Venn-diagram       |
| 1:15–1:25 | Pauze                      |
| 1:25–2:25 | Walk-through art 26        |
| 2:25–2:50 | Beslispunten               |
| 2:50–3:00 | Afronding                  |

*Onderstaande uitwerkingen zijn draaiboek voor de facilitator — niet bedoeld om tijdens de sessie op scherm te tonen.*

---

## Deel 1 — Kennismaking + kennis-oogst (0:00–0:45)

Rondje (naam + rol + 1 zin "recente case"). Daarna brown paper + sticky-notes:
- 🟨 Kern van kwijtschelding
- 🟩 Wat loopt goed
- 🟥 Wat loopt stroef / vragen

5 min schrijven → 10 min plakken → 10 min clusteren.

---

## Deel 2 — Scope + Venn (0:45–1:15)

Venn-print op tafel. 3 dots per kleur per deelnemer: 🟢 "kennen we", 🔴 "blinde vlek". Dan F1–F6 doorlopen.

| #   | Vraag                                                                                        | Keuze           | [ ] |
| --- | -------------------------------------------------------------------------------------------- | --------------- | --- |
| F1  | 26.1 algemene uitgangspunten: volledig in MR-scope, of alleen gate-gronden 26.1.2 en 26.1.9? | vol / gates     | [ ] |
| F2  | 26.2 particulieren: alle 20 subartikelen meewegen, of kern (vermogen+bc+uitsluiting)?        | vol / kern      | [ ] |
| F3  | 26.3 ondernemers: in HHNK-MR of buiten scope?                                                | in / buiten     | [ ] |
| F4  | 26.4 administratief beroep: in MR of puur proces?                                            | in / uit        | [ ] |
| F5  | HHNK-toelichting (#11): alleen informatief zonder MR, akkoord?                               | ja / nee        | [ ] |
| F6  | Laag-D-wetten (BRP/Pw/Zvw/Wet IB): via `source:` of via caller-parameter?                    | source / caller | [ ] |

---

## Pauze (1:15–1:25)

---

## Walk-through protocol (Deel 3)

Per output — splitview formulas.md + workshop-md:

1. **Vertellen** (1 min): quote + formule voorlezen uit formulas.md
2. **Toetsen** (2–8 min): 3 micro-vragen
   - "Dekt de formule de wettekst?" → klik [x] of notitie
   - "Klopt de bron-verwijzing?" → klik [x] of notitie
   - "Kennen jullie een case die dit breekt?" → open case naar notitie
3. **Fixeren** (30s): "Output X: 2 vinkjes, 1 open case. Door."

Time-box: O1 = 7m · O2 = 25m · O3 = 12m · O4 = 10m · Buffer = 6m.

**O2 fijnmazig**: g₂..g₇ elk 2 min · g₁-splitsing via 1-2-4-all (7m) · 4 untranslatables bulk (3m).

---

## Claude-rol (YAML-orakel)

Alleen droog-feitelijke vragen: "wat zegt de YAML?", "wat zou de engine doen bij X?". Geen juridisch oordeel. Formulering: *"De YAML zegt: {quote}. Klopt dat met jullie praktijk?"* Bij onenigheid: trekt zich terug, punt parkeren.

---

## Deel 3 — 4 MR-outputs walk-through (1:25–2:25, 60 min)

*Gebruik het protocol hierboven per output. Laptop-splitview: formulas.md links, workshop-md rechts.*

Per output kijkt de groep naar: (a) de wettekst-quote, (b) de boolean-formule, (c) de gaps.

### Output 1 — `beleidsregel_vereist_verzoek ≡ true`

**Wettekst 26.1.2**: *"Het verzoek om kwijtschelding moet worden ingediend bij de ontvanger waaronder de belastingschuldige ressorteert, via digitale weg door middel van 'Mijn loket' op www.hhnk.nl (zie verder artikel 26.7), of op een daartoe ingesteld kwijtscheldingsformulier."*

- [ ] Formule = `true` (constant) dekt de intentie: elke beoordeling start bij een verzoek. Notitie: —
- [ ] 26.1.1 ambtshalve terugbetaling — in 2026 geen 3-maanden-refund-cap meer, volledig kwijtscheldingsbedrag terug. Staat nu als untranslatable. Acceptabel? Notitie: —
- [ ] 26.7 "Mijn loket" verwijzing — heeft dat gevolgen voor indienings-kanaal-check in MR? Notitie: —

### Output 2 — `uitgesloten_van_kwijtschelding` = OR van 9 gronden

**Wettekst 26.1.9**: 13 bullets in hoofdlijst (waarvan 1 placeholder "niet van toepassing") + 2 eindblok-gronden. Dus **12 echte gronden + 2 eindblok**.

Formule dekt **9 gronden** (bullets 1+2+3 zijn in een **pre-workshop voorstel** gesplitst — zie bekrachtiging hieronder):

- g₁ₐ = `aanvraag_gegevens_onvolledig_of_onjuist` (bullet 1 — formele onvolledigheid aanvraag, herstel 2 wkn)
- g₁ᵦ = `onevenredige_uitgaven_inkomen_onopgehelderd` (bullet 2 — inhoudelijke discrepantie + onvoldoende opheldering, discretie ontvanger)
- g₁ᵧ = `aangifte_niet_ingediend` (bullet 3 — aangifte-spoor via inspecteur, niet via formulier)
- g₂ = `bezwaar_of_beroep_aanhangig` (bullet 4)
- g₃ = `zekerheid_gesteld` (bullet 5)
- g₄ = `meerdere_belastingschuldigen` (bullet 6)
- g₅ = `derde_aansprakelijk_gesteld` (bullet 7)
- g₆ = `verwijtbaarheid_belastingschuld` (bullet 8 — subpunten a, c, d, e; b/f/g/h "niet van toepassing")
- g₇ = `in_faillissement_of_surseance_zonder_akkoord` (bullets 9+10 — met akkoord-uitzonderingen art 138/252/329 FW)

Niet in formule (nu untranslatable):
- Bullet 11 = placeholder "niet van toepassing" (geen echte grond)
- Bullet 12 "nadere voorwaarden niet nagekomen"
- Bullet 13 "gemeentelijke sociale dienst vergoedt"
- Eindblok 1: wisselende inkomens hoger binnen 2 jaar
- Eindblok 2: verbeteringsverwachting financieel binnen 1 jaar

**Dekking-samenvatting**: 9 van 12 echte gronden in formule (75%) + 2 van 2 eindblok-gronden buiten formule (0%) = 9 van 14 totaal (64%).

> ⚠ **Pre-workshop voorstel — nog te bekrachtigen door experts.**
> Op 2026-04-22 is de 2023-MR's enkele parameter `gegevens_onvolledig_of_onjuist` (die bundelde bullets 1+2+3) **voorstelsgewijs gesplitst** in g₁ₐ/g₁ᵦ/g₁ᵧ. De redenering: de drie bullets hebben verschillende herstelroutes en discretie-ruimte, dus eigen parameters maken beschikking-motivering (26.1.6) beter onderbouwbaar. **Dit is niet-gevalideerde interpretatie** — experts moeten het bekrachtigen, terugdraaien, of bijsturen.

- [ ] **Bekrachtigen splitsing g₁**: is de juridische redenering correct? Voldoet de splitsing aan hoe HHNK in de praktijk beschikt? (Hypothesen: verschillende herstelroutes per bullet, en discretie "naar het oordeel van de ontvanger" in bullet 2.) Notitie: —
- [ ] **Terugdraaien?** Als experts bundeling prefereren: welke motivering — is dit in praktijk een samengestelde grond waar splitsing niks toevoegt? Notitie: —
- [ ] **Fijner splitsen?** Bv. bullet 1 zelf heeft subtypes ("niet verstrekt" vs. "niet volledig verstrekt" vs. "onjuist") — moet daar ook onderscheid gemaakt worden? Notitie: —
- [ ] g₆ — subpunten a-e vallen onder 1 verwijtbaarheids-parameter. Jurist levert toets? Notitie: —
- [ ] 4 niet-geëncodeerde gronden als untranslatable **acceptabel** of moet ontvanger deze ook als MR-boolean kunnen zetten? Notitie: —
- [ ] HHNK-specifieke 8e grond nodig? Notitie: —
- [ ] Schuldsaneringsregeling-**met-akkoord** als uitzondering op g₇ — correct? Notitie: —

### Output 3 — `kan_kwijtschelding_worden_verleend` (orchestrator-AND)

Formule:
```
verzoek_ingediend
∧ belastingsoort_in_scope
∧ ondernemer_komt_in_aanmerking  (alleen zinvol bij ondernemer)
∧ ¬uitgesloten_van_kwijtschelding
∧ uri_hoogte_kwijtschelding > 0
```

- [ ] Volgorde van gates willekeurig (AND) — klopt dat echt, of heeft bv. scope-check beleidsmatig voorrang? Notitie: —
- [ ] `ondernemer_komt_in_aanmerking` via HHNK-verordening art 5 — geldt default TRUE voor particulieren? Notitie: —
- [ ] `uri_hoogte_kwijtschelding > 0` als gate: als vermogen + 0.8×bc ≥ aanslag dan afwijzing. Logisch, of wil HHNK drempel "minimaal X te schelden"? Notitie: —

### Output 4 — `hoogte_kwijtschelding`

Formule:
```
IF kan_kwijtschelding_worden_verleend
   THEN uri_hoogte_kwijtschelding    (= MAX(0, aanslag − vermogen − 0.8×bc) uit URI art 11)
   ELSE 0
```

- [ ] Delegatie naar URI art 11 — klopt deze is authoritatief voor HHNK? Notitie: —
- [ ] HHNK heeft **geen eigen afwijking** van URI-berekening (geen verhoging, geen andere 80%)? Notitie: —
- [ ] ELSE = 0 — beschikking wordt dan een *afwijzing*, niet "0 euro toegekend". Schema-interpretatie correct? Notitie: —

---

## Deel 4 — Subartikel-sweep 26.1 → 26.4 *(optioneel, als tijd)*

> **In de herziene 3-uurs planning valt de subartikel-sweep buiten het hoofdblok**. Gebruik als tijd-buffer / parkeerruimte: individuele rijen waar in Deel 3 twijfel ontstond, werk je hier in bulk af. Zonder tijdnood sla je dit blok over en pak je het in een vervolg-sessie.

Per subartikel: staat-in-MR / untranslatable / niet-relevant / nieuwe MR-behoefte.

| Subartikel | Onderwerp                             | Huidige status                                                   | Check | Beslissing |
| ---------- | ------------------------------------- | ---------------------------------------------------------------- | ----- | ---------- |
| 26.1.1     | Terugbetaling reeds betaalde aanslag  | Untranslatable (2026: refund-cap weg)                            | [ ]   |            |
| 26.1.2     | Indienen verzoek                      | Gate O1                                                          | [ ]   |            |
| 26.1.3     | Onvolledig formulier, 2-weken herstel | Untranslatable                                                   | [ ]   |            |
| 26.1.4     | Gegevens + normen op indieningsmoment | N.v.t. (engine gebruikt calc_date)                               | [ ]   |            |
| 26.1.5     | Voorwaardelijke toewijzing            | Untranslatable                                                   | [ ]   |            |
| 26.1.6     | Motivering afwijzing                  | Proces, niet in MR                                               | [ ]   |            |
| 26.1.7     | 14-dagen wachttijd invordering        | Proces                                                           | [ ]   |            |
| 26.1.8     | Mondelinge mededeling                 | Proces                                                           | [ ]   |            |
| 26.1.9     | Uitsluitingsgronden                   | Gate O2 (7 gronden + 4 untranslatable)                           | [ ]   |            |
| 26.1.10    | Ex-ondernemer                         | Niet in MR (rolwissel-parameter)                                 | [ ]   |            |
| 26.1.11    | Andere instellingen                   | "Niet van toepassing voor HHNK"                                  | [ ]   |            |
| 26.2.1     | Vermogen definitie                    | Via URI art 12                                                   | [ ]   |            |
| 26.2.2     | Inboedel (vervallen 2023)             | N.v.t.                                                           | [ ]   |            |
| 26.2.3     | Motorvoertuigen €3 350 drempel        | Via Leidraad 2008 override op URI                                | [ ]   |            |
| 26.2.4     | Saldo op bankrekening                 | Via URI art 12 (incl. 2026-nieuw schade-uitkering + smartengeld) | [ ]   |            |
| 26.2.5     | Eigen woning                          | Via URI art 12                                                   | [ ]   |            |
| 26.2.6     | Vermogen van kinderen                 | Via URI art 12                                                   | [ ]   |            |
| 26.2.7     | Nalatenschappen                       | Via URI art 12                                                   | [ ]   |            |
| 26.2.8     | Levensloopregeling (vervallen)        | N.v.t.                                                           | [ ]   |            |
| 26.2.9     | Beroepsvermogen (vervallen)           | N.v.t.                                                           | [ ]   |            |
| 26.2.10    | Betalingscapaciteit definitie         | Via URI art 13                                                   | [ ]   |            |
| 26.2.11    | Vakantiegeld                          | Via URI art 13                                                   | [ ]   |            |
| 26.2.12    | Studiefinanciering (2026: €80/€70)    | `definitions` in YAML, niet in formule                           | [ ]   |            |
| 26.2.13    | Bijzondere bijstand                   | Untranslatable                                                   | [ ]   |            |
| 26.2.13a   | PGB                                   | Via URI art 14                                                   | [ ]   |            |
| 26.2.14    | Betalingen op belastingschulden       | Niet in MR                                                       | [ ]   |            |
| 26.2.15    | Voorhuwelijkse schulden               | Niet in MR (edge-case)                                           | [ ]   |            |
| 26.2.16    | Onderhoudsverplichtingen              | Via URI art 15                                                   | [ ]   |            |
| 26.2.17    | WSNP                                  | Via uitsluiting g₇                                               | [ ]   |            |
| 26.2.18    | Inkomen wikker (vervallen 2022)       | N.v.t.                                                           | [ ]   |            |
| 26.2.19    | Normpremie Zvw (2026: €47/€106)       | `definitions` in YAML, niet in formule                           | [ ]   |            |
| 26.2.20    | Gezinsleden buitenland                | Untranslatable                                                   | [ ]   |            |
| 26.3.*     | Ondernemers-kwijtschelding            | **Scope-vraag F3**                                               | [ ]   |            |
| 26.4.*     | Administratief beroep                 | Puur proces, niet in MR                                          | [ ]   |            |

---

## Deel 5 — 2026-wijzigingen + beslispunten (2:25–2:50, 25 min) — *werkvorm: 1-2-4-all*

Per beslispunt:
1. 30s individueel: *"wat zou jij kiezen?"* op sticky / in notitie
2. 1 min paren: "match jullie?"
3. 2 min viertallen (als groep groot genoeg): "waar wijken we af?"
4. 1 min groepsoordeel + notitie — **minderheids-standpunten expliciet vastleggen**


Specifieke wijzigingen t.o.v. 2023 die HHNK nu moet bekrachtigen:

| #   | 2026-wijziging                                                       | Huidige MR-behandeling                    | Stem                       | Actie |
| --- | -------------------------------------------------------------------- | ----------------------------------------- | -------------------------- | ----- |
| B1  | 26.1.1 refund-cap (3 mnd) **verviel**                                | Untranslatable bijgewerkt naar 2026-tekst | akkoord/wijzigen           | [ ]   |
| B2  | 26.1.9 opsomming uitsluitingsgronden                                 | Identiek aan 2023, MR dekt 7 van 11       | akkoord/wijzigen           | [ ]   |
| B3  | 26.2.4 schade-uitkering 1-jr + smartengeld 5-jr vrijstelling (nieuw) | Niet in MR, via URI art 12 impliciet      | akkoord/encoderen          | [ ]   |
| B4  | 26.2.12 student-forfait €67→€80 HO, €60→€70 MBO                      | Nu als `definitions` (niet in formule)    | definitions/source/formule | [ ]   |
| B5  | 26.2.19 normpremie Zvw €3→€47 alleenstaand, €50→€106 echtg.          | Nu als `definitions`                      | definitions/source/formule | [ ]   |
| B6  | 26.3.8 saneringsakkoord-looptijd 10→12 mnd                           | Nu als `definitions`                      | definitions/formule        | [ ]   |
| B7  | 26.4.2 herhaald verzoek herschreven                                  | Niet in MR (proces)                       | akkoord/encoderen          | [ ]   |

---

## Deel 6 — Afronding + terugblik (2:50–3:00, 10 min)

**Werkvorm: 1 zin per persoon** — *"wat neem ik mee"*. Ieders zin noteren.

Dan action-items tabel invullen (zie onder), en als er nog 2 min over is: dankwoord + datum vervolgsessie voorstellen.


**Action items**:

| #   | Wat | Wie | Deadline |
| --- | --- | --- | -------- |
| 1   |     |     |          |
| 2   |     |     |          |
| 3   |     |     |          |

**Volgende sessie** (URI-keten, of 26.3-ondernemers als die wel in scope): —

**Open juridische vragen voor vervolgonderzoek**:
-

---

## Voorbereiding + risico's (facilitator-only)

**Materialen**: Venn-print A2 · formulas.md per persoon · wettekst art 26 PDF · sticky-notes 3 kleuren · dot-stickers · markers · brown paper.

**Laptop**: Obsidian (workshop-md + formulas.md splitview) · terminal · browser op CVDR756485.

**Risico's + mitigatie**: experts getoetst → framing "jullie schieten het stuk"; dominante stem → 1-2-4-all + expliciete naamrondes; verzanding → 5-min-regel + parkeren; abstractie → case-based ("mw. Jansen met €300"); besluiteloos → "beslissingen zijn niet voor eeuwig".

**Backup**: case-vertelling door de wettekst heen, of groep laten zelf decision-tree tekenen.

**Neem iemand mee** voor co-facilitatie/notulatie.

---

## Post-workshop checklist

Na afloop te committen:

1. Deze file zelf — alle vinkjes + notities erin (wordt het workshop-verslag)
2. `docs/audit/hhnk-leidraad-art-26.md` — [ ]→☑ invullen + afwijkingen noteren
3. Per beslispunt B1–B7 aparte commit met YAML-wijziging (indien nodig)
4. `just audit-boolean` regenereert `.formulas.md` automatisch
5. `just bdd` groen vóór push
6. Nieuwe commit op branch `feat/hhnk-kwijtschelding-machine-readable`
