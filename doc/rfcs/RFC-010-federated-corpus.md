# RFC-010: Federated Corpus — Clean Forks voor Decentrale Regelgeving

**Status:** Proposed
**Date:** 2026-03-18
**Authors:** Anne Schuth

## Context

Het corpus (`regulation/nl/`) zit in de regelrecht-mvp repo, samen met de engine, pipeline, admin en editor. Dat werkt voor een MVP, maar past niet bij de wetgevende realiteit: regelgeving is decentraal. Gemeenten, provincies, waterschappen en ministeries maken hun eigen regelgeving, vaak als invulling van hogere wetten.

RFC-007 introduceert Inversion of Control (IoC) met `open_terms` en `implements`. Hiermee kan een gemeente een hogere wet invullen zonder die hogere wet aan te raken. Technisch werkt dat al cross-repo, mits de engine alle relevante wetten laadt. Wat ontbreekt is de infrastructuur daarvoor:

- **Discovery**: hoe vindt de engine gemeentelijke regelgeving?
- **Laden**: hoe haalt de engine wetten op uit meerdere bronnen?
- **Scope-validatie**: hoe voorkomt de engine dat een gemeente zich als een andere gemeente voordoet?
- **Write-back**: hoe kan een gemeente via de editor regelgeving bijhouden in hun eigen repo?

De delegation-benadering uit RFC-003 gaat uit van een centraal corpus met `select_on`-criteria. Dat patroon blijft geldig voor delegatie, maar voor het federatiemodel is een ander mechanisme nodig: meerdere bronnen met elk hun eigen eigenaarschap.

## Decision

Vier samenhangende beslissingen vormen het federatiemodel.

### 1. Corpus als eigen repo

Het corpus (`regulation/nl/`) verhuist naar een eigen repository, bijvoorbeeld `MinBZK/regelrecht-corpus`. De regelrecht-mvp repo bevat dan alleen de engine, pipeline, admin en editor.

Clean forks zijn forks of clones van regelrecht-corpus, of eigen repos die hetzelfde YAML-schema volgen. Een gemeente als Amsterdam maakt een repo `gemeente-amsterdam/regelrecht-amsterdam` met daarin hun gemeentelijke verordeningen die `implements` declareren op wetten uit het centrale corpus.

### 2. Registry-manifest

Een YAML-manifest beschrijft welke bronnen de engine moet laden:

```yaml
schema_version: "1.0"
sources:
  - id: minbzk-central
    name: "MinBZK Centraal Corpus"
    type: github
    github:
      owner: MinBZK
      repo: regelrecht-corpus
      branch: main
      path: regulation/nl
    scopes: []
    priority: 100

  - id: amsterdam
    name: "Gemeente Amsterdam"
    type: github
    github:
      owner: gemeente-amsterdam
      repo: regelrecht-amsterdam
      branch: main
      path: regulation/nl
    scopes:
      - type: gemeente_code
        value: "GM0363"
    priority: 50
    auth_ref: amsterdam

  - id: local-dev
    name: "Lokale ontwikkelomgeving"
    type: local
    local:
      path: ./regulation/nl
    scopes: []
    priority: 200
```

**Merging-regels:**

- Centraal manifest: `corpus-registry.yaml` (in de repo, geversioned)
- Lokale overrides: `corpus-registry.local.yaml` (gitignored)
- Merge: lokale sources worden toegevoegd aan de centrale lijst
- Bij zelfde `id`: lokale entry vervangt de centrale
- `type: local` voor filesystem-bronnen (handig bij ontwikkeling)

**Scopes:**

Scopes zijn **claims**: een bron declareert voor welke jurisdictie(s) zij regelgeving levert. Een scope is geen routing-mechanisme ("gebruik deze bron als iemand om GM0363 vraagt") maar een eigenaarschapsverklaring ("deze bron bevat regelgeving van gemeente Amsterdam").

De engine gebruikt scopes voor twee dingen:

