# RegelRecht demo

De demo-werkruimte van RegelRecht: presentatie, wettenbrowser, afhankelijkheidsgraaf,
scenario-runner, burger-/ondernemersportaal en zaaksysteem. Opvolger van de losse
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
- **Toestand** (profiel, aanvragen, correcties) in `src/store/demoStore.js`, bewaard in
  `localStorage`; "Demo resetten" in het menu wist het.
- **Scenario's** draaien met de gedeelde Gherkin-runner uit
  `@regelrecht/frontend-shared/gherkin` (canonieke grammar); `src/data/gherkinNl.js`
  geeft de stappen in het Nederlands weer.

## Draaien

```bash
just dev-demo        # WASM bouwen + Vite op :7400
just bdd-demo        # de demo-scenario's natively, met cargo
```

## Eigen CSS bovenop het design system

`src/css/main.css` bevat, naast een box-sizing/body-reset, alleen hooks waarvoor het
design system geen component heeft:

| Selector | Waarom |
|---|---|
| `.slide-stage`, `.slide` | Dia-podium: gecentreerde stapel en `zoom: 1.5` zodat de typografie van `nldd-title` leesbaar is vanaf de achterste rij. Er is geen presentatiecomponent. |
| `.org-logo` | Vaste 40px/24px box voor organisatielogo's; `nldd-image` vult altijd de volle breedte. |
| `.yaml-tree*` | Opvouwbare YAML-boom met kruiswet-links; `nldd-code-viewer` highlight wel YAML maar vouwt niet en kent geen links. |
| `.gherkin*` | Gherkin-weergave met slaag/faal-markering per stap en tabellen; de viewer kent Gherkin als taal maar geen stapstatus. |
| `.trace` | Monospace box-drawing-trace. |
| `.tile-body`, `.data-tree` | Verticale stapel in een tegel en de inspringing van de herkomstboom. |
| `.case-board` | Drie kolommen die op smalle schermen onder elkaar vallen (`nldd-container layout="grid"` zit op 280px-kolommen vast met eigen padding). |
| `.graph-canvas`, `.graph-node*`, `.graph-dim` | vue-flow heeft een expliciete hoogte nodig; knopen en dimmen van niet-geselecteerde knopen. Een graafcanvas bestaat niet in het design system (zelfde uitzondering als de editor). |
