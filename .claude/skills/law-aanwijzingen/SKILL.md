---
name: law-aanwijzingen
description: >
  Toetst de tekst van een Nederlandse wet (op artikelniveau) tegen de Aanwijzingen
  voor de regelgeving (KCBR) en levert per bevinding een concrete suggestie. De
  bevindingen worden als JSON naar een outputpad geschreven met een TextQuoteSelector
  (exact/prefix/suffix) zodat ze als W3C-annotatie aan de wettekst verankerd kunnen
  worden. Gebruik deze skill wanneer iemand een wet bewerkt en wetgevingskwaliteits-
  feedback wil, of wanneer 'aanwijzingen', 'aanwijzingen voor de regelgeving',
  'wetgevingskwaliteit' of 'KCBR' genoemd wordt in de context van een wet-YAML.
allowed-tools: Read, Grep, Glob, Write
user-invocable: true
---

# Law Aanwijzingen — Toets wettekst tegen de Aanwijzingen voor de regelgeving

Deze skill leest de tekst van een wet-YAML en toetst die, **artikel voor artikel**,
tegen de Aanwijzingen voor de regelgeving (de officiële wetgevingskwaliteits-
richtlijnen van het KCBR). Per bevinding schrijf je een **suggestie** — je wijzigt de
wet zelf NIET. De output is een JSON-bestand dat de aanroeper omzet naar verankerde
annotaties in de editor, die de gebruiker per stuk accepteert of afwijst.

**CRITICAL — alleen suggereren, niet bewerken.** Gebruik nooit Edit op de wet-YAML.
Je enige schrijfactie is het JSON-outputbestand. De mens beslist.

## Invoer

De aanroeper geeft je:
- `LAW_YAML` — pad naar de wet-YAML (article-based, conform `schema/latest/schema.json`).
- `OUTPUT_PATH` — pad waar je het JSON-resultaat schrijft.
- Optioneel `ARTICLE_NUMBER` — beperk je tot dit artikel. Ontbreekt dit, toets alle artikelen.

## Setup

1. Lees `LAW_YAML`. Voor elk artikel telt het `text`-veld als de te toetsen tekst.
2. Lees `.claude/skills/law-aanwijzingen/reference.md` — de gecondenseerde kernset
   aanwijzingen met nummers en toetsvragen.
3. Beperk je tot `ARTICLE_NUMBER` als die is opgegeven.

## Fundamentele regel: blijf binnen de tekst

Toets uitsluitend wat er in het `text`-veld van het artikel staat. Beoordeel de
**formulering en structuur van de wettekst**, niet of het beleid wenselijk is en niet
de `machine_readable`-sectie (dat doet `law-reverse-validate`). Verzin geen bevindingen:
elke suggestie moet wijzen naar een concrete, letterlijk in de tekst aanwezige passage.

## Werkwijze

Loop per artikel door `reference.md`. Voor elke aanwijzing die van toepassing kan zijn,
stel de bijbehorende toetsvraag. Levert die een concreet probleem op in déze tekst, maak
dan één bevinding. Geen probleem → geen bevinding (liever weinig, rake bevindingen dan
veel ruis).

Per bevinding bepaal je:
- de **exacte passage** in de wettekst waar het over gaat (letterlijk overgenomen);
- genoeg **prefix/suffix** eromheen om de passage uniek te maken binnen het artikel
  (de tekst eromheen, niet verzonnen — kopieer het);
- het **aanwijzingsnummer** (uit `reference.md`; gebruik `null` als je een algemeen
  kwaliteitsprobleem signaleert dat niet aan één nummer hangt);
- een **severity** (`info` | `suggestie` | `belangrijk`);
- een **suggestietekst** in het Nederlands: wat is er aan de hand en wat is het advies;
- optioneel een **proposed_replacement**: de letterlijke vervangende tekst voor `exact`,
  alleen als je een concrete herformulering kunt geven die 1-op-1 de passage vervangt.

## Outputformaat (CRITICAL — exact dit contract)

Schrijf met de Write-tool naar `OUTPUT_PATH` één JSON-object:

```json
{
  "law_id": "<het $id uit de YAML>",
  "findings": [
    {
      "article_number": "2",
      "exact_quote": "de belanghebbende",
      "prefix": "aanspraak heeft ",
      "suffix": ", bedoeld in",
      "aanwijzing_nr": "3.56",
      "severity": "suggestie",
      "suggestion_text": "De term 'belanghebbende' is hier niet gedefinieerd terwijl elders 'aanvrager' wordt gebruikt. Aanwijzing 3.56 vraagt om consistente terminologie. Overweeg één term consequent te gebruiken.",
      "proposed_replacement": "de aanvrager"
    }
  ]
}
```

Regels voor het outputcontract:
- `exact_quote` is de te markeren passage uit de artikeltekst, **met genormaliseerde
  witruimte**: vervang elk regeleinde en elke reeks spaties/tabs door één spatie, en
  trim begin/eind. De wettekst in de YAML bevat harde regelafbrekingen midden in zinnen;
  normaliseer die weg zodat de quote één doorlopende zin is. De resolver verankert
  whitespace-tolerant (fuzzy), dus een genormaliseerde quote matcht ook als het origineel
  een `\n` op die plek had. Kopieer verder de woorden letterlijk — verzin of parafraseer niets.
- `prefix`/`suffix` zijn de tekst direct vóór/na `exact_quote`, óók met genormaliseerde
  witruimte. Mogen leeg ("") zijn, maar vul ze (een paar woorden) als de passage anders
  meerdere keren in het artikel voorkomt, zodat de verankering ondubbelzinnig is.
- `article_number` is een string en matcht het `number`-veld van het artikel.
- `aanwijzing_nr` is een string of `null`.
- `proposed_replacement` is optioneel; laat weg als je geen concrete vervanging hebt.
- Schrijf **geen** Markdown of proza buiten het JSON-object. Het bestand is puur data.
- Lege bevindingenlijst is een geldig en goed resultaat: `{"law_id": "...", "findings": []}`.

## Afronding

Schrijf het JSON-bestand. Rapporteer aan de aanroeper kort: aantal getoetste artikelen
en aantal bevindingen per severity. Wijzig niets aan de wet.
