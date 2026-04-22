# Audit — URI 1990 art 17 (€136-drempel ondernemers)

**Wet**: Uitvoeringsregeling Invorderingswet 1990
**`$id`**: `uitvoeringsregeling_invorderingswet_1990`
**Wet-URL**: https://wetten.overheid.nl/BWBR0004766/2026-01-01#Artikel17
**YAML-bestand**: `corpus/regulation/nl/ministeriele_regeling/uitvoeringsregeling_invorderingswet_1990/2026-01-01.yaml`
**Laatste review**: —
**Reviewer(s)**: —

---

## Werkwijze

Art 17 is een *drempel-artikel*: als iemand naast de belastingschuld ook
aflossingen op *andere* schulden heeft van meer dan €136/maand, dan wordt
kwijtschelding alleen in "zeer bijzondere omstandigheden" verleend. Dat
is een extra gate bovenop art 11. Op dit moment is art 17 WEL machine-
readable maar NIET geketend in HHNK-leidraad art 26 — zie open punten.

---

## Output 1 — `drempel_overschreden`

**Wettekst-excerpt** — art 17 kerngedeelte:

> "Indien geen betalingscapaciteit aanwezig is of wel betalingscapaciteit
> aanwezig is doch die betalingscapaciteit niet voldoende is om de
> belastingaanslagen waarvoor kwijtschelding is verzocht te voldoen, wordt
> ingeval sprake is van aflossingen op schulden, andere dan die waarmee
> bij de berekening van de betalingscapaciteit rekening is gehouden en
> die aflossingen meer dan € 136 per maand bedragen, slechts in zeer
> bijzondere omstandigheden kwijtschelding verleend."

| | |
|---|---|
| **Formule** | `drempel_overschreden = aflossingen_andere_schulden_maand > €136` |
| **Drempel-waarde** | `drempel_aflossingen_andere_schulden_maand = 13600` eurocent |
| **YAML-locatie** | `articles[17].machine_readable.actions[0]` |

**Review**:

- ☐ Drempel €136/maand = 13600 eurocent correct overgenomen.
- ☐ "Aflossingen op schulden *andere* dan die in betalingscapaciteit zijn meegerekend" — caller moet dit onderscheid maken. Is in formule niet gevangen; caller-verantwoordelijkheid.

---

## Output 2 — `bijzondere_omstandigheden_vereist`

**Wettekst-excerpt** — zelfde passage, alleen de voorwaarde:

| | |
|---|---|
| **Formule** | `bijzondere_omstandigheden_vereist = drempel_overschreden ∧ ¬betalingscapaciteit_toereikend` |
| **YAML-locatie** | `articles[17].machine_readable.actions[1]` |

**Review**:

- ☐ Tekst zegt "indien geen betalingscap aanwezig OF niet voldoende" — gedekt door `!betalingscapaciteit_toereikend`?
- ☐ Tekst: "EN aflossingen > €136" — gedekt door `drempel_overschreden`?
- ☐ Correctheid: AND van deze twee geeft "moment waarop extra toets nodig is".

---

## Output 3 — `kwijtschelding_blokkade_art_17`

**Wettekst-excerpt** — "slechts in zeer bijzondere omstandigheden":

| | |
|---|---|
| **Formule** | `blokkade = bijzondere_omstandigheden_vereist ∧ ¬zeer_bijzondere_omstandigheden` |
| **YAML-locatie** | `articles[17].machine_readable.actions[2]` |

**Review**:

- ☐ Blokkade true betekent kwijtschelding *geweigerd* op grond van art 17.
- ☐ "Zeer bijzondere omstandigheden" is caller-parameter (beoordeeld door ontvanger). Criteria niet in formule.
- ☐ Als caller `zeer_bijzondere_omstandigheden = true` zet, wordt de blokkade opgeheven — juist?

---

## Niet-getranslateerd

Geen `untranslatables` expliciet; wel een feitelijke beoordeling:
`zeer_bijzondere_omstandigheden` is caller-state, geen formule.

---

## Open punten voor workshop

1. **KRITIEK: art 17 wordt NIET als gate gebruikt in HHNK-leidraad art 26.**
   HHNK-leidraad sourcet vermogen (art 12) en betalingscap (art 13) uit URI,
   maar niet `kwijtschelding_blokkade_art_17`. Dat betekent: het €136-drempel-
   beleid werkt op dit moment niet door op HHNK-kwijtschelding. Moet dit wel
   meegenomen worden? (Antwoord: ja, want HHNK-leidraad zegt "ondernemers-
   regeling volgt rijks" — dan hoort art 17 er ook bij.)
2. **Ondernemer-only?** De tekst van art 17 staat in het ondernemers-deel van
   URI (afdeling 3). Is de drempel ook van toepassing op particulieren?
3. **"Zeer bijzondere omstandigheden"** — niet in formule; beleidsmatig
   beoordeeld. Zou een `untranslatable` rechtvaardigen om dit expliciet te
   benoemen?
