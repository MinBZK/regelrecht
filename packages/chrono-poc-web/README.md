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
| `GET /api/world` | het beeld: klok, instellingen, cellen met hun kronieken, de acties die nu kunnen (met per actie haar voorwaarden), wat er over een celgrens ging, de waarschuwingen |
| `POST /api/actions/{id}` | voer een actie uit; body = het formulier van die actie. Antwoord: `{ "snapshot": …, "events": … }` |
| `POST /api/advance` | `{"until": "2024-04-01"}`; de triggers gaan onderweg af. Zelfde antwoord |
| `GET /api/cells/{cel}/lexostatus/{naam}` | één reductie, alleen lezen. Parameters als queryparameters, `op_moment` optioneel |
| `GET /api/cells/{cel}/chronicles/{stroom}/grams/{n}/receipt` | het RFC-013 uitvoeringsreceipt van één decretogram, alleen lezen. `n` is de plek in de kroniek, geteld vanaf nul |
| `PUT /api/settings` | wijzig instellingen die nog niet vast staan |
| `POST /api/reset` | terug naar de startstand uit het wereldbestand |
| `GET /api/portaal` | het `portaal` uit het wereldbestand — actor, label, persona's met hun ingevulde vragen, en de vragen als sjabloon — of `null` als er geen is |
| `PUT /api/persona` | `{"id": "aanvrager-a"}` of `{"id": null}`: kies een persona voor deze sessie. Legt niets vast; antwoord is het beeld, met `persona` en de voorinvulling van de aanvrager. `reset` laat de keuze staan |

Alles onder `/api/` staat achter de rol uit `CHRONO_POC_REQUIRED_ROLE`. De
frontend komt uit `STATIC_DIR` met een SPA-fallback, zoals editor-api dat doet.
Eén pad valt buiten die fallback: `/favicon.ico` geeft `favicon.svg` uit de
bundel, met het mediatype van dat bestand. Een browser vraagt dat pad uit zichzelf
op en kreeg er anders `index.html` onder een 404 terug — HTML aangeboden als
plaatje.

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

Naast de uitkomst draagt het antwoord een blok `reductie`: welke gegevens de cel
gelezen heeft en hoe ze die reduceerde, met een verwijzing naar elk gebruikt gram.
Wat erin staat, staat in `packages/simulator/README.md` onder
"[Hoe het antwoord tot stand kwam](../simulator/README.md#hoe-het-antwoord-tot-stand-kwam)";
deze laag geeft het door zoals de cel het gaf.

### Het zaakkenmerk

Eén parameter vraagt meer uitleg dan de andere: het **zaakkenmerk**. Een BSN weet
de vrager; een zaakkenmerk niet, want het bestaat nergens voordat er een besluit
genomen is. Het ontstaat uit een **sjabloon** bij de besluit-definitie
(`zaakkenmerk: '{partij}/{jaar}'`), wordt bij het besluit ingevuld uit de
parameters, en komt als vast veld in het decretogram te staan. De kroniek met
beschikkingen groepeert erop, en een lexostatus over die kroniek vraagt het
daarom als sleutel. Zie [`packages/simulator/README.md`](../simulator/README.md)
— "Wat een decretogram draagt", de ingebouwde verwijzing `$zaakkenmerk` bij de
vier inputvormen, en "Het zaakkenmerk moet bij precies één zaak horen" voor de
twee regels die bewaken dat twee zaken er nooit één worden.

Omdat het nergens vandaan lijkt te komen, draagt `GET /api/world` het mee: per
lexostatus-definitie haar `doc`, haar parameters met hun type en — bij een
kroniekfilter — de `key` (de stroom en het sleutelveld), en per besluit-definitie
het zaakkenmerk-**sjabloon** met de kroniek waarin haar decretogrammen landen.
Daarmee kan een client uitleggen wat er in dat veld hoort, welke vorm het heeft,
en welke kenmerken er nu in die kroniek liggen — alles uit hetzelfde beeld, zonder
het wereldbestand ernaast te leggen. De frontend doet dat in het tabblad
Lexostatus.

### Het receipt van een decretogram