1. **Validatie** - als een wet uit een bron met scope `gemeente_code: GM0363` zelf `gemeente_code: GM0518` declareert, genereert de engine een warning. De bron claimt Amsterdam te zijn maar levert een wet voor een andere gemeente.
2. **Filtering** - als je de engine draait voor een specifieke gemeente, kun je op basis van scopes bepalen welke bronnen relevant zijn en welke je kunt overslaan.

Een bron zonder scopes (zoals het centrale corpus) levert wetten zonder jurisdictie-beperking. Een bron kan meerdere scopes hebben, bijvoorbeeld een provincie die regelgeving levert voor meerdere scope-types:

```yaml
scopes:
  - type: provincie_code
    value: "PV27"
  - type: waterschap_code
    value: "WS0155"
```

**Priority:**

Hogere priority wint bij conflicten. Als twee bronnen dezelfde wet-ID leveren, wordt de versie uit de bron met de hoogste priority geladen. Lokale bronnen krijgen standaard de hoogste priority (handig voor development).

### 3. Authenticatie

Credentials staan **volledig los** van het registry-manifest. Het manifest bevat geen tokens, wachtwoorden of secrets. Het enige wat het manifest bevat is een optionele `auth_ref`: een string die verwijst naar een entry in een apart auth-bestand. Zonder `auth_ref` gaat de engine ervan uit dat de bron publiek is.

**Auth types** (enum):

| Type | Beschrijving |
|------|-------------|
| `none` | Publieke repo, geen auth nodig (default als `auth_ref` ontbreekt) |
| `github_pat` | GitHub Personal Access Token |
| `github_app` | GitHub App installation token (voor organisaties) |

Twee mechanismen om credentials te configureren, allebei geldig:

**Convention-based environment variables:**
```
CORPUS_AUTH_AMSTERDAM_TOKEN=ghp_abc123...
CORPUS_AUTH_MINBZK_CENTRAL_TOKEN=ghp_def456...
```

De naamconventie is `CORPUS_AUTH_{SOURCE_ID_UPPERCASE}_TOKEN`, waarbij streepjes in de source ID underscores worden. Bij env vars is het type altijd `github_pat`.

**Auth config file** (`corpus-auth.yaml`, gitignored):
```yaml
amsterdam:
  type: github_pat
  token: ghp_abc123...

minbzk-central:
  type: github_app
  app_id: 12345
  private_key_path: /etc/regelrecht/keys/minbzk.pem
  installation_id: 67890
```

De lookup-volgorde is: env var eerst, dan auth config file. Env vars zijn handig in CI/CD en containers; het auth config file is handiger bij lokaal ontwikkelen met meerdere bronnen.

**Editor auth:** de editor slaat een GitHub token op in de browser (localStorage of sessionStorage). Dit token staat niet in het registry-manifest of de auth config - het is per-gebruiker en wordt alleen client-side gebruikt voor de write path.

### 4. Discovery, laden en schrijven

#### Read path (engine + admin)

```
corpus-registry.yaml
        │
        ▼
┌─────────────────┐
│ CorpusRegistry   │  parsed manifest, merged met lokale overrides
└────────┬────────┘
         │
    ┌────┴────┐
    ▼         ▼
┌────────┐ ┌──────────┐
│GitHub  │ │Filesystem│
│Fetcher │ │Fetcher   │
└───┬────┘ └────┬─────┘
    │           │
    ▼           ▼
┌─────────────────┐
│  RuleResolver    │  +source_map, +load_sourced_law()
└─────────────────┘
```

- `CorpusRegistry` parsed het manifest en dispatcht naar de juiste fetcher per bron
- `GitHubFetcher` gebruikt de GitHub Trees API (1 call per repo voor de directory-structuur) en de Contents API (per bestand, voor de YAML-inhoud)
- ETag-caching: de fetcher slaat ETags op en stuurt `If-None-Match` headers mee, zodat een 304 geen bandbreedte kost
- Rate limit tracking: de fetcher leest `X-RateLimit-Remaining` headers en waarschuwt bij lage limieten
- `RuleResolver` krijgt een `source_map: HashMap<String, SourceId>` die per geladen wet bijhoudt uit welke bron die komt
- `load_sourced_law(law_id, source_id)`: laadt een specifieke wet uit een specifieke bron
- Bij conflicten (zelfde `$id` uit meerdere bronnen) wint de bron met de hoogste priority

