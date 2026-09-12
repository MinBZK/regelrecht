# De verankeringszoektocht

Per statement: staat wat hier beweerd wordt óók in een norm? Het antwoord bepaalt de bucket,
en het is het enige onderdeel van de methode waar een *negatief* resultaat het waardevolste
resultaat is — de `niet-gevonden`-statements worden de jurist-vragen.

Daarom de harde eis: **een negatieve bevinding zonder vastgelegde zoektermen is geen
bevinding.** De verbatim-gate weigert een `niet-gevonden` zonder `search_terms`. Zonder die
termen kan niemand de zoektocht overdoen, en "ik kon het niet vinden" is dan niet te
onderscheiden van "ik heb slecht gezocht".

## De drie uitkomsten

| Status | Betekenis | Verplicht vast te leggen |
|---|---|---|
| `verankerd` | de normtekst stelt het | `norm_ref` (vindplaats) + `norm_quote` (verbatim) |
| `geparafraseerd` | de norm zegt iets aangrenzends in andere woorden | `norm_ref` + `norm_quote`, zodat beide teksten naast elkaar staan |
| `niet-gevonden` | geen normtekst bevat het | `search_terms` + `searched_in` |

`geparafraseerd` is geen tussencategorie voor twijfel. Gebruik hem als de norm *dezelfde*
eis stelt in andere bewoordingen (een spiegelzijde, een samenvatting, een ruimere
formulering). Als de secundaire tekst een eis toevoegt die de norm niet stelt, is het
`niet-gevonden` — ook al gaat de norm over hetzelfde onderwerp. Dat verschil is precies wat
`toelichting-bleed` van `herformulering` scheidt.

## Werkwijze

1. **Bepaal de zoekruimte** en leg hem vast in `searched_in`: welke regelingen, welke paden.
   Neem de hele keten mee — de rijksregeling, de lagere regeling én de gemeentelijke of
   waterschapsregeling. Een statement dat in de verordening niet staat, staat soms in de
   uitvoeringsregeling erboven.

   **Sluit het document dat je analyseert uit van de zoekruimte.** Secundaire teksten staan
   vaak zélf in het corpus (een toelichting als `regulatory_layer: UITVOERINGSBELEID`). Zoek
   je zonder uitsluiting, dan vindt de grep het statement terug in zijn eigen bron en label je
   `verankerd` waar `niet-gevonden` hoort — precies de fout die de hele methode moet
   voorkomen. Noteer de uitsluiting in `searched_in`:

   ```yaml
   searched_in:
     - regulation/**/*.yaml
     - '!regulation/**/uitvoeringsbeleid/**'   # de geanalyseerde tekst zelf
   ```

   **Bij een HTML-bron: begin bij `links.tsv`.** Uitvoeringsteksten linken hun eigen
   verwijzingen vaak rechtstreeks naar `wetten.overheid.nl`, mét BWB-ID en artikelanker. Dat
   is de auteur die zegt welke norm hij bedoelt — je hoeft het niet te raden. Maar het is een
   *claim*, geen bewijs: een link kan naar een kapstok-artikel wijzen terwijl de operatieve
   norm elders staat, of naar een inmiddels gewijzigde versie. Gebruik hem als eerste spoor
   en verifieer alsnog in de normtekst; noteer de link als `norm_ref`-kandidaat, niet als
   uitkomst.

2. **Zoek op meerdere formuleringen, niet op één.** Een norm gebruikt zelden het woord van de
   toelichting. Werk minstens drie sporen af:
   - de **kernterm** uit het statement (*"vrijlating"*)
   - het **getal of de eenheid** los (*"50"*, *"50%"*, *"vijftig"*)
   - het **wettelijke synoniem** (*"aangewend"*, *"aanwending"*, *"buiten beschouwing"*)

   Getallen zijn het betrouwbaarste spoor: normen schrijven ze soms voluit (*"ten minste
   tachtig percent"*), dus zoek beide vormen.

3. **Zoek in de verbatim `text:`-velden, niet in `machine_readable`.** Het model is precies
   wat je toetst; als je erin zoekt bevestig je je eigen aanname. Beperk daarom tot de
   normtekst:

   ```bash
   grep -rn --include='*.yaml' -i -e 'vrijlat' -e 'aangewend' -e '80 percent' regulation/
   ```

4. **Leg de uitkomst vast, ook als hij negatief is.** De zoektermen die *niets* opleverden
   horen er net zo goed in als de term die wel raak was.

## Vastleggingsformaat

```yaml
anchoring:
  status: niet-gevonden
  search_terms: ['50%', 'vijftig procent', 'vrijlating', 'vrijgelaten', 'aangewend']
  searched_in: ['regulation/nl/**/*.yaml', 'regulation/local/**/*.yaml']
```

```yaml
anchoring:
  status: verankerd
  norm_ref: fictieve_verordening art 3 lid 1
  norm_quote: ten minste 80 percent van het besteedbaar inkomen is aangewend
```

Bij `verankerd` en `geparafraseerd` is het norm-citaat verbatim uit de normtekst. Dat maakt
de vergelijking in het register controleerbaar zonder dat de lezer twee bestanden hoeft open
te slaan — en het legt meteen bloot wanneer "verankerd" eigenlijk "geparafraseerd" was.

## De valkuil

De verleiding is om na een korte zoektocht `niet-gevonden` te noteren en het statement als
MODELFOUT te classificeren, omdat "het model dit niet doet". Dat is de omkering: als het niet
in de norm staat, is het model **goed** en is het statement een jurist-vraag. Elke
`niet-gevonden` die je als MODELFOUT labelt, zet de toelichting boven de wet. Zie
`classificatie.md`.
