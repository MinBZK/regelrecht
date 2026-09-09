---
title: "Deployment"
description: "How components are built and deployed to ZAD (RIG/Quattro/rijksapps) through GitHub Actions."
---

All components are deployed to ZAD (RIG/Quattro/rijksapps) via GitHub Actions. Docker images are pushed to GitHub Container Registry (GHCR).

## How deployment works

### On pull request

A PR only builds and deploys when it carries the `deploy:preview` label. Without that label nothing is built, because a preview costs about half of the repository's runner budget and no required check depends on it. Add the label and the build starts, whether the PR was opened a minute or a month ago:

1. Changed components are detected automatically
2. Docker images are built and pushed to `ghcr.io/minbzk/regelrecht-{component}:sha-{commit}`
3. A preview deployment named `pr{N}` is created on ZAD
4. The PR gets a comment with preview URLs

Every later commit on a labelled PR repeats this. Remove the label and the preview and its images are cleaned up.

Only changed components are rebuilt. Any component can also be forced to build by adding its `deploy:<component>` label to the PR (for example `deploy:editor`); those labels are additive and do nothing on their own, since `deploy:preview` is what opens the gate.

### On merge to main

When a PR merges to main, production deployment runs:

1. All changed components are rebuilt with the merge commit SHA
2. Components are deployed to the `regelrecht` deployment on ZAD
3. Production URLs update within minutes

### On PR close, or when the label is removed

The preview deployment and its GHCR images are cleaned up automatically.

## Deployed components

| Component | Image | Production URL |
|-----------|-------|----------------|
| Editor | `regelrecht-editor` | `editor.regelrecht.rijks.app` |
| Admin | `regelrecht-admin` | `harvester-admin.regelrecht.rijks.app` |
| Harvester Worker | `regelrecht-harvester-worker` | (no web UI) |
| Enrich Worker | `regelrecht-enrich-worker` | (no web UI) |
| Pipeline API | `regelrecht-pipeline-api` | (no public URL; reached in-cluster) |
| Lawmaking | `regelrecht-lawmaking` | `lawmaking.regelrecht.rijks.app` |
| Demo | `regelrecht-demo` | `demo.regelrecht.rijks.app` (nog niet aangesloten, zie het plan hieronder) |
| Demo | `regelrecht-demo` | `demo.regelrecht.rijks.app` (not wired into deploy.yml yet) |
| Docs | `regelrecht-docs` | `docs.regelrecht.rijks.app` + `regelrecht.rijks.app` (landing) |
| Grafana | `regelrecht-grafana` | `grafana.regelrecht.rijks.app` |

The docs image also serves `/roadmap`, a read-only rendering of the werkpakketten in `docs/src/content/roadmap/` and the JSON file in `docs/src/data/`. It is not a component of its own and has no write path: changing the roadmap means editing those files through a pull request, and every werkpakket page links to its own source on GitHub. The page is deliberately not linked from the navigation or the landing page.

## De demo uitrollen (plan)

De demo (`frontend-demo/`, doel `demo.regelrecht.rijks.app`) is als enige component nog niet aangesloten. Wat er al is: `frontend-demo/Dockerfile` (bouwt de engine als WASM, bundelt `corpus/demo`, serveert met dezelfde unprivileged nginx als lawmaking en docs, poort 8000), `frontend-demo/nginx.conf`, en de component `demo` in `script/deploy-filters.mjs` (raakt de engine-crate, `frontend-demo/`, `packages/frontend-shared/`, `corpus/demo/` en `deploy/nginx/`). Wat ontbreekt, in de volgorde waarin het moet:

1. **ZAD-component aanmaken** (eenmalig, met de hand, door iemand met `ZAD_API_KEY`). Dit gaat vóór de workflow: `deploy-production` zet alle componenten in één taak, en een component die ZAD niet kent laat die taak falen.

   ```bash
   zad component add demo \
       --image ghcr.io/minbzk/regelrecht-demo:latest \
       --deployment regelrecht \
       --port 8000 \
       --service publish-on-web
   ```

   Daarna de hostnaam `demo.regelrecht.rijks.app` aan de component koppelen, zoals bij `lawmaking`. De image bestaat op dat moment nog niet; ZAD start de component pas bij de eerste deploy.

2. **Image bouwen in `deploy.yml`.** Naast `build-lawmaking` een `build-demo` met `image-name: minbzk/regelrecht-demo`, `dockerfile: frontend-demo/Dockerfile`, `cache-scope: demo`, en `needs.changes.outputs.demo == 'true'` als voorwaarde. Daarvoor krijgt de `changes`-job een output `demo: ${{ steps.filter.outputs.demo }}`; het filterscript levert die al.

3. **Component meenemen in beide deploys.** In `deploy-preview` en `deploy-production`: `build-demo` in `needs`, `DEMO: ${{ needs.build-demo.result }}` in de env en `if [ "$DEMO" = success ]; then add demo regelrecht-demo; fi` in de componentenlijst. Ook in de `if:` die bepaalt of er iets te deployen is, en in de containerlijst van `cleanup-preview` (`regelrecht-demo`, tag `pr-N`).

4. **Eerst een preview.** Label de PR die dit toevoegt met `deploy:preview` (en `deploy:demo` als de wijziging het filter niet raakt). Controleer op `pr{N}`: de WASM-engine laadt (netwerktab: `wasm/pkg/*.wasm` als `application/wasm`), de dia's, het portaal van Merijn en Claudia, een aanvraag tot in het zaaksysteem. Let op `nginx.conf`: de SPA-fallback naar `index.html` en het MIME-type voor `.wasm`.

5. **Merge naar main** rolt de demo productie in; de tabel hierboven en `CLAUDE.md` krijgen dan de definitieve regel zonder "still to be wired". Het image valt vanzelf onder `scheduled-cleanup.yml`, dat op `sha-`-tags en de draaiende deployment toetst.

De demo heeft geen backend en geen secrets nodig; de bouw duurt langer dan de andere frontends door de Rust-naar-WASM-stap (de `wasm-builder`-stage is gepind op de Rust-versie uit `rust-toolchain.toml` en de `wasm-bindgen`-versie uit `packages/Cargo.lock`, en faalt luid als die uit elkaar lopen).

## ZAD CLI

Use [`zad-cli`](https://github.com/RijksICTGilde/zad-cli) to manage deployments directly:

```bash
# Install
uv tool install git+https://github.com/RijksICTGilde/zad-cli.git

# List deployments
zad deployment list

# Get logs
zad logs --deployment regelrecht --lines 50

# Add a new component
zad component add docs \
    --image ghcr.io/minbzk/regelrecht-docs:latest \
    --deployment regelrecht \
    --port 8000 \
    --service publish-on-web
```

Configure `ZAD_API_KEY` and `ZAD_PROJECT_ID` in `.env`.

## Required secrets

- `RIG_API_KEY` - API key for ZAD Operations Manager (configured in GitHub repository secrets)
- `GITHUB_TOKEN` - used for GHCR image pushes (provided automatically by GitHub Actions)

## Further reading

- [CI/CD Pipeline](./ci-cd) - the continuous integration checks that run before deployment
- [Contributing](./contributing) - the PR workflow that triggers deployment
