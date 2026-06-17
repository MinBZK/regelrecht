# Taxonomie van wetgevings-fouten

Categorieën, ernst-niveaus en het per-fout entry-format voor de
`wetgevingsfouten-analyse`. Generiek; de categorieën zijn algemene typen
wetgevings-defecten (geen casus-inhoud).

## Categorieën

| Categorie | Wat | Typische ernst |
|---|---|---|
| **Lege delegaties** | Verplichte uitvoeringsregel ("bij ministeriële regeling / AMvB …") nooit afgekondigd → de concrete norm die de wet vooronderstelt ontbreekt | KRITIEK |
| **Achterhaalde institutionele referenties** | Verwijzing naar een orgaan/wet die door reorganisatie of wetsvervanging niet meer bestaat; nooit formeel bijgewerkt | KRITIEK |
| **Wettekst-fouten** | Feitelijke schrijffouten in de geldende tekst (verkeerd begrip, verkeerd lid-/hoofdstuk-nummer, errata-haakjes in de wet zelf) | KRITIEK |
| **Inconsistenties met andere wetten** | Begrip/grondslag verschilt tussen wetten die naar elkaar verwijzen; verschillende namen voor dezelfde wet | ZORG |
| **Onuitvoerbare voorwaarden zonder beslisser** | Open norm zonder kenbare beslisser, kader of grond (zie `classification.md`) — circulair of niet-toetsbaar | ZORG |
| **Onuitgewerkte regelingen** | Wel grondslag, geen invulling (bv. een aangekondigde tabel/schaal die ontbreekt) | ZORG |
| **Procedurele inconsistenties** | Procedure (bezwaar/beroep/termijn) strijdig met een algemene procesregel of intern inconsistent | ZORG/KRITIEK |
| **Impliciete koppelingen** | Eén bedrag/artikel met dubbelfunctie, zodat een wijziging elders onbedoeld doorwerkt | ZORG |
| **Geografisch-territoriale lacunes** | Aanspraak/bevoegdheid buiten het gebied waar de uitvoerder bevoegd is, zonder procedure | ZORG/KRITIEK |

## Ernst-niveaus

- **KRITIEK** — het stelsel is op dit punt niet uitvoerbaar zoals het er staat; een
  geautomatiseerde uitvoering vindt geen kenbare regel.
- **ZORG** — uitvoerbaar in de praktijk, maar juridisch onzeker, onbillijk, of
  kwetsbaar voor toekomstige wijzigingen.

## Per-fout entry-format

Elke fout volgt dezelfde structuur (zie `templates/wetgevingsfouten-analyse.md`):

1. **Titel + locatie** — categorie-nummer + wet + artikel/lid.
2. **Wettekst** — letterlijk citaat van de geldende tekst (met bron-id).
3. **Probleem** — waarom dit fout/achterhaald/onuitvoerbaar is.
4. **Implicatie** — gevolg voor de uitvoering en/of de burger (kan hij zijn recht
   kennen/berekenen/betwisten?). Hier ligt de scherpte: maak concreet wat onuitvoerbaar
   wordt.
5. **Wat zou moeten gebeuren** — concrete reparatie (technisch wijzigingsbesluit, AMvB
   afkondigen, term vervangen, criterium toevoegen). Onderscheid "klein technisch" van
   "vergt beleidsinhoudelijk werk".
6. *(optioneel)* **Mermaid-diagram** dat het probleem visualiseert (circulair criterium,
   dubbelfunctie, mismatch, territoriale lacune) — zeer effectief voor lezers.
7. *(optioneel)* **COMMENT** — ruimte voor menselijke reviewer-nuance/weerlegging.

## Closing-secties van de analyse

- **"Wat is precies uitvoeren van dit stelsel?"** — concrete opsomming van wat een
  burger vandaag *niet* geautomatiseerd kan vaststellen. Maakt de optelsom voelbaar.
- **Aanbevolen acties** in prioriteits-volgorde — eerst de kleine onbetwiste technische
  reparaties, dan het beleidsinhoudelijke werk.
- **Adressering** — aan wie de notitie is gericht (wetgevings-eigenaar + uitvoerder).
