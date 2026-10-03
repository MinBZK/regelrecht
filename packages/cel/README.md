# cel

Een proof of concept bij RFC-022: een runtime voor cellen en processen, met de
indeling van de positionpaper.

- Een **cel** legt feiten vast als chronolexogram in een eigen kroniek en
  reduceert die kroniek tot een lexostatus. Meer doet ze niet.
- Een **proces** handelt: het informeert (synthese van lexostatussen uit
  cellen), concludeert (de engine, de handelingen) en vraagt een cel vast te
  leggen. Met een portaal laat het een indiening toetsen door een artikel (een
  `TOETS`) en indienen; met een behandeling ziet een behandelaar een
  werkvoorraad, opent een zaak en doet er handelingen in: het besluit (een
  stage-decretogram), de stages erna zoals de bekendmaking (met de
  bezwaartermijn die de wet dan uitrekent), een betaling, en de feiten van het
  zaakverloop. Elke handeling eerst op proef, dan vastgelegd door de cel.

De cel waarin een proces vastlegt, is voor het proces een bron zoals elke
andere: het leest haar lexostatussen en kroniek langs dezelfde routes. Een cel
is configuratie, geen code: een map met een `cell.yaml`. Een proces volgt uit
het uitvoeringsbeleid van de actor en drie deploymentbestanden (RFC-047, zie
"Het proces uit beleid"). De code noemt geen casus; de tests draaien op de
generieke fixtures in `tests/fixtures/`. De docs-pagina
`docs/src/content/docs/components/cel.md` beschrijft dezelfde opzet, met de
afwijkingen van RFC-022 en de open vragen.


Open vragen over de lezing van de positionpaper, voor een gesprek met de
bedenkers van chronolexografie, staan in [GESPREKSPUNTEN.md](GESPREKSPUNTEN.md).

## Starten

```bash
just cel          # runtime op :7170, frontend op :7171, op de fixtures
```

## Configuratie

| Variabele | Betekenis |
|---|---|
| `CELLS_PATH` | Map met een submap per cel, elk met een `cell.yaml`. |
| `CELL_CHANNELS` | `channels.yaml` van de deployment: per cel-id de techniek van elk kanaal dat het beleid noemt. Optioneel: zonder draaien alleen de cellen; met volgen de processen uit het beleid (RFC-047). |
| `CELL_SYNTHESIS` | `synthesis.yaml` van de deployment: per cel-id `synthesis`, `assessment_rows` en `action_rows`, tot RFC-045, gevalideerd tegen `schema/chronolex/v0.3.0/synthesis.json`. Alleen met `CELL_CHANNELS`. |
| `CELL_EXAMPLES` | `examples.yaml` van de deployment: per cel-id de standaardgegevens van een proefopstelling. Alleen met `CELL_CHANNELS`. |
| `CELL_REGISTERS` | Optioneel koppelbestand van de registers (RFC-045 §1): per `<beleid>#<naam van het register>` de cel en kroniek die de input zonder bron (`source: {}`) van dat beleid vullen. Zonder houdt een beleid dat een register bevraagt de runtime tegen. Zie `src/register.rs`. |
| `CELL_REDUCTION` | Experiment A: `dsl` (standaard), `engine` (elke lexostatus als engine-run) of `compare` (allebei; elk verschil is een fout). `engine` en `compare` vragen `CELL_ENGINE_BINDING`. |
| `CELL_ENGINE_BINDING` | Experiment A: het koppelbestand van lexostatus naar regeling (zie `src/lexostatus_engine.rs`). Alleen samen met `engine` of `compare`. |
| `REGULATION_PATH` | Map met regelingen, gedeeld door de hele runtime; elk YAML-bestand met `$id` en `articles` wordt geladen. |
| `DATA_DIR` | Map voor de kronieken: per cel een submap `<id>/`. |
| `CELL_PORT` | Poort, standaard 7170. De runtime luistert op `0.0.0.0`. |
| `CELL_READ_TOKEN` | Optioneel leestoken (minstens 16 tekens) dat runtimes delen die elkaars cellen mogen lezen: een cel neemt het aan in `x-cell-read-token`. Zonder leest alleen de runtime zelf haar cellen. |
| `CELL_READ_TOKEN_SOURCES` | Optioneel: de basis-urls (komma's ertussen) van de runtimes die het leestoken delen; alleen een bron met zo'n url krijgt het mee over HTTP. |

```yaml
# <CELLS_PATH>/<map>/cell.yaml, schema schema/chronolex/v0.3.0/cell.json
id: <cel-id>                      # routes onder /cells/<id>/api/
recording_actor: <actor>          # elke stroom van de cel heeft deze actor
streams: [<pad>, ...]             # stroombestanden of mappen, relatief aan deze map
lexostatuses: <pad>               # lexostatus-definities
initial_state: <pad>              # optioneel: grammen voor een lege kroniek
```

## Het proces uit beleid

Er is geen procesbestand (RFC-047). De runtime leidt per actor een proces af
uit het uitvoeringsbeleid van zijn bevoegd gezag, de cellen en de
deploymentbestanden. Het beleid noemt de kanalen in
`produces.extensions.chronolex` van een artikel:

```yaml
extensions:
  chronolex:
    channels:
      <kanaal>:
        kind: portal | handling | counter   # bepaalt de routes van de rol
        role: <rol>                         # optioneel; zonder: de kanaalnaam
        identifies: {<veld>: [<grondslag>, ...]}   # of een lijst veldnamen
        owner: <veld van de indiening>      # wie een zaak volgt
        submits: <regeling>#<artikel>       # precies één kanaal per actor: het portaal
        assesses: {output: <output van dat artikel>}
        offers: {regulation, output, deadline?, windows?, start?, opening?}
        form: {document: <pad vanaf de corpuswortel>, screen: <id>}
        legal_basis: [<regeling>#<artikel>, ...]   # de eerste is de grondslag van de rol
    supplies:                               # wat een kanaal in de indiening levert
      <kanaal>: {<veld van het gram>: <veld van het kanaal> | $channel | $submitted_on}
    mandates:                               # ook handelen namens een ander gezag (Awb 10:1)
      - {authority: <naam>, legal_basis: <regeling>#<artikel>}
```

Wat de runtime daaruit afleidt:

- het proces-id is de id van de cel die de indiening van het portaal
  vastlegt, de actor de `recording_actor` van die stroom, het gezag dat van
  het beleid;
- het portaal-event is het ene indieningsevent dat het `submits`-artikel
  vestigt, de toets de ene lexostatus van die cel die het event leest en een
  parameter van dat artikel afleidt;
- de werkvoorraad is de ingebouwde lijst `worklist`: de zaken waarop nog niet
  elk gevraagd besluit is genomen, met de kolommen `received_at`,
  `recorded_at`, het eigenaarsveld van het portaalkanaal en het veld met
  origin-rol `TIJDVAK`;
- naast de werkvoorraad staat de ingebouwde lijst `cases`: elke zaak van die
  indiening, ook een beslote, met dezelfde kolommen en `decided_at` (de datum
  van het laatste besluitgram, leeg zonder besluit). Langs die lijst bereikt
  de behandelaar een zaak na het besluit, voor de bekendmaking en de betaling;
- de handelingen zijn de events van de cel met een intake die een kanaal van
  `kind: handling` noemt, genoemd naar hun event (een vervolg bij meer
  besluiten: `<event>_<besluit-event>`), met het artikel uit de wet (zie
  "Handelingen in een zaak");
- de origin-controle is altijd strict: een parameter zonder origin is een
  fout.

Welk wetsartikel een beleidsartikel uitwerkt, staat in `executes`
(RFC-047), optioneel met `as`: `fact_finding`, `interpretation` of
`weighing`, de drie onderwerpen van een beleidsregel in Awb 1:3 lid 4. De
uitleg bij een formulierveld ("Waarom?") toont het als stap met de term uit
de Awb en de bron: "voert wet_op_de_politieke_partijen#102 uit (vaststelling
van feiten, Awb 1:3 lid 4)". Zonder `as` staat er alleen "voert
wet_op_de_politieke_partijen#102 uit".

