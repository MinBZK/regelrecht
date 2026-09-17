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
  casus die de hele keten van wet naar uitvoering laat zien, van harvester tot
  en met de engine. Het gaat om de belangrijkste capabilities — het
  ontwikkelen en implementeren van een specificatie — end-to-end voor één
  representatieve regeling, zodat zichtbaar wordt of harvester, enricher,
  specificatietaal, editor en engine daadwerkelijk op elkaar aansluiten
  zonder handmatige tussenstappen. Pas als deze ene casus volledig door de
  keten loopt, is aangetoond dat het ecosysteem als geheel werkt; de overige
  capabilities (zoals simuleren, publiceren, analyseren en verifiëren) volgen
  in "Referentie casus II".

  **Voorlopige Definition of Done** (werkhypothese bij de vraag wanneer de
  integratie "volledig" is):

  - Een werkend endpoint tot de casus (proces en rechtsgevolg) waar een
    aanvraag doorheen kan lopen die leidt tot een rechtsgevolg met
    traceability.
  - Datakoppelingen: het moet mogelijk zijn om data te koppelen aan een
    implementatie van de regels in het systeem.
  - De regels hebben een validatieprocedure doorlopen en de validiteit van de
    implementatie is vastgesteld.

  **Voorlopige toedeling van capabilities:** naast ontwikkelen en
  implementeren horen ook data-integratie en validatie 0.1 bij deze eerste
  casus.
volgorde: 2000
onderzoeksvragen:
  - Welke regeling en welk scenario zijn representatief genoeg om als eerste,
    volledige referentiecasus te dienen?
  - Met welke externe afhankelijkheden hebben we rekening te houden die
    volledige implementatie kunnen belemmeren?
  - Welke aanpassingen aan harvester, enricher, specificatietaal, editor en
    engine zijn nodig om deze casus zonder handmatige tussenstappen door de
    hele keten te laten lopen?
  - Aan welke criteria moet voldaan zijn om deze verticale integratie
    "volledig" te noemen?
  - Welke capabilities horen bij deze eerste casus (ontwikkelen,
    implementeren), en welke verschuiven naar de overige capabilities in
    "Referentie casus II"?
  - Welke elementen van de referentiecasus zijn herbruikbaar en abstraheerbaar
    voor "Referentie casus II"?
bouw: deels
rfcs:
  - 6
samenhangIds: []
---
