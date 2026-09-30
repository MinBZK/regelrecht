# Gherkin-runner: shims naar de gedeelde runner

De vijf `.js`-bestanden hier zijn shims. De runner zelf staat in
`packages/frontend-shared/src/gherkin/`, dezelfde die de editor gebruikt.
Deze JS-runner voert de canonieke stappen uit tegen de WASM-engine en is
daarmee de BDD-route voor deze casus.

Waarom shims en geen kopie meer: deze bestanden waren gevendord uit een
losstaande PoC-repo en werden met de hand bijgewerkt. Dat is stil misgegaan.
`bdd/codegen/gen-js.mjs` schrijft `grammar.generated.js` alleen naar
`packages/frontend-shared/`, dus de kopie hier liep achter op de grammatica
zonder dat iets dat meldde: de canonieke runner kende inmiddels
`output "x" is absent`, collecties en `VALUE_TYPING`, en deze kopie nog niet.
Een shim kan niet verouderen.

Bijwerken gaat dus niet meer via kopiëren. De grammatica verandert in
`bdd/grammar.yaml`, waarna `just bdd-codegen` het gegenereerde bestand
opnieuw schrijft. Raak deze shims niet aan.
