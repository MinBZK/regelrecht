# Audit — URI 1990 art 13 (betalingscapaciteit)

**Wet**: Uitvoeringsregeling Invorderingswet 1990
**`$id`**: `uitvoeringsregeling_invorderingswet_1990`
**Wet-URL**: https://wetten.overheid.nl/BWBR0004766/2026-01-01#Artikel13
**YAML-bestand**: `corpus/regulation/nl/ministeriele_regeling/uitvoeringsregeling_invorderingswet_1990/2026-01-01.yaml`
**Laatste review**: —
**Reviewer(s)**: —

---

## Werkwijze

Art 13 definieert de *betalingscapaciteit* als het positieve verschil
tussen het per-jaar netto-besteedbare inkomen en de kosten van bestaan —
over 12 maanden. Formule is compact.

---

## Output — `betalingscapaciteit`

**Wettekst-excerpt** — art 13 lid 1 en 2:

> "1. Onder betalingscapaciteit, bedoeld in artikel 11, wordt verstaan het
> positieve verschil in de periode van 12 maanden vanaf de datum waarop
> het verzoek om kwijtschelding is ingediend van het gemiddeld per maand
> te verwachten netto-besteedbare inkomen van de belastingschuldige in die
> periode en de gemiddeld per maand te verwachten kosten van bestaan in
> die periode.
> 2. Het netto-besteedbare inkomen van de belastingschuldige, bedoeld in
> het eerste lid, wordt vermeerderd met het gemiddeld per maand te
> verwachten netto-besteedbare inkomen in de periode van twaalf maanden
> vanaf de datum waarop het verzoek om kwijtschelding is ingediend van zijn
> echtgenoot, bedoeld in artikel 3 van de Algemene bijstandswet."

| | |
|---|---|
| **Formule** | `B = max(0, 12 × (inkomen_self + inkomen_partner − extra_uitgaven − kostennorm))` |
| **Bron kostennorm** | source → URI art 16 |
| **extra_uitgaven** | optional parameter (bv. kinderopvang-kosten via HHNK-verordening art 3) |
| **YAML-locatie** | `articles[13].machine_readable.actions[0]` |

**Review**:

- ☐ `12 ×` factor correct (kwartaal- of maandsommen zouden fout zijn).
- ☐ MAX met 0 correct — geen negatieve betalingscap.
- ☐ Optellen partner-inkomen (lid 2) gedekt.
- ☐ "Vermeerderd met" tekst betekent optellen, niet gemiddeld — correct.
- ☐ `extra_uitgaven_maand` als bredere inname van uitgaven — dekt dit ook kinderopvang + andere gemeentelijke uitgaven?
- ☐ "In de periode van 12 maanden vanaf indiening" — we hanteren vaste 12×. Verandert dit bij korter dan 12 mnd?

---

## Niet-getranslateerd

Geen `untranslatables` in dit artikel — formule direct in operations-set.

---

## Open punten voor workshop

1. **"Gemiddeld per maand te verwachten" vs. huidige situatie** — caller moet een verwachting leveren, niet een snapshot. Hoe wordt dat aangeleverd?
2. **Periode 12 maanden vanaf indiening** — als iemand bijvoorbeeld over 3 maanden een baan krijgt, verandert het gemiddelde. Wordt dat in de parameter-waarde verrekend?
3. **Echtgenoot-inkomen** — zelfde als lid 1 berekening (12×) of anders? Nu zelfde (in onze formule).
