# Modellering-fixes-plan — wat onze YAML moet aanpassen om bij de wet te kloppen

**Datum**: {datum} · **Scope**: {welke wetten} · **Bron**: {validatie-review(s)}

> Dit document bevat **modellering-fouten**: gevallen waar onze YAML/feature afwijkt van
> de (correcte) wettekst. Voor fouten in de wet zelf, zie `{pad naar wetgevingsfouten-analyse}`.

## Bevindingen die NIET opnieuw onderzocht hoeven worden

*Vastgesteld in de validatie-review; direct fixbaar.*

### Kritieke fouten in YAML + features
| # | Wet / artikel | Fout | Bron-bevestiging |
|---|---|---|---|
| 1 | {wet art X} | {wat klopt niet t.o.v. wettekst} | {YAML-regels / feature-regels} |

### Kritieke fouten alleen in YAML
| # | Wet / artikel | Fout |
|---|---|---|
| {n} | {wet art X} | {ontbrekend lid / verkeerde operator / verkeerde eenheid} |

### Structurele problemen
| Probleem | Bestanden |
|---|---|
| {duplicaat-`$id` / verkeerde map / inconsistente schema-URL / lege placeholder} | {paden} |

### Wél getrouw (niet aanraken)
- {wat correct is — expliciet, om dubbel werk te voorkomen}

## Fix-plan

| Fix | Wet/artikel | Wat | Validatie na fix |
|---|---|---|---|
| F1 | {…} | {concrete YAML-wijziging} | {commando → verwacht resultaat} |

## Open vraag (te verifiëren)

- {bijv. komt een waarde uit een nog-niet-geharveste cross-law-bron? → naar tracker}
