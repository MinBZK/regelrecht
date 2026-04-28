# Financieel CV — output walkthrough per regeling

Voor de kick-off met het regelhulp Financieel CV-team. In de stijl van
de HHNK-workshop notes (2026-04-23): per output een **wettekst-citaat**,
**formule** zoals nu in de YAML, **attributie** naar het bron-artikel,
**interpretatie-keuzes** en **checkboxen voor expert-bekrachtiging**.

NRP is volledig uitgeschreven als blauwdruk. Voor de zes andere
regelingen één representatieve output uitgewerkt; de overige outputs
volgen hetzelfde patroon en zijn één-op-één terug te lezen uit de
machine_readable-blokken in de YAML's.

> **Hoe te gebruiken in de workshop**: per output, lees de wettekst,
> kijk de formule, beslis met de jurist of formule en attributie
> kloppen, vink de checkboxen af. Onbekrachtigde outputs blijven open
> punten voor de volgende sessie.

---

# 1. NRP — No-riskpolis (Ziektewet artikel 29b)

## Output 1 — `voldoet_aan_lid_1`

**Wettekst (Ziektewet art. 29b lid 1):**

> "De werknemer:
> a. die onmiddellijk voorafgaand aan een dienstbetrekking als bedoeld
>    in artikel 3, 4 of 5, recht had op een uitkering op grond van de
>    Wet werk en inkomen naar arbeidsvermogen,
> b. van wie in een arbeidskundig onderzoek is vastgesteld dat hij op
>    de eerste dag na afloop van de wachttijd ... minder dan 35%
>    arbeidsongeschikt is, [+ vier sub-voorwaarden 1° t/m 4°],
> c. die de leeftijd van achttien jaar nog niet heeft bereikt en in
>    verband met ziekte of gebrek een belemmering ondervindt of heeft
>    ondervonden bij het volgen van onderwijs en binnen vijf jaar na
>    afronding van dat onderwijs arbeid in dienstbetrekking gaat
>    verrichten, of
> d. die geen werknemer is als bedoeld in het tweede lid, onderdeel a,
>    achttien jaar is of ouder en in verband met ziekte of gebrek
>    een belemmering ondervindt of heeft ondervonden bij het volgen
>    van onderwijs ..."

**Formule (YAML):**
```
voldoet_aan_lid_1 ≡
    is_wia_uitkeringsgerechtigd
  ∨ is_wia_min_35_arbeidsongeschikt
  ∨ is_jonggehandicapt_schoolverlater
```

**Attributie:** `Ziektewet art. 29b lid 1` (BWBR0001888). Onderdeel
1.b's vier sub-voorwaarden zijn samengevat in de input
`is_wia_min_35_arbeidsongeschikt` (UWV-vaststelling). Onderdelen 1.c
en 1.d zijn samengevat in `is_jonggehandicapt_schoolverlater` (Wajong
art. 1:1 doelgroepstub).

**Interpretatie-keuzes:**

- Onderdelen c en d hebben een vijfjaarstermijn voor "binnen vijf jaar
  na afronding onderwijs". Onze formule controleert deze termijn niet;
  hij zit als untranslatable gemarkeerd (zie `Output 5`). De input
  `is_jonggehandicapt_schoolverlater` veronderstelt dat UWV deze toets
  vooraf heeft gedaan.
- Onderdeel 1.b is samengevat in één boolean — de cumulatieve toets
  van 1°+2°+3°+4° wordt door UWV uitgevoerd, niet door deze formule.

- [ ] Formule klopt met wettekst
- [ ] Attributie verwijst naar juiste bron-artikel
- [ ] Vijfjaarstermijn als untranslatable acceptabel

---

## Output 2 — `voldoet_aan_lid_2`

**Wettekst (Ziektewet art. 29b lid 2):**