De deployment houdt alleen de techniek, per cel-id gegroepeerd:

```yaml
# CELL_CHANNELS: channels.yaml
<cel-id>:
  <kanaal>:
    adapter: simulated            # nagebootste login; geen register, geen gecertificeerde login
    label: <tekst>
    explanation: <tekst>          # optioneel
    intake: <pad>                 # optioneel: onder $intake.<pad>.<veld>; zonder: de kanaalnaam
    role_label: <tekst>           # optioneel; zonder: de rolnaam
    fields:                       # in loginvolgorde; de grondslag geeft identifies in het beleid
      <veld>: {label: <tekst>, pattern: <regex>, check: elfproef, message: <tekst>, numeric: true}

# CELL_SYNTHESIS: synthesis.yaml (tot RFC-045)
<cel-id>:
  synthesis:
    - {cell: <cel-id>, lexostatus: <naam>, case: true}   # een lexostatus van de zaak
    - cell: <id van de bron-cel>
      url: <http://host:poort>    # optioneel; zonder url: intern transport
      lexostatus: <naam bij de bron>
      input:
        <input van de bron>: {lexostatus: <lexostatus van de zaak of eerdere bron>, field: <parameter of extra veld>}
        <input van de bron>: {value: <vaste waarde>}
      parameters: [<naam>, ...]   # expliciet, geen wildcard; dezelfde naam bij bron en afnemer
      # of: parameters: {<naam bij de bron>: <parameter van de afnemer>}
      extra_fields: [<naam>, ...] # optioneel: invoer voor een latere bron
      legal_basis: [<regeling>#<artikel>, ...]   # waarop de vertaling rust; verplicht als de bron vertaalt
  assessment_rows: [...]          # synthese per regel van de toets
  action_rows:                    # synthese per regel per handeling (de eventnaam)
    <handeling>:
      - parameter: <array-parameter>
        table: {lexostatus: <van de zaak of een bron>, field: <tabelveld>}
        columns: {<kolom van de tabel>: <kolom van de parameter>}
        sources:
          - cell: <id>
            url: <http://host:poort>   # optioneel; zonder url: intern
            lexostatus: <naam bij de bron>
            input:
              <input>: {column: <kolom van de regel>}
              <input>: {lexostatus: <van de zaak of een bron>, field: <naam>}
              <input>: {parameter: <naam>}
              <input>: {regulation: <$id>, output: <output>}   # de wet leidt de invoer af
              <input>: {value: <vaste waarde>}
            columns: {<naam bij de bron>: <kolom van de parameter>}
            legal_basis: [...]    # als bij een synthese-bron

# CELL_EXAMPLES: examples.yaml, paden relatief aan dit bestand
<cel-id>:
  logins: [<pad>, ...]
  application: <pad>
  actions: {<handeling>: <pad>}   # {form: {...}}; "$today" wordt de datum van vandaag
```

Het portaal, de handelingen en de bronnen met `case: true` noemen dezelfde
cel: in deze stap handelt een proces over de zaken van een cel, en die cel
draait in dezelfde runtime. Een bron met `case: true` is een lexostatus van
de zaak zelf, met als enige input `root`: het besluit vraagt haar met het id
van de wortel van de zaak en neemt al haar parameters en extra velden over. Het
formulierbestand levert alleen labels, soorten en volgorde; het bepaalt nooit
het gedrag.

## Routes

| Route | Doet |
|---|---|
| `GET /api/cells` | de cellen, met per cel haar kronieken en haar lexostatussen (inputs, parameters, extra velden) |
| `GET /api/processes` | de processen, met per proces de actor, de cel, portaal, rollen, behandeling en de synthese-bronnen |
| `GET /cells/<id>/api/chronicle` | runtime- of leestoken: de grammen, elk met YAML |
| `GET /cells/<id>/api/cases/<root>` | runtime- of leestoken: de grammen van één zaak, elk met YAML; de cel filtert, 404 als ze de zaak niet kent |
| `GET /cells/<id>/api/lexostatus/<naam>?<input>=...` | runtime- of leestoken: een reductie; de inputs als query, en optioneel `as_of` en `known_at` (zie "Tijd"); `case_state` biedt de runtime aan (zie "De stand van een zaak") |
| `POST /cells/<id>/api/lexostatus/<naam>/trial` | alleen met het runtime-token: `{draft, inputs}`: de cel bouwt het gram van het concept in het geheugen en reduceert de kroniek mét dat gram (`inputs` mag een peil dragen; zonder input `root` telt de wortel van het concept); er wordt niets vastgelegd |
| `POST /cells/<id>/api/grams` | alleen met het runtime-token: `{actor, stream, event, intake, external, refers_to?, decision?, root_grams?}`: `refers_to` geeft per verwijzing van het event het id van het gram waarnaar het nieuwe gram verwijst. De cel bouwt het gram, geeft het een id, valideert het, controleert de actor en de verwijzingen, en legt het vast (201); 400 als een verwijzing een gram noemt dat de cel niet heeft of dat niet past bij `to`; 409 als er al een gram met die stage naar hetzelfde gram verwijst, als een besluit van hetzelfde event al naar hetzelfde gram verwijst, als het `effective_at` op een dag ligt voor een gram waarnaar het verwijst, of als de wortel niet meer `root_grams` grammen heeft |
| `GET /cells/<id>/api/stream` | de stroomdefinities van de cel, met hun hash; open |
| `GET /processes/<id>/api/examples` | de voorbeelden per handeling, zonder login |
| `GET /processes/<id>/api/map` | de kaart van het proces (pagina Opbouw): welke configuratie, events, lexostatussen en artikelen het gebruikt en hoe ze samenhangen; open |
| `GET /processes/<id>/api/law/<regeling>[/<artikel>]` | de YAML van een geladen regeling in de versie die vandaag geldt, of van een artikel daarvan; open, 404 voor wat niet geladen is |
| `GET /processes/<id>/api/config/<config>?anchor=<sleutel>` | een configuratiebestand dat het proces laadde (`form`, `stream/<id>`, `cell`, `lexostatuses`, `registers`, `channels`, `synthesis`, `examples`), of het blok van `anchor` daarin; open, 404 voor wat niet geladen is |
| `POST /processes/<id>/api/channels/<kanaal>/login`, `GET .../session`, `POST .../logout` | met rollen: de velden van het kanaal (en `role` als er langs het kanaal meer rollen inloggen) naar een sessie `{role, channel, fields}` |
| `GET /processes/<id>/api/session` | met rollen: wie er is ingelogd, langs welk kanaal ook |
| `GET /processes/<id>/api/form` | routes `portal`: de stroom en de formuliervelden |
| `POST /processes/<id>/api/application/assessment` | routes `portal`: proefreductie in de cel, synthese, engine |
| `POST /processes/<id>/api/application` | routes `portal`: de cel legt het gram vast |
| `GET /processes/<id>/api/possibilities` | routes `portal`: wat het aanbod zegt per tijdvak dat het beleid aanbiedt (`offer.windows`) |
| `POST /processes/<id>/api/counter/application` | routes `counter`: `{applicant, received_at, external}`; een aanvraag die langs een andere weg binnenkwam, met de dag van ontvangst als `effective_at` (niet na vandaag, niet vóór `offer.opening`) |
| `GET /processes/<id>/api/worklist` | routes `handling`: de werkvoorraad, een lijst uit de cel |
| `GET /processes/<id>/api/cases` | routes `handling`: alle zaken, ook beslote, met `decided_at`; een lijst uit de cel |
| `GET /processes/<id>/api/inspection/<cel>/chronicle`, `.../lexostatus/<naam>?...` | routes `handling`: inzage in een cel die het proces leest (de eigen cel en de bronnen zonder url); het proces geeft door wat de cel antwoordt |
| `GET /processes/<id>/api/cases/<root>` | routes `handling`: de grammen van de zaak, de procedure van de zaak (de stages zonder besluit), de besluiten met per besluit zijn stages, de rechtsbescherming die daaruit volgt en de handelingen die erop handelen, en per handeling of zij kan, op welk besluit, haar formulier en een proef zonder formulier |
| `POST /processes/<id>/api/cases/<root>/actions/<naam>/trial` | routes `handling` (en de rol van de handeling): `{form, decision?}` naar een handeling op proef, met in `decision` het id van het besluit waarop de handeling werkt (zonder: het laatste); niets wordt vastgelegd |
| `POST /processes/<id>/api/cases/<root>/actions/<naam>` | idem: `{form, decision?, happened?}` naar een vastgelegde handeling (201), of een weigering (409); met `happened: true` een gebeurd feit dat de proef om de inhoud tegenhield |

