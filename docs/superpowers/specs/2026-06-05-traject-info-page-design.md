# Traject-info pagina — design

**Datum:** 2026-06-05
**Component:** `frontend/` (regelrecht editor)
**Branch:** `feat/traject-info`

## Doel

Een read-only informatiepagina voor het **actieve traject**, geopend vanuit de
traject-dropdown, vormgegeven als dezelfde sheet als het "Nieuw traject"
aanmaakscherm. De pagina toont de bij het aanmaken ingevulde gegevens plus de
locatie van de repo en de traject-branch, waarbij de repo-locatie een link is
die de traject-branch op GitHub in een nieuw tabblad opent.

## Context

- De dropdown (`frontend/src/components/TrajectMenu.vue`) toont trajecten uit
  `GET /api/trajects` (`TrajectSummary`: `id`, `name`, `description`, `scope`,
  `status`, `role`, `ref`). Deze lijst bevat **geen** repo/branch-gegevens.
- De repo/branch-gegevens zitten in `GET /api/trajects/:id` (`TrajectDetail`),
  in de `sources`-array. De relevante bron is die met `is_writable_own === true`
  en draagt: `gh_owner`, `gh_repo`, `gh_branch`, `gh_base_branch`, `gh_path`.
- De ledendialoog (`TrajectMembersDialog.vue` + `useTrajectMembers.js`) haalt
  dit detail-endpoint al op, maar negeert `sources`.

## Beslissingen (uit brainstorm)

1. **Repo-link wijst naar de traject-branch:**
   `https://github.com/{owner}/{repo}/tree/{branch}`, geopend in nieuw tabblad
   (`target="_blank" rel="noopener noreferrer"`).
2. **Scope: alleen het actieve traject.** Het menu-item verschijnt — net als
   "Beheer leden…" — uitsluitend wanneer er een traject actief is.

## Componenten

### 1. Dropdown-uitbreiding — `TrajectMenu.vue`

Voeg een `nldd-menu-item` `Traject-info…` toe (icon `info`), zichtbaar via
`v-if="activeTraject"`, geplaatst direct boven `Beheer leden…`. Een
`openInfoForActive()` zet `infoTrajectId`/`infoTrajectName` en opent de
info-dialoog — spiegelt exact `openMembersForActive()`.

### 2. Data-ophaal — `useTrajectDetail.js` (nieuw)

Kleine composable die `GET /api/trajects/:id` ophaalt en het volledige
`TrajectDetail`-object reactief teruggeeft (incl. `sources`), met `loading`/
`error`-state. Reden voor een nieuwe composable i.p.v. uitbreiden van
`useTrajectMembers`: die composable is gericht op ledenbeheer; de info-pagina
heeft alleen leesbehoefte aan het detail (incl. sources). Een aparte, kleine
loader houdt beide gescheiden en herbruikbaar.

Vorm:

```js
export function useTrajectDetail() {
  const detail = ref(null);
  const loading = ref(false);
  const error = ref(null);
  async function load(trajectId) { /* fetch /api/trajects/:id */ }
  return { detail, loading, error, load };
}
```

### 3. Nieuwe component — `TrajectInfoDialog.vue`

Kloon van de sheet-schil van `TrajectMembersDialog.vue`:

- `nldd-sheet` `placement="right"` `width="520px"` `full-height`, geteleporteerd
  naar `body`, header `nldd-top-title-bar` tekst "Traject-info" met dismiss.
- Props: `modelValue` (Boolean, v-model), `trajectId` (String), `trajectName`
  (String). Bij openen → `load(trajectId)`.
- Body: `nldd-list variant="box"` met `nldd-text-cell` labels en de waarde als
  **read-only tekst** in `nldd-cell` (geen `nldd-text-field`). Rijen:
  - **Naam** — `detail.name`
  - **Beschrijving** — `detail.description` (of "—" indien leeg)
  - **Scope** — `detail.scope` (of "—" indien leeg)
  - **Status** — `detail.status` (bezig/afgerond)
  - **Jouw rol** — `detail.role` (owner/contributor)
  - **Repo** — link: tekst `{owner}/{repo}`,
    `href="https://github.com/{owner}/{repo}/tree/{branch}"`,
    `target="_blank" rel="noopener noreferrer"`
  - **Branch** — `gh_branch` (tekst)
  - **Base branch** — `gh_base_branch` (tekst)
  - **Subpath** — `gh_path` of "repo-root" indien leeg
- Repo/branch-rijen komen uit de bron met `is_writable_own === true`; bij
  ontbreken (defensief) tonen we "onbekend".
- Loading- en error-state zoals in de ledendialoog.
- Footer: één `nldd-button` ghost "Sluiten" om het sheet-ritme te volgen.

## Data flow

```
TrajectMenu (activeTraject) --click "Traject-info…"-->
  TrajectInfoDialog(open, trajectId)
    --> useTrajectDetail.load(id) --> GET /api/trajects/:id
    --> render summary-velden + writable-own source (repo/branch link)
```

## Aanvullende CSS

Streven: geen of minimale extra CSS. Hergebruik bestaande sheet-/lijststijlen.
De repo-link krijgt standaard NDD-linkstyling. Eventuele toegevoegde overrides
(bv. read-only waardecel-uitlijning) worden expliciet aan de gebruiker gemeld
conform het design-system-rapportagebeleid.

## Backend

Geen wijzigingen nodig — `GET /api/trajects/:id` levert al alle benodigde
gegevens.

## Out of scope

- Bewerken van traject-gegevens vanaf de info-pagina (PATCH bestaat wel, maar
  valt buiten deze taak).
- Info per individueel traject in de lijst (alleen actief traject).