Het beeld draagt het receipt **niet**: het bevat wandkloktijd, en een contract dat
per run verschilt is geen contract. Het gram draagt het wél — een decretogram *is*
het RFC-013 Execution Receipt van het besluit (RFC-022 §1.2) — en deze route is de
weg ernaartoe op verzoek, zodat dat na te kijken is zonder het beeld te vervuilen.

```
GET /api/cells/toeslagen/chronicles/beschikkingen/grams/0/receipt
```

`n` telt vanaf nul, in precies de volgorde waarin `GET /api/world` de grammen van
die stroom geeft. Het antwoord is het receipt zelf — `provenance`, `engine_config`,
`scope`, `execution`, `results` gaan ongewijzigd door — met daarnaast `gram` (van
welk gram dit het receipt is, met het zaakkenmerk en het moment in de *logische*
tijd), `accepted_values` (per waarde de bron-cel, het bevoegd gezag dat die bron
noemde, het moment en het zaakkenmerk) en `timestamp` (de wandkloktijd, met erbij
dat het dát is).

Wijst het pad naar een gram dat geen decretogram uit het besluit-pad is, dan is dat
een **404**: er is geen receipt, want er heeft nooit een uitvoering gedraaid. Dat
is iets anders dan een leeg receipt.

### Wat geen fout is

- **"Niets vastgesteld"** is een antwoord met een reden: HTTP 200, met
  `outcome.not_established`. Een cel die op het gevraagde moment geen feit had, is
  niet stuk.
- **Een actie die nu niet kan** is een 409 met de uitleg van de wereld erin: het
  verhaal is nog niet zover, en dat is een stand en geen vergissing.
- **Een besluit dat de cel weigert** is óók een 409, met haar eigen reden erin: er
  is niets vastgesteld over een input die het besluit nodig heeft (bij de cel zelf
  of bij een ander), de cel is het bevoegd gezag niet dat de regeling aanwijst, of
  de aansturende uitkomst is op dat moment geen beschikking. De cel legt dan niets
  vast, en dat is precies wat ze hoort te doen.
- **Een verstreken termijn** is een waarschuwing in het antwoord en geen fout. De
  uitvoerder mag alsnog besluiten; de wet zegt alleen wat de termijn was.
