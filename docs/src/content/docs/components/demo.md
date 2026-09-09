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
6. **Burger.nl / Overheid.nl**: het portaal van de actieve persona. Elke regeling wordt live berekend; onder "Gebruikte gegevens" staat waar elk gegeven vandaan komt, met per gegeven de mogelijkheid het te corrigeren. Een regeling met een beschikking wordt vanuit het portaal aangevraagd, in een paneel dat de flow van de POC volgt: de wet rekent met wat de overheid al weet en vraagt één voor één alleen wat in geen register staat (de huurprijs, de terraslocatie), rekent na elk antwoord opnieuw, laat de uitkomst en de gebruikte gegevens controleren, en na indiening de status; na een besluit kan de burger daar bezwaar maken. Het portaal springt nooit naar het zaaksysteem, dat is de andere wereld.
7. **Zaaksysteem**: de behandelaarskant, per uitvoerende organisatie: een bord met ingediende, lopende en besloten zaken, de regelingen die de organisatie uitvoert, de herberekening door de engine naast het aangevraagde resultaat, correcties van burgers ter beoordeling, toekennen of afwijzen, bezwaar.

Het menu wisselt van profiel (Merijn, burger; Claudia, ondernemer), zet handmatige beoordeling aan of uit, schakelt het kleurschema en reset de demo.

## Hoe het werkt

Er is geen backend. De engine draait als WebAssembly in de browser, dezelfde engine als in de editor. Het demo-corpus wordt bij de build naar `public/data` gekopieerd. Persona-data (`profiles.yaml`) wordt via `bindings.yaml` per wet gematerialiseerd tot records en als wet-gebonden databron in de engine gezet; correcties van de burger komen daar als tweede bron met hogere prioriteit bovenop. Aanvragen en correcties leven in `localStorage`.

## Lokaal draaien

```bash
just dev-demo          # bouwt de WASM-engine en start Vite op :7400
```

Scenario's natively controleren, buiten de browser:

```bash
just bdd-demo
```

## Verder lezen

- [Deployment](/operations/deployment)