**Scope-validatie:** scopes zijn claims, geen hard afdwingbare grenzen. Een wet uit een bron met scope `gemeente_code: GM0363` die zelf `gemeente_code: GM0518` declareert genereert een warning, geen fout. De engine vertrouwt de bron maar signaleert afwijkingen.

#### Write path (editor)

De editor kan direct naar een GitHub-fork schrijven via de GitHub Contents API, zonder tussenkomst van een backend.

Flow:
1. Gebruiker bewerkt YAML in de editor
2. Preview en validatie tegen het schema
3. Commit via `PUT /repos/{owner}/{repo}/contents/{path}` met het GitHub token uit de browser
4. Branch management: de editor werkt op een feature branch en kan een PR aanmaken via de GitHub API (`POST /repos/{owner}/{repo}/pulls`)

Dit maakt de editor een volwaardige YAML-editor voor gemeenten: bewerken, valideren, committen, PR maken, zonder dat ze een lokale ontwikkelomgeving nodig hebben.

#### Admin API

Nieuwe endpoints op de admin service:

| Endpoint | Methode | Beschrijving |
|----------|---------|-------------|
| `/api/sources` | GET | Bronnenlijst met status (laatst gesynchroniseerd, aantal wetten, errors) |
| `/api/corpus/laws` | GET | Alle geladen wetten met source-metadata (bron-ID, priority, scopes) |
| `/api/sources/{id}/sync` | POST | Forceer re-fetch van een specifieke bron |

#### Editor UI

- **Source badge**: elke wet toont een badge met de bronnaam (bijv. "MinBZK Centraal" of "Gemeente Amsterdam")
- **Source picker**: dropdown of sidebar om tussen bronnen te wisselen
- **Bron toevoegen**: dialoog om een GitHub owner/repo in te voeren en als bron toe te voegen
- **Cross-source `implements` visualisatie**: toon welke gemeentelijke wet welke hogere wet invult
- **Opslaan naar fork**: knop die de commit-dialoog opent (branch kiezen, commit message, PR aanmaken)

## Why

### Benefits

- **Past bij IoC**: het `implements`-mechanisme uit RFC-007 werkt cross-repo zodra je de wetten maar laadt. Het registry maakt dat laden expliciet en configureerbaar.
- **Decentraal eigenaarschap**: gemeenten, provincies en waterschappen beheren hun regelgeving in hun eigen repo. Geen PR naar een centrale repo nodig om een gemeentelijke verordening te wijzigen.
- **Transparant**: het registry-manifest is een YAML-bestand in Git. Wie wil zien welke bronnen de engine laadt, kijkt in het manifest.
- **Scope-validatie als trust boundary**: zonder complexe PKI-infra kan de engine via scopes valideren dat een bron geen wetten levert buiten haar jurisdictie.
- **Incrementeel adopteerbaar**: begin met het centrale corpus en voeg bronnen toe als gemeenten er klaar voor zijn. De `type: local` optie maakt lokale ontwikkeling makkelijk.

### Tradeoffs

- **GitHub API rate limits**: 60 requests per uur zonder auth, 5.000 met een token. Bij veel bronnen of grote repos kan dit een bottleneck zijn. Mitigatie: ETag-caching, Trees API (1 call per repo), en tokens per bron.
- **Complexiteit write path**: branch management, merge conflicts, en PR-afhandeling in de editor is niet triviaal. Dit is bewust in een latere fase gepland.
- **Schema-versie compatibiliteit**: als het YAML-schema verandert, moeten alle bronnen meebewegen. De engine moet omgaan met bronnen die op een oudere schema-versie zitten.
- **Dependency op GitHub**: het hele model leunt op GitHub als hosting-platform. De `type: local` fallback en het feit dat het manifest een simpel YAML-bestand is maken het uitbreidbaar naar andere platforms (GitLab, Gitea), maar dat is nu geen prioriteit.

