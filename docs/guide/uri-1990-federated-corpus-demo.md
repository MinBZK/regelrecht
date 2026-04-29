# URI 1990 in de regelrecht-mvp editor draaien (federated corpus)

Dit is wat je nodig hebt om PR #38 (private) en PR #593 (publiek) samen te
draaien en URI 1990 art 11 met scenarios in de editor te zien.

## Wat is "federated corpus"?

De editor leest wetten via `corpus-registry.yaml` op de project-root. Daarin
staan `local` en `github` sources. **Daarnaast** kan je een gitignored
`corpus-registry.local.yaml` aanmaken die extra sources toevoegt — dat
gebruiken we hier om de URI 1990-YAML uit de **private repo** te koppelen aan
de editor in de **publieke repo**, zonder te kopiëren of een token te zetten.

```
regelrecht-mvp                regelrecht-mvp-private
├─ corpus-registry.yaml       └─ corpus/regulation/nl/...
├─ corpus-registry.local.yaml      └─ uitvoeringsregeling_invorderingswet_1990/
│   └─ verwijst naar  ───────────────  └─ 2026-01-01.yaml
│                                      └─ scenarios/art-11-kwijtschelding.feature
└─ frontend/scripts/copy-laws.js
    leest beide registries en kopieert
    YAMLs + scenario-files naar
    frontend/public/data/
```

## Vereisten

- Rust (toolchain wordt gepind via `rust-toolchain.toml`)
- `wasm-bindgen-cli`: `cargo install wasm-bindgen-cli`
- `just`: `cargo install just` of `brew install just`
- Node 20+ en npm
- GitHub-token in `GITHUB_TOKEN` (nodig voor `@minbzk/storybook` uit GitHub
  Packages — zie `frontend/.npmrc`)
- Docker + Docker Compose (alleen voor de "volledige" stack)

## Stap 1 — Clone allebei naast elkaar

De relatieve paden in `corpus-registry.local.yaml` gaan ervan uit dat de
twee repos broers zijn in dezelfde parent-directory.

```bash
mkdir -p ~/workspace && cd ~/workspace
git clone git@github.com:MinBZK/regelrecht-mvp.git
git clone git@github.com:MinBZK/regelrecht-mvp-private.git
```

## Stap 2 — Branches uitchecken

**Private** — PR #38 met de YAML-fixes en scenarios:

```bash
cd ~/workspace/regelrecht-mvp-private
git fetch origin fix/uri-1990-keten-parameter-forwarding
git switch fix/uri-1990-keten-parameter-forwarding
```

**Publiek** — PR #593 met engine + light-mode fix:

```bash
cd ~/workspace/regelrecht-mvp
git fetch origin fix/engine-support-schema-v0.5.2
git switch fix/engine-support-schema-v0.5.2
```

## Stap 3 — Federated corpus koppelen

In de **publieke repo** (`regelrecht-mvp`), maak `corpus-registry.local.yaml`
op de project-root. Dit bestand staat in `.gitignore`, dus het komt nooit in
een commit.

```bash
cd ~/workspace/regelrecht-mvp
cat > corpus-registry.local.yaml << 'EOF'
---
schema_version: '1.0'
sources:
  - id: hhnk-uri-1990
    name: HHNK Kwijtschelding (regelrecht-mvp-private)
    type: local
    local:
      path: ../regelrecht-mvp-private/corpus/regulation/nl
    scopes: []
    priority: 0
EOF
```

> **Let op het pad** — `../regelrecht-mvp-private/corpus/regulation/nl`
> resolved relatief vanaf de project-root van `regelrecht-mvp`. Pas aan als
> je layout anders is. Een absoluut pad werkt ook.

## Stap 4 — WASM-engine bouwen

De editor laadt een WASM-build van de engine in de browser. PR #593 voegt
schema v0.5.2 toe aan de engine, dus deze build moet vers zijn.

```bash
cd ~/workspace/regelrecht-mvp
just wasm-build
```

Output verschijnt in `frontend/public/wasm/pkg/`.

## Stap 5 — Frontend dependencies

```bash
cd ~/workspace/regelrecht-mvp/frontend
npm install
```

(Zorg dat `GITHUB_TOKEN` is gezet — `@minbzk/storybook` komt uit
GitHub Packages.)

## Stap 6 — Draaien

Twee opties.

### Optie A — Volledige stack (`just dev`)

Start postgres + admin-API + frontend met hot-reload:

```bash
cd ~/workspace/regelrecht-mvp
just dev
```

Vereist Docker draaiend voor postgres. Dit is de canonical setup.
Frontend op `http://localhost:3000`.

