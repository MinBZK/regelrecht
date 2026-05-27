# Cyclus {N} — Resolutie-tracker (comments + wetgevings-fouten)

**Datum start**: {datum} · **Plan**: `{pad}` · **Bron-dossier**: `{pad}`
**Laatste update**: {datum} ({wat is bijgewerkt})

## Doel

Per inline-comment en per wetgevings-fout vastleggen of die deze cyclus is geresolveerd
of nog open staat. Leidende status-administratie voor de cyclus.

## Legenda — status

| Status | Betekenis |
|---|---|
| `open` | Niets ondernomen deze cyclus |
| `in-progress` | Bron-onderzoek loopt of resolutie wordt geschreven |
| `verifieerd` | Externe bron bevestigt de claim (referentie in bron-kolom) |
| `weerlegd` | Externe bron weerlegt de claim |
| `gedeeltelijk` | Bron resolveert deel; rest blijft open |
| `onverifieerbaar` | Geen uitspraak mogelijk via bronnen; mens/dossier nodig |
| `out-of-scope` | Bewust niet behandeld deze cyclus |

## Legenda — scope

{Codes voor de thema's van deze cyclus, bijv.} **{X}** = {thema} · **{Y}** = {thema} ·
**—** = buiten scope.

## A. Inline-comments ({aantal})

| # | Locatie | Scope | Kern | Status | Bron | Voorlopig gevolg |
|---|---|---|---|---|---|---|
| 1 | §{x} | {X/Y/—} | {kern van de comment} | {status} | {bron-bestand(en)} | {wat het betekent} |

## B. Wetgevings-fouten ({aantal})

| ID | Titel | Scope | Status | Bron | Voorlopig gevolg |
|---|---|---|---|---|---|
| §{n.m} | {titel} | {X/Y/—} | {status} | {bron} | {gevolg} |

## Stand {datum} — einde {micro-cyclus}

- **In scope, open te resolveren**: {lijst ID's}
- **Out-of-scope** (volgende cyclus): {aantal} comments + {aantal} fouten
- **Verwacht patroon**: {welke worden weerlegd/verifieerd/onverifieerbaar}
</content>
