# regelrecht-chrono-poc-web

De HTTP-laag om [`regelrecht-simulator`](../simulator/README.md): één **wereld per
browsersessie**, in geheugen, als JSON, achter de login van RegelRecht.

Wat deze crate toevoegt is precies één ding: dat een mens de opstelling kan
bespelen. De simulator kent geen HTTP, geen sessie en geen casus; hier komt daar
een server om, en verder niets. Elke route is één aanroep op `World`, en er wordt
nergens iets bijgehouden dat niet in een cel ligt.

Lokaal starten (login uit, publieke wereld, corpus uit deze checkout):

```bash
just chrono-poc
# → http://localhost:7160
```

## Drie keuzes die de vorm bepalen

**Geen database.** De sessies staan in geheugen (`tower-sessions` `MemoryStore`)
en de werelden ook. Dat betekent één replica en een herstart die alles wist, en
dat is voor een opstelling om iets aan te tonen de juiste ruil: een wereld is een
gedachte-experiment en geen dossier.

**De casus zit niet in de code.** Het wereldbestand en de regelingen komen bij het
starten uit een bron die de omgeving noemt — een pad op schijf of een repo met een
token. Het image dat hieruit rolt bevat dus geen casusinhoud, en een andere casus
is een andere env-variabele. Daarom staat de bron ook niet in
`corpus-registry.yaml`: dat is een publiek bestand, en de naam van een privérepo
hoort daar niet in.

**Een wereld per thread.** Een `World` is niet `Send` — een cel houdt haar engine
in een `RefCell` en die engine kan tijdens een besluit een `Rc<dyn CellResolver>`
dragen. Een `DashMap<SessionId, World>` kan dus niet: axum wil zijn state
`Send + Sync`. Elke sessie krijgt daarom een eigen thread die haar wereld bezit, en
wat er in de map staat is de *greep* op die thread. Dat levert op wat een wereld in
de map ook opgeleverd zou hebben — sessies raken elkaar niet — plus iets extra's:
twee sessies wachten ook niet op elkaar, want ze delen geen slot. De prijs is een
thread per actieve sessie, en de TTL van een uur is wat die prijs begrenst. Zie de
moduledocs van `src/worlds.rs`.

## Configuratie

| variabele | wat |
|---|---|
| `CHRONO_POC_WORLD_SOURCE` | **verplicht.** `local:<pad>` of `github:<owner>/<repo>@<ref>:<pad>` naar het wereldbestand |
| `CHRONO_POC_CORPUS_SOURCE` | de regelingenmap, dezelfde twee vormen. Afwezig: `REGULATION_PATH`, anders `corpus/regulation` in deze checkout |
| `CHRONO_POC_AUTH_REF` | de sleutel waaronder het GitHub-token opgezocht wordt; standaard de reponaam van de bron |
| `CHRONO_POC_REQUIRED_ROLE` | de rol waarachter `/api/*` staat; standaard `editor-reader` |
| `CHRONO_POC_PORT` | de poort; standaard `8000` (de container). Lokaal 7100-7300 |
| `STATIC_DIR` | de map met de gebouwde frontend; standaard `static` |
| `OIDC_*`, `KEYCLOAK_*`, `BASE_URL` | de login, zoals in editor-api en admin. Zonder `OIDC_CLIENT_ID` staat de login **uit** en is elke route open — alleen lokaal |

Het **token** volgt de conventie van de rest van de workspace:
`CORPUS_AUTH_<SLUG>_TOKEN`, opgelost door
`regelrecht_corpus::auth::CredentialResolver`. De slug is standaard de reponaam
van de bron (`owner/mijn-corpus` → `CORPUS_AUTH_MIJN_CORPUS_TOKEN`), en
`CHRONO_POC_AUTH_REF` kiest een kortere naam. De opzoeking is **strikt**: het
gedeelde `CORPUS_GIT_TOKEN` van de harvester-tijd gaat nooit naar een repo die
deze app aanwijst.

Het opstarten **faalt luid**. Geen wereldbestand, geen leesbare wereld, geen
regelingen of een ref die niet bestaat: het proces stopt met de reden. Leesbaar is
daarbij niet genoeg, dus het opstarten bouwt één wereld voordat de listener
opengaat (`WorldRegistry::check_buildable`): zo komt ook een wereldbestand boven
dat een regeling noemt die niet in de opgehaalde map staat — een corpusbron die
één map te hoog wijst, een ref waarin die wet nog niet bestond. Een server die
opkomt zonder wereld zou op elke healthcheck groen staan en op elk verzoek
dezelfde fout geven.

## De API

Veldnamen Engels, meldingen Nederlands. De vorm van een antwoord is het contract
dat `Snapshot` al vastlegt, en dat is Engels; elke fout die een mens leest komt uit
de simulator, en die praat Nederlands.

