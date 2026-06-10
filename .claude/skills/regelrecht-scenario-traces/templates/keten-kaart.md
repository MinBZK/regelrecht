# {Endpoint} — keten-kaart

De DAG van tussen-normen van leaf-feiten naar de endpoint, met coverage per schakel. Bron
voor de keten-checkpoints en de branch-coverage.

## De keten (DAG)

```
{leaf_a} ─┐
{leaf_b} ─┴► {tussen-norm 1} ─┐
{leaf_c} ───► {tussen-norm 2} ─┴► {endpoint}
                  ▲
{wet Y}.{output} ─┘   (cross-law scharnier)
```

## Knopen

| Knoop | Type | Hangt af van | Rol | Assert-prioriteit |
|---|---|---|---|---|
| {tussen-norm 1} | grond / uitsluiting / tegen-uitsluiting / scharnier | {leafs / knopen / cross-law} | {wat deze knoop bepaalt} | kritiek / nuttig / overslaan |
| {endpoint} | endpoint | {knopen} | eind-uitkomst | kritiek |

**Scharnierpunten (cross-law)**: {wet Y}.{output} → {hier de keten in}. Assert minstens
deze.

## Kritieke paden per persona

| Persona | Pad (knopen die de uitkomst dragen) | Te asserten checkpoints |
|---|---|---|
| {persona} | {knoop → knoop → endpoint} | {knoop=waarde, ..., endpoint=waarde} |

## Branch-inventaris

Elke conditie/operatie met beide kanten — voor de branch-coverage.

| Knoop / conditie | Waar-kant gedekt door | Onwaar-kant gedekt door | Status |
|---|---|---|---|
| {conditie} | {persona} | {persona / —} | beide ✓ / half / ongetest |

## Sensitiviteits-paren

| Twin-paar | Schakel onder test | Verwacht-flippende knoop |
|---|---|---|
| {A} ↔ {B} | {conjunctie/disjunctie/neutralisatie bij knoop} | {knoop} |
