# Audit — URI 1990 art 12 (vermogen)

**Wet**: Uitvoeringsregeling Invorderingswet 1990
**`$id`**: `uitvoeringsregeling_invorderingswet_1990`
**Wet-URL**: https://wetten.overheid.nl/BWBR0004766/2026-01-01#Artikel12
**YAML-bestand**: `corpus/regulation/nl/ministeriele_regeling/uitvoeringsregeling_invorderingswet_1990/2026-01-01.yaml`
**Laatste review**: —
**Reviewer(s)**: —

---

## Werkwijze

Art 12 definieert *wat vermogen is* voor de kwijtschelding: bezittingen
minus bevoorrechte schulden. Bevat de drempel €2269 voor inboedel/auto
(lid 2 onder a en c), een open_term `verhoging_financiele_middelen_
vrijstelling` (lid 2 onder d → door HHNK-verordening art 4 ingevuld) en
uitzonderingen. Leidraad 2008 art 26.2.3 `overrides` de auto-drempel
naar €3350 — dat is een lex-pro-cive correctie die via de engine doorwerkt.

---

## Output 1 — `inboedel_als_vermogen`

**Wettekst-excerpt** — art 12 lid 2 onder a:

> "Onder bezittingen wordt niet begrepen:
> a. de inboedel voor zover de waarde hiervan niet meer bedraagt dan € 2269"

| | |
|---|---|
| **Formule** | `inboedel_als_vermogen = IF inboedel_waarde > €2269 THEN inboedel_waarde ELSE 0` |
| **Drempel** | `drempelwaarde_inboedel_auto = 226900` eurocent (€2269) |
| **YAML-locatie** | `articles[12].machine_readable.actions[0]` |

**Review**:

- ☐ Drempel €2269 correct overgenomen.
- ☐ Bij waarde > drempel: *gehele* waarde telt (niet waarde − drempel).
- ☐ Bij waarde ≤ drempel: 0 (niet in vermogen).

---

## Output 2 — `auto_als_vermogen`

**Wettekst-excerpt** — art 12 lid 2 onder c:

> "c. een auto die op het moment van het verzoek een waarde heeft van €2269
> of minder; een auto met een waarde van meer dan €2269 wordt niet als
> vermogen beschouwd indien jegens de ontvanger aannemelijk kan worden
> gemaakt dat die auto absoluut onmisbaar is voor de uitoefening van een
> beroep dan wel absoluut onmisbaar is in verband met invaliditeit"

| | |
|---|---|
| **Formule** | `auto_als_vermogen = IF onmisbaar THEN 0 ELIF auto_waarde > drempel THEN auto_waarde ELSE 0` |
| **Drempel URI** | €2269 (tekstueel) |
| **Drempel Leidraad 2008** | €3350 — via `overrides` op dit artikel |
| **YAML-locatie** | `articles[12].machine_readable.actions[1]` |

**Review**:

- ☐ URI-drempel €2269 tekstueel correct, maar in praktijk wordt €3350 (Leidraad 2008 26.2.3) gehanteerd. Is `overrides`-declaratie van Leidraad 2008 hier voldoende bekend?
- ☐ Onmisbaarheid-check als eerste (voorkomt vermogens-telling ongeacht waarde).
- ☐ Criterium "absoluut onmisbaar voor beroep of wegens invaliditeit" — caller moet juist beoordelen.

---

## Output 3 — `financiele_middelen_vrijstelling`

**Wettekst-excerpt** — art 12 lid 2 onder d:

> "d. het totale bedrag aan financiële middelen, andere dan de onder f
> bedoelde, voor zover dat bedrag de ingevolge artikel 16 in aanmerking te
> nemen kosten van bestaan vermeerderd met een bedrag ter grootte van het
> per maand gemiddelde bedrag van de uitgaven bedoeld in artikel 15,
> onderdelen b en c, niet te boven gaat"

| | |
|---|---|
| **Formule** | `vrijstelling = kostennorm_bedrag + verhoging_financiele_middelen_vrijstelling` |
| **Bron kostennorm** | source → URI art 16 |
| **Bron verhoging** | open_term (vervuld door HHNK-verordening art 4: €2000 / €1800 / €1500 per huishoudtype) |
| **YAML-locatie** | `articles[12].machine_readable.actions[2]` |

