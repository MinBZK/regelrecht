# frontend-demo

Burger-demo Vue-app voor regelrecht. Uitvoering gebeurt volledig in de browser via WASM (geen backend nodig voor berekeningen).

## Lokaal draaien

```bash
just demo-wasm       # build de engine als WASM (éénmalig, of na engine-wijzigingen)
just demo-assets     # bundel corpus-YAML + persona-JSON in public/demo-assets/
just demo-dev        # start vite op http://localhost:7180
```

`just demo-dev` roept `demo-assets` automatisch aan.

## Productie-build

```bash
just demo-build      # bundelt assets + wasm + vite build
```

Output in `dist/` is een statische bundle die door elke webserver (nginx, caddy) geserveerd kan worden.

## Structuur

```
src/
├── App.vue
├── main.js
├── router.js
├── composables/
│   └── useEngine.js   // laadt WASM-engine, beheert profielen, voert laws uit
└── views/
    ├── Dashboard.vue       // /  — persona + wet-lijst
    ├── LawDetail.vue       // /wet/:lawId — invoer, uitkomst, trace
    └── LawSimulation.vue   // /wet/:lawId/simulatie — populatie-simulatie (Fase 6)
```

## Design

Alleen `@minbzk/storybook` (prefix `nldd-`). Geen tailwind, geen andere UI-libs.

## LLM-uitleg

De "Uitleg"-knop (Fase 7) doet een fetch naar `/api/explain`, geproxied naar `packages/demo-api/` op poort 7181. Zonder draaiende demo-api is die knop niet-functioneel.