### Alternatives Considered

**Alternative 1: Git clone in plaats van API**

Clone de hele repo naar het filesystem en lees de bestanden lokaal. Simpeler conceptueel, maar vereist een git binary in de container, meer disk space, en maakt het lastiger om de editor direct naar een fork te schrijven zonder backend. De GitHub API-benadering is lichter en werkt zowel vanuit de engine (server-side) als de editor (client-side).

**Alternative 2: Centraal discovery service**

Een aparte service die bijhoudt welke repos er zijn en hun metadata cachet. Introduceert een extra component om te beheren en deployen. Het manifest-in-Git patroon is transparanter, reviewbaar, en heeft geen runtime dependency op een extra service.

**Alternative 3: Git submodules**

Gebruik submodules om externe repos in het corpus op te nemen. Te rigide: elke toevoeging of update van een bron vereist een commit in de parent repo. Bij tientallen gemeenten wordt dat onbeheersbaar. Het registry-manifest ontkoppelt het registreren van bronnen van het laden ervan.

### Implementation Notes

De implementatie is in zeven fases gepland, elke fase levert een werkend geheel op.

**Fase 1 - Corpus loskoppelen**
Verplaats `regulation/nl/` naar een eigen repo (`MinBZK/regelrecht-corpus`). Pas CI, tests en engine-configuratie aan om vanuit de nieuwe repo te laden.

**Fase 2 - Registry + lokale multi-source**
Implementeer `CorpusRegistry` en `corpus-registry.yaml` parsing. Breid `RuleResolver` uit met `source_map` en priority-based conflict resolution. Ondersteun `type: local` zodat de huidige workflow blijft werken.

**Fase 3 - GitHub fetcher**
Implementeer `GitHubFetcher` met Trees API, Contents API, ETag-caching en rate limit tracking. Auth via environment variables en `corpus-auth.yaml`.

**Fase 4 - Admin API**
Voeg `/api/sources`, `/api/corpus/laws` en `/api/sources/{id}/sync` endpoints toe aan de admin service.

**Fase 5 - Editor multi-source lezen**
Source picker, source badge per wet, en direct GitHub-access vanuit de editor (via de GitHub API, niet via de admin backend).

**Fase 6 - Editor schrijven**
Commit via GitHub Contents API, branch management, en PR-aanmaak vanuit de editor.

**Fase 7 - Validatie en polish**
Scope-validatie, schema-versie compatibiliteit checks, collision reporting, en edge case afhandeling.

### Geraakt door deze RFC

| Bestand | Rol | Wijziging |
|---------|-----|-----------|
| `packages/engine/src/resolver.rs` | Law registry + indexes | +`source_map`, +`load_sourced_law()` |
| `packages/engine/src/article.rs` | `ArticleBasedLaw` struct | Ongewijzigd (source is metadata op resolver, niet op law) |
| `packages/engine/src/service.rs` | `LawExecutionService` | +registry loading, +source in trace output |
| `packages/engine/src/uri.rs` | `regelrecht://` URI parsing | Ongewijzigd (URIs verwijzen naar law_id, niet naar source) |
| `packages/corpus/src/config.rs` | Bestaande corpus config | Patroon hergebruiken voor auth config |
| `packages/admin/src/handlers.rs` | Admin API endpoints | +`/api/sources`, +`/api/corpus/laws` |
| `frontend/src/composables/useLaw.js` | Law loading | +multi-source, +GitHub direct access |
| `frontend/src/EditorApp.vue` | Editor UI | +source picker, +badge, +write-back |

## References

- RFC-007: Inversion of Control met `open_terms` en `implements`
- RFC-003: Delegation Pattern for Multi-Level Regulations (blijft geldig voor delegatie; deze RFC adresseert federatie)
- GitHub Trees API: `GET /repos/{owner}/{repo}/git/trees/{tree_sha}?recursive=1`
- GitHub Contents API: `GET /repos/{owner}/{repo}/contents/{path}`, `PUT /repos/{owner}/{repo}/contents/{path}`