| route | wat |
|---|---|
| `GET /health` | leeft dit proces. Zonder login |
| `GET /api/world` | het beeld: klok, instellingen, cellen met hun kronieken, de acties die nu kunnen, wat er over een celgrens ging, de waarschuwingen |
| `POST /api/actions/{id}` | voer een actie uit; body = het formulier van die actie. Antwoord: `{ "snapshot": …, "events": … }` |
| `POST /api/advance` | `{"until": "2024-04-01"}`; de triggers gaan onderweg af. Zelfde antwoord |
| `GET /api/cells/{cel}/lexostatus/{naam}` | één reductie, alleen lezen. Parameters als queryparameters, `op_moment` optioneel |
| `PUT /api/settings` | wijzig instellingen die nog niet vast staan |
| `POST /api/reset` | terug naar de startstand uit het wereldbestand |

Alles onder `/api/` staat achter de rol uit `CHRONO_POC_REQUIRED_ROLE`. De
frontend komt uit `STATIC_DIR` met een SPA-fallback, zoals editor-api dat doet.

### Parameters van een lexostatus

De parameters komen als gewone queryparameters binnen en worden omgezet naar het
type dat de lexostatus **documenteert**:

```
GET /api/cells/brp/lexostatus/partnerschap?bsn=999993653&op_moment=2024-06-01
```

`bsn` is daar tekst en geen getal, omdat de definitie `type: string` zegt. Zonder
die omzetting zou elke waarde tekst zijn (en zou een filter op een getal nooit iets
vinden) of elke cijferreeks een getal (en zou een BSN met een nul vooraan stil van
waarde veranderen). De types worden uit het wereldbestand gelezen en niet aan de cel
gevraagd: een consument vraagt een gepubliceerde naam, hij inspecteert geen cel
(RFC-022 §4.1).

`op_moment` is gereserveerd en optioneel; afwezig betekent "op de stand van de
klok". Een moment ná de klok is een 409 — dat zou een voorspelling zijn.

### Wat geen fout is

- **"Niets vastgesteld"** is een antwoord met een reden: HTTP 200, met
  `outcome.not_established`. Een cel die op het gevraagde moment geen feit had, is
  niet stuk.
- **Een actie die nu niet kan** is een 409 met de uitleg van de wereld erin: het
  verhaal is nog niet zover, en dat is een stand en geen vergissing.
- **Een verstreken termijn** is een waarschuwing in het antwoord en geen fout. De
  uitvoerder mag alsnog besluiten; de wet zegt alleen wat de termijn was.

Fouten zijn altijd `{"error": "…"}`. De status volgt de foutvariant van de
simulator: 404 voor wat het pad aanwijst maar niet bestaat, 409 voor een wereld die
er niet naar staat, 400 voor een verzoek dat niet klopt tegen wat een definitie
belooft, 500 voor de rest. De hele afbeelding staat in `src/error.rs`.

## Wereldbestanden

Een **wereldbestand** is wat een wereld *is*: een klok, de cellen, de
instellingen, de startstand, de acties en de termijnen. Een scenariobestand draagt
zijn wereld in dezelfde velden inline en zet er een run overheen (`act`, `decide`,
`queries`); dit is het omgekeerde — een wereld zonder run, om door een mens
bespeeld te worden.

De publieke wereld staat in
[`packages/simulator/worlds/publieke_wereld.yaml`](../simulator/worlds/publieke_wereld.yaml)
(`burger`, `brp`, `belastingdienst`, `toeslagen`) en is waar `just chrono-poc` en
de tests van deze crate op draaien.

## Tests

`cargo test -p regelrecht-chrono-poc-web` draait mee in `just test`. De
HTTP-tests draaien de **echte** router over die **echte** wereld en het echte
corpus, met de login uit: het beeld, een actie, de klok vooruit, een besluit dat
een waarde over een celgrens accepteert, instellingen, terugzetten, en twee sessies
die twee werelden zijn. Een wereldbestand dat onleesbaar wordt, wordt hier rood en
niet in een container.

`tests/github_source.rs` doet hetzelfde voor het pad dat lokaal nooit aan bod komt:
een wiremock-GitHub in plaats van de echte (via de `GITHUB_API_BASE`-naad van
`regelrecht-github`), met een echt tarball erachter. Dat pint vast dat het archief
uitgepakt wordt zonder de `{owner}-{repo}-{sha}/`-laag, dat twee bronnen in
dezelfde repo op dezelfde ref **één** verzoek kosten, dat het token uit
`CORPUS_AUTH_<SLUG>_TOKEN` meegaat, en dat de tijdelijke map weer opgeruimd wordt.