> "De werknemer:
> a. die voorafgaand aan zijn dienstbetrekking ... recht had of heeft
>    gehad op een arbeidsongeschiktheidsuitkering of arbeidsondersteuning
>    op grond van de Wajong,
> b. die een arbeidsovereenkomst heeft gesloten met een werkgever als
>    bedoeld in artikel 7 van de Wet sociale werkvoorziening,
> c. wiens dienstbetrekking ... is aangevangen voordat zijn recht op een
>    arbeidsongeschiktheidsuitkering of arbeidsondersteuning op grond
>    van de Wajong ontstond, omdat die dienstbetrekking is aangevangen
>    voordat hij achttien jaar werd,
> d. die onmiddellijk voorafgaande aan zijn dienstbetrekking ... een
>    Wsw-dienstbetrekking had ...,
> e. die ... [doelgroep banenafspraak / loonkostensubsidie Pwet] ...,
> f. die arbeid verricht in een dienstbetrekking als bedoeld in artikel
>    10b, eerste lid, van de Participatiewet, niet zijnde een werknemer
>    als bedoeld in onderdeel e."

**Formule (YAML):**
```
voldoet_aan_lid_2 ≡
    is_wajong_gerechtigd                    (a en c)
  ∨ is_wsw_werknemer                        (b en d)
  ∨ is_banenafspraak_doelgroep              (e — variant 1)
  ∨ is_pwet_loonkostensubsidie              (e — variant 2)
  ∨ is_beschut_werk                         (f)
```

**Attributie:** `Ziektewet art. 29b lid 2 onderdelen a t/m f`
(BWBR0001888). Onderdelen a en c worden beide gedekt door
`is_wajong_gerechtigd` omdat de doelgroepstub (Wajong art. 1:1)
"Wajong-gerechtigd" breed definieert. Onderdelen b en d worden beide
gedekt door `is_wsw_werknemer`.

**Interpretatie-keuzes:**

- Onderdeel e splitst zich in **banenafspraak** (UWV-doelgroepregister)
  óf **LKS-ontvanger** (Pwet 10d). In de wet als alternatieven
  gepresenteerd ("of"), in onze formule als OR.
- Onderdeel f noemt **beschut werk** (Pwet 10b) expliciet als
  *aanvullend* op onderdeel e — onze OR-formule maakt dit symmetrisch.
- **MvT-attentiepunt:** ten tijde van MvT 34194 (2015) was beschut werk
  uitgesloten; per huidige wettekst (2025-01-01) is het via lid 2.f
  ingesloten. De wettekst is leidend.

- [ ] Formule klopt met wettekst
- [ ] Attributie verwijst naar juiste bron-artikel
- [ ] Beschut-werk-inclusie via lid 2.f acceptabel

---

## Output 3 — `voldoet_aan_lid_4`

**Wettekst (Ziektewet art. 29b lid 4):**

> "De werknemer die recht heeft op een uitkering op grond van de Wet
> werk en inkomen naar arbeidsvermogen en ten aanzien van wie een
> dienstbetrekking ... bij diens werkgever wordt voortgezet nadat dat
> recht is vastgesteld, heeft vanaf de eerste dag van zijn ongeschiktheid
> tot werken recht op ziekengeld over perioden van ongeschiktheid tot
> werken wegens ziekte die zijn aangevangen in de vijf jaren na
> vaststelling van het recht op uitkering."

**Formule (YAML):**
```
voldoet_aan_lid_4 ≡ heeft_voortgezet_wia_recht
```

**Attributie:** `Ziektewet art. 29b lid 4` (BWBR0001888). Pass-through
naar Wet WIA art. 1 doelgroepstub via input
`heeft_voortgezet_wia_recht`.

**Interpretatie-keuzes:**