**Review**:

- ☐ Som is correct: kostennorm + open_term verhoging.
- ☐ Tekst zegt "kosten van bestaan vermeerderd met gemiddelde van uitgaven art 15 b+c" — is dit gelijk aan de open_term-verhoging? Deels: de open_term dekt de bij verordening ingestelde verhoging, niet per se de art 15-uitgaven-component.
- ☐ Kinderopvangkosten (verordening art 3) — komen die hier in de vrijstelling of in de betalingscap? Huidige keten: in betalingscap.

**Open**:

- Strict gelezen: de wettekst noemt "gemiddelde art 15 b+c uitgaven" als onderdeel van de vrijstelling. Dat is niet letterlijk gemodelleerd; we hanteren alleen de `verhoging_financiele_middelen_vrijstelling` open_term. Controleerbaar verschil?

---

## Output 4 — `liquide_als_vermogen`

**Wettekst-excerpt** — afgeleid van lid 2 onder d (liquide middelen boven de vrijstelling tellen als vermogen):

> [art 12 lid 2 onder d leest als: liquide middelen *boven* de vrijstelling
> vallen wél onder bezittingen]

| | |
|---|---|
| **Formule** | `liquide_als_vermogen = max(0, liquide_middelen − vrijstelling)` |
| **YAML-locatie** | `articles[12].machine_readable.actions[3]` |

**Review**:

- ☐ Subtract met MAX(0,…) correct — géén negatief vermogen.
- ☐ Uitsluitingen onder (b) uitvaartuitkeringen en (e) studiefinancieringsleningen zijn niet in de formule. Moet de caller deze uit liquide_middelen filteren? Nu impliciet.

---

## Output 5 — `totaal_bezittingen`

| | |
|---|---|
| **Formule** | `totaal = inboedel_vm + auto_vm + onroerend_goed_waarde + liquide_vm + andere_bezittingen` |
| **YAML-locatie** | `articles[12].machine_readable.actions[4]` |

**Review**:

- ☐ Volgorde van som doet niet uit (commutatief).
- ☐ Onroerend goed wordt *volledig* als bezitting geteld (tekst lid 2 onder f-a: geen drempel). Klopt.
- ☐ "Andere bezittingen" is verzamelnaam — valt hier alles onder dat niet in bovenstaande categorieën zit?

---

## Output 6 — `vermogen_bedrag`

**Wettekst-excerpt** — art 12 lid 1:

> "Onder vermogen als bedoeld in artikel 11 wordt verstaan de waarde in het
> economische verkeer van de bezittingen van de belastingschuldige en van
> zijn echtgenoot, bedoeld in artikel 3 van de Algemene bijstandswet,
> verminderd met de schulden van de belastingschuldige en deze persoon die
> hoger bevoorrecht zijn dan de rijksbelastingen."

| | |
|---|---|
| **Formule** | `vermogen = max(0, totaal_bezittingen − hoger_bevoorrechte_schulden)` |
| **YAML-locatie** | `articles[12].machine_readable.actions[5]` |

**Review**:

- ☐ MAX met 0 correct — vermogen is nooit negatief (anders zouden schulden kwijtschelding verhogen).
- ☐ Verwijzing "artikel 3 Algemene bijstandswet" ongebruikt in formule; bezittingen echtgenoot zijn opgeteld in de parameter-layer door de caller (semantisch dus wel gedekt).

---

## Open punten voor workshop

1. **Auto-drempel conflict**: URI zegt €2269, Leidraad 2008 zegt €3350. De `overrides:` op Leidraad 2008 26.2.3 → URI art 12 werkt door. Is dit expliciet genoeg bekend bij workshop-deelnemers?
2. **Art 15 b+c component in vrijstelling** — letterlijk niet gemodelleerd.
3. **Liquide-uitsluitingen** (studiefinanciering, PGB, uitvaartuitkering) — caller-verantwoordelijkheid of expliciet modelleren?
4. **Echtgenoot-bezittingen** — caller-verantwoordelijkheid om op te tellen, niet in formule. Is dat duidelijk?
