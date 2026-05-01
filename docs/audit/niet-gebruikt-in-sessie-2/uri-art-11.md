# Audit — URI 1990 art 11 (kwijtschelding-hoofdbepaling)

**Wet**: Uitvoeringsregeling Invorderingswet 1990
**`$id`**: `uitvoeringsregeling_invorderingswet_1990`
**Type**: Ministeriële regeling (Min. van Financiën)
**Wet-URL**: https://wetten.overheid.nl/BWBR0004766/2026-01-01#Artikel11
**YAML-bestand**: `corpus/regulation/nl/ministeriele_regeling/uitvoeringsregeling_invorderingswet_1990/2026-01-01.yaml`
**Laatste review**: —
**Reviewer(s)**: —

---

## Werkwijze

Art 11 URI is de *materiële* hoofdbepaling voor kwijtschelding in de
federale keten: hij stelt vast wanneer en hoeveel kwijtschelding wordt
verleend, op basis van vermogen (art 12) + betalingscapaciteit (art 13).
HHNK-leidraad art 26 neemt deze formule zelf over (eigen composition);
HHNK rijks-burger-tegenhanger zou het via art 11 gaan.

Symbolisch:

- `V` = `vermogen` (parameter, normatief gelijk aan URI art 12 output)
- `B` = `betalingscapaciteit` (via source → URI art 13)
- `A` = `aanslagbedrag`

---

## Output 1 — `aanwendbare_betalingscapaciteit`

**Wettekst-excerpt** — art 11 onderdeel b, punt 2°:

> "b. het openstaande bedrag van de belastingaanslag dat resteert nadat:
> 1°. het aanwezige vermogen is aangewend ter voldoening van de
>     belastingaanslag;
> 2°. ten minste 80 percent van de betalingscapaciteit is aangewend"

| | |
|---|---|
| **Formule** | `aanwendbare_bc = 0.8 × B` |
| **YAML-locatie** | `articles[11].machine_readable.actions[0]` |

**Review**:

- ☐ 80% correct (0.8).
- ☐ "Ten minste 80%" in praktijk = 80%, niet meer.
- ☐ Bron `B` via interne source naar art 13 klopt.

---

## Output 2 — `hoogte_kwijtschelding`

**Wettekst-excerpt** — art 11 onderdelen a + b gecombineerd:

> "Kwijtschelding wordt verleend voor:
> a. het gehele op de belastingaanslag openstaande bedrag indien geen
>    vermogen en geen betalingscapaciteit aanwezig is;
> b. het openstaande bedrag van de belastingaanslag dat resteert nadat
>    het vermogen + 80% betalingscapaciteit is aangewend;
> een en ander onverminderd het bepaalde in artikel 8, artikel 17 en
> artikel 18."

| | |
|---|---|
| **Formule** | `hoogte = max(0, A − V − 0.8·B)` |
| **YAML-locatie** | `articles[11].machine_readable.actions[1]` |

**Review**:

- ☐ MAX met 0 dekt zowel onderdeel a (niets aanwezig → heel bedrag) als b.
- ☐ Subtract-volgorde: aanslag minus vermogen minus aanwendbare_bc.
- ☐ Clausule "onverminderd art 8, 17, 18" — voor art 11 zelf niet als extra AND-gate gemodelleerd; die artikelen zijn scope/drempels die elders checken.

**Open**:

- Art 8 URI schrijft uitsluitingen voor specifieke belastingsoorten; moet `scope` als AND-gate op kan_kwijtschelding? Nu impliciet via HHNK-verordening art 1, maar voor rijks-invordering geen check.
- Art 17 URI (€136-drempel) — niet geïntegreerd in art 11 flow; zie separate audit uri-art-17.md.

---

## Output 3 — `kan_kwijtschelding_worden_verleend`

**Wettekst-excerpt** — impliciet uit onderdeel a ("indien geen vermogen en
geen betalingscapaciteit aanwezig is") + onderdeel b:

> de voorwaarden impliceren dat er *iets* over moet blijven om kwijt te
> schelden; als hoogte = 0, is er niets te verlenen.

| | |
|---|---|
| **Formule** | `kan = hoogte > 0` |
| **YAML-locatie** | `articles[11].machine_readable.actions[2]` |

**Review**:

- ☐ Kan-booleanreflectie van hoogte>0 correct.
- ☐ Geen extra gates (bv. 26.1.9-uitsluitingen) — die horen ÉN in de beleidsregel ÉN in de HHNK-leidraad, niet in URI art 11 zelf.

---

## Niet-getranslateerd

Geen `untranslatables` in dit artikel — de formule is volledig in de operation-set vatbaar.

---

## Open punten voor workshop

1. **Clausule "onverminderd art 8, 17, 18"** — hoe werkt dit door op de rijks-toets? Art 8 scope, art 17 €136, art 18 andere uitzondering. Nu niet als AND-gate in art 11.
2. **Art 11 wordt na refactor niet meer door HHNK-leidraad gebruikt** — HHNK doet eigen composition uit art 12 + 13. Blijft art 11 relevant voor rijks-ontvanger? Ja — dit is dan de enige audit-lijn voor federaal.