- De vijfjaarstermijn in lid 4 ("vijf jaren na vaststelling van het
  recht op uitkering") wordt door UWV bewaakt en is niet in de formule
  gemodelleerd. De input is een puntmoment-boolean.
- Lid 4 is in de wet structureel een uitbreiding van lid 1.a — niet
  een alternatief. Werkgever moet *al* de werkgever zijn die de
  WIA-vaststelling kreeg. Onze formule respecteert dit doordat de
  caller deze context kent (UWV koppelt werkgever aan
  WIA-vaststelling).

- [ ] Formule klopt met wettekst
- [ ] Vijfjaarstermijn als UWV-uitvoeringscontext acceptabel
- [ ] Werkgever-continuïteit als vooronderstelling acceptabel

---

## Output 4 — `heeft_recht_op_no_risk_polis`

**Wettekst (Ziektewet art. 29b — totaal):**

> "Aanspraak op ziekengeld op grond van de no-riskpolis bestaat als de
> werknemer onder ten minste één van de doelgroepen lid 1, lid 2 of
> lid 4 valt." (interpretatie van art. 29b in zijn geheel)

**Formule (YAML):**
```
heeft_recht_op_no_risk_polis ≡
    voldoet_aan_lid_1
  ∨ voldoet_aan_lid_2
  ∨ voldoet_aan_lid_4
```

**Attributie:** `Ziektewet art. 29b` als geheel (BWBR0001888). Geen
letterlijke tekst-regel, maar interpretatie-keuze van deze
machine_readable als orchestrator van de drie lid-routes.

**Interpretatie-keuzes:**

- De wet kent geen expliciete "OR-gate" tussen de leden — die is
  juridisch impliciet (de leden zijn alternatieve toegangsroutes).
  Voor de jurist: bevestig dat dit klopt.
- Werkgevers hoeven slechts via één lid-route te kwalificeren. Bij
  meerdere kwalificaties: geen cumulatie, één recht.

- [ ] OR-tussen-leden klopt met wettekst (impliciet)
- [ ] Geen cumulatie bij meerdere lid-routes acceptabel

---

## Output 5 — `duur_no_risk_polis_jaren`

**Wettekst:** Lid 1.b 4° "binnen vijf jaar na die dag in dienstbetrekking
werkzaamheden gaat verrichten"; lid 1.c en 1.d "binnen vijf jaar na
afronding van dat onderwijs"; lid 4 "vijf jaren na vaststelling van het
recht op uitkering"; lid 2 — geen termijn genoemd.

**Formule (YAML):**
```
duur_no_risk_polis_jaren ≡
  IF voldoet_aan_lid_2 → 0
  IF voldoet_aan_lid_1 → 5
  IF voldoet_aan_lid_4 → 5
  default 0
```

**Attributie:** `Ziektewet art. 29b lid 1, 2, 4` (BWBR0001888). De
volgorde van de IF-cases is inhoudelijk: bij overlap moet de
"onbeperkte" lid-2-route herkenbaar blijven (vandaar `0` als
gemodelleerd niet-tijdsgebonden).

**Interpretatie-keuzes** (untranslatable, `accepted: true`):

- **Lid 2-doelgroepen krijgen `0` jaar** in de formule, terwijl in
  werkelijkheid het recht onbeperkt geldt zolang de dienstbetrekking
  voortduurt. Onze engine kent geen "onbeperkt"-waarde. Voorstel voor
  toekomstige iteratie: aparte boolean `tijdsgebonden_duur` of
  speciale waarde `null`/`-1`.
- **Vijfjaarstermijn bij onderbroken dienstverbanden** is in de wet
  niet eenduidig. Bij een tweede dienstverband binnen vijf jaar:
  begint de termijn opnieuw, doorloopt hij, of telt hij op? Vereist
  juridische interpretatie.
- **Output ontbreekt `type_spec.unit: years`** — inconsistent met
  PP `max_duur_proefplaatsing_maanden`. Zie code-review-actiepunt.

- [ ] Lid 2 als `0` jaar acceptabel als demo-modellering
- [ ] Vijfjaarstermijn-onderbreking als untranslatable acceptabel
- [ ] Toevoeging `type_spec.unit: years` afgesproken

---

# 2. PP — Proefplaatsing (WW art. 76a) — voorbeeld-output

## Output — `mag_proefplaatsing_aangaan`

**Wettekst (WW art. 76a lid 1 + 3):**

> "Het UWV kan toestemming verlenen aan de werknemer, die recht heeft op
> een uitkering op grond van hoofdstuk II, om op een proefplaats bij
> een werkgever gedurende maximaal zes maanden onbeloonde werkzaamheden
> te verrichten." [+ lid 3 a t/m d cumulatieve voorwaarden]

**Formule (YAML):**
```
mag_proefplaatsing_aangaan ≡
  heeft_recht_op_ww_uitkering ∧ voldoet_aan_lid_3_voorwaarden

voldoet_aan_lid_3_voorwaarden ≡
    in_staat_tot_werkzaamheden                         (a)
  ∧ aansprakelijkheidsverzekering_aanwezig             (b)
  ∧ niet_eerder_proefplaatsing_zelfde_werkgever        (c)
  ∧ reeel_uitzicht_op_dienstbetrekking_zes_maanden     (d)
```

**Attributie:** `WW art. 76a lid 1 + 3` (BWBR0004045).

- [ ] Cumulativiteit lid 3 a-d klopt
- [ ] Pass-through `heeft_recht_op_ww_uitkering` als parameter (i.p.v.
      cross-law naar art. 14 e.v.) acceptabel voor demo

(Overige outputs `voldoet_aan_lid_3_voorwaarden`,
`max_duur_proefplaatsing_maanden`, `ww_uitkering_blijft_bestaan` —
zie YAML.)

---

# 3. LIV — Lage-inkomensvoordeel (Wtl art. 3.1 + 3.2) — voorbeeld

## Output — `hoogte_liv_per_jaar_eurocent`

**Wettekst (Wtl art. 3.2.1):**

> "Een lage-inkomensvoordeel bedraagt € 0,49 per verloond uur van de
> werknemers die voldoen aan de voorwaarde, bedoeld in artikel 3.1,
> eerste lid, onderdeel a, doch ten hoogste € 960 per werknemer per
> kalenderjaar."

**Formule (YAML):**
```
hoogte_liv_per_jaar_eurocent ≡
  IF heeft_recht_op_liv → MIN(49 × verloonde_uren, 96000)
  default 0
```

**Attributie:** `Wtl art. 3.2 lid 1` (BWBR0037522). Bedragen in
eurocent: 49 = € 0,49 / uur, 96000 = € 960,00 / jaar.

**Interpretatie-keuzes:**

- Bedragen voor 2024 hardgecodeerd. Lid 4 schrijft jaarlijkse
  aanpassing voor via min. regeling (open_term `liv_uurloongrenzen_per_jaar`).
- LIV is per 2025-01-01 afgeschaft (Wet 36458). Peildatum van deze YAML
  is 2024-01-01.

- [ ] € 0,49/uur en € 960/jaar als correcte 2024-bedragen
- [ ] MIN-formule als correcte cap-implementatie
- [ ] Afschaffing per 2025 verwerkt in peildatum acceptabel

---

# 4. LKV — Loonkostenvoordeel (Wtl art. 2.1 + 2.7/9/13/17) — voorbeeld

## Output — `bedrag_per_uur_eurocent`

**Wettekst (Wtl art. 2.7, 2.9, 2.13, 2.17):**

> Art. 2.7 (oudere werknemer): "€ 3,05 per verloond uur ... ten hoogste
> € 6.000 per werknemer per kalenderjaar."
> Art. 2.9 (arbeidsgehandicapt): idem.
> Art. 2.13 (banenafspraak): "€ 1,01 per verloond uur ... ten hoogste
> € 2.000 per werknemer per kalenderjaar."
> Art. 2.17 (herplaatsen): idem als 2.7.

**Formule (YAML):**
```
bedrag_per_uur_eurocent ≡
  IF categorie_lkv == "banenafspraak" → 101
  default 305
```

**Attributie:** `Wtl art. 2.7, 2.9, 2.13, 2.17` (BWBR0037522).

**Interpretatie-keuzes:**

- Categorie-bepaling via IF-cascade: oudere → arbeidsgehandicapt →
  herplaatsen → banenafspraak. Bij meerdere kwalificaties wint de
  eerste. **De wet zwijgt expliciet over deze prioriteit** —
  juridische check vereist.
- Banenafspraak is per 2025 structureel zonder doelgroepverklaring.
  YAML peildatum 2024 dekt dit nog niet.

- [ ] € 3,05 / € 1,01 als 2024-tarieven
- [ ] IF-volgorde voor categorie-prioriteit acceptabel of: ander
      toewijzingsregime gewenst?

---

# 5. LKS — Loonkostensubsidie (Pwet art. 10c + 10d) — voorbeeld

## Output — `hoogte_lks_eurocent_per_maand`

**Wettekst (Pwet art. 10d lid 4):**

> "De hoogte van de loonkostensubsidie ... is het verschil tussen het
> wettelijk minimumloon vermeerderd met de aanspraak op vakantiebijslag
> ... en de loonwaarde van die persoon vermeerderd met de voor die
> persoon naar rato van de loonwaarde rechtens geldende vakantiebijslag,
> maar is ten hoogste 70 procent van het totale bedrag van het wettelijk
> minimumloon en de aanspraak op vakantiebijslag ..., vermeerderd met
> een bij ministeriële regeling vastgestelde vergoeding voor
> werkgeverslasten."

**Formule (YAML):**
```
bruto_subsidie       ≡ minimumloon_plus_VB - loonwaarde_per_maand
maximum_subsidie     ≡ minimumloon_plus_VB × 70 / 100
hoogte_lks_per_maand ≡ IF heeft_recht_op_lks → MAX(0, MIN(bruto, max))
                       default 0
```

**Attributie:** `Pwet art. 10d lid 4` (BWBR0015703).

**Interpretatie-keuzes:**

- **`loonwaarde_eurocent_per_maand` is dubbelzinnig** — bevat het de
  loonwaarde mét of zonder VB? De wettekst telt VB expliciet ook bij
  loonwaarde op. Onze formule rekent met "all-in"-loonwaarde. Zie
  code-review-actiepunt.
- **Werkgeverslastenvergoeding** (open_term, MR onder 10d.4) niet in
  deze hoogte verwerkt — moet apart toegevoegd worden voor de
  daadwerkelijk uitbetaalde subsidie.
- **Lid 5 50%-regeling eerste 6 mnd** voor het 1.b-traject (zonder
  loonwaardevaststelling) niet gemodelleerd — zie untranslatable.

- [ ] Definitie loonwaarde-input (incl. VB?) verduidelijken
- [ ] Werkgeverslastenvergoeding toevoegen aan hoogte? Of apart laten?
- [ ] 70%-cap correct toegepast

---

# 6. LDP — Loondispensatie (Wajong art. 2:20) — voorbeeld

## Output — `heeft_recht_op_loondispensatie`

**Wettekst (Wajong art. 2:20 lid 1):**

> "Indien de arbeidsprestatie van een werknemer die recht heeft op
> arbeidsondersteuning ... maar geen functie waarin hij werkzaam is
> als werknemer in de zin van de Wet sociale werkvoorziening of op een
> arbeidsovereenkomst als bedoeld in artikel 7 van die wet, ten gevolge
> van ziekte of gebrek duidelijk minder is dan de arbeidsprestatie die
> een geldelijke beloning van het minimumloon rechtvaardigt, vermindert
> het Uitvoeringsinstituut werknemersverzekeringen op verzoek van de
> betrokken werkgever of werknemer de hoogte van de aanspraak ..."

**Formule (YAML):**
```
heeft_recht_op_loondispensatie ≡
    heeft_recht_op_arbeidsondersteuning_wajong
  ∧ ¬is_wsw_werknemer
  ∧ arbeidsprestatie_duidelijk_minder_dan_minimumloon
  ∧ aanvraag_loondispensatie_ingediend
```

**Attributie:** `Wajong art. 2:20 lid 1` (BWBR0008657).

**Interpretatie-keuzes** (untranslatable, `accepted: true`):

- "Duidelijk minder dan minimumloon-equivalent" — UWV-discretie,
  niet-meetbaar.
- "Naar evenredigheid" — UWV-uitvoeringsbeleid. Het percentage zelf
  valt onder open_term `dispensatiepercentage` (BELEIDSREGEL).

- [ ] Cumulatieve AND-voorwaarden compleet
- [ ] Lid 2 nietigheidsclausule als constante `true` acceptabel
- [ ] Dispensatiepercentage als BELEIDSREGEL-open_term acceptabel

---

# 7. JC + WPA — Jobcoaching + Werkplekaanpassingen (Wet WIA art. 35) — voorbeeld

## Output — `heeft_recht_op_jobcoaching`

**Wettekst (Wet WIA art. 35 lid 1 + 2.d + 4):**

> Lid 1: "Het UWV kan aan de persoon met een naar het oordeel van het
> UWV structurele functionele beperking, en die arbeid in dienstbetrekking
> verricht of die arbeid in dienstbetrekking gaat verrichten, doch niet
> werkzaam is of zal zijn als werknemer in de zin van de Wet sociale
> werkvoorziening, ... op aanvraag voorzieningen toekennen ..."
>
> Lid 2: "Onder voorzieningen als bedoeld in het eerste lid worden
> uitsluitend verstaan: ... d. noodzakelijke persoonlijke ondersteuning
> bij het verrichten van de aan de persoon opgedragen taken, indien die
> ondersteuning een compensatie vormt voor zijn beperkingen."
>
> Lid 4: "Dit artikel is niet van toepassing op de persoon: a. die recht
> heeft op arbeidsondersteuning op grond van de Wajong; b. voor zover
> voor diens ondersteuning bij arbeidsinschakeling op grond van artikel
> 7, eerste lid, onderdeel a, van de Participatiewet het college ... zorg
> draagt of ... [t/m 2 jaar minimumloon zonder LKS] ..."

**Formule (YAML):**
```
artikel_35_van_toepassing ≡
    ¬heeft_recht_op_arbeidsondersteuning_wajong
  ∧ ¬pwet_college_draagt_zorg_uitsluiting

voldoet_aan_basisvoorwaarden_lid_1 ≡
    heeft_structurele_functionele_beperking
  ∧ heeft_arbeidsverhouding_of_voorbereiding
  ∧ ¬is_wsw_werknemer

heeft_recht_op_jobcoaching ≡
    artikel_35_van_toepassing
  ∧ voldoet_aan_basisvoorwaarden_lid_1
  ∧ aanvraag_jobcoaching_ingediend
```

**Attributie:** `Wet WIA art. 35 lid 1 + 2.d + 4` (BWBR0019057).

**Interpretatie-keuzes** (untranslatable, `accepted: true`):

- "Structurele functionele beperking" — UWV-arts beoordeelt op grond
  van Schattingsbesluit.
- Lid 4.b's "tot het moment dat het inkomen ... gedurende twee
  aaneengesloten jaren ten minste het minimumloon ... bedraagt en
  in die twee jaren geen LKS is verleend" — samengevat in één boolean
  `pwet_college_draagt_zorg_uitsluiting`. UWV evalueert de tijdgebonden
  toets vooraf.
- "Noodzakelijke persoonlijke ondersteuning" en "compensatie voor
  beperkingen" — UWV-discretionair.

- [ ] Drielagige gating (van_toepassing → basisvoorwaarden → recht) klopt
- [ ] Lid 4.b 2-jaars/LKS-toets als boolean acceptabel
- [ ] Reïntegratiebesluit (AMvB onder lid 5) nog niet als
      `implements`-relatie geharvest — wel doen?

---

## Hoe verder na de kick-off

Per regeling per output is er nu een sjabloon. In vervolgsessies:

1. Loop per regeling de overige outputs door (in YAML staan ze):
   - PP: `voldoet_aan_lid_3_voorwaarden`, `max_duur_proefplaatsing_maanden`,
     `ww_uitkering_blijft_bestaan`
   - LIV: `gemiddeld_uurloon_eurocent`, `voldoet_aan_uurloongrens`,
     `voldoet_aan_minimum_verloonde_uren`, `heeft_recht_op_liv`
   - LKV: `categorie_lkv`, `heeft_recht_op_lkv`,
     `maximum_per_jaar_eurocent`, `hoogte_lkv_per_jaar_eurocent`
   - LKS: `heeft_recht_op_lks`, `bruto_subsidie_eurocent_per_maand`,
     `maximum_subsidie_eurocent_per_maand`
   - LDP: `beding_lagere_beloning_is_nietig`
   - JC/WPA: `heeft_recht_op_werkplekaanpassing`,
     `artikel_35_van_toepassing`, `voldoet_aan_basisvoorwaarden_lid_1`

2. Vink per output beide checkboxes ("formule klopt", "attributie
   correct") aan zodra de jurist heeft bekrachtigd. Onbevestigde
   outputs blijven open punten.

3. Per regeling expliciet beslissen over de juridische open vragen uit
   `mvt-referenties.md` — die dragen naar de inhoud van de regelhulp.
