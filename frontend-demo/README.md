# RegelRecht demo

De demo-werkruimte van RegelRecht: presentatie, wettenbrowser, afhankelijkheidsgraaf,
scenario-runner, simulatie, burger-/ondernemersportaal en zaaksysteem. Opvolger van de losse
`poc-machine-law`-repository; bestemd voor `demo.regelrecht.rijks.app`.

## Architectuur

- **Geen backend.** De regelrecht-engine draait als WebAssembly in de browser
  (`public/wasm/pkg`, gebouwd door `just wasm-build` of de Dockerfile).
- **Corpus** in `corpus/demo/`: wetten (schema v0.5.7), scenario's, `bindings.yaml`,
  `profiles.yaml`, `demo-config.yaml`, `services.yaml`. `scripts/copy-demo-corpus.mjs`
  kopieert het bij `predev`/`prebuild` naar `public/data` en schrijft `index.json`.
- **Data**: `src/data/materialize.js` past de bindings toe op de persona-tabellen en
  levert per wet records op die als wet-gebonden databron in de engine gaan
  (`engine.registerDataSourceForLaw`). Correcties van de burger komen als bron
  `correcties` met hogere prioriteit bovenop. Dezelfde module gebruikt de
  scenario-converter (`corpus/demo/tools/convert_features.mjs`).
- **Aanvragen** gebeuren in het portaal (`ApplicationSheet.vue`), just in time zoals de
  POC: `src/data/askedInputs.js` bepaalt uit de laatste uitvoeringstrace welke input die
  alleen de burger kent de engine daadwerkelijk tegenkwam, vraagt die één voor één en
  laat de engine na elk antwoord opnieuw rekenen. Antwoorden worden claims (`selfDeclared`)
  op de wet, gesleuteld op de identiteit van die wet (`kvk_nummer` voor een bedrijfswet), en
  gelden ook als parameters bij het materialiseren, zodat een terraslocatie de juiste
  registerrij vindt. Een wet die een andere wet kruiswet aanroept zonder die parameters
  krijgt ze van de engine uit diezelfde antwoorden (RFC-036, regel 5).
- **Toestand** (profiel, aanvragen, correcties) in `src/store/demoStore.js`, bewaard in
  `localStorage`; "Demo resetten" in het menu wist het.
- **Scenario's** draaien met de gedeelde Gherkin-runner uit
  `@regelrecht/frontend-shared/gherkin` (canonieke grammar); `src/data/gherkinNl.js`
  geeft de stappen in het Nederlands weer.
- **Simulatie** (`src/simulation/`): een gegenereerde populatie burgers of bedrijven
  (`population.js`, seeded) in dezelfde tabelvorm als `profiles.yaml`, door de
  materialiser en de engine gehaald voor elke regeling van het portaal
  (`runner.js`), samengevat en uitgesplitst (`stats.js`). Constanten uit de
  `definitions` van een wet zijn per run aan te passen (`lawParameters.js`): de
  wet wordt met gewijzigde waarden herladen en na de run teruggezet. De
  simulatie vervangt tijdelijk de persona-data in de engine en zet die daarna
  terug. Grafieken met echarts, zoals in de editor.

## Draaien

```bash
just dev-demo        # WASM bouwen + Vite op :7400
just bdd-demo        # de demo-scenario's natively, met cargo
```

## Ontbrekende registerwaarden

De materialiser (`src/data/materialize.js`) vult een `amount`- of `number`-input
waarvoor geen registerrij bestaat met `0`, niet met `null`. Dat is de
POC-semantiek (een optelling sloeg ontbrekende operanden over) en het houdt de
tegels rekenbaar voor persona's zonder loon, uitkering of vermogen. De keerzijde:
een wet die `$inkomen == null` toetst ziet een nul, geen onbekende. Voor andere
typen blijft een ontbrekende rij `null`, zodat de null-checks in de wetten werken.

## Eigen CSS bovenop het design system

`src/css/main.css` bevat, naast een box-sizing/body-reset, alleen hooks waarvoor het
design system geen component heeft:

| Selector | Waarom |
|---|---|
| `.slide-stage`, `.slide`, `.slide-controls` | Dia-podium: gecentreerde stapel, `zoom: 1.5` zodat de typografie van `nldd-title` leesbaar is vanaf de achterste rij, en het Rijkshuisstijl-blauwe verloop van de POC (hard-coded, een merkkleur volgt het kleurschema niet). `color-scheme: dark` op het podium laat de design-system-componenten erin hun donkere variant kiezen, dus lichte tekst zonder eigen styling. Er is geen presentatiecomponent. |
| `.org-logo` | Vaste 40px/24px box voor organisatielogo's; `nldd-image` vult altijd de volle breedte. |
| `.yaml-tree*` | Opvouwbare YAML-boom met kruiswet-links; `nldd-code-viewer` highlight wel YAML maar vouwt niet en kent geen links. |
| `.gherkin*` | Gherkin-weergave met slaag/faal-markering per stap en tabellen; de viewer kent Gherkin als taal maar geen stapstatus. |
| `.trace` | Monospace box-drawing-trace. |
| `.tile-body`, `.data-tree` | Verticale stapel in een tegel en de inspringing van de herkomstboom. |
| `.case-board` | Drie kolommen die op smalle schermen onder elkaar vallen (`nldd-container layout="grid"` zit op 280px-kolommen vast met eigen padding). |
| `.graph-canvas`, `.graph-node*`, `.graph-dim` | vue-flow heeft een expliciete hoogte nodig; knopen en dimmen van niet-geselecteerde knopen. Een graafcanvas bestaat niet in het design system (zelfde uitzondering als de editor). |

Eén afwijking buiten CSS: `App.vue` roept na elke routewissel `_evaluateScrollMode()` van
`nldd-app-view` aan. Het design system leidt bij het koppelen af of het document of elk
paneel scrolt, maar onze split view komt pas later (lazy routes) en de afleiding blijft dan
op "document". Zonder die aanroep scrollen de panelen niet zelf en blijven de kopregels
niet staan.
