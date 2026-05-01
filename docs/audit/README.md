# Audit-documenten

Per-artikel review-checklists voor de machine-readable vertaling van wet- en
regelgeving. Elk document koppelt een machine_readable output aan de
wettekst waaruit hij is afgeleid, zodat een jurist item-voor-item kan
controleren of de automatische interpretatie correct is.

## Format

Per artikel één Markdown-bestand met:

- **Kopblok**: wet-identiteit, YAML-pad, wet-URL, laatste-review-datum
- **Output-tabellen**: één rij per `machine_readable` output, met kolommen:
  - *Wettekst-excerpt* — letterlijk citaat uit het artikel (of onderliggende
    artikelen via `source:`), met exacte subsectie-aanduiding
  - *Formule* — de afgeleide logica in wiskundige of natuurlijke taal
    notatie (AND/OR/IF/MAX/MULTIPLY, geen YAML)
  - *YAML-locatie* — pad in het YAML-bestand: `articles[N].machine_readable.actions[M]`
  - *Review-checkboxes* — punten die een jurist apart moet bevestigen
- **Untranslatables-tabel**: wat *bewust* niet is gecodeerd, met reden
- **Open punten**: vragen of onduidelijkheden die nog beslist moeten worden

## Workshop-gebruik

Voor een workshop met juristen is het format ontworpen om:

1. Per artikel kort te presenteren — één schermvullend tabel per output
2. Item-voor-item te doorlopen — elke rij is een beslismoment
3. Directe koppeling naar wetten.overheid.nl / lokaleregelgeving.overheid.nl
   via de URL bovenaan en de subsectie-labels bij elk excerpt
4. In git geversioneerd — afvinken gebeurt in commits zodat later traceerbaar
   is wie wat wanneer heeft goedgekeurd

## Afvinken

Checkboxes `☐` → `☑` invullen met een korte toelichting indien relevant. Bij
afwijking: niet aanpassen zonder overleg — open een discussie in de PR of
een issue, en *noteer* in dit document welke wettekst niet (volledig) door
de formule is gedekt.

## Genereren

Voorlopig handmatig per artikel. Als het format bevalt kan `script/
generate-audit.js` dit later auto-genereren uit de YAMLs, met template-
placeholders voor wettekst-excerpts.

## Bestaande audits

**HHNK-keten (beleidsregel + verordening):**

- [hhnk-leidraad-art-26.md](hhnk-leidraad-art-26.md) — HHNK-leidraad
  invordering waterschapsbelastingen, artikel 26 (kwijtschelding) — de
  orchestrator.

**Federale ministeriele regeling (URI 1990):**

- [uri-art-11.md](uri-art-11.md) — Kwijtschelding-hoofdbepaling (kan + hoogte)
- [uri-art-12.md](uri-art-12.md) — Vermogen (bezittingen, drempels, open_term)
- [uri-art-13.md](uri-art-13.md) — Betalingscapaciteit (12× (inkomen − uitgaven − kostennorm))
- [uri-art-14.md](uri-art-14.md) — Netto-besteedbaar inkomen (5 categorieën)
- [uri-art-15.md](uri-art-15.md) — Uitgaven (6 categorieën, woonlasten met cap)
- [uri-art-16.md](uri-art-16.md) — Kostennorm (open_term + 7 SHORTCUT definitions)
- [uri-art-17.md](uri-art-17.md) — €136-drempel ondernemers

**Nog te doen:**

- Regeling kwijtschelding belastingen medeoverheden art 2, 3, 4
- HHNK-verordening art 1, 2, 3, 4, 5
- Leidraad Invordering 2008 art 26.1.9, 26.2.2, 26.2.3
- Zorgverzekeringswet art 41
- BRP afgeleid-kwijtschelding
