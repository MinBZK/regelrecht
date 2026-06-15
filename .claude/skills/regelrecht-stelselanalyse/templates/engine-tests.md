# Engine-executie-tests — {datum}

**Scope**: {welke wetten/keten} · **Engine**: {versie}

## Persona-scenario's

Scenario's die elk een ander kern-pad door de keten raken (cross-law indien van
toepassing). Elk scenario = invoer + verwachte output.

| Scenario | Persona / casus (fictief) | Invoer (kern) | Verwachte output |
|---|---|---|---|
| 1 | {korte beschrijving} | {parameters} | {output = waarde} |

## Resultaten

| Wet / artikel | Output | Engine-resultaat | Oordeel |
|---|---|---|---|
| {wet art X} | `{output}` | {waarde} | PASS / **FAIL** |

{Bij FAIL: classificeer — engine-limitatie (→ `engine-limitaties`), modellering-fout
(→ fixes-plan), of wetgevings-fout (→ fouten-analyse). Een FAIL is niet automatisch een
corpus-fout.}

## Hoe gedraaid

- `{scenario-runner-commando}` ({N/N} PASS)
- {verwijzing naar de scenario-bestanden / feature-files}

## Meta-check

{Valideren deze scenario's de wet of de YAML? Als ze uit dezelfde (mogelijk foute)
interpretatie komen als de YAML, bewijzen ze geen juridische correctheid.}