Een proces met rollen heeft een sessie per gebruiker (een cookie per proces);
wie als de andere rol inlogt, vervangt de sessie. Een sessie vervalt na acht
uur zonder gebruik, en een proces houdt er hooguit tienduizend: wie daarboven
inlogt, verdringt de langst ongebruikte. Elke route hoort bij een
routegroep (`portal`, `handling`, `counter`); een rol noemt de groepen die
ze mag, en een andere rol krijgt 403. Een cel kent geen login, maar haar
leesroutes (kroniek, zaken, lexostatus) zijn niet open: een gram draagt de
identiteit en de intake van wie indiende. Ze vragen het runtime-token, of het
leestoken van `CELL_READ_TOKEN` (header `x-cell-read-token`) dat runtimes delen
die elkaars cellen mogen lezen; alleen de stroomdefinities zijn open. Een
behandelaar ziet de cellen van zijn proces via `.../api/inspection`, een aanvrager
alleen zijn eigen indiening. Een aanvrager die een zaak wil volgen, moet die
zaak kennen (een gram met zijn waarde van het `owner`-veld van zijn
kanaal); dat leidt de cel af (`owner` in de `case_state`), en het proces
leest het.

Vastleggen (`POST .../grams`) en op proef reduceren (`POST .../trial`) mag
alleen een proces van de runtime zelf. De runtime maakt bij elke start een
willekeurig runtime-token dat alleen in haar geheugen staat; het interne
transport stuurt het mee in de header `x-cell-runtime-token`, en de cel
antwoordt zonder token 401 en met een ander token 403. Een HTTP-transport
stuurt het alleen mee als het er uitdrukkelijk een kreeg
(`Http::with_runtime_token`; de runtime zelf doet dat nu nergens, want een
proces legt alleen vast in een cel van dezelfde runtime), en de runtime geeft
het nooit aan een transport naar een andere runtime. Dit is geen autorisatie tussen organisaties (RFC-022
par. 2 laat die aan de beveiligingscontext); het voorkomt alleen dat iedereen
die de poort bereikt een gram met een willekeurige actor en intake in een
kroniek zet. Het maakt de processen zelf niet veiliger: wie de poort bereikt,
kan nog steeds via de nep-logins van een proces een aanvraag indienen of een
besluit laten nemen, en dat proces legt dan vast.

De cel weigert een gram (403) als de `actor` van het verzoek niet de
`recording_actor` van de stroom is.

## De vier lagen

Laag 1 is van niemand; een cel heeft de lagen 2 tot en met 4. Het proces staat
erboven: het leest lexostatussen en vraagt de cel vast te leggen.

1. **Lexogram.** De regelingen onder `REGULATION_PATH`, ongewijzigd. Een
   artikel declareert welke parameters het nodig heeft.
2. **Stroomdefinitie** (`stream`, schema `stream.json`). Welke feiten de cel
   vastlegt, door wie, in welke kroniek en op welke `legal_basis` (een lijst; een
   artikelnummer mag een spatie hebben; `<regeling>#<artikel> lid <n>` noemt
   een lid, dat bij het opstarten in de artikeltekst moet staan). Een veld bindt aan `$intake.*`, aan
   `$external.*` of is een constante. Een tabelveld declareert zijn kolommen.
   Een indiening van `subtype: aanvraag` heeft `fields.core` (Awb 4:2 lid 1) en
   `fields.content`. `not_reduced` noemt met reden de velden die geen
   afleiding of filter leest. Een event met een zaak mag een RFC-008-stage
   dragen: een besluit `BESLUIT`, een aanvraag `AANVRAAG`, een bekendmaking
   `BEKENDMAKING`; alleen op een decretogram, een indiening of een handeling.
   Een event kan naar andere grammen verwijzen: `refers_to` noemt per naam
   uit de wettekst (`on_application`, `application`, `decision`, `amends`,
   `concerns`) wat het gram waarnaar verwezen wordt moet zijn (`to`: een
   artikel dat het vestigt, een eventnaam of `{stage: <STAGE>}`) en of het
   proces de verwijzing moet meegeven (`required`). Een event dat het
   vestigende artikel noemt (`establishes`), haalt verwijzingen, type,
   stage, grondslag en velden uit dat artikel (`produces.extensions.chronolex`).
   Wat een event voor een zaak is, volgt daaruit en staat niet in de stroom:
   een event waarnaar andere events kunnen verwijzen en dat zelf nergens naar
   verwijst, opent een zaak (de aanvraag); een event met een verwijzing volgt
   er een. Een event met stage `BESLUIT` is een besluit, met een
   `amends`-verwijzing een besluit dat een ander wijzigt; een event waarvan
   een verwijzing alleen naar een besluit kan wijzen, volgt dat besluit, zoals
   de bekendmaking of een betaling.
   `effective_at: {source, legal_basis}` bindt het
   moment waarop het feit rechtens geldt aan een ingediende waarde (zie
   "Tijd").
