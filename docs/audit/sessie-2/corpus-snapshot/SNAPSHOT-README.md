# Corpus-snapshot — sessie 2 (URI 1990 keten-walk, 2026-04-30)

Dit zijn de YAMLs zoals ze tijdens **sessie 2** (2026-04-30, walk-through van URI 1990 keten in de editor) door de engine zijn gelezen. **Bevroren kopieën** — niet bijwerken bij latere corpus-wijzigingen. De trace-citaten in `HHNK workshop 2 - Verslag voor Claude.md` komen van exact deze YAML-state.

## Bron

Deze snapshot is gemaakt uit twee repo's, beide op een specifieke commit:

| Bron | Tag | Commit | Beschrijving |
|---|---|---|---|
| **regelrecht-mvp-private** | `workshop-hhnk-sessie-2-corpus` | `128d1ee` | URI 1990 PR-staat (`fix/uri-1990-keten-parameter-forwarding`); gemount in editor via worktree `.worktrees/uri-1990-pr` |
| **regelrecht-mvp** (public) | `workshop-hhnk-sessie-2-public-state` | `6e4bea1` | bron van `invorderingswet_1990__2023-05-01.yaml` (deze YAML zit niet in private repo) |

De **private**-tag bevat de hoofdmoot: URI 1990 met machine_readable + scenarios, plus alle HHNK-regelingen en de federale Leidraad 2008.

De **public**-tag is dezelfde commit als sessie-1's `workshop-hhnk-sessie-1-state` — Daan heeft tussen de sessies niets naar origin gepushed. Tim de Jager heeft 6e4bea1 op origin force-pushed weg na sessie 2; de tag boven redt 'm.

## Setup tijdens sessie 2

In de editor (zie `frontend/` op localhost:3001) is de corpus-bron-keten gewezen via lokale `corpus-registry.local.yaml` op de project-root van public:

```yaml
sources:
  - id: hhnk-uri-1990
    type: local
    local:
      path: '.../regelrecht-mvp-private/.worktrees/uri-1990-pr/corpus/regulation/nl'
    priority: 0
  - id: local
    type: local
    local:
      path: /dev/null/disabled-for-demo  # publieke corpus uitgeschakeld
    priority: 99
```

De `local`-bron (publieke `.scope/`) was bewust gedisabled. Daarom komen alle YAMLs in deze snapshot uit de **private** worktree, met als enige uitzondering `invorderingswet_1990` (zie hieronder).

## Bestanden in deze snapshot

| Bestand | Wet/regeling | Bron |
|---|---|---|
| `uitvoeringsregeling_invorderingswet_1990__2026-01-01.yaml` | URI 1990 — **kern van sessie 2** (art 11/12/13/15/16) | private |
| `uitvoeringsregeling_invorderingswet_1990__art-11-kwijtschelding.feature` | 3 scenarios voor URI 1990 art 11 — **gebruikt voor de trace-walk** | private |
| `invorderingswet_1990__2023-05-01.yaml` | Invorderingswet 1990 — grondslag-wet | public (zat niet in private) |
| `leidraad_invordering_2008__2026-01-01.yaml` | Federale Leidraad Invordering 2008 | private |
| `regeling_kwijtschelding_belastingen_medeoverheden__2022-09-17.yaml` | Mandaat-regeling waterschappen/gemeenten | private |
| `leidraad_invordering_waterschapsbelastingen_hhnk__2026-02-07.yaml` | HHNK Leidraad — context (zie sessie 1 voor diepgaande validatie) | private |
| `kwijtscheldingsregeling_waterschapsbelastingen_hhnk__2023-01-01.yaml` | HHNK Kwijtscheldingsregeling | private |

## Wat er **niet** in zit

- **Engine-versie**: de WASM-engine die deze YAMLs uitvoerde was gebouwd uit `packages/engine` op het moment van sessie 2 (zelfde public-tag, schema v0.5.2). Voor exacte engine-binary: rebuild via `just wasm-build` op tag `workshop-hhnk-sessie-2-public-state`.
- **Frontend-staat**: editor op localhost:3001 draaide vite-dev op zelfde public-tag.

## Verificatie

```
# Reproduceer private-bron:
cd regelrecht-mvp-private
git checkout workshop-hhnk-sessie-2-corpus

# Reproduceer public-bron:
cd regelrecht-mvp
git checkout workshop-hhnk-sessie-2-public-state

# Check één YAML tegen huidige private-corpus:
diff -u sessie-2/corpus-snapshot/uitvoeringsregeling_invorderingswet_1990__2026-01-01.yaml \
       /pad/naar/regelrecht-mvp-private/.worktrees/uri-1990-pr/corpus/regulation/nl/ministeriele_regeling/uitvoeringsregeling_invorderingswet_1990/2026-01-01.yaml
```
