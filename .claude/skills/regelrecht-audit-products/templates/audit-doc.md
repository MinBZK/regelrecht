# Audit — {Wet/regeling} {artikel}

**Wet**: {volledige naam + versie/jaar}
**`$id`**: `{law_id}`
**Type**: {wet / ministeriële regeling / beleidsregel / verordening / …}
**Wet-URL**: {officiële publicatie-URL}
**YAML-bestand**: `{pad naar YAML}`
**Laatste review**: —
**Reviewer(s)**: —

---

## Werkwijze

{Eén of meer zinnen: hoe is dit artikel gestructureerd, en hoe is de
machine_readable-vertaling opgezet (bijv. alles wat de beschikking bepaalt in één
`machine_readable`-block; per output gekoppeld aan de wettekst).}

Symbolische kortschriften die in formules worden gebruikt:

- `{symbool}` = `{parameter/output-naam}` ({korte uitleg})
- …

---

## (optioneel) Wijzigingen t.o.v. vorige versie

*Alleen invullen als deze YAML een nieuwere versie van een eerdere is.*

| Subartikel | Oude tekst | Nieuwe tekst | Impact op MR |
|---|---|---|---|
| {x} | {…} | {…} | {Geen / wel — toelichting} |

**Totaal-impact op machine_readable**: {NIHIL / beperkt / substantieel — onderbouw}.

---

## Output {n} — `{output_naam}`

**Wettekst-excerpt** — uit {wet} {subsectie} *"{kopje}"*:

> "{letterlijk citaat uit de wettekst}"

🔗 {wet-URL}#{anker} (subsectie {x})

| | |
|---|---|
| **Formule** | `{formule in natuurlijke/wiskundige notatie — AND/OR/IF/MAX/...}` |
| **Operanden / bron** | {korte uitleg van elke operand; `source:`-verwijzingen} |
| **Interpretatie** | {wat de formule betekent en welke aannames erin zitten} |
| **YAML-locatie** | `articles[…].machine_readable.execution.actions[{i}]` |

**Review**:

- [ ] {Punt dat de jurist apart moet bevestigen — bv. dekt de formule de wettekst?} Notitie: —
- [ ] {Klopt de bron-/source-verwijzing?} Notitie: —
- [ ] {Specifieke twijfel / interpretatie-keuze die bekrachtigd moet worden.} Notitie: —

**Open** *(indien van toepassing)*:

- {vraag of onduidelijkheid die nog beslist moet worden}

---

*(herhaal het Output-blok per `machine_readable` output)*

---

## Niet-getranslateerd (`untranslatables`)

Wat in {artikel} staat maar bewust niet in een formule is vertaald. Per grond:
**factual** (feitelijk vaststelbaar → kán alsnog gate worden) of **judgment**
(oordeel/prognose → blijft untranslatable).

| Subsectie | Wettekst-kern | Type | Reden | Review |
|---|---|---|---|---|
| {x} | {citaat-kern} | factual / judgment | {waarom niet gemodelleerd} | [ ] Accepteerbaar? |

---

## Externe override(s) — juridische nuance

*Alleen invullen als een andere regeling een waarde/formule in deze keten override't.*

| Override | Bron | Doel | Effect |
|---|---|---|---|
| {wat} | {bron-regeling art X} | `{output}` | {nieuwe waarde/gedrag} |

### ⚠ Twijfel over grondslag *(indien van toepassing)*

{Werkt de override mechanisch correct, maar klopt de juridische grondslag-attributie?
Beschrijf de discrepantie YAML ↔ juridische werkelijkheid en de mogelijke opties.}

- [ ] {Welke optie?} Notitie: —

---

## Open punten voor de sessie

1. {open punt — formule-twijfel, missende grond, koppeling die ontbreekt, …}
2. …

---

## Vervolg

Na review: vink `[ ]` → `[x]` en schrijf afwijkingen op in een "Bevindingen"-blok
onderaan. Bij *wijzigen* van een interpretatie: aparte commit met de YAML-update,
testen groen, formules-doc opnieuw genereren.
</content>