### Optie B — Frontend + mock admin API

Als je geen Docker wil draaien voor de demo, kan je de admin-API mocken
vanuit de statische `public/data/`. (Dit is wat ik hier in de container
gebruik gehad.) Vraag mij om `mock-admin-api.js` als je deze route wil —
het is een ~50-regels Node-script dat `/api/corpus/laws/...`,
`/api/feature-flags`, `/api/favorites` en `/auth/status` serveert.

```bash
# Frontend (predev draait copy-laws.js die corpus-registry.local.yaml leest)
cd ~/workspace/regelrecht-mvp/frontend
npm run dev
# in een 2e shell:
node /pad/naar/mock-admin-api.js
```

Frontend op `http://localhost:3000` (default), mock op `:8000`.

## Stap 7 — Verifiëren

Open `http://localhost:3000/library` (of `:7100` als je vite hebt overruled).

1. **Library** — links zie je een lijst wetten. Filter op
   "Uitvoeringsregeling Invorderingswet 1990" — als die er staat,
   werkt de federated corpus.
2. **Editor** — klik door naar `editor/uitvoeringsregeling_invorderingswet_1990/11`.
3. **Scenario's-pane** (rechts, naast Machine en YAML) — toont 3 scenarios:
   - "Volledige kwijtschelding voor iemand zonder inkomen"
   - "Gedeeltelijke kwijtschelding bij beperkte betalingscapaciteit"
   - "Geen kwijtschelding wanneer betalingscapaciteit aanslag overstijgt"
4. Klik **"Toon resultaat"** op een scenario — je krijgt een execution-trace
   die door de cross-article keten heen loopt:
   `art 11 → art 12/13 → art 16` voor kostennorm, `art 12 → art 15` voor
   gemiddelde uitgaven b/c/g.

Verwachte uitkomsten (alle drie groen):

| Scenario | aanwendbare BC | hoogte | kan |
|---|---|---|---|
| Volledige (NBI=0) | 0 | €100,00 | ja |
| Gedeeltelijke (NBI=€1300, aanslag €500) | €371,04 | €128,96 | ja |
| Geen (NBI=€2000, aanslag €100) | €7091,04 | 0 | nee |

## Wat doen deze 2 PR's eigenlijk?

**PR #38 — regelrecht-mvp-private** (`fix/uri-1990-keten-parameter-forwarding`):
- `accepted: true` op de twee `untranslatables:` blokken (art 11 + art 14).
  Zonder dat weigert de engine in default Error-mode het hele article uit te
  voeren.
- Threading van 9 parameters door de source-keten:
  - `is_kostendeler`, `kostendelersnorm_bedrag`, `woont_buiten_nederland`,
    `woonland_factor` — vereist door art 16 lid 2/4
  - `betalingen_belastingschulden_maand`, `betaalde_alimentatie_maand`,
    `aflossingen_belastingschulden_maand`, `kostgangerskosten_maand`,
    `overige_noodzakelijke_uitgaven_maand` — vereist omdat de engine eager
    alle outputs van art 15 evalueert, ook al sourced art 12 alleen
    `gemiddelde_uitgaven_b_c_g_maand`
- 3 scenarios in editor-DSL formaat in
  `corpus/.../uitvoeringsregeling_invorderingswet_1990/scenarios/`

**PR #593 — regelrecht-mvp** (`fix/engine-support-schema-v0.5.2`):
- `SUPPORTED_SCHEMAS` voegt `v0.5.2` toe — corpus YAMLs op die schema-versie
  faalden voorheen bij het laden
- `<html data-scheme="light">` forceert light mode in de editor (anders
  volgt-ie OS-voorkeur via `prefers-color-scheme`)

## Wijzigingen achteraf

- YAML-aanpassen in `regelrecht-mvp-private` → re-run `node frontend/scripts/copy-laws.js`
  in `regelrecht-mvp/frontend/`. Vite hot-reload pakt de nieuwe
  `public/data/index.json` op.
- Engine-aanpassing → `just wasm-build` opnieuw, herlaad browser.

## Wat zit er nog NIET in (Phase 2)

- HHNK end-to-end: de verhoging-vrijstelling (€1500/€1800/€2000), de
  scope-restrictie tot woonruimte, de ondernemer-gate en de 7 weigergronden
  uit de Leidraad-orchestrator. Dat vereist meer YAMLs ingeladen + cross-law
  `implements:` resolution. De huidige scenarios gebruiken open_term defaults
  (verhoging=0, kostennorm=0,9).
- Workshop-bevinding "HHNK past 40% BC toe i.p.v. 80%" zit niet in de YAML;
  scenarios volgen de federale wet (80%).