3. **Reductie tot lexostatus** (`reduction`, schema `lexostatus.json`). Een
   definitie beperkt de kroniek met `filter` en kiest met `pick: latest` zo
   nodig een gram. Per parameter een afleiding:
   - op het gekozen gram: `field`, `year_of` (het jaartal van een datum),
     `period_of` (de periode waarin een datum valt, als haar eerste dag:
     `period: year | quarter | month`, of zonder `period` de
     `temporal.period_type` van de parameter uit de regeling), `filled`, `equals`, `table` met `each_row` of `one_row` (en
     `only_where`), `moment` (`effective_at` of `recorded_at`);
   - over de grammen die door een eigen `filter` komen: `exists: true`
     (optioneel met `filled: <veld>`: alleen een gram met dat veld gevuld
     telt), `collect` (een lijst met een regel per gram),
     `sum: <veld>`, `pick: latest` met `field: <pad>`, `year_of: <pad>`,
     `period_of: <pad>` of `moment` (en optioneel `no_gram: <waarde>`, de lezing van afwezigheid)
     of met `contains: {field, value}`.

   Met `group_by: root` is de lexostatus een **lijst**: een regel per
   zaak waarvan ten minste een gram door `filter` komt, en met `without:
   {filter}` geen gram door dat filter. `pick` en de afleidingen werken per
   zaak; de afleidingen zijn kolommen, geen parameters. Een lijst gaat nooit
   naar de engine. Zonder `pick` staat een zaak op de volgorde van haar eerste
   gram; zo kan een lijst zonder `filter` elke zaak tonen met hoe ver zij is
   (`exists` op een stage, `sum` van de betalingen).

   Een filtersleutel is een veld van het gram zelf (`id`, `root`, `name`,
   `type`, `subtype`, `stage`, `recording_actor`, `chronicle`,
   `legal_character`, `decision_type`, `regulation`, `competent_authority`),
   een verwijzing `refers_to.<naam>`, of een veldpad onder `fields`; `$x`
   komt uit de inputs. Met `refers_to.decision` filtert een lexostatus per
   besluit in een zaak. Geen gram is "nee" bij `exists` en
   `contains`, en nul bij `sum`: de cel spreekt alleen over haar eigen kroniek.
   Een waarde die er niet is (`field` op een leeg veld, `pick` zonder gram)
   blijft weg, tenzij de definitie met `no_gram` zegt hoe zij het ontbreken
   van een gram leest (bijvoorbeeld null: niet gebeurd); de cel vult nooit aan. `extra_fields` levert waarden die geen
   parameter zijn, zoals de invoer van een synthese-bron; ze gaan nooit naar de
   engine. Een afleiding kan haar `legal_basis` dragen (een lijst
   `<regeling>#<artikel>`, optioneel met ` lid <n>`): het artikel dat het feit
   vraagt of de lezing draagt, zoals het register dat de cel bijhoudt (geen
   gram is nee). De cel spreekt de taal van haar eigen regeling; vraagt een
   afnemer het feit onder een eigen naam, dan vertaalt de synthese van zijn
   proces (`parameters` als tabel). `levert_aan` bestaat niet meer.
4. **Het gram** (`chronicle`, schema `gram.json`). Een JSON-regel per gram in
   `DATA_DIR/<cel>/<chronicle>.jsonl`, alleen toevoegen. Een niet-ingevuld veld
   staat erin als `null`. Wat niet in de vorm van de stroom past, weigert de
   cel met 400 en het veldpad. Een gram uit de startstand draagt
   `provenance: initial_state`. Een gram van een handeling die een proces uitrekende
   draagt `inputs` (elke parameter met haar waarde en herkomst), `receipt` en
   `acting_actor`; een besluit (een decretogram) daarnaast
   `legal_character`, `decision_type`, `regulation`, `regulation_valid_from`
   en zo nodig `competent_authority`. Elk gram heeft twee
   tijden, `effective_at` en `recorded_at` (zie "Tijd"); een gram waarvan een
   van beide geen moment met tijdzone is, valideert niet. Elk gram heeft een
   `id` (een uuid v7, dat de cel onder het schrijfslot geeft) en, als zijn
   event verwijst, `refers_to` met het id per verwijzing. De wortel (het gram
   zonder verwijzing waar de verwijzingen op uitkomen, zoals de aanvraag)
   staat niet in het gram: de kroniek houdt een index van id naar wortel. Een
   kroniek van voor chronolex v0.2.0 (met `zaakkenmerk` en `besluitkenmerk`,
   zonder `id`) wordt niet omgezet: de runtime weigert haar te laden en vraagt
   om een lege `DATA_DIR`.

   Het bestand is de bron; de runtime houdt de grammen daarnaast in het
   geheugen, met een index per id en per wortel, en maakt de YAML van een gram een keer.
   Een reductie of een zaakvraag leest het bestand dus niet opnieuw, en wie
   de runtime draait schrijft niet zelf in het bestand. Een regel telt pas
   als ze met een regeleinde eindigt: een onvolledige laatste regel (de
   runtime stopte tijdens het schrijven) wordt bij het openen afgekapt en
   gemeld, een onleesbare regel daarvoor houdt de runtime tegen (en dan
   wordt er niets afgekapt). Elke schrijfactie begint op de lengte die de
   cel kent, dus de rest van een eerder mislukte schrijfactie wordt
   overschreven. Lezers wachten niet op de schijf: alleen het schrijven
   wacht op fsync.

## Tijd

