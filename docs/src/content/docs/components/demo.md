---
title: "Demo"
description: "De RegelRecht-demo: presentatie, wetten, graaf, scenario's, simulatie, burgerportaal en zaaksysteem, met de engine als WASM in de browser."
---

De demo laat in één werkruimte zien wat RegelRecht doet: de machine-leesbare wet, de afhankelijkheden tussen wetten, de scenario's die de wet toetsen, en de uitvoering voor één persoon op een portaal en in een zaaksysteem. Het is de opvolger van de losse `poc-machine-law`-repository.

## Overzicht

- **Taal**: Vue 3 / Vite, `@nldd/design-system`
- **Locatie**: `frontend-demo/`
- **Corpus**: `corpus/demo/`
- **Productie-URL**: `demo.regelrecht.rijks.app` (nog niet in `deploy.yml` opgenomen; zie het plan onder [Deployment](/operations/deployment))

## Wat het doet

De werkruimte heeft zeven tabbladen, in de volgorde van een presentatie:

1. **Presentatie**: het dek in Rijkshuisstijl-blauw. De intro staat voluit; daarna staat het dek links als rail en opent elke dia zelf het tabblad waar het over gaat, wisselt van persona en wijst aan wat de presentator bedoelt. De dia's zijn inhoud (`demo-config.yaml`), geen code. Esc sluit het dek en laat de demo staan; Shift+P opent het overal.
2. **Wetten**: de machine-leesbare wet als opvouwbare YAML-boom, per verhaal klaargezet (`expanded_paths` in `demo-config.yaml`: een pad opent zichzelf en alles erboven, de rest blijft dicht). Elke `source.regulation` is een link die de verwezen wet opent; een terugknop loopt de gevolgde verwijzingen terug. De lijst met alle wetten, per organisatie, is een zijpaneel dat standaard dicht staat.
3. **Graaf**: per wet de bronnen, de invoer uit andere wetten en de uitvoer, met lijnen naar de leverende wet en de waarden van de persona erop, in een kleur per organisatie. Het profiel kiest de wetten van het verhaal (`graph_laws`); getoond worden die wetten en alles wat er direct aan hangt. Het hele corpus ligt vast, dus "Alles" voegt wetten toe zonder dat er iets verschuift. Een gekozen wet kleurt de lijnen die ze leest groen en de lijnen waarlangs anderen haar lezen rood.
4. **Scenario's**: de Gherkin-scenario's per wet, in het Nederlands weergegeven. "Uitvoeren" draait een scenario in de browser tegen de engine en toont de volledige uitvoeringstrace.
5. **Simulatie**: een gegenereerde populatie burgers of ondernemers (aantal, leeftijds- en inkomensverdeling, bedrijfstype, seed) wordt door de engine door alle regelingen van dat portaal gehaald. Het resultaat: wie voldoet aan de voorwaarden en voor hoeveel, uitgesplitst naar leeftijd, inkomen, partner, bedrijfstype of grootte, en voor burgers het besteedbaar inkomen per maand (inkomen min belastingen plus toeslagen en uitkeringen; welke uitvoer meetelt en per maand of per jaar staat in `simulation.disposable_income`). Constanten uit de wetten (drempels, percentages) zijn per run aan te passen; runs staan naast elkaar ter vergelijking en zijn als CSV of JSON te exporteren.
6. **Burger.nl / Overheid.nl**: het portaal van de actieve persona. Elke regeling wordt live berekend; onder "Gebruikte gegevens" staat waar elk gegeven vandaan komt, met per gegeven de mogelijkheid het te corrigeren. Een regeling met een beschikking wordt vanuit het portaal aangevraagd, in een paneel dat de flow van de POC volgt: de wet rekent met wat de overheid al weet en vraagt één voor één alleen wat in geen register staat (de huurprijs, de terraslocatie), rekent na elk antwoord opnieuw, laat de uitkomst en de gebruikte gegevens controleren, en na indiening de status. Zodra het besluit is bekendgemaakt staat er tot wanneer bezwaar mogelijk is: die datum komt uit de Awb zelf (artikel 6:7 voor de termijn, 6:8 voor de einddatum), niet uit het scherm. Het portaal springt nooit naar het zaaksysteem, dat is de andere wereld.
7. **Zaaksysteem**: de behandelaarskant, per uitvoerende organisatie: een bord met zaken die te beoordelen zijn, zaken die bekend te maken zijn en zaken die bekendgemaakt zijn, de regelingen die de organisatie uitvoert, de herberekening door de engine naast het aangevraagde resultaat, correcties van burgers ter beoordeling, toekennen of afwijzen, bekendmaken, bezwaar. Bekendmaken is een eigen handeling, omdat de Awb het besluit (artikel 1:3) en het bekendmaken ervan (artikel 3:41) als twee momenten kent en pas het tweede de bezwaartermijn laat lopen.

Het menu wisselt van profiel (Merijn, burger; Claudia, ondernemer), zet handmatige beoordeling aan of uit, schakelt het kleurschema en reset de demo.

## Hoe het werkt

Er is geen backend. De engine draait als WebAssembly in de browser, dezelfde engine als in de editor. Het demo-corpus wordt bij de build naar `public/data` gekopieerd. Persona-data (`profiles.yaml`) wordt via `bindings.yaml` per wet gematerialiseerd tot records en als wet-gebonden databron in de engine gezet; correcties van de burger komen daar als tweede bron met hogere prioriteit bovenop. Aanvragen en correcties leven in `localStorage`.

Een aanvraag voor een beschikking loopt door de fasen die de Awb eraan geeft (RFC-007, RFC-008). De engine vuurt per fase de haken die erbij horen en zegt welk gegeven hij nog mist; de demo levert dat aan op het moment dat het bestaat, en bewaart de stand bij de zaak. Zo komt de bezwaartermijn als datum uit de wet, en telt een bijzondere wet die van artikel 6:7 afwijkt vanzelf mee.

## Lokaal draaien

```bash
just demo              # bouwt de WASM-engine, start Vite op :7400 en opent de browser
just dev-demo          # hetzelfde, zonder de browser te openen
```

Controleren, buiten de browser:

```bash
just demo-check        # wetten, scenario's, frontend-tests, WASM en build
just bdd-demo          # alleen de scenario's
just validate-demo     # alleen de wetten (schema en typecontrole)
```

## Verder lezen

- [Deployment](/operations/deployment)
