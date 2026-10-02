---
id: referentie-casus-i
titel: Referentie casus I
faseId: wat
disciplineId: techniek
prioriteit: hoog
omvang: ''
categorie: bet
capability: ''
capaciteit: ''
toelichting: |-
  Dit werkpakket bouwt een verticale referentie-implementatie: één complete
  casus die de keten van wet tot werkend aanvraagsysteem laat zien. Het corpus
  modelleert de rekenregels van één regeling volgens het schema; de vraag is
  of die regels in de praktijk ook een systeem kunnen dragen.

  Om die rekenregels heen ontbreken nog technische constructen die een
  aanvraagsysteem nodig heeft. Er moet een herbruikbare frontend op te bouwen
  zijn. En het beoordelen van de codering moet patronen en gereedschap
  opleveren waar een volgende casus mee verder kan.

  Wat onderweg wordt aangepast, doen we zo generiek als op dat moment kan.
  Deze eerste casus beslist niet wat de generieke vorm is: hij levert een
  werkende specifieke implementatie en de signalen over wat generiek moet
  worden. Die signalen landen in de Specificaties-reeks.

  Pas als deze ene casus volledig door de keten loopt, is aangetoond dat het
  ecosysteem als geheel werkt; de overige capabilities (zoals simuleren,
  publiceren, analyseren en verifiëren) volgen in "Referentie casus II".

  **Voorlopige Definition of Done** (werkhypothese bij de vraag wanneer de
  integratie "volledig" is):

  - Een werkend aanvraagsysteem voor de casus (proces en rechtsgevolg) waar
    een aanvraag doorheen kan lopen die leidt tot een rechtsgevolg met
    traceability.
  - Datakoppelingen: het moet mogelijk zijn om data te koppelen aan een
    implementatie van de regels in het systeem.
  - Uit de manier waarop de analist de werking van deze regeling heeft
    beoordeeld, zijn de patronen en het gereedschap gedestilleerd die zich
    laten opwerken tot generieke tooling of een generiek proces.

  **Voorlopige toedeling van capabilities:** naast ontwikkelen en
  implementeren horen ook data-integratie en validatie 0.1 bij deze eerste
  casus.
volgorde: 2000
onderzoeksvragen:
  - >-
    Technisch: In het corpus worden de rekenregels uit de regelgeving
    gemodelleerd, volgens het schema. Welke technische constructen daarbuiten
    ontbreken nog om daarop in de praktijk een aanvraagsysteem te bouwen?
    Daarbij hoort het koppelen van data of representatieve overgangsdata, en
    optioneel raakvlakken met andere systemen.
  - >-
    Frontend: Kan een herbruikbare frontend worden vormgegeven op basis van
    machine-uitvoerbare regelgeving (YAML's), en welke generieke componenten
    zijn daarvoor nodig?
  - >-
    Methode: Wat zijn patronen en tooling in het analyseren en valideren van de
    regelgevings-YAML's die herbruikbaar zijn voor volgende casussen, en waar
    houdt gereedschap op en begint menselijk oordeel?
  - >-
    Generiek: Welke verbeteringen aan bestaande onderdelen van het ecosysteem,
    zoals het schema en de editor, komen uit deze casus naar boven, en hoe
    leggen we ze vast zodat een volgende casus ze niet opnieuw hoeft te
    ontdekken?
  - >-
    Generiek: Welke elementen van de referentiecasus zijn herbruikbaar en
    abstraheerbaar voor "Referentie casus II"?
bouw: deels
belegging:
  stand: opgepakt
  sinds: '2026-09-17'
rfcs:
  - 6
samenhangIds:
  - aansluiten-op-bronnen-chronolexografie
  - vaststelling-van-specificaties
  - specificaties-ii-enrichment-overhaul-landen
  - specificaties-iii-gaten-vinden-met-de-enricher
---