Een gram heeft twee tijden (paper, "Het chronolexogram": "Op 3 april heeft de
gemeente-ambtenaar vastgesteld dat ... per 2 april"):

- `effective_at`: wanneer het feit rechtens geldt of plaatsvond. Standaard het
  moment van vastleggen. Een event kan het binden aan een ingediende waarde,
  altijd met grondslag: `effective_at: {source: $intake.<pad> | $external.<pad>,
  legal_basis: [...]}`; het gram draagt de grondslag als `effective_at_legal_basis`.
  Zo zegt de stroom per event wie het rechtsmoment opgeeft. Aan `$external`:
  de handelende actor, als deel van de handeling (de besluitdatum, de dag van
  bekendmaking volgens Awb 3:41, de dag van betaling of van een mededeling);
  de behandelaar kan zo later vastleggen dan het gebeurde. Aan `$intake`: het
  ontvangstkanaal, als de indiener het moment niet zelf mag kiezen; zo krijgt
  een aanvraag die langs een andere weg binnenkwam de dag van ontvangst die
  het loket opgeeft (Awb 4:1, 4:13), en het portaal levert die niet. De
  waarde is een datum (het begin van die dag) of een moment met tijdzone,
  binnen twee grenzen die de cel afdwingt: niet later dan het vastleggen, en
  bij een gram dat verwijst niet op een dag voor het `effective_at` van een
  gram waarnaar het verwijst (409). De proef van een handeling past dezelfde
  regel toe voordat zij handelt: een datum voor een gram waarnaar de handeling
  verwijst (de wortel van de zaak, of het besluit waarop zij werkt) houdt zij
  tegen, en een later feit in de zaak waarnaar de handeling niet verwijst,
  bindt haar niet. Zij vergelijkt per dag, omdat een gebonden datum het begin
  van die dag is. Beide grenzen noemt de proef voordat er iets wordt vastgelegd.
- `recorded_at`: wanneer de cel het vastlegde, altijd haar eigen klok,
  gezet onder het schrijfslot en nooit voor de regel ervoor: de volgorde in
  het bestand is die van `recorded_at`. Bij een startstand de laadtijd.

`pick: latest` kiest het laatste `effective_at`, bij gelijk moment het laatste
`recorded_at`, en daarna het laatst toegevoegde.

Een reductie kan op een eerder moment peilen ("tijdreizen", paper P:94), met
de query-parameters `as_of` en `known_at` (een datum, dan telt de hele
dag, of een moment met tijdzone):

- `as_of`: de stand zoals die rechtens gold op T, met wat nu bekend is
  (grammen met `effective_at` op of voor T);
- `known_at`: de stand zoals de cel die kende op T (grammen met
  `recorded_at` op of voor T);
- samen: bitemporeel. Zonder peil telt elk gram, ook een feit dat pas later
  ingaat.

Het proces geeft een peil mee: een handeling leest de wet en elke cel op
haar peildatum, de dag van het `effective_at` dat haar event aan een veld van
het formulier bindt (de besluitdatum, de dag van bekendmaking of van
betaling), en anders op vandaag; de toets op vandaag, en het aanbod voor een
tijdvak dat nog moet beginnen op de eerste dag daarvan. Welke dag dat is, zegt
het beleid: `offer.start` noemt een uitkomst van de regeling van het aanbod,
uitgerekend met alleen het gekozen tijdvak; zonder `start` peilt het aanbod op
vandaag. De synthese per regel geeft hetzelfde peil aan elke bron. Geen lexostatus mag
een input `as_of` of `known_at` hebben (het schema weert ze).

## Startstand

`initial_state.jsonl` heeft per regel `stream`, `name`, `effective_at` (met de
hand gezet: wanneer het besluit of de vaststelling rechtens geldt),
`provenance: initial_state`, `fields` en optioneel `id` en `refers_to` (naar
een eerder gram van de startstand, met de namen van het event; zonder `id`
krijgt het gram een vast id dat uit de regel volgt). Geen
`recorded_at`: dat is de laadtijd, het moment waarop de runtime de
startstand in de lege kroniek zet. Een regel met een `effective_at` na de
laadtijd houdt de start tegen, zoals bij elk gram: wat nog moet gebeuren, is
geen feit. De rest volgt uit de stroom. De velden moeten precies die van het event zijn. De runtime zet de
startstand in de kroniek als elke kroniek van de cel leeg is, en daarna nooit
meer. Elk bestand wordt in een keer geschreven (een tijdelijk bestand, dan
hernoemd), zodat een onderbroken start geen half bestand achterlaat; een
achtergebleven tijdelijk bestand ruimt de volgende start op. Beslaat de
startstand meer dan een kroniek, dan geldt dat per bestand: een start die
tussen twee hernoemingen stopt, laat de andere kronieken leeg, en die vult de
runtime daarna niet meer aan. Zo'n gram is geplaatst, niet berekend: er is geen engine-trace bij.

## Synthese en transport

De toets vraagt eerst de cel het concept op proef te reduceren tot de
toets-lexostatus (`POST /cells/<cel>/api/lexostatus/<naam>/trial`). Daarna
vraagt ze elke andere synthese-bron om haar lexostatus, met de invoer uit die
lexostatus, via `/cells/<cel>/api/lexostatus/<naam>`. Het transport (`transport`) is intern
als de bron-cel in dezelfde runtime draait (dezelfde aanroep door de router,
zonder netwerk) en HTTP als de bron een `url` heeft. Een bron krijgt drie
seconden. Van het antwoord neemt de toets alleen de parameters uit
`parameters`. Het antwoord van de toets noemt per parameter de herkomst (de
eigen lexostatus, of cel en lexostatus met het transport) en per bron de
status (`queried`, `unreachable`, `error`, `not_queried`). De herkomst
`own` betekent: een lexostatus van de zaak, uit de cel waarin het proces
vastlegt. Niets uit de
synthese wordt vastgelegd. Is een bron onbereikbaar en de uitkomst daardoor
niet te beoordelen, dan zegt de toets "niet te beoordelen: bron <id>
onbereikbaar", en vult ze niets aan.

## Synthese per regel

Een artikel kan een tabel als parameter vragen waarvan de indiener maar een
deel invult; de rest stelt de instantie zelf vast, per regel, uit registers
van andere cellen. `rows` zegt welk tabelveld (van een lexostatus van de
zaak of van een bron die het doorgeeft) de regels levert, welke kolom onder welke naam meegaat, en welke bron per regel
met welke invoer wordt bevraagd. Per regel gaat de cel langs de bronnen, in
volgorde, zodat een bron een kolom kan gebruiken die een eerdere leverde. De
invoer komt uit de regel (`column`), uit een lexostatus van de zaak (`lexostatus`
en `field`), uit de samengevoegde parameters (`parameter`), uit de wet
(`regulation` en `output`: een uitkomst die het proces een keer vóór de regels
uitrekent met de samengevoegde parameters, zoals een peildatum; de uitslag
noemt haar onder `from_law`) of is een vaste waarde (`value`). De
configuratie zet niets om.
Een bron levert een kolom uit haar `parameters` of haar `extra_fields`. Het
antwoord van een bron (per regel of in de synthese) moet een lexostatus zijn,
met ten minste `name` en `parameters`; iets anders is een fout van de bron,
geen lege lexostatus.
Ontbreekt een invoer, is een bron onbereikbaar, of levert ze de waarde niet,
dan blijft die kolom weg; `missing` noemt welke. Er wordt niets aangevuld. Is
het tabelveld geen lijst van objecten, dan komt er geen tabel (met `error` in
de uitslag), in plaats van een regel die stil wegvalt. De regels worden
tegelijk bevraagd (hooguit zestien tegelijk), elk met haar bronnen na elkaar,
en de tabel houdt de volgorde van het tabelveld.

De toets kent hetzelfde blok onder `assessment_rows` in `synthesis.yaml`. Daar komt de tabel
uit de proefreductie van het concept (de toets-lexostatus) of uit een bron die
haar doorgeeft; de toets bouwt de rijen op vóór de engine, zoals het besluit.
Een rijen-blok levert alleen aan de uitvoering waar het staat: de rijen van
de toets gelden in de controle op herkomst voor de toets, die van het besluit
voor het besluit.

## De stand van een zaak

Elke cel met een event dat een zaak opent of volgt, biedt een lexostatus die
geen `lexostatuses.yaml` noemt: `case_state`, met input `root`, het id van het
wortelgram. De zaak is de groep rond die wortel: de aanvraag en elk gram dat
er, direct of via een ander gram, naar verwijst. De cel neemt de grammen
onder de wortel en leidt af, als extra velden die nooit naar de
engine gaan: `grams` (het aantal; het proces stuurt het terug als
`root_grams`), `events` (per `<stroom>/<event>` het aantal),
`latest_effective_at`, `stages` (voor de stages die bij geen besluit horen,
zoals de aanvraag: per stage het gram dat haar tot stand bracht: event,
tijden, regeling, velden en de waarden van de invoer), `decisions` (per
besluit in de zaak zijn `id` (dat van het besluitgram), het event dat het
vastlegde, het besluit dat het wijzigt (`amends`), zijn stages en per event het aantal grammen dat het
volgt) en, met de inputs `owner_path` en `owner`, `owner`. Het proces
leest de zaak alleen zo: welke handelingen kunnen en op welk besluit, het
besluit waarop een vervolg verdergaat, de rechtsbescherming per besluit en
of een aanvrager de zaak kent. `latest_effective_at` is ter informatie: de
ondergrens van een nieuw `effective_at` volgt uit de grammen waarnaar het
verwijst, niet uit de zaak als geheel. De grammen die het een behandelaar toont, zijn het dossier en
geen invoer. De runtime biedt haar aan, niet de configuratie: de wortel, de
verwijzingen, de besluiten en een stage per besluit zijn begrippen van de
runtime, niet van een corpus. Een cel mag de naam daarom niet zelf gebruiken.

De cel toetst de verwijzingen van een gram onder het schrijfslot. Elk gram
waarnaar verwezen wordt, staat in de kronieken van de cel en past bij wat de
verwijzing mag aanwijzen, en ze hebben allemaal dezelfde wortel; anders 400.
Elk besluit doorloopt elke stage een keer: een tweede gram met dezelfde stage
dat naar hetzelfde gram verwijst, weigert de cel (409). Een tweede besluit van
hetzelfde event dat naar hetzelfde gram verwijst ook (409): een ander besluit
over dezelfde aanvraag vraagt een eigen grondslag, een event met een
`amends`-verwijzing (zoals Awb 4:49).

## Handelingen in een zaak

De afleiding (RFC-047) geeft per handeling een artikel (een regeling en
uitkomsten) en het event waarin de cel haar vastlegt. Een handeling is een
event van de cel met een intake die een kanaal van `kind: handling` noemt; ze
heet naar dat event. Het artikel van een besluit is het artikel dat het event
vestigt, dat van een vervolg het artikel van zijn besluit, en dat van een
feit het beleidsartikel dat een artikel uit de grondslag van het event
uitvoert met een parameter met origin-rol `BESLUIT`, of anders het artikel
waarvan de uitkomsten afhangen van wat de lexostatussen uit het event lezen.
Wat een handeling nodig heeft en van wie, volgt uit de stage van het event
(RFC-008) en uit de origin van de parameters (RFC-048). Er is een route voor
elke handeling, `cases/<root>/actions/<naam>` en `.../trial`; er zijn geen
routes per soort besluit. Het event zegt welke soort een handeling is:

- **Besluit**: het event heeft een stage die de procedure van het
  rechtskarakter van het artikel kent, en het is de eerste zo'n handeling op
  dat artikel. Een zaak kan meer besluiten hebben, elk van een eigen artikel
  (een voorschot, een vaststelling, een terugvordering); het event heeft stage
  `BESLUIT`. Een besluit dat een ander wijzigt, legt vast in een event met een
  `amends`-verwijzing naar het artikel van het gewijzigde besluit. Laat de wet een uitkomst van een wijziging leeg
  (null), dan neemt het proces haar niet. Het formulier zijn de parameters met origin `OORDEEL` (met het
  label na "Naam:" in hun omschrijving; herkomst `handler`). Wat een
  latere stage pas vraagt (zoals de bekendmaking in stage `BEKENDMAKING`), is
  bij het besluit nog niet gebeurd: een boolean is false, al het andere null,
  herkomst `state_at_decision`; wat een lexostatus van de zaak al afleidt
  (geen gram: null) staat daar niet bij. Het event legt de uitkomsten vast, en
  verder alleen wat een oordeel meegeeft (zoals de besluitdatum als
  `effective_at`).
- **Vervolg**: een latere stage van hetzelfde artikel, zoals de bekendmaking,
  op het laatste besluit dat de handeling van dat artikel in de zaak
  vastlegde (het event verwijst met `decision` naar het besluit), tenzij de
  behandelaar er een noemt (`decision` naast het formulier, het id van het
  besluitgram; zo ook bij een feit of wijziging). De engine voert die stage uit (`execute_stage`) op de invoer en de
  uitkomsten van het vastgelegde besluit: het gram van het besluit is de
  toestand van RFC-008. Het formulier is wat de stage vraagt (`requires`,
  met het label van de parameter van het besluit). De haken die de wet op die
  stage laat vuren (RFC-007, zoals Awb 6:8 op `BEKENDMAKING`) rekenen mee; hun
  uitkomsten komen bij de uitkomsten van de handeling, en het event legt ze
  vast. Geeft een haak geen waarde, dan neemt het proces het vervolg niet
  uit zichzelf; gemeld als gebeurd legt de cel het vast, met een lege
  termijn.
- **Feit**: het event heeft geen stage, zoals een verzoek om aanvulling, een
  ontvangst of een betaling. Volgt het event een besluit (een betaling die
  het uitvoert), dan wijst zijn `decision`-verwijzing het artikel van dat
  besluit aan, en wacht het feit tot dat besluit er ligt. Het formulier zijn de `$external`-velden van het
  event die geen uitkomst zijn, met het type van de parameter die een
  lexostatus uit dat veld afleidt. Op proef laat de cel de lexostatussen van
  de zaak reduceren alsof het feit al vastlag (`POST .../trial` met het
  concept): de uitkomsten laten zien wat het feit doet. Een handeling is
  pas te nemen als elk feit is ingevuld.

Een parameter komt uit precies een bron: een lexostatus van de zaak, een
andere synthese-bron, de synthese per regel, het formulier of de stand van
wat nog niet gebeurd is. Een handeling vraagt alleen de synthese-bronnen die
een parameter van haar artikel leveren (en de bronnen die hun een invoer
doorgeven); een betaling vraagt de registers van de aanvraag niet.

Legt het event een executogram vast (de uitvoering van een besluit), en staat
het artikel in zijn grondslag als `TOETS`, dan telt elke booleaanse uitkomst
waarvan de `legal_basis` die bepaling is (het artikel, en het lid als de
grondslag er een noemt): onwaar is een conclusie van het proces, en dan
betaalt het niet uit zichzelf. Zo zegt Awb 4:52 lid 1 dat een betaling die
samen met de eerdere boven het vastgestelde bedrag komt, niet overeenkomstig
de vaststelling is; de configuratie noemt die toets niet.

Het proces concludeert voor het handelt, en weigert niet wat gebeurd is. Zegt
de proef om de inhoud nee (een toets, of een haak zonder waarde), dan staat in
het antwoord `reportable`. Een uitkomst die de wet niet volledig kan uitrekenen
(een waarde of een bron mist) raakt de vorm: die ligt ook gemeld niet vast.
Meldt de behandelaar dat het feit toch gebeurde (`{form, happened:
true}`), dan legt de cel het vast, gaat de reden mee als waarschuwing, en
tonen de lexostatussen de gevolgen: een betaling boven het bedrag telt mee in
wat betaald is, en de wet zegt wat onverschuldigd is betaald. Wat de vorm
raakt (een leeg formulier, een vervolg zonder besluit, een moment na vandaag
of voor een gram waarnaar het verwijst) houdt ook een melding tegen. Een besluit wordt niet gemeld
(400): dat neemt het proces zelf.

De peildatum van een handeling is de dag van het `effective_at` dat haar event
aan een veld van het formulier bindt, met de grondslag uit de stroom (de
besluitdatum, de dag van bekendmaking, de dag van betaling); anders vandaag.
De engine leest de regeling op die dag, en elke cel peilt erop.

Het antwoord op een proef noemt de soort, de peildatum en waar die vandaan
komt, de uitkomsten (ook als de handeling niet te nemen is), de toetsen, per
parameter de herkomst, `not_delivered` en de reden als de handeling niet te
nemen is. Niets wordt vastgelegd.

## Een handeling vastleggen

`POST /processes/<id>/api/cases/<root>/actions/<naam>` rekent
hetzelfde uit en laat de cel het gram vastleggen, met als velden per
`$external`-sleutel van het event een uitkomst of een waarde uit het
formulier. Het gram draagt `inputs` met per parameter haar waarde en haar
herkomst (RFC-013 `accepted_values`), een `receipt` met de geladen regelingen
en de stromen van de cel, met een SHA-256 over beide, en `acting_actor`:
rol, kanaal, identiteit, de grondslag van de rol, en bij een besluit of
vervolg `on_behalf_of` en bij mandaat `mandate`. Een decretogram draagt daarnaast
wat het besluit tot besluit maakt: `legal_character` en `decision_type` uit
`produces`, `regulation`, `regulation_valid_from` en `competent_authority`.

Bij een besluit en een vervolg komt het bevoegd gezag uit de regeling (het
artikel, anders de regeling zelf) en wordt het letterlijk getoetst tegen het
gezag waarvoor het proces handelt: gelijk betekent vastleggen, een gezag uit `mandates`
vastleggen in mandaat, een ander gezag weigeren; noemt de regeling er geen,
dan legt de cel vast met een waarschuwing en zonder `competent_authority`. Zo
blijven de drie assen van RFC-022 par. 2 gescheiden: `recording_actor`,
`competent_authority` en de handelende actor.

Deze leiden tot een weigering met 409 en zonder gram: de proef is niet te
nemen en het feit is niet als gebeurd gemeld (of kan dat niet, om de vorm),
de cel weigert omdat er al een gram met die stage naar hetzelfde gram verwijst,
omdat een besluit van hetzelfde event al naar hetzelfde gram verwijst (een
wijziging vraagt een event met een `amends`-verwijzing), omdat het
`effective_at` op een dag ligt voor een gram waarnaar het verwijst, of omdat de
zaak veranderde sinds het proces haar las, of de wet wijst een ander gezag
aan zonder mandaat. Het proces geeft de cel mee hoeveel grammen de zaak had
(`root_grams`); de cel legt alleen vast als dat onder haar slot nog zo is.
Wat het proces uitrekende (zoals wat er nog te betalen is), gold voor de zaak
zoals die toen was: twee gelijktijdige betalingen komen er niet allebei door,
net zo min als twee besluiten. Het gram wordt voor het vastleggen tegen
`gram.json` gevalideerd.

## Rechtsbescherming

`GET .../cases/<root>` noemt de procedure van het besluit, met per
stage of er een gram van ligt, en de rechtsbescherming die daaruit volgt
(RFC-022 par. 3.3): na de laatste stage die in de zaak ligt de volgende, als
geen handeling haar vastlegt (zoals `BEZWAAR`, die na de bekendmaking vanzelf
loopt), met de uitkomsten van de haken die de wet op de laatste stage liet
vuren, zoals ze in het gram staan (het einde van de bezwaartermijn, Awb 6:7
en 6:8). Geen regel en geen configuratie declareert de route: zij volgt uit
de procedure en de haken in de wet.

## Controles bij het opstarten

Faalt er een, dan start de runtime niet. Een melding over een cel begint met
`cell '<id>':`, een over een proces met `proces '<id>':`.

Per cel:

1. Celdefinitie, stromen en lexostatus-definities valideren tegen hun schema;
   de lexostatussen horen bij deze cel en elke stroom heeft haar
   `recording_actor`.
2. Een afleiding wijst naar iets wat bestaat: een parameter van een artikel uit
   de grondslag van een event dat haar filter aanwijst (of uit haar eigen
   `legal_basis`), en veldpaden van dat event. Elk artikel uit de grondslag van
   een afleiding is geladen en heeft het lid dat ze noemt. Een afleiding op het
   gekozen gram vraagt `pick`.
3. Geen weesveld: elk veld wordt door een afleiding of filter gelezen, of
   staat in `not_reduced`.
4. Een parameter krijgt maar een afleiding.
5. Elke verwijzing van een event kan naar een event van de cel wijzen: haar
   `to` noemt een artikel dat een event vestigt, een bestaand event of een
   stage die een event heeft.
6. De startstand past in de stromen van de cel.
7. De `without` van een lijst (`group_by: root`) wijst een event aan; elk
   gram heeft een wortel, dus `filter` en `without` mogen elk event aanwijzen. Haar kolommen hoeven geen
   parameter te zijn en botsen niet met die van andere lexostatussen.
8. Een lexostatus levert iets: ten minste een afleiding of een extra veld.

Per proces:

1. De afleiding uit het beleid (RFC-047) vindt precies één portaalkanaal,
   één portaal-event, één toets-lexostatus en per handeling een artikel; elk
   kanaal van het beleid heeft een adapter in `channels.yaml` en andersom.
   Wat een beleidsartikel `executes`, is geldig en blijft binnen de
   bevoegdheid van zijn eigen gezag (Awb 4:81). Het portaal, de handelingen
   en de bronnen van de zaak noemen een cel, dezelfde, en die draait in deze
   runtime.
2. De `actor` is de `recording_actor` van elke stroom waarin het proces
   vastlegt (die van het portaal en die van elke handeling). Het gezag van het
   beleid is een gezag dat een geladen regeling noemt; een
   mandaat noemt zo'n gezag, niet het eigen, en een grondslag die een geladen
   artikel aanwijst. Elk kanaal heeft unieke velden, leesbare patronen en een
   eigenaar die een veld is. Het eerste deel van het intake-prefix van
   een kanaal is niet `channel` of `supplied`: die namen onder `$intake` houdt de runtime
   voor de route van binnenkomst en voor wat het ontvangende kanaal levert.
   De grondslag van een kanaal of veld wijst geladen
   artikelen aan; elke rol noemt een bestaand kanaal. Routes
   `portal` en `handling` passen bij de blokken; routes `counter` vragen een
   portaal-event dat `effective_at` aan `$intake` bindt, met een grondslag
   voor een opgegeven moment (`effective_at.legal_basis`). Zonder die
   grondslag zegt de wet alleen waarom het moment van vastleggen telt, en kan
   de balie de dag van ontvangst niet opgeven.
3. Het portaal wijst naar een bestaand event van die cel, dat alleen
   `$intake`-paden leest die de kanalen van het portaal leveren, een lexostatus die dat event
   leest en een gram kiest (geen lijst, geen andere input dan `root`), en een
   uitkomst van een artikel uit de grondslag van het event. Het aanbod noemt een
   bestaande uitkomst en een termijn uit hetzelfde artikel, en leunt alleen op
   wat vooraf vaststaat: elke parameter van zijn artikel heeft origin `KANAAL`
   of `REGISTER`, of `BELANGHEBBENDE` met `rol: TIJDVAK` (het tijdvak; de
   grondslag Awb 4:2 lid 1 alleen maakt een parameter geen tijdvak, met `offer.windows`: een uitkomst van dezelfde regeling uit een
   artikel zonder verplichte parameters). Een `legal_basis` in het
   formulierbestand (bij een veld of kolom) wijst een geladen artikel aan, met
   een lid dat het heeft.
4. Synthese: alleen met een portaal of handelingen; elke invoer komt uit een
   veld van de toets-lexostatus of een lexostatus van de zaak, of van een
   eerdere bron die het doorgeeft (in rondes, zo diep als nodig), of is een vaste
   waarde; elke parameter (de naam bij de afnemer) is een parameter van het
   artikel van de toets, het besluit of het aanbod, of van een artikel dat een
   van die transitief aanroept (via `source`); een parameter komt uit maar een
   bron; een gewone bron is een andere cel dan die van het proces. De
   `legal_basis` van een bron (ook per regel) wijst geladen artikelen aan, en
   elke bron die vertaalt draagt (een andere naam bij de
   afnemer, of een vaste waarde in de invoer) er een: de vertaling is een
   lezing van de wet.
5. Behandeling: de werkvoorraad is een lijst; een bron van de zaak vraagt een
   behandeling; de lexostatussen van de zaak hebben als enige input
   `root`. Per handeling: een unieke naam; een rol die ze noemt, mag
   de behandeling; de uitkomsten komen uit een artikel (bij een vervolg ook
   uit de haken van zijn stage); elke parameter uit het formulier, de stand
   van wat nog niet gebeurd is of een rijen-definitie moet de aanroeper van
   het artikel leveren; een parameter komt uit maar een bron. Het
   vastleg-event bestaat en verwijst naar een gram van de zaak; een
   stage erop staat in de procedure van het artikel. Een besluit legt elke
   uitkomst vast en verder alleen oordelen; een vervolg legt vast wat de
   stage vraagt en wat de haken uitrekenen, en niets anders.
6. Synthese per regel: de tabel komt uit een lexostatus van de zaak of een
   bron die haar levert; elke kolomnaam komt uit maar een plek (de tabel of een
   bron); een invoer `column` wijst een kolom aan die ervoor gevuld wordt; een
   bron is een andere cel.
7. Als synthese en handelingen kloppen: elke parameter die de aanroeper van
   de toets, het aanbod of een uitkomst van een handeling (behalve een
   vervolg, dat op het vastgelegde besluit rekent) moet leveren, heeft een
   leverancier die bij zijn geldende origin past, en geen die er niet bij past
   (RFC-048; zie `origin`). Een verkeerde bron is altijd een fout, ook bij
   `required: false`. Of een afleiding van de eigen cel van de belanghebbende
   of uit het dossier komt, volgt uit wat haar filters doorlaten: grammen van
   type `submission` (wat de aanvrager aanlevert) of andere grammen van de
   actor (het verloop van de zaak). Zonder leverancier start de runtime niet,
   behalve bij `required: false`: dan krijgt de engine hem niet en rekent ze
   met een onbekende waarde, en is het een waarschuwing. Een parameter zonder
   origin is een fout (altijd strict, RFC-047); een `BELANGHEBBENDE`-parameter zonder `required: false` (behalve het
   tijdvak) is een waarschuwing. Een bron met een url, of een interne cel die
   niet draait, telt, met een waarschuwing per bron over wat niet na te gaan
   is. Een `register` dat niet geladen is, is een fout; een grondslag in een
   regeling die niet geladen is, een waarschuwing. `origins` in
   uitvoeringsbeleid van het gezag waarvoor het proces handelt overschrijft de origin uit de wet; twee
   botsende overschrijvingen zijn een fout. Al bij het laden van het corpus
   houdt een origin die niet te lezen is, of een REGISTER zonder `register`,
   de runtime tegen, met bestand, artikel en parameter.
8. De voorbeelden bestaan en hebben de goede vorm, en horen bij een handeling
   die het proces heeft.

Of een bron bereikbaar is en de lexostatus met die parameters en inputs
aanbiedt, controleert de runtime na het starten via `GET /api/cells` bij de
bron. Een probleem daar is een waarschuwing, geen weigering: de bron mag later
komen.

## Modules

| Module | Taak |
|---|---|
| `runtime` | cellen en processen laden en controleren, kronieken openen, router over alles |
| `cell` | een cel uit haar map laden |
| `process` | een afgeleid proces laden, en de controles op cel, actor en portaal |
| `config` | omgeving, `cell.yaml` en de procesdefinitie die de runtime afleidt |
| `policy` | kanalen, `supplies` en mandaten uit het uitvoeringsbeleid, en de controle op `executes` (RFC-047) |
| `deployment` | `channels.yaml`, `synthesis.yaml` en `examples.yaml` |
| `derive` | de procesdefinitie uit beleid, cellen en deployment: portaal, toets, rollen en handelingen |
| `stream` | stroomdefinitie laden en valideren, gram bouwen uit intake en external |
| `gram` | het vastgelegde gram, met invoer en receipt van elke berekende handeling, en het lezen van een veldpad |
| `reduction` | kroniek reduceren tot lexostatus; `reduction::definition` laadt de lexostatus-definities, `reduction::as_of` peilt op een eerder moment |
| `initial_state` | grammen voor een lege kroniek |
| `chronicle` | append-only opslag, in het geheugen met een index per zaak, en herstel van een half geschreven regel |
| `check` | de controles bij het opstarten |
| `synthesis` | bronnen bevragen, samenvoegen met herkomst, en de controles erop |
| `origin` | wie een parameter levert volgens de wet (RFC-048): de controle bij het opstarten, de aanbodregel, het tijdvak en het besluitformulier |
| `transport` | intern en HTTP |
| `channel` | kanalen en rollen: de vorm van een login, de intake, de eigenaar, de controles |
| `authority` | het gezag waarvoor een proces handelt en zijn mandaten, en de toets tegen de wet |
| `session` | sessies per rol |
| `assessment` | parameters aan de engine, een of meer uitkomsten evalueren |
| `action` | de handelingen in een zaak (besluit, vervolg, feit): voorbereiden bij het laden, op proef, vastleggen, de stand per zaak en de rechtsbescherming, en de controles op behandeling |
| `rows` | synthese per regel: een tabelveld wordt een array-parameter |
| `possibility` | wat het aanbod per tijdvak zegt (`offers` van het portaalkanaal) |
| `examples` | de voorbeelden per handeling uit `examples.yaml` |
| `api` | de routes: `api::cell` (de cel), `api::process` (de router van een proces), `api::session`, `api::portal`, `api::counter` en `api::handling` |
| `cell_client` | hoe een proces de cel vraagt: zaak lezen, vastleggen, proefreductie, als typen |
| `date` | momenten lezen, peildatum, jaartal en het `TimePoint` van een peil |
| `load` | bestanden en mappen lezen, YAML valideren tegen zijn schema |
| `regulations`, `form`, `schema` | laden en valideren |

## De engine en een losse uitkomst

De engine voert bij een gevraagde uitkomst het hele artikel uit, ook de
invoer uit andere artikelen en regelingen, en stopt bij de eerste waarde die
ontbreekt. Een toets is dus alleen te beoordelen als alles wat het artikel
aanraakt aanwezig is, ook feiten van de instantie zelf. Het proces vult dan
niets aan en meldt "niet te beoordelen: mist <parameter>". De test
`engine_eist_het_hele_artikel_bij_een_uitkomst` in `src/assessment.rs` legt dit
gedrag vast.