- **Een voorwaarde die niet waar is** houdt een actie niet tegen. Een actie kan
  voorwaarden dragen — een uitkomst van een regeling die de actor zelf laadt, of
  een lexostatus van haar eigen stand — en die staan in het beeld onder
  `actions[].conditions` met `outcome` (`waar`, `onwaar`, `onbekend`), het artikel
  en de reden. `POST /api/actions/{id}` voert de actie gewoon uit, ook als een
  voorwaarde `onwaar` of `onbekend` zegt: juridisch mag iedereen een aanvraag
  indienen (Awb art. 4:1), en wie niet gerechtigd is, krijgt een afwijzing en geen
  409. Een 409 blijft voor wat de wereld technisch niet kan (`available: false`).
  De actor rekent de voorwaarden zelf uit en vraagt niets over een celgrens, dus
  `GET /api/world` levert nog steeds geen verkeer op. Zie
  [Voorwaarden op een actie](../simulator/README.md#voorwaarden-op-een-actie-tonen-niet-blokkeren).

Fouten zijn altijd `{"error": "…"}`. De status volgt de **foutvariant** van de
simulator en niet de tekst van de melding: 404 voor wat het pad aanwijst maar niet
bestaat, 409 voor een wereld die er niet naar staat of een cel die weigert, 400
voor een verzoek dat niet klopt tegen wat een definitie belooft, 500 voor de rest.
De hele afbeelding staat in `src/error.rs`.

## Wereldbestanden

Een **wereldbestand** is wat een wereld *is*: een klok, de cellen, de
instellingen, de startstand, de acties en de termijnen. Een scenariobestand draagt
zijn wereld in dezelfde velden inline en zet er een run overheen (`act`, `decide`,
`queries`); dit is het omgekeerde — een wereld zonder run, om door een mens
bespeeld te worden.

De publieke wereld staat in
[`packages/simulator/worlds/publieke_wereld.yaml`](../simulator/worlds/publieke_wereld.yaml)
(`burger`, `brp`, `belastingdienst`, `toeslagen`) en is waar `just chrono-poc` en
de tests van deze crate op draaien. De aanvraag van `burger` draagt er twee
voorwaarden: of ze binnen de termijn van Awir art. 15 valt (uit de wet, die
`burger` daarvoor laadt) en of er al een aanvraag ligt (uit haar eigen kroniek).

## Tests

`cargo test -p regelrecht-chrono-poc-web` draait mee in `just test`. De
HTTP-tests draaien de **echte** router over die **echte** wereld en het echte
corpus, met de login uit: het beeld, een actie, de klok vooruit, een besluit dat
een waarde over een celgrens accepteert, instellingen, terugzetten, en twee sessies
die twee werelden zijn. Een wereldbestand dat onleesbaar wordt, wordt hier rood en
niet in een container.

### De bewijsronde in de browser

Wat de HTTP-tests niet zien is de opstelling zoals een mens haar bespeelt: de
bundel, de webcomponenten van het ontwerpsysteem, en het verhaal van aanvraag
tot vaststelling in één sessie. Daarvoor staat er een Playwright-suite in
[`frontend-chrono-poc/e2e/`](../../frontend-chrono-poc/e2e):

```bash
just chrono-poc-e2e            # bundel + server + veertig checks in Chromium
just chrono-poc-e2e --headed   # of --ui / --debug; alles achter het recept gaat door
```

Het recept bouwt eerst de bundel en de binary; daarna start
`playwright.config.js` deze server zelf, op een vrije poort uit 7180-7300, met
`CHRONO_POC_WORLD_SOURCE=local:packages/simulator/worlds/publieke_wereld.yaml`
en zonder login. De testnamen zijn de check-id's van de ronde die eerder met de
hand liep (`L1`…`E6`), zodat een rode regel in CI dezelfde naam draagt als het
bewijs: lay-out, prefill, journaal, de velden van een decretogram, cross-law
binnen de cel, een lexostatus op een moment, de weigerflows en de
executogrammen van de vier termijnen.

Elke bewering die tekst uit een nldd-component leest, gaat langs de
`textAll`-helper in `e2e/world.js`: die componenten dragen hun label in een
attribuut of achter een shadow root, en `innerText` van de pagina ziet daar
niets van.

De suite draait bij elke PR (baan *E2E chronolexografie*). Eén check staat als
`fixme` geparkeerd: een besluit dat de wereld weigert komt er vandaag als 500
uit in plaats van als 4xx. Dat is een openstaande fout en geen keuze — zodra de
foutafbeelding dat rechttrekt, is het woord `fixme` weghalen het hele werk.

**Een ander doel, dezelfde suite.** Drie omgevingsvariabelen verzetten waar er
tegenaan gedraaid wordt, zonder een tweede suite te onderhouden:

| variabele | wat |
|---|---|
| `E2E_WORLD` | een ander wereldbestand (pad vanaf de repo-root of absoluut). De checks over de casus van de publieke wereld horen dan niet te slagen; wat meereist is de vorm van de opstelling |
| `E2E_BASE` | een opstelling die al draait — een deployment, of een server die je zelf startte. Er wordt dan niets gestart |
| `E2E_COOKIE` | de sessiecookie (`naam=waarde`) voor zo'n opstelling achter de login |

Een wereld met een **privécorpus** blijft daarmee buiten deze repo: die start je
zelf (`CHRONO_POC_CORPUS_SOURCE=github:…` plus het token) en wijst je met
`E2E_BASE` aan. In de repo staat alleen de publieke wereld, en in CI draait
alleen die.

`tests/github_source.rs` doet hetzelfde voor het pad dat lokaal nooit aan bod komt:
een wiremock-GitHub in plaats van de echte (via de `GITHUB_API_BASE`-naad van
`regelrecht-github`), met een echt tarball erachter. Dat pint vast dat het archief
uitgepakt wordt zonder de `{owner}-{repo}-{sha}/`-laag, dat twee bronnen in
dezelfde repo op dezelfde ref **één** verzoek kosten, dat het token uit
`CORPUS_AUTH_<SLUG>_TOKEN` meegaat, en dat de tijdelijke map weer opgeruimd wordt.
