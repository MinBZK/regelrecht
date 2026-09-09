---
title: "Demo"
description: "De RegelRecht-demo: presentatie, wetten, graaf, scenario's, simulatie, burgerportaal en zaaksysteem, met de engine als WASM in de browser."
---

De demo laat in één werkruimte zien wat RegelRecht doet: de machine-leesbare wet, de afhankelijkheden tussen wetten, de scenario's die de wet toetsen, en de uitvoering voor één persoon op een portaal en in een zaaksysteem. Het is de opvolger van de losse `poc-machine-law`-repository.

## Overzicht

- **Taal**: Vue 3 / Vite, `@nldd/design-system`
- **Locatie**: `frontend-demo/`
- **Corpus**: `corpus/demo/`
- **Productie-URL**: `demo.regelrecht.rijks.app` (nog niet in `deploy.yml` opgenomen)

## Wat het doet

De werkruimte heeft zeven tabbladen, in de volgorde van een presentatie:

1. **Presentatie**: de openingsdia's; op de laatste dia gaat de pijl naar rechts door naar de wetten.
2. **Wetten**: alle demo-wetten, gegroepeerd per uitvoerende organisatie, als opvouwbare YAML-boom. Elke `source.regulation` is een link die de verwezen wet in een nieuw wettabblad opent.
3. **Graaf**: de afhankelijkheden tussen de wetten, per profiel of voor het hele corpus.
4. **Scenario's**: de Gherkin-scenario's per wet, in het Nederlands weergegeven. "Uitvoeren" draait een scenario in de browser tegen de engine en toont de volledige uitvoeringstrace.
5. **Simulatie**: een gegenereerde populatie burgers of ondernemers (aantal, leeftijds- en inkomensverdeling, bedrijfstype, seed) wordt door de engine door alle regelingen van dat portaal gehaald. Het resultaat: wie voldoet aan de voorwaarden en voor hoeveel, uitgesplitst naar leeftijd, inkomen, partner, bedrijfstype of grootte. Constanten uit de wetten (drempels, percentages) zijn per run aan te passen; runs staan naast elkaar ter vergelijking en zijn als CSV of JSON te exporteren.
6. **Burger.nl / Overheid.nl**: het portaal van de actieve persona. Elke regeling wordt live berekend; onder "Gebruikte gegevens" staat waar elk gegeven vandaan komt, met per gegeven de mogelijkheid het te corrigeren. Een regeling met een beschikking wordt vanuit het portaal aangevraagd, in een paneel dat de flow van de POC volgt: de wet rekent met wat de overheid al weet en vraagt één voor één alleen wat in geen register staat (de huurprijs, de terraslocatie), rekent na elk antwoord opnieuw, laat de uitkomst en de gebruikte gegevens controleren, en na indiening de status; na een besluit kan de burger daar bezwaar maken. Het portaal springt nooit naar het zaaksysteem, dat is de andere wereld.
7. **Zaaksysteem**: de behandelaarskant: ingediende aanvragen, de herberekening door de engine naast het aangevraagde resultaat, correcties van burgers ter beoordeling, toekennen of afwijzen, bezwaar.

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
