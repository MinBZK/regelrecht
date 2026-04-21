# Cases

Een *case* is een herbruikbare scope-configuratie: een lijst van wetten en
bijbehorende test-scenarios die samen een concreet vraagstuk dekken (bv.
kwijtschelding voor een specifiek waterschap, toeslagen voor een doelgroep,
etc.).

## Structuur

```
cases/
  <slug>/
    README.md         — context van de casus
    scope.yaml        — manifest met wetten en scenarios
    ...               — eventueel sample-inputs, notities
```

Zie [`hhnk-kwijtschelding/scope.yaml`](hhnk-kwijtschelding/scope.yaml) voor
het schema.

## Waarom een manifest

Regelrecht-wetten bestaan in meerdere ecosystemen (regelrecht-mvp editor,
poc-machine-law graph, toekomstige consumenten). Elk systeem heeft een eigen
corpus-layout. Het manifest legt alleen de **identiteiten** van de betrokken
wetten vast; paden/formaten worden door elke consumer zelf geresolveerd. Zo
blijft hetzelfde manifest bruikbaar wanneer een wet in meerdere ecosystemen
bestaat, en expliciet beperkt wanneer dat (nog) niet zo is.

## Laden

```sh
just case-load <slug>     # laadt in regelrecht-mvp editor (backend + Vite)
just case-list            # toont beschikbare cases
just case-clear           # verwijdert lokale scope-overschrijving
```

De `case-load` command regenereert `.scope/` met symlinks en schrijft
`corpus-registry.local.yaml`. Het herstart servers niet — dat doe je zelf.

Voor poc-machine-law is de loader nog niet geïmplementeerd. Zie de TODO in
individuele case-READMEs.
