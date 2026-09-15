---
id: specificaties-iii-gaten-vinden-met-de-enricher
titel: 'Specificaties III: gaten vinden met de enricher op schaal'
faseId: wat
disciplineId: techniek
prioriteit: hoog
omvang: L
categorie: bet
capability: basis
capaciteit: engineer, analisten, rekenkracht
toelichting: |-
  De taal verbetert door hem stuk te laten lopen op echte wetgeving. Dit
  werkpakket zet de enricher op grote hoeveelheden wetten aan het werk, verzamelt
  waar hij vastloopt, past de taal aan, en draait opnieuw over hetzelfde
  materiaal. Die lus is het werk.

  Het doel is niet een compleet corpus. Het doel is een specificatie die goed
  genoeg is dat er betrouwbare interpretaties uit komen; het corpus dat onderweg
  ontstaat is een bijproduct. Dat verschil bepaalt ook wat we meten. De maat is
  welke normen de taal structureel niet kan uitdrukken; hoeveel artikelen een
  model kreeg zegt daar niets over.

  **Waar de gaten zichtbaar worden**

  De enricher laat markeringen achter op de plekken waar hij niet verder kan.
  Sinds de splitsing uit Specificaties II staan die in twee kanalen: een norm die
  in een nog niet gevonden document wordt ingevuld, en een norm die de taal niet
  aankan. Alleen het tweede kanaal stuurt dit werkpakket. Het eerste is voer voor
  de harvester.

  **Rekenkracht is een randvoorwaarde**

  Draaien op schaal vraagt meer capaciteit dan een losse run. Dat werk staat
  apart op de roadmap onder "Infrastructuur voor lokale AI" en gaat hieraan
  vooraf.
volgorde: 1200
onderzoeksvragen:
  - Welke markeringen wijzen op een gat in de taal, en welke op een gat in het
    corpus of in de werkvoorraad van de enricher?
  - Over hoeveel wetten moet een ronde lopen voordat het patroon van ontbrekende
    concepten stabiel is?
  - Hoe vergelijken we twee rondes over hetzelfde materiaal, zodat een aanpassing
    aan de taal aantoonbaar iets heeft opgelost?
  - vraag: >-
      Wat publiceren we over de normen die het formaat nog niet kan uitdrukken, en
      hoe voorkomen we dat zo'n markering wegvalt tegen de indruk dat de wet wel
      volledig is gecodeerd?
    paper: sec:untranslatables
  - vraag: >-
      Welke structurele fouten in een gecodeerde wet komen bij analyse op schaal
      pas aan het licht, en wat zegt de frequentie ervan over het formaat zelf?
    paper: sec:structuralanalysis
onderzoek: open
bouw: deels
rfcs:
  - 12
  - 35
afhankelijkVan:
  - specificaties-ii-enrichment-overhaul-landen
  - infrastructuur-voor-lokale-ai
samenhangIds:
  - specificaties-i-documentatie-op-orde
  - specificaties-ii-enrichment-overhaul-landen
  - specificaties-iv-de-ontbrekende-delen-bouwen
  - infrastructuur-voor-lokale-ai
---
