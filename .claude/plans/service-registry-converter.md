# Architectuurvisie: Regelrecht als regelgeving-platform

## Context

De conversie van wettekst naar machine-executable YAML zit nu in WIAT (https://gitlab.com/digilab.overheid.nl/ecosystem/wiat). Dat hoort bij hier, regelrecht — het platform voor machine-leesbare regelgeving. Regelrecht moet een zelfstandig platform worden dat het hele corpus juris (+ lokale regelingen, uitvoeringsbeleid) beheert als machine-leesbare YAML.

---

## Componentafbakening

| Component | Verantwoordelijkheid | Input | Output | Status |
|-----------|---------------------|-------|--------|--------|
| **Harvester** | Scrapen: wettekst ophalen van overheid.nl en opslaan als tekst-only YAML | BWB/CVDR bronnen (overheid.nl) | Tekst-only YAML op `main` | Bestaand (`packages/harvester/`) |
| **Converter** | Machine-readable maken: tekst-only YAML omzetten naar executeerbare YAML via LLM + engine validatie | Tekst-only YAML (van `main`) | YAML + `machine_readable` secties op `draft-conversions` | **Nieuw** (deze RFC) |
| **Engine** | Uitvoeren: machine-readable YAML evalueren met parameters | YAML + parameters + datum | Berekend resultaat | Bestaand (`packages/engine/`) |
| **Service** | API: registry, execution en conversion endpoints aanbieden | HTTP requests | JSON/YAML responses | **Nieuw** (deze RFC) |

De harvester weet niets van machine-readability — het is een scraper die wettekst in het juiste YAML-formaat zet. De converter is een apart proces dat daar bovenop machine_readable secties genereert.

---

## Kernprincipes

1. **Git is de registry** — geen database voor artefacten. Git repo met YAML files. Branches voor kwaliteitsniveaus.
2. **Alles Rust** — past bij de bestaande codebase, één taal, één build.
3. **LLM-agnostisch** — niet gebonden aan Claude. Generieke interface voor elk model.
4. **Queue-driven conversie** — harvester triggert, converter draait op achtergrond, mens cherry-pickt.
5. **Volledig corpus** (north-star) — alle Nederlandse wetgeving + lokale regelingen + uitvoeringsbeleid. Initieel: bestaande test-wetten.

---

## 1. Git-based registry: branches als kwaliteitsniveaus

De directorystructuur volgt het bestaande harvester-patroon (al in gebruik):

```
regulation/nl/                         # Dit IS de registry (git repo)
│
├── main branch                        # Bron van waarheid
│   ├── wet/
│   │   ├── wet_op_de_zorgtoeslag/     # Slug (harvester genereert uit titel)
│   │   │   └── 2025-01-01.yaml        # valid_from datum. Bevat bwb_id in YAML.
│   │   ├── participatiewet/
│   │   │   └── 2022-03-15.yaml
│   │   └── ...
│   ├── amvb/
│   ├── ministeriele_regeling/
│   │   └── regeling_standaardpremie/
│   │       └── 2025-01-01.yaml
│   ├── gemeentelijke_verordening/
│   │   ├── amsterdam/                 # Gemeente als extra niveau
│   │   │   └── apv_erfgrens/
│   │   │       └── 2024-01-01.yaml
│   │   └── diemen/
│   │       └── afstemmingsverordening_participatiewet/
│   │           └── 2015-01-01.yaml
│   ├── uitvoeringsbeleid/
│   └── ...
│
└── draft-conversions branch           # LLM-gegenereerd: zelfde structuur + machine_readable
    └── (zelfde paden, maar YAML bevat machine_readable secties)
```

**Identificatie in de YAML** (al zo door de harvester):
```yaml
$id: wet_op_de_zorgtoeslag              # Slug (voor cross-law referenties)
bwb_id: BWBR0018451                     # Formele identifier
regulatory_layer: WET
valid_from: '2025-01-01'
url: https://wetten.overheid.nl/BWBR0018451/2025-01-01
```

De API kan opzoeken via zowel slug (`wet_op_de_zorgtoeslag`), BWB-ID (`BWBR0018451`), als padpatroon.

### Flow: twee gescheiden transformaties

```
                    STAP 1: Scrapen                         STAP 2: Machine-readable maken
                    (harvester)                             (converter)

[overheid.nl]  ── XML parsen ──→  tekst-only YAML  ── LLM + engine validatie ──→  YAML + machine_readable
  (BWB/CVDR)                      commit op main                                   commit op draft-conversions
                                                                                          │
                                                                                   mens reviewt PR
                                                                                          │
                                                                                          ▼
                                                                              merge naar main
```

De harvester weet niets van machine-readability. De converter weet niets van scrapen. Ze communiceren via git: de converter detecteert nieuwe tekst-only YAML op `main` via `git diff`.

### Waarom git en niet een database

- **Versiebeheer is gratis** — elke wet-versie is een commit, diffs zijn triviaal
- **Review workflow = pull request** — bestaande tooling (GitHub/GitLab)
- **Audit trail = git log** — wie, wanneer, wat
- **Offline beschikbaar** — `git clone` en je hebt het hele corpus
- **Geen migraties** — schema is het YAML-formaat, gevalideerd door de engine
- **Reproduceerbaar** — elke commit is een snapshot van het hele corpus

### Kwaliteitsniveaus via branches

| Branch | Inhoud | Wie schrijft | Wie reviewt |
|--------|--------|-------------|-------------|
| `main` | Tekst-only wetten (harvester) + goedgekeurde machine_readable (cherry-picked) | Harvester (tekst) + mens (cherry-pick) | — |
| `draft-conversions` | Tekst + LLM-gegenereerde machine_readable | Converter (automatisch) | Mens reviewt, cherry-pickt naar main |

### Branch-divergentie beperken

`main` en `draft-conversions` zullen uiteenlopen naarmate de harvester nieuwe wetten toevoegt en de converter achterblijft. Mitigatie:

- **Regelmatig rebasen** — `draft-conversions` wordt periodiek gerebased op `main` zodat de diff alleen machine_readable-toevoegingen bevat
- **PR per wet** — de converter maakt per wet een pull request aan (niet een bulk cherry-pick), zodat review behapbaar blijft
- **Geen directe cherry-pick** — review via PR; cherry-pick als merge-strategie, niet als handmatige actie

### Harvester: CLI met cron-trigger

De harvester (`packages/harvester/`) is een CLI tool, geen langlopend proces. Aansturing:
- Een **GitHub Action of cron job** draait de harvester periodiek (bv. dagelijks)
- Nieuwe/gewijzigde wetten worden gecommit op `main`
- De converter queue detecteert deze via `git diff` tussen `main` en `draft-conversions`

---

## 2. Service-architectuur

Eén Rust (Axum) service met drie rollen:

```
regelrecht-mvp/
  packages/
    engine/             # BESTAAND: Law execution (library)
    harvester/          # BESTAAND: BWB downloader (library/CLI)
    service/            # NIEUW: API + converter
      src/
        main.rs
        config.rs

        api/
          registry.rs   # GET /laws/... (leest uit git repo)
          execution.rs  # POST /execute (engine in-process)
          conversion.rs # POST /convert (sync + async)

        converter/
          pipeline.rs   # Orchestrator: LLM → validate → repair → scenarios
          generator.rs  # Prompt + LLM call
          validator.rs  # Engine validatie + auto-fixes
          repairer.rs   # Repair loop
          scenarios.rs  # Scenario generatie + uitvoering
          queue.rs      # Job queue (leest git, pikt nieuwe wetten)

        llm/
          client.rs     # Trait: LlmClient
          anthropic.rs  # Claude implementatie
          openai.rs     # OpenAI implementatie

        git/
          repo.rs       # Git operaties (read/write/branch/commit)

      prompts/
        generate.md
        generate_batch.md
        repair.md
        scenarios.md
```

### Engine als library, niet als subprocess

```rust
// Direct in-process
use regelrecht_engine::LawExecutionService;

let service = LawExecutionService::new();
service.load_law(&yaml)?;
let result = service.evaluate_law_output("zorgtoeslagwet", "heeft_recht", &params, date)?;
```

### Concurrency-patroon

`LawExecutionService` is `Send`-safe — het struct bevat alleen `RuleResolver` en `DataSourceRegistry`, geen `Rc`. De `Rc<RefCell<TraceBuilder>>` die de engine gebruikt wordt per-evaluatie aangemaakt in `ResolutionContext`, niet opgeslagen op de service.

Pattern voor concurrent access in async Axum handlers:

- **`RwLock<LawExecutionService>`** — read-lock voor `evaluate_law_output` (meerdere readers tegelijk), write-lock voor `load_law` (exclusief)
- **`spawn_blocking`** — engine-evaluatie is CPU-bound; wrap in `tokio::task::spawn_blocking` om de async runtime niet te blokkeren

### Observability

Structured logging via `tracing` (al een dependency van de engine). De service voegt spans toe per API-request en per conversie-job.

Metrics om te monitoren:
- **Conversie success/failure rate** — per wet, per LLM-provider
- **Queue depth** — aantal wachtende conversie-jobs
- **Engine evaluatie-duur** — per wet/output combinatie

---

## 3. Converter: queue-driven pipeline

### Queue mechanisme

De converter monitort de git repo op changes in `main`:

1. **Poll**: periodiek `git pull` op main, vergelijk met `draft-conversions`
2. **Detect**: welke YAML files zijn nieuw/gewijzigd op main maar niet (of outdated) op draft-conversions?
3. **Enqueue**: voor elke nieuwe/gewijzigde wet → conversie-job
4. **Process**: converter draait pipeline, commit resultaat op `draft-conversions`

```rust
// Pseudo-code
async fn queue_loop(repo: &GitRepo, converter: &Pipeline) {
    loop {
        let changed = repo.diff_branches("main", "draft-conversions")?;
        for law_path in changed.new_or_modified() {
            let law_yaml = repo.read_file("main", &law_path)?;
            let result = converter.convert(&law_yaml).await?;
            repo.write_and_commit("draft-conversions", &law_path, &result)?;
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
```

### Conversie-pipeline (per wet)

Dezelfde bewezen flow als in wiat, maar in Rust:

```
[1. Parse wet YAML]
    ↓
[2. LLM generatie]  ←── prompt template + JSON schema
    ↓                     (batched als >25 artikelen)
[3. Validatie]      ←── engine (in-process) + jsonschema
    ↓ errors
[4. Repair]         ←── LLM met error feedback (max 2x)
    ↓
[5. Scenario's]     ←── LLM genereert tests
    ↓
[6. Uitvoeren]      ←── engine evalueert scenarios
    ↓ failures → terug naar 2 (max 3 iteraties)
    ↓ all pass
[7. Commit op draft-conversions]
```

### Ad-hoc conversie (async job)

Naast de queue-driven (harvester) conversie is er een ad-hoc conversie via de API (zie sectie 5 voor het volledige endpoint design). Dezelfde pipeline, maar:
- **Trigger**: API call (niet harvester)
- **Output**: response body (niet git commit)
- **Stateless**: resultaat wordt niet opgeslagen in registry
- **Input**: optioneel diff-aware (before_prose + before_machine + after_prose → after_machine)

---

## 4. LLM-abstractielaag

```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Structured output: prompt + JSON schema → validated JSON
    async fn structured_output(
        &self,
        prompt: &str,
        schema: &serde_json::Value,
    ) -> Result<serde_json::Value, LlmError>;
}

pub struct AnthropicClient { api_key: String, model: String }
pub struct OpenAiClient { api_key: String, model: String }
```

Beide providers ondersteunen structured output via hun API:
- **Anthropic**: `tool_use` met JSON schema
- **OpenAI**: `response_format.json_schema`

Na de LLM-response: lokale validatie met `jsonschema` crate (al in regelrecht-mvp).

---

## 5. API Design

### Identificatie

Volgt het bestaande harvester-patroon:
- **Slug** als directory naam en `$id` in YAML (bv. `wet_op_de_zorgtoeslag`)
- **BWB-ID** als veld in de YAML (`bwb_id: BWBR0018451`)
- **Gemeente** als extra niveau voor lokale regelingen (`amsterdam/apv_erfgrens`)

De API accepteert zowel slug als BWB-ID als lookup-key.

### Registry (leest uit git repo, resolved intern)

```
GET  /api/v1/laws                          # Lijst alle wetten
GET  /api/v1/laws/{id}                     # Metadata (?date=)
GET  /api/v1/laws/{id}/yaml                # YAML content (beste beschikbare versie)
GET  /api/v1/laws/{id}/versions            # Alle versies
POST /api/v1/laws/lookup                   # Batch lookup
```

`{id}` accepteert slug (`wet_op_de_zorgtoeslag`) of BWB-ID (`BWBR0018451`). De service resolved intern: zoekt eerst op `main` (reviewed/goedgekeurd), dan op `draft-conversions` (LLM-draft). Consumer ziet alleen het resultaat + kwaliteitsindicator:

```
X-Quality: reviewed       # of "draft" of "text-only"
X-Has-Machine-Readable: true
```

### Execution (engine in-process)

```
POST /api/v1/execute                       # Evalueer output
POST /api/v1/execute/scenarios             # Run scenarios
POST /api/v1/validate                      # Valideer YAML
```

### Conversion (async, lang-lopend)

Conversie kan minuten tot een uur duren. Daarom: **async job met drie notificatiekanalen**.

#### Job aanmaken

```
POST /api/v1/convert/jobs
{
  "after_prose": "...",              # VERPLICHT: wettekst (na wijziging, of de enige versie)

  "before_prose": "...",             # OPTIONEEL: wettekst vóór wijziging
  "before_machine": "...",           # OPTIONEEL: machine-readable YAML vóór wijziging

  "callback_url": "https://..."     # OPTIONEEL: webhook bij completion
}
```

**Alle before-velden zijn optioneel**. Als je alleen `after_prose` stuurt → simpele conversie. Als je alle drie stuurt → diff-aware conversie (de LLM krijgt before-context en past alleen de gewijzigde artikelen aan).

**Response: `202 Accepted`**
```json
{
  "job_id": "abc-123",
  "status": "pending",
  "poll_url": "/api/v1/convert/jobs/abc-123",
  "events_url": "/api/v1/convert/jobs/abc-123/events"
}
```

#### Job state management

Ad-hoc API jobs (`POST /convert/jobs`) hebben state nodig voor status-polling:

- **In-memory `DashMap<JobId, JobState>`** — snel, geen externe dependencies. State is ephemeral: gaat verloren bij restart, maar dat is acceptabel voor conversie-jobs (herstart = opnieuw submitten)
- Queue-driven jobs (harvester → draft-conversions) zijn fire-and-forget: de converter pakt werk op, commit resultaat. Geen status-tracking nodig — het resultaat is de git commit

#### Notificatiekanalen

**1. Polling (simpelst, universeel — eerste implementatie)**
```
GET /api/v1/convert/jobs/{job_id}
→ 200 OK
{
  "job_id": "abc-123",
  "status": "generating",           # pending|generating|validating|repairing|testing|completed|failed
  "progress": "Batch 2/4 gereed",
  "iteration": 1,
  "created_at": "...",
  "result_yaml": null               # gevuld bij completed
}
```

**2. SSE stream (later: real-time voortgang voor editor)**

> SSE is een latere toevoeging. Start met polling-only; SSE pas als de editor real-time voortgang nodig heeft.

```
GET /api/v1/convert/jobs/{job_id}/events
Accept: text/event-stream

data: {"type":"progress","stage":"generating","detail":"Batch 2/4 gereed..."}
data: {"type":"progress","stage":"validating","detail":"Engine validatie..."}
data: {"type":"progress","stage":"repairing","detail":"Artikel 3 repareren..."}
data: {"type":"completed","result_yaml":"..."}
```

**3. Webhook callback (server-to-server)**
```
POST {callback_url}
{
  "job_id": "abc-123",
  "status": "completed",
  "result_yaml": "..."
}
```

#### Queue (background harvester-triggered jobs)

```
GET  /api/v1/convert/queue                 # Overzicht van queue-jobs
GET  /api/v1/convert/jobs/{job_id}         # Status (zelfde als hierboven)
```

De queue-driven converter (harvester → draft-conversions) gebruikt dezelfde job-infra als ad-hoc conversies. Het verschil is de trigger (automatisch vs. API-call) en de output (git commit vs. response body).

#### Stateless voor hypothetische versies

Ad-hoc conversies (via API) zijn **stateless**: het resultaat wordt niet opgeslagen in de registry. Dit is essentieel voor:
- Voorliggende wetswijzigingen (nog niet definitief, amendementen mogelijk)
- Editor-experimenten (artikel wijzigen, kijken wat er verandert)
- Wiat impact-analyses (hypothetische "na"-versie)

---

## 6. Authenticatie & deployment

### API-authenticatie

- **API key via `X-API-Key` header** — simpel, voldoende voor server-to-server en editor
- Geen user-authenticatie nodig (geen multi-tenant)

### LLM API keys

- Via environment variables (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`)
- Niet in config files of git

### Deployment

- Deploy op **RIG** (Quattro/rijksapps), naast de bestaande frontend
- Zelfde project (`regel-k4c`), apart component (`service`)
- Docker image met de compiled Rust binary

---

## 7. Prompt templates

Migratie vanuit wiat. Start met `include_str!()` — compile-time embedded templates met `format!()` voor variabelen. Tera (Rust Jinja2-equivalent) pas als templates runtime-conditionals nodig hebben die `format!()` niet aankan.

| Template | Doel |
|----------|------|
| `generate.md` | Volledige wet → machine_readable (≤25 artikelen) |
| `generate_batch.md` | Batch artikelen → machine_readable |
| `repair.md` | Reparatie van kapotte artikelen met error feedback |
| `scenarios.md` | Genereer testscenario's |

---

## 8. Wetswijzigingsdocumenten (toekomstvisie)

Wijzigingsdocumenten (Staatsblad) worden ook YAML:

```yaml
type: amendment
source: Stb-2025-123
target_law: zorgtoeslagwet
effective_date: 2026-01-01
changes:
  - article: "2"
    operation: replace
    new_text: "..."
  - article: "3a"
    operation: insert_after
    after: "3"
    text: "..."
```

De converter kan dan automatisch:
1. Wijzigingsdocument + huidige wet → nieuwe wet-versie
2. Nieuwe versie converteren naar machine_readable
3. Commit op draft-conversions

---

## 9. Integratie met wiat

Wiat wordt een pure API-consumer:

```python
class RegelrechtClient:
    async def get_law_yaml(self, law_id: str, date: str) -> str | None:
        """GET /api/v1/laws/{law_id}/yaml?date={date}
        Krijgt beste beschikbare versie (reviewed > draft > text-only)."""

    async def start_conversion(
        self,
        after_prose: str,
        before_prose: str | None = None,
        before_machine: str | None = None,
    ) -> str:
        """POST /api/v1/convert/jobs → returns job_id"""

    async def poll_conversion(self, job_id: str) -> ConversionStatus:
        """GET /api/v1/convert/jobs/{job_id} → status + result"""

    async def execute(self, law_id: str, output: str, params: dict, date: str) -> dict:
        """POST /api/v1/execute"""
```

### Wiat impact-analyse flow

```
1. "Voor"-wet ophalen     → GET /laws/{id}/yaml (registry)
                            → als text-only: start_conversion(after_prose=tekst)
                              en poll tot klaar

2. Wijziging toepassen     → wiat zelf (apply_law_changes)
                            (hypothetische versie — nog niet definitief)

3. "Na"-wet converteren    → start_conversion(
                                after_prose=gewijzigde_tekst,
                                before_prose=originele_tekst,
                                before_machine=voor_yaml
                             )
                             → poll tot klaar (stateless, niet in registry)

4. Vergelijken voor/na     → wiat's eigen logica
```

### Editor flow

```
1. Gebruiker wijzigt artikel in editor
2. Editor → start_conversion(
               after_prose=gewijzigd_artikel,
               before_prose=origineel_artikel,
               before_machine=huidige_machine_readable
            )
3. Editor pollt status (later: SSE voor real-time voortgang)
4. Bij completion: toon diff machine_readable voor/na
```

Wiat's `regelrecht_generate.py` (1.084 regels) wordt vervangen door deze client.

---

## 10. Tech stack

| Component | Technologie | Status in regelrecht-mvp |
|-----------|------------|--------------------------|
| Web framework | Axum 0.8 | Al in gebruik |
| Async runtime | Tokio | Al in gebruik |
| Engine | regelrecht-engine (library) | Al in gebruik |
| HTTP client | reqwest | Toe te voegen |
| Templating | `include_str!()` + `format!()` (Tera later indien nodig) | — |
| JSON Schema validatie | jsonschema 0.42 | Al in gebruik |
| Git operaties | subprocess `git` (niet git2/libgit2) | Toe te voegen |
| Serialization | serde + serde_json + serde_yaml | Al in gebruik |
| Job state | DashMap (in-memory) | Toe te voegen |
| Logging | tracing | Al in gebruik |

### Git via subprocess

Subprocess `git` in plaats van `git2` (libgit2 bindings):
- **Simpeler** — geen C-library dependency, geen complexe binding-API
- **Battle-tested** — dezelfde git die iedereen gebruikt
- **Serialisatie** — `Mutex<()>` rond write-operaties (commit, checkout) om race conditions te voorkomen; reads zijn veilig parallel

---

## 11. Migratiefasen

> **Scope**: "Volledig corpus juris" is de north-star visie, niet een near-term deliverable. De eerste fasen richten zich op de bestaande test-wetten (zorgtoeslag, participatiewet, regeling standaardpremie, etc.) en de infra om dat te laten werken.

### Fase 0: Scaffolding
- `packages/service/` met Cargo.toml
- Axum skeleton + API key auth
- Git repo setup (corpus structuur)
- **Milestone**: service start, health endpoint

### Fase 1: Registry + Execution
- Git-based law lookup (subprocess `git`)
- Engine als library (in-process, `RwLock` + `spawn_blocking`)
- Registry + execute endpoints
- **Milestone**: `GET /laws/zorgtoeslagwet/yaml` + `POST /execute`

### Fase 2: Converter
- LlmClient trait + Anthropic implementatie
- Conversie-pipeline in Rust
- Ad-hoc endpoint: `POST /convert/jobs` + polling
- Queue: poll git, convert, commit op draft-conversions
- **Milestone**: automatische conversie van bestaande test-wetten

### Fase 3: wiat als consumer
- RegelrechtClient in wiat
- regelrecht_generate.py → API wrapper
- Oude code verwijderen

### Fase 4: Schalen
- SSE voor real-time voortgang in editor
- OpenAI provider
- Harvester uitbreiden (lokale regelingen, beleid)
- Review UI (diff-viewer main ↔ draft-conversions)
- Wetswijzigingsdocumenten

---

## 12. Status

**Architectuurvisie** — denkoefening, geen implementatie nu. Volgende stap: bespreken met team.
