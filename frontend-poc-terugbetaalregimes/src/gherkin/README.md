# Gevendorde Gherkin-runner

Deze bestanden komen letterlijk uit `regelrecht/frontend/src/gherkin/`
(parser, steps, actions, context en de gegenereerde grammar). De Rust
BDD-runner van regelrecht kan alleen scenario's binnen die repo draaien;
deze JS-runner voert dezelfde canonieke stappen uit tegen de WASM-engine
en is daarmee de BDD-route voor dit losstaande project.

Bijwerken: kopieer de vijf `.js`-bestanden opnieuw uit de
regelrecht-checkout (na `just bdd-codegen` daar) en draai `just test`.
