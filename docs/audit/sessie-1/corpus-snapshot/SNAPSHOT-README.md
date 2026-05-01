# Corpus-snapshot — sessie 1 (HHNK kwijtschelding workshop, 2026-04-23)

Dit zijn de YAMLs zoals ze tijdens **sessie 1** (2026-04-23, HHNK Leidraad art 26-validatie) in de editor / op tafel lagen. **Bevroren kopieën** — deze bestanden worden bewust niet bijgewerkt wanneer de bron-corpus verandert. Voor het lezen van het bijbehorende verslag (`hhnk-workshop-2026-04-23.md`) is dit de canonieke YAML-state.

## Bron

Deze snapshot is gemaakt vanaf:

- **regelrecht-mvp-private** (private repo)
- **Tag**: `workshop-hhnk-sessie-1-verslag`
- **Commit**: `6ff04f5` ("workshop: verslagen 2026-04-23 — intern + HHNK-terugkoppeling")

Drie commits markeren de sessie-1-keten in de private repo (tags zijn lokaal, commits zijn op `origin/workshops/hhnk-kwijtschelding-2026-04-23`):

| Tag | Commit | Inhoud |
|---|---|---|
| `workshop-hhnk-sessie-1-pre` | `cf3cfef` | corpus pre-staat (vóór deelnemers-bevindingen) |
| `workshop-hhnk-sessie-1-post` | `767db6c` | corpus post-staat met deelnemers-bevindingen |
| `workshop-hhnk-sessie-1-verslag` | `6ff04f5` | + verslagen toegevoegd (bron van deze snapshot) |

## Public-context

Op het moment van sessie 1 stond de **public** repo (regelrecht-mvp) op:

- Branch: `feat/hhnk-kwijtschelding-machine-readable`
- Tag: `workshop-hhnk-sessie-1-state`
- Commit: `6e4bea1` ("docs(audit): facilitator-brief — kort draaiboek voor tijdens workshop")

Let op: deze commit is na de sessie door een collega force-pushed weg op origin (Tim de Jager heeft de feature-branch geherorganiseerd naar zijn eigen werk). De tag boven redt de commit lokaal. Push de tag naar origin om 'm voor anderen reproduceerbaar te maken.

## Bestanden in deze snapshot

| Bestand | Wet/regeling |
|---|---|
| `leidraad_invordering_waterschapsbelastingen_hhnk__2026-02-07.yaml` | HHNK Leidraad invordering 2026 — **kern van sessie 1** (art 26-validatie) |
| `leidraad_invordering_waterschapsbelastingen_hhnk__2023-01-01.yaml` | Voorgaande HHNK Leidraad-versie — voor diff-context |
| `kwijtscheldingsregeling_waterschapsbelastingen_hhnk__2023-01-01.yaml` | HHNK Kwijtscheldingsregeling — bevoegdheids-grondslag |
| `uitvoeringsregeling_invorderingswet_1990__2026-01-01.yaml` | URI 1990 — federale wet die HHNK-leidraad uitwerkt |

## Verificatie

```
# Check een bestand tegen huidige bron-corpus:
diff -u sessie-1/corpus-snapshot/leidraad_invordering_waterschapsbelastingen_hhnk__2026-02-07.yaml \
       /pad/naar/regelrecht-mvp-private/corpus/regulation/nl/waterschaps_verordening/hhnk/leidraad_invordering_waterschapsbelastingen/2026-02-07.yaml

# Reproduceer bron in private repo:
cd regelrecht-mvp-private
git checkout workshop-hhnk-sessie-1-verslag
```
