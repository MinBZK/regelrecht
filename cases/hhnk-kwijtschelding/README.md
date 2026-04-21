# Casus: HHNK Kwijtschelding

Scope-manifest voor het beoordelen en bewerken van de kwijtschelding-
waterschapsbelasting-keten van het Hoogheemraadschap Hollands Noorderkwartier.

## Wat zit in scope

11 wetten in drie regulatieve lagen plus de federale basis — zie
[`scope.yaml`](scope.yaml) voor de precieze lijst met notities.

## Laden in regelrecht-mvp

Vanaf repo root:

```sh
just case-load hhnk-kwijtschelding
```

Dat commando:
1. Leest `cases/hhnk-kwijtschelding/scope.yaml`
2. Regenereert `.scope/` met symlinks naar de 11 corpus-directories
3. Schrijft `corpus-registry.local.yaml` dat de hoofdregistry overschrijft
   met `.scope/` als enige bron (de GitHub-source `minbzk-central` wordt
   uitgezet)
4. Herstart eventuele draaiende `editor-api` en Vite niet — dat doe je zelf

## Laden in poc-machine-law (TODO)

De HHNK-wetten bestaan alleen in regelrecht-mvp schema v0.5.2 formaat en
zijn nog niet geport naar het poc-machine-law formaat (v0.1.x). Opties:

- **Converter bouwen**: lees schema v0.5.2 → schrijf v0.1.x naar een tmp-dir,
  zet die op `EXTRA_LAWS_DIRS`.
- **Handmatig porten**: schrijf equivalente YAMLs in `laws/<id>/` van de PoC.
- **PoC uitbreiden**: lader van `machine/utils.py` leert schema v0.5.2 lezen.

Geen van deze is nu geïmplementeerd.

## BDD-scenarios bij deze casus

- `features/kwijtschelding_hhnk.feature` — 11 golden-path scenarios
  (alleenstaande bijstand, echtgenoten net boven norm, AOW-er, vermogen,
  ondernemer, etc.)
- `features/hhnk_kwijtschelding_machinereadable_units.feature` — 24 unit-
  scenarios die de individuele machine_readable blocks direct testen
  (grenzen Regeling medeoverheden, URI art 17 €136-drempel,
  Leidraad 2008 vermogensdrempels, HHNK-leidraad art 25/26/48/80).
