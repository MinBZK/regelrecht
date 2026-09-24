---
id: specificaties-i-documentatie-op-orde
titel: 'Specificaties I: documentatie op orde'
faseId: wat
disciplineId: techniek
prioriteit: hoog
omvang: L
categorie: bet
capability: basis
capaciteit: conceptueel schrijver, analisten voor het omzetwerk
toelichting: |-
  De taal waarin een wet machine-uitvoerbaar wordt vastgelegd bestaat uit het
  schema en de engine die het uitvoert. Het ontwerp daarvan staat nu verspreid
  over de RFC's, en die reeks loopt uiteen in omvang en impact: de een
  legt één veld vast, de ander beschrijft een heel subsysteem. We passen ze
  bovendien voortdurend aan omdat de code verder is, en daarmee vervalt waar een
  RFC voor bedoeld is, een voorstel op een moment.

  Dit werkpakket zet die documentatie om naar geversioneerde specificaties: de
  taal op één plek, bijgewerkt als de taal verandert, met een versienummer dat
  zegt welke toestand je leest. De ontwerpdiscussie blijft bestaan en komt los te
  staan van de specificatie die eruit volgt.

  **Schema en engine los van de subsystemen**

  In dezelfde reeks zitten documenten over de harvester, de enricher en de
  validatie van interpretaties. Die beschrijven hoe wij werken en niet wat de
  taal is; een tweede partij die een eigen engine bouwt heeft er niets aan. Het
  uit elkaar halen van die twee soorten is onderdeel van dit werkpakket.

  **Twee dingen heten nu specificatie**

  Het ene is de specificatie van de taal, waar dit werkpakket over gaat. Het
  andere is de machine-leesbare interpretatie van een wet, het ding dat een
  jurist vaststelt. Die dubbele betekenis zit ook in de werkpakketten
  "Vaststelling van specificaties" en "Juridische status van een specificatie",
  die over het tweede gaan. Welk woord waar hoort is een open punt.

  **Stand**: het schema documenteert zichzelf en staat per versie op
  /reference/schema (RFC-040); de taal als één geversioneerde specificatie, los
  van de RFC's en de subsystemen, bestaat nog niet.
volgorde: 1000
onderzoeksvragen:
  - Welke van de bestaande RFC's beschrijven de taal zelf, en welke beschrijven
    een subsysteem dat toevallig van ons is?
  - Hoe versioneren we de specificatie van de taal? Het schema heeft al vijftien
    versies achter zich zonder vastgelegd release- of deprecatiebeleid.
  - 'Wat doen we met een RFC waarvan het ontwerp achterhaald is: intrekken,
    markeren als vervangen, of laten staan als verslag van een keuze?'
  - vraag: >-
      Voor wie schrijven we de specificatie van de taal: voor een tweede
      engine-bouwer, voor een jurist die een codering leest, of voor beide met
      verschillende documenten?
    paper: sec:stewarding
onderzoek: ''
bouw: deels
rfcs:
  - 1
  - 2
  - 4
  - 11
  - 16
  - 21
  - 23
  - 24
  - 36
  - 37
  - 38
  - 40
  - 41
samenhangIds:
  - specificaties-ii-enrichment-overhaul-landen
  - specificaties-iii-gaten-vinden-met-de-enricher
  - specificaties-iv-de-ontbrekende-delen-bouwen
  - specificaties-v-beproeving-van-de-taal
  - juridische-status-van-een-specificatie
  - analyseren-van-wet-en-regelgeving
---
