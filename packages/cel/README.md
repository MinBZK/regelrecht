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
andere: het leest haar lexostatussen en kroniek langs dezelfde routes. Cel en
proces zijn configuratie, geen code: een map met een `cel.yaml`, en een map
met een `proces.yaml`. De code noemt geen casus; de tests draaien op de
generieke fixtures in `tests/fixtures/`. De docs-pagina
`docs/src/content/docs/components/cel.md` beschrijft dezelfde opzet, met de
afwijkingen van RFC-022 en de open vragen.

## Starten

```bash
just cel          # runtime op :7170, frontend op :7171, op de fixtures
```

## Configuratie

| Variabele | Betekenis |
|---|---|
| `CELLS_PATH` | Map met een submap per cel, elk met een `cel.yaml`. |
| `PROCESSES_PATH` | Map met een submap per proces, elk met een `proces.yaml`. Optioneel: zonder draaien alleen de cellen. |
| `REGULATION_PATH` | Map met regelingen, gedeeld door de hele runtime; elk YAML-bestand met `$id` en `articles` wordt geladen. |
| `DATA_DIR` | Map voor de kronieken: per cel een submap `<id>/`. |
| `CEL_PORT` | Poort, standaard 7170. De runtime luistert op `0.0.0.0`. |
| `CEL_LEES_TOKEN` | Optioneel leestoken (minstens 16 tekens) dat runtimes delen die elkaars cellen mogen lezen: een cel neemt het aan in `x-cel-lees-token`, een HTTP-transport stuurt het mee. Zonder leest alleen de runtime zelf haar cellen. |

```yaml
# <CELLS_PATH>/<map>/cel.yaml, schema schema/chronolex/v0.1.0/cel.json
id: <cel-id>                      # routes onder /cellen/<id>/api/
recording_actor: <actor>          # elke stroom van de cel heeft deze actor
stromen: [<pad>, ...]             # stroombestanden of mappen, relatief aan deze map
lexostatussen: <pad>              # lexostatus-definities
startstand: <pad>                 # optioneel: grammen voor een lege kroniek
```

```yaml
# <PROCESSES_PATH>/<map>/proces.yaml, schema schema/chronolex/v0.1.0/proces.json
id: <proces-id>                   # routes onder /processen/<id>/api/
actor: <actor>                    # recording_actor van elke stroom waarin het vastlegt
herkomst: streng                  # optioneel; streng: een parameter zonder origin is een fout (standaard ruim)
namens: {gezag: <naam>}           # of {regeling: <$id>}: het bevoegd gezag waarvoor het proces handelt; verplicht met behandeling
mandaten:                         # optioneel: ook handelen namens een ander gezag (Awb 10:1)
  - {gezag: <naam>, grondslag: <regeling>#<artikel>}
kanalen:                          # nagebootste logins; geen register, geen gecertificeerde login
  <kanaal>:
    label: <tekst>
    uitleg: <tekst>               # optioneel
    velden:
      - {naam: <veld>, label: <tekst>, patroon: <regex>, controle: elfproef, melding: <tekst>, numeriek: true}
    eigenaar: <veld>              # optioneel: wie een zaak volgt, moet haar met deze waarde kennen
    intake: <pad>                 # optioneel: onder $intake.<pad>.<veld>; zonder: de id van het kanaal
rollen:                           # optioneel; zonder rollen geen login
  <rol>: {kanaal: <kanaal>, routes: [portaal, behandeling, loket], label: <tekst>, grondslag: <regeling>#<artikel>}
portaal:                          # optioneel, vraagt een rol met routes portaal
  cel: <cel-id>                   # waar de indiening wordt vastgelegd; in deze runtime
  stroom: <$id van de stroom>
  event: <event dat een indiening wordt>
  toets:
    lexostatus: <naam>
    regeling: <$id>
    uitkomst: <output>
    rijen: [...]                  # optioneel: synthese per regel, als bij het besluit
  aanbod:                         # optioneel
    regeling: <$id>
    uitkomst: <output>
    termijn: <output>             # optioneel
    tijdvakken: <output>          # als het artikel een tijdvak vraagt (origin met rol: TIJDVAK): de tijdvakken uit het beleid
    begin: <output>               # optioneel: de eerste dag van een tijdvak, het peil van het aanbod
    openstelling: <output>        # optioneel: de opening van een tijdvak; het loket weigert een ontvangst daarvoor
  formulier: {pad: <pad>, scherm: <id>}   # optioneel
synthese:                         # optioneel, alleen met een portaal of handelingen
  - {cel: <cel-id>, lexostatus: <naam>, zaak: true}   # een lexostatus van de zaak
  - cel: <id van de bron-cel>
    url: <http://host:poort>      # optioneel; zonder url: intern transport
    lexostatus: <naam bij de bron>
    invoer:
      <input van de bron>: {lexostatus: <lexostatus van de zaak of eerdere bron>, veld: <parameter of extra veld>}
      <input van de bron>: {waarde: <vaste waarde>}
    parameters: [<naam>, ...]     # expliciet, geen wildcard; dezelfde naam bij bron en afnemer
    # of: parameters: {<naam bij de bron>: <parameter van de afnemer>}
    extra_velden: [<naam>, ...]   # optioneel: invoer voor een latere bron
behandeling:                      # optioneel, vraagt een rol met routes behandeling
  werkvoorraad: {cel: <cel-id>, lexostatus: <lijst-lexostatus>}
  handelingen:                    # de handelingen in een zaak (zie "Handelingen in een zaak")
    - naam: <naam>                # uniek; de route is zaken/<z>/handelingen/<naam>
      label: <tekst>              # optioneel
      rol: <rol>                  # optioneel: alleen deze rol (met routes behandeling)
      regeling: <$id>             # optioneel: anders de beschikking van het gezag van namens
      uitkomsten: [<output>, ...] # van een en hetzelfde artikel; bij een vervolg komen de haken erbij
      rijen:                      # optioneel: synthese per regel (zie hieronder)
        - parameter: <array-parameter>
          tabel: {lexostatus: <van de zaak of een bron>, veld: <tabelveld>}
          kolommen: {<kolom van de tabel>: <kolom van de parameter>}
          bronnen:
            - cel: <id>
              url: <http://host:poort>   # optioneel; zonder url: intern
              lexostatus: <naam bij de bron>
              invoer:
                <input>: {kolom: <kolom van de regel>}
                <input>: {lexostatus: <van de zaak of een bron>, veld: <naam>}
                <input>: {parameter: <naam>}
                <input>: {regeling: <$id>, uitkomst: <output>}   # de wet leidt de invoer af
                <input>: {waarde: <vaste waarde>}
              kolommen: {<naam bij de bron>: <kolom van de parameter>}
      vastleggen: {cel: <cel-id>, stroom: <$id>, event: <event met zaak: volgt>}
voorbeelden:                      # optioneel: standaardgegevens per handeling
  inloggen: [<pad>, ...]
  aanvraag: <pad>
  handelingen: {<naam>: <pad>}    # {formulier: {...}}; "$vandaag" wordt de datum van vandaag
```

Paden in `proces.yaml` zijn relatief aan de map van het proces. Het portaal,
de werkvoorraad, het besluit en de bronnen met `zaak: true` noemen dezelfde
cel: in deze stap handelt een proces over de zaken van een cel, en die cel
draait in dezelfde runtime. Een bron met `zaak: true` is een lexostatus van
de zaak zelf, met als enige input `zaakkenmerk`: het besluit vraagt haar met
het zaakkenmerk en neemt al haar parameters en extra velden over. Het
formulierbestand levert alleen labels, soorten en volgorde; het bepaalt nooit
het gedrag.

## Routes

| Route | Doet |
|---|---|
| `GET /api/cellen` | de cellen, met per cel haar kronieken en haar lexostatussen (inputs, parameters, extra velden) |
| `GET /api/processen` | de processen, met per proces de actor, de cel, portaal, rollen, behandeling en de synthese-bronnen |
| `GET /cellen/<id>/api/kroniek` | runtime- of leestoken: de grammen, elk met YAML |
| `GET /cellen/<id>/api/zaken/<zaakkenmerk>` | runtime- of leestoken: de grammen van één zaak, elk met YAML; de cel filtert, 404 als ze de zaak niet kent |
| `GET /cellen/<id>/api/lexostatus/<naam>?<input>=...` | runtime- of leestoken: een reductie; de inputs als query, en optioneel `peilmoment` en `bekend_op` (zie "Tijd"); `zaakstand` biedt de runtime aan (zie "De stand van een zaak") |
| `POST /cellen/<id>/api/lexostatus/<naam>/proef` | alleen met het runtime-token: `{concept, inputs}`: de cel bouwt het gram van het concept in het geheugen en reduceert de kroniek mét dat gram (`inputs` mag een peil dragen); er wordt niets vastgelegd |
| `POST /cellen/<id>/api/grammen` | alleen met het runtime-token: `{actor, stroom, event, intake, external, zaakkenmerk?, besluit?, zaak_grammen?}`: de cel bouwt het gram, valideert het, controleert de actor en de zaak, en legt het vast (201); 409 als die stage al vastligt in de zaak, als het `op_moment` op een dag voor de zaak ligt, of als de zaak niet meer `zaak_grammen` grammen heeft |
| `GET /cellen/<id>/api/stroom` | de stroomdefinities van de cel, met hun hash; open |
| `GET /processen/<id>/api/voorbeelden` | de voorbeelden per handeling, zonder login |
| `POST /processen/<id>/api/kanalen/<kanaal>/login`, `GET .../sessie`, `POST .../logout` | met rollen: de velden van het kanaal (en `rol` als er langs het kanaal meer rollen inloggen) naar een sessie `{rol, kanaal, velden}` |
| `GET /processen/<id>/api/sessie` | met rollen: wie er is ingelogd, langs welk kanaal ook |
| `GET /processen/<id>/api/formulier` | routes `portaal`: de stroom en de formuliervelden |
| `POST /processen/<id>/api/aanvraag/toets` | routes `portaal`: proefreductie in de cel, synthese, engine |
| `POST /processen/<id>/api/aanvraag` | routes `portaal`: de cel legt het gram vast |
| `GET /processen/<id>/api/mogelijkheden` | routes `portaal`: wat het aanbod zegt per tijdvak dat het beleid aanbiedt (`aanbod.tijdvakken`) |
| `POST /processen/<id>/api/loket/aanvraag` | routes `loket`: `{aanvrager, ontvangen_op, external}`; een aanvraag die langs een andere weg binnenkwam, met de dag van ontvangst als `op_moment` (niet na vandaag, niet vóór `aanbod.openstelling`) |
| `GET /processen/<id>/api/werkvoorraad` | routes `behandeling`: de werkvoorraad, een lijst uit de cel |
| `GET /processen/<id>/api/inzage/<cel>/kroniek`, `.../lexostatus/<naam>?...` | routes `behandeling`: inzage in een cel die het proces leest (de eigen cel en de bronnen zonder url); het proces geeft door wat de cel antwoordt |
| `GET /processen/<id>/api/zaken/<zaakkenmerk>` | routes `behandeling`: de grammen van de zaak, de procedure met de stages die er liggen, de rechtsbescherming die daaruit volgt, en per handeling of zij kan, haar formulier en een proef zonder formulier |
| `POST /processen/<id>/api/zaken/<zaakkenmerk>/handelingen/<naam>/proef` | routes `behandeling` (en de rol van de handeling): `{formulier}` naar een handeling op proef; niets wordt vastgelegd |
| `POST /processen/<id>/api/zaken/<zaakkenmerk>/handelingen/<naam>` | idem: `{formulier, gebeurd?}` naar een vastgelegde handeling (201), of een weigering (409); met `gebeurd: true` een gebeurd feit dat de proef om de inhoud tegenhield |

Een proces met rollen heeft een sessie per gebruiker (een cookie per proces);
wie als de andere rol inlogt, vervangt de sessie. Een sessie vervalt na acht
uur zonder gebruik, en een proces houdt er hooguit tienduizend: wie daarboven
inlogt, verdringt de langst ongebruikte. Elke route hoort bij een
routegroep (`portaal`, `behandeling`, `loket`); een rol noemt de groepen die
ze mag, en een andere rol krijgt 403. Een cel kent geen login, maar haar
leesroutes (kroniek, zaken, lexostatus) zijn niet open: een gram draagt de
identiteit en de intake van wie indiende. Ze vragen het runtime-token, of het
leestoken van `CEL_LEES_TOKEN` (header `x-cel-lees-token`) dat runtimes delen
die elkaars cellen mogen lezen; alleen de stroomdefinities zijn open. Een
behandelaar ziet de cellen van zijn proces via `.../api/inzage`, een aanvrager
alleen zijn eigen indiening. Een aanvrager die een zaak wil volgen, moet die
zaak kennen (een gram met zijn waarde van het `eigenaar`-veld van zijn
kanaal); dat leidt de cel af (`eigenaar` in de zaakstand), en het proces
leest het.

Vastleggen (`POST .../grammen`) en op proef reduceren (`POST .../proef`) mag
alleen een proces van de runtime zelf. De runtime maakt bij elke start een
willekeurig runtime-token dat alleen in haar geheugen staat; het interne
transport stuurt het mee in de header `x-cel-runtime-token`, en de cel
antwoordt zonder token 401 en met een ander token 403. Een HTTP-transport
stuurt het alleen mee als het er uitdrukkelijk een kreeg
(`Http::met_runtime_token`; de runtime zelf doet dat nu nergens, want een
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
2. **Stroomdefinitie** (`stroom`, schema `stream.json`). Welke feiten de cel
   vastlegt, door wie, in welke kroniek en op welke `grondslag` (een lijst; een
   artikelnummer mag een spatie hebben; `<regeling>#<artikel> lid <n>` noemt
   een lid, dat bij het opstarten in de artikeltekst moet staan). Een veld bindt aan `$intake.*`, aan
   `$external.*` of is een constante. Een tabelveld declareert zijn kolommen.
   Een indiening van soort `aanvraag` heeft `fields.kern` (Awb 4:2 lid 1) en
   `fields.inhoud`. `niet_gereduceerd` noemt met reden de velden die geen
   afleiding of filter leest. Een event met een zaak mag een RFC-008-stage
   dragen: een besluit `BESLUIT`, een aanvraag `AANVRAAG`, een bekendmaking
   `BEKENDMAKING`; alleen op een decretogram, een indiening of een handeling.
   `op_moment: {bron, grondslag}` bindt het
   moment waarop het feit rechtens geldt aan een ingediende waarde (zie
   "Tijd").
3. **Reductie tot lexostatus** (`reductie`, schema `lexostatus.json`). Een
   definitie beperkt de kroniek met `filter` en kiest met `kies: laatste` zo
   nodig een gram. Per parameter een afleiding:
   - op het gekozen gram: `veld`, `jaar_van` (het jaartal van een datum),
     `gevuld`, `gelijk`, `tabel` met `elke_regel` of `een_regel` (en
     `alleen_waar`), `moment` (`op_moment` of `vastgelegd_op`);
   - over de grammen die door een eigen `filter` komen: `bestaat: true`
     (optioneel met `gevuld: <veld>`: alleen een gram met dat veld gevuld
     telt), `verzamel` (een lijst met een regel per gram),
     `som: <veld>`, `kies: laatste` met `veld: <pad>`, `jaar_van: <pad>` of
     `moment` (en optioneel `geen_gram: <waarde>`, de lezing van afwezigheid)
     of met `bevat: {veld, waarde}`.

   Met `groepeer: zaakkenmerk` is de lexostatus een **lijst**: een regel per
   zaak waarvan ten minste een gram door `filter` komt, en met `zonder:
   {filter}` geen gram door dat filter. `kies` en de afleidingen werken per
   zaak; de afleidingen zijn kolommen, geen parameters. Een lijst gaat nooit
   naar de engine. Zonder `kies` staat een zaak op de volgorde van haar eerste
   gram; zo kan een lijst zonder `filter` elke zaak tonen met hoe ver zij is
   (`bestaat` op een stage, `som` van de betalingen).

   Een filtersleutel is een veld van het gram zelf (`name`, `type`, `soort`,
   `stage`, `zaakkenmerk`, `recording_actor`, `chronicle`) of een veldpad onder
   `fields`; `$x` komt uit de inputs. Geen gram is "nee" bij `bestaat` en
   `bevat`, en nul bij `som`: de cel spreekt alleen over haar eigen kroniek.
   Een waarde die er niet is (`veld` op een leeg veld, `kies` zonder gram)
   blijft weg, tenzij de definitie met `geen_gram` zegt hoe zij het ontbreken
   van een gram leest (bijvoorbeeld null: niet gebeurd); de cel vult nooit aan. `extra_velden` levert waarden die geen
   parameter zijn, zoals de invoer van een synthese-bron; ze gaan nooit naar de
   engine. Een afleiding kan haar `grondslag` dragen (een lijst
   `<regeling>#<artikel>`, optioneel met ` lid <n>`): het artikel dat het feit
   vraagt of de lezing draagt, zoals het register dat de cel bijhoudt (geen
   gram is nee). De cel spreekt de taal van haar eigen regeling; vraagt een
   afnemer het feit onder een eigen naam, dan vertaalt de synthese van zijn
   proces (`parameters` als tabel). `levert_aan` bestaat niet meer.
4. **Het gram** (`kroniek`, schema `gram.json`). Een JSON-regel per gram in
   `DATA_DIR/<cel>/<chronicle>.jsonl`, alleen toevoegen. Een niet-ingevuld veld
   staat erin als `null`. Wat niet in de vorm van de stroom past, weigert de
   cel met 400 en het veldpad. Een gram uit de startstand draagt
   `herkomst: startstand`. Een gram van een handeling die een proces uitrekende
   draagt `inputs` (elke parameter met haar waarde en herkomst), `receipt` en
   `handelende_actor`; een besluit (een decretogram) daarnaast
   `legal_character`, `decision_type`, `regulation`, `regulation_valid_from`
   en zo nodig `competent_authority`. Elk gram heeft twee
   tijden, `op_moment` en `vastgelegd_op` (zie "Tijd"); een gram waarvan een
   van beide geen moment met tijdzone is, valideert niet. Een regel van voor
   `vastgelegd_op` laadt nog: die krijgt zijn `op_moment`, met een
   waarschuwing.

   Het bestand is de bron; de runtime houdt de grammen daarnaast in het
   geheugen, met een index per zaak, en maakt de YAML van een gram een keer.
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

- `op_moment`: wanneer het feit rechtens geldt of plaatsvond. Standaard het
  moment van vastleggen. Een event kan het binden aan een ingediende waarde,
  altijd met grondslag: `op_moment: {bron: $intake.<pad> | $external.<pad>,
  grondslag: [...]}`; het gram draagt de grondslag als `op_moment_grondslag`.
  Zo zegt de stroom per event wie het rechtsmoment opgeeft. Aan `$external`:
  de handelende actor, als deel van de handeling (de besluitdatum, de dag van
  bekendmaking volgens Awb 3:41, de dag van betaling of van een mededeling);
  de behandelaar kan zo later vastleggen dan het gebeurde. Aan `$intake`: het
  ontvangstkanaal, als de indiener het moment niet zelf mag kiezen; zo krijgt
  een aanvraag die langs een andere weg binnenkwam de dag van ontvangst die
  het loket opgeeft (Awb 4:1, 4:13), en het portaal levert die niet. De
  waarde is een datum (het begin van die dag) of een moment met tijdzone,
  binnen twee grenzen die de cel afdwingt: niet later dan het vastleggen, en
  bij een gram dat een zaak volgt niet op een dag voor het laatste
  `op_moment` in die zaak (409). De proef van een handeling noemt beide
  vooraf.
- `vastgelegd_op`: wanneer de cel het vastlegde, altijd haar eigen klok,
  gezet onder het schrijfslot en nooit voor de regel ervoor: de volgorde in
  het bestand is die van `vastgelegd_op`. Bij een startstand de laadtijd.

`kies: laatste` kiest het laatste `op_moment`, bij gelijk moment het laatste
`vastgelegd_op`, en daarna het laatst toegevoegde.

Een reductie kan op een eerder moment peilen ("tijdreizen", paper P:94), met
de query-parameters `peilmoment` en `bekend_op` (een datum, dan telt de hele
dag, of een moment met tijdzone):

- `peilmoment`: de stand zoals die rechtens gold op T, met wat nu bekend is
  (grammen met `op_moment` op of voor T);
- `bekend_op`: de stand zoals de cel die kende op T (grammen met
  `vastgelegd_op` op of voor T);
- samen: bitemporeel. Zonder peil telt elk gram, ook een feit dat pas later
  ingaat.

Het proces geeft een peil mee: een handeling leest de wet en elke cel op
haar peildatum, de dag van het `op_moment` dat haar event aan een veld van
het formulier bindt (de besluitdatum, de dag van bekendmaking of van
betaling), en anders op vandaag; de toets op vandaag, en het aanbod voor een
tijdvak dat nog moet beginnen op de eerste dag daarvan. Welke dag dat is, zegt
het beleid: `aanbod.begin` noemt een uitkomst van de regeling van het aanbod,
uitgerekend met alleen het gekozen tijdvak; zonder `begin` peilt het aanbod op
vandaag. De synthese per regel geeft hetzelfde peil aan elke bron. Geen lexostatus mag
een input `peilmoment` of `bekend_op` hebben (het schema weert ze).

## Startstand

`startstand.jsonl` heeft per regel `stroom`, `name`, `op_moment` (met de
hand gezet: wanneer het besluit of de vaststelling rechtens geldt),
`herkomst: startstand`, `fields` en optioneel `zaakkenmerk`. Geen
`vastgelegd_op`: dat is de laadtijd, het moment waarop de runtime de
startstand in de lege kroniek zet. Een regel met een `op_moment` na de
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
toets-lexostatus (`POST /cellen/<cel>/api/lexostatus/<naam>/proef`). Daarna
vraagt ze elke andere synthese-bron om haar lexostatus, met de invoer uit die
lexostatus, via `/cellen/<cel>/api/lexostatus/<naam>`. Het transport (`transport`) is intern
als de bron-cel in dezelfde runtime draait (dezelfde aanroep door de router,
zonder netwerk) en HTTP als de bron een `url` heeft. Een bron krijgt drie
seconden. Van het antwoord neemt de toets alleen de parameters uit
`parameters`. Het antwoord van de toets noemt per parameter de herkomst (de
eigen lexostatus, of cel en lexostatus met het transport) en per bron de
status (`bevraagd`, `onbereikbaar`, `fout`, `niet_bevraagd`). De herkomst
`eigen` betekent: een lexostatus van de zaak, uit de cel waarin het proces
vastlegt. Niets uit de
synthese wordt vastgelegd. Is een bron onbereikbaar en de uitkomst daardoor
niet te beoordelen, dan zegt de toets "niet te beoordelen: bron <id>
onbereikbaar", en vult ze niets aan.

## Synthese per regel

Een artikel kan een tabel als parameter vragen waarvan de indiener maar een
deel invult; de rest stelt de instantie zelf vast, per regel, uit registers
van andere cellen. `rijen` zegt welk tabelveld (van een lexostatus van de
zaak of van een bron die het doorgeeft) de regels levert, welke kolom onder welke naam meegaat, en welke bron per regel
met welke invoer wordt bevraagd. Per regel gaat de cel langs de bronnen, in
volgorde, zodat een bron een kolom kan gebruiken die een eerdere leverde. De
invoer komt uit de regel (`kolom`), uit een lexostatus van de zaak (`lexostatus`
en `veld`), uit de samengevoegde parameters (`parameter`), uit de wet
(`regeling` en `uitkomst`: een uitkomst die het proces een keer vóór de regels
uitrekent met de samengevoegde parameters, zoals een peildatum; de uitslag
noemt haar onder `uit_de_wet`) of is een vaste waarde (`waarde`). De
configuratie zet niets om.
Een bron levert een kolom uit haar `parameters` of haar `extra_velden`. Het
antwoord van een bron (per regel of in de synthese) moet een lexostatus zijn,
met ten minste `naam` en `parameters`; iets anders is een fout van de bron,
geen lege lexostatus.
Ontbreekt een invoer, is een bron onbereikbaar, of levert ze de waarde niet,
dan blijft die kolom weg; `mist` noemt welke. Er wordt niets aangevuld. Is
het tabelveld geen lijst van objecten, dan komt er geen tabel (met `fout` in
de uitslag), in plaats van een regel die stil wegvalt. De regels worden
tegelijk bevraagd (hooguit zestien tegelijk), elk met haar bronnen na elkaar,
en de tabel houdt de volgorde van het tabelveld.

De toets kent hetzelfde blok onder `portaal.toets.rijen`. Daar komt de tabel
uit de proefreductie van het concept (de toets-lexostatus) of uit een bron die
haar doorgeeft; de toets bouwt de rijen op vóór de engine, zoals het besluit.
Een rijen-blok levert alleen aan de uitvoering waar het staat: de rijen van
de toets gelden in de controle op herkomst voor de toets, die van het besluit
voor het besluit.

## De stand van een zaak

Elke cel met een event dat een zaak opent of volgt, biedt een lexostatus die
geen `lexostatussen.yaml` noemt: `zaakstand`, met input `zaakkenmerk`. De cel
filtert de grammen van de zaak en leidt af, als extra velden die nooit naar de
engine gaan: `grammen` (het aantal; het proces stuurt het terug als
`zaak_grammen`), `events` (per `<stroom>/<event>` het aantal),
`laatste_op_moment`, `stages` (per stage het gram dat haar tot stand bracht:
event, tijden, regeling, velden en de waarden van de invoer) en, met de inputs
`eigenaar_pad` en `eigenaar`, `eigenaar`. Het proces leest de zaak alleen zo:
welke handelingen kunnen, het besluit waarop een vervolg verdergaat, de
rechtsbescherming, de ondergrens van een nieuw `op_moment` en of een aanvrager
de zaak kent. De grammen die het een behandelaar toont, zijn het dossier en
geen invoer. De runtime biedt haar aan, niet de configuratie: de zaak, het
zaakkenmerk en een stage per zaak zijn begrippen van de runtime, niet van een
corpus. Een cel mag de naam daarom niet zelf gebruiken.

## Handelingen in een zaak

`behandeling.handelingen` noemt per handeling een artikel (een regeling en
uitkomsten) en het event waarin de cel haar vastlegt. Wat een handeling nodig
heeft en van wie, staat er niet in: het volgt uit de stage van het event
(RFC-008) en uit de origin van de parameters (RFC-043). Er is een route voor
elke handeling, `zaken/<z>/handelingen/<naam>` en `.../proef`; er zijn geen
routes per soort besluit. Het event zegt welke soort een handeling is:

- **Besluit**: het event heeft een stage die de procedure van het
  rechtskarakter van het artikel kent, en het is de eerste zo'n handeling op
  dat artikel. Het formulier zijn de parameters met origin `OORDEEL` (met het
  label na "Naam:" in hun omschrijving; herkomst `behandelaar`). Wat een
  latere stage pas vraagt (zoals de bekendmaking in stage `BEKENDMAKING`), is
  bij het besluit nog niet gebeurd: een boolean is false, al het andere null,
  herkomst `stand_bij_besluit`; wat een lexostatus van de zaak al afleidt
  (geen gram: null) staat daar niet bij. Het event legt de uitkomsten vast, en
  verder alleen wat een oordeel meegeeft (zoals de besluitdatum als
  `op_moment`).
- **Vervolg**: een latere stage van hetzelfde artikel, zoals de bekendmaking.
  De engine voert die stage uit (`execute_stage`) op de invoer en de
  uitkomsten van het vastgelegde besluit: het gram van het besluit is de
  toestand van RFC-008. Het formulier is wat de stage vraagt (`requires`,
  met het label van de parameter van het besluit). De haken die de wet op die
  stage laat vuren (RFC-007, zoals Awb 6:8 op `BEKENDMAKING`) rekenen mee; hun
  uitkomsten komen bij de uitkomsten van de handeling, en het event legt ze
  vast. Geeft een haak geen waarde, dan neemt het proces het vervolg niet
  uit zichzelf; gemeld als gebeurd legt de cel het vast, met een lege
  termijn.
- **Feit**: het event heeft geen stage, zoals een verzoek om aanvulling, een
  ontvangst of een betaling. Het formulier zijn de `$external`-velden van het
  event die geen uitkomst zijn, met het type van de parameter die een
  lexostatus uit dat veld afleidt. Op proef laat de cel de lexostatussen van
  de zaak reduceren alsof het feit al vastlag (`POST .../proef` met het
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
de proef om de inhoud nee (een toets, een haak zonder waarde, of de wet kan
niet uitrekenen wat een feit doet), dan staat in het antwoord `te_melden`.
Meldt de behandelaar dat het feit toch gebeurde (`{formulier, gebeurd:
true}`), dan legt de cel het vast, gaat de reden mee als waarschuwing, en
tonen de lexostatussen de gevolgen: een betaling boven het bedrag telt mee in
wat betaald is, en de wet zegt wat onverschuldigd is betaald. Wat de vorm
raakt (een leeg formulier, een vervolg zonder besluit, een moment na vandaag
of voor de zaak) houdt ook een melding tegen. Een besluit wordt niet gemeld
(400): dat neemt het proces zelf.

De peildatum van een handeling is de dag van het `op_moment` dat haar event
aan een veld van het formulier bindt, met de grondslag uit de stroom (de
besluitdatum, de dag van bekendmaking, de dag van betaling); anders vandaag.
De engine leest de regeling op die dag, en elke cel peilt erop.

Het antwoord op een proef noemt de soort, de peildatum en waar die vandaan
komt, de uitkomsten (ook als de handeling niet te nemen is), de toetsen, per
parameter de herkomst, `niet_geleverd` en de reden als de handeling niet te
nemen is. Niets wordt vastgelegd.

## Een handeling vastleggen

`POST /processen/<id>/api/zaken/<zaakkenmerk>/handelingen/<naam>` rekent
hetzelfde uit en laat de cel het gram vastleggen, met als velden per
`$external`-sleutel van het event een uitkomst of een waarde uit het
formulier. Het gram draagt `inputs` met per parameter haar waarde en haar
herkomst (RFC-013 `accepted_values`), een `receipt` met de geladen regelingen
en de stromen van de cel, met een SHA-256 over beide, en `handelende_actor`:
rol, kanaal, identiteit, de grondslag van de rol, en bij een besluit of
vervolg `namens` en bij mandaat `mandaat`. Een decretogram draagt daarnaast
wat het besluit tot besluit maakt: `legal_character` en `decision_type` uit
`produces`, `regulation`, `regulation_valid_from` en `competent_authority`.

Bij een besluit en een vervolg komt het bevoegd gezag uit de regeling (het
artikel, anders de regeling zelf) en wordt het letterlijk getoetst tegen het
gezag van `namens`: gelijk betekent vastleggen, een gezag uit `mandaten`
vastleggen in mandaat, een ander gezag weigeren; noemt de regeling er geen,
dan legt de cel vast met een waarschuwing en zonder `competent_authority`. Zo
blijven de drie assen van RFC-022 par. 2 gescheiden: `recording_actor`,
`competent_authority` en de handelende actor.

Deze leiden tot een weigering met 409 en zonder gram: de proef is niet te
nemen en het feit is niet als gebeurd gemeld (of kan dat niet, om de vorm),
de cel weigert omdat de stage al in de zaak ligt (het wijzigen van een besluit
valt buiten deze stap), omdat het `op_moment` voor de zaak ligt of omdat de
zaak veranderde sinds het proces haar las, of de wet wijst een ander gezag
aan zonder mandaat. Het proces geeft de cel mee hoeveel grammen de zaak had
(`zaak_grammen`); de cel legt alleen vast als dat onder haar slot nog zo is.
Wat het proces uitrekende (zoals wat er nog te betalen is), gold voor de zaak
zoals die toen was: twee gelijktijdige betalingen komen er niet allebei door,
net zo min als twee besluiten. Het gram wordt voor het vastleggen tegen
`gram.json` gevalideerd.

## Rechtsbescherming

`GET .../zaken/<zaakkenmerk>` noemt de procedure van het besluit, met per
stage of er een gram van ligt, en de rechtsbescherming die daaruit volgt
(RFC-022 par. 3.3): na de laatste stage die in de zaak ligt de volgende, als
geen handeling haar vastlegt (zoals `BEZWAAR`, die na de bekendmaking vanzelf
loopt), met de uitkomsten van de haken die de wet op de laatste stage liet
vuren, zoals ze in het gram staan (het einde van de bezwaartermijn, Awb 6:7
en 6:8). Geen regel en geen configuratie declareert de route: zij volgt uit
de procedure en de haken in de wet.

## Controles bij het opstarten

Faalt er een, dan start de runtime niet. Een melding over een cel begint met
`cel '<id>':`, een over een proces met `proces '<id>':`.

Per cel:

1. Celdefinitie, stromen en lexostatus-definities valideren tegen hun schema;
   de lexostatussen horen bij deze cel en elke stroom heeft haar
   `recording_actor`.
2. Een afleiding wijst naar iets wat bestaat: een parameter van een artikel uit
   de grondslag van een event dat haar filter aanwijst (of uit haar eigen
   `grondslag`), en veldpaden van dat event. Elk artikel uit de grondslag van
   een afleiding is geladen en heeft het lid dat ze noemt. Een afleiding op het
   gekozen gram vraagt `kies`.
3. Geen weesveld: elk veld wordt door een afleiding of filter gelezen, of
   staat in `niet_gereduceerd`.
4. Een parameter krijgt maar een afleiding.
5. Een filter of input op `zaakkenmerk` wijst alleen events met een zaak aan.
6. De startstand past in de stromen van de cel.
7. Een lijst (`groepeer`) wijst alleen events met een zaak aan, ook in
   `zonder`, en `zonder` wijst een event aan. Haar kolommen hoeven geen
   parameter te zijn en botsen niet met die van andere lexostatussen.
8. Een lexostatus levert iets: ten minste een afleiding of een extra veld.

Per proces:

1. De procesdefinitie valideert tegen `proces.json`. Het portaal, de
   werkvoorraad, het besluit en de bronnen van de zaak noemen een cel, dezelfde,
   en die draait in deze runtime.
2. De `actor` is de `recording_actor` van elke stroom waarin het proces
   vastlegt (die van het portaal en die van elke handeling). `namens` noemt een
   gezag dat een geladen regeling noemt (verplicht met een behandeling); een
   mandaat noemt zo'n gezag, niet het eigen, en een grondslag die een geladen
   artikel aanwijst. Elk kanaal heeft unieke velden, leesbare patronen en een
   eigenaar die een veld is; elke rol noemt een bestaand kanaal. Routes
   `portaal` en `behandeling` passen bij de blokken; routes `loket` vragen een
   portaal-event dat `op_moment` aan `$intake` bindt.
3. Het portaal wijst naar een bestaand event van die cel, dat alleen
   `$intake`-paden leest die de kanalen van het portaal leveren, een lexostatus die dat event
   leest en een gram kiest (geen lijst, alleen input `zaakkenmerk`), en een
   uitkomst van een artikel uit de grondslag van het event. Het aanbod noemt een
   bestaande uitkomst en een termijn uit hetzelfde artikel, en leunt alleen op
   wat vooraf vaststaat: elke parameter van zijn artikel heeft origin `KANAAL`
   of `REGISTER`, of `BELANGHEBBENDE` met `rol: TIJDVAK` (het tijdvak; de
   grondslag Awb 4:2 lid 1 alleen maakt een parameter geen tijdvak, met `aanbod.tijdvakken`: een uitkomst van dezelfde regeling uit een
   artikel zonder verplichte parameters). Een `grondslag` in het
   formulierbestand (bij een veld of kolom) wijst een geladen artikel aan, met
   een lid dat het heeft.
4. Synthese: alleen met een portaal of handelingen; elke invoer komt uit een
   veld van de toets-lexostatus of een lexostatus van de zaak, of van een
   eerdere bron die het doorgeeft (in rondes, zo diep als nodig), of is een vaste
   waarde; elke parameter (de naam bij de afnemer) is een parameter van het
   artikel van de toets, het besluit of het aanbod, of van een artikel dat een
   van die transitief aanroept (via `source`); een parameter komt uit maar een
   bron; een gewone bron is een andere cel dan die van het proces.
5. Behandeling: de werkvoorraad is een lijst; een bron van de zaak vraagt een
   behandeling; de lexostatussen van de zaak hebben als enige input
   `zaakkenmerk`. Per handeling: een unieke naam; een rol die ze noemt, mag
   de behandeling; de uitkomsten komen uit een artikel (bij een vervolg ook
   uit de haken van zijn stage); elke parameter uit het formulier, de stand
   van wat nog niet gebeurd is of een rijen-definitie moet de aanroeper van
   het artikel leveren; een parameter komt uit maar een bron. Zonder
   `regeling` is het artikel de enige beschikking waarvoor het gezag van
   `namens` bevoegd is. Het vastleg-event bestaat en volgt een zaak; een
   stage erop staat in de procedure van het artikel. Een besluit legt elke
   uitkomst vast en verder alleen oordelen; een vervolg legt vast wat de
   stage vraagt en wat de haken uitrekenen, en niets anders.
6. Synthese per regel: de tabel komt uit een lexostatus van de zaak of een
   bron die haar levert; elke kolomnaam komt uit maar een plek (de tabel of een
   bron); een invoer `kolom` wijst een kolom aan die ervoor gevuld wordt; een
   bron is een andere cel.
7. Als synthese en handelingen kloppen: elke parameter die de aanroeper van
   de toets, het aanbod of een uitkomst van een handeling (behalve een
   vervolg, dat op het vastgelegde besluit rekent) moet leveren, heeft een
   leverancier die bij zijn geldende origin past, en geen die er niet bij past
   (RFC-043; zie `origin`). Een verkeerde bron is altijd een fout, ook bij
   `required: false`. Of een afleiding van de eigen cel van de belanghebbende
   of uit het dossier komt, volgt uit wat haar filters doorlaten: grammen van
   type `indiening` (wat de aanvrager aanlevert) of andere grammen van de
   actor (het verloop van de zaak). Zonder leverancier start de runtime niet,
   behalve bij `required: false`: dan krijgt de engine hem niet en rekent ze
   met een onbekende waarde, en is het een waarschuwing. Een parameter zonder
   origin is een waarschuwing, en met `herkomst: streng` in `proces.yaml` een
   fout; een `BELANGHEBBENDE`-parameter zonder `required: false` (behalve het
   tijdvak) is een waarschuwing. Een bron met een url, of een interne cel die
   niet draait, telt, met een waarschuwing per bron over wat niet na te gaan
   is. Een `register` dat niet geladen is, is een fout; een grondslag in een
   regeling die niet geladen is, een waarschuwing. `origins` in
   uitvoeringsbeleid van het gezag van `namens` overschrijft de origin uit de wet; twee
   botsende overschrijvingen zijn een fout. Al bij het laden van het corpus
   houdt een origin die niet te lezen is, of een REGISTER zonder `register`,
   de runtime tegen, met bestand, artikel en parameter.
8. De voorbeelden bestaan en hebben de goede vorm, en horen bij een handeling
   die het proces heeft.

Of een bron bereikbaar is en de lexostatus met die parameters en inputs
aanbiedt, controleert de runtime na het starten via `GET /api/cellen` bij de
bron. Een probleem daar is een waarschuwing, geen weigering: de bron mag later
komen.

## Modules

| Module | Taak |
|---|---|
| `runtime` | cellen en processen laden en controleren, kronieken openen, router over alles |
| `cel` | een cel uit haar map laden |
| `proces` | een proces uit zijn map laden, en de controles op cel, actor en portaal |
| `config` | omgeving, `cel.yaml` en `proces.yaml` |
| `stroom` | stroomdefinitie laden en valideren, gram bouwen uit intake en external |
| `gram` | het vastgelegde gram, met invoer en receipt van elke berekende handeling, en het lezen van een veldpad |
| `reductie` | kroniek reduceren tot lexostatus; `reductie::definitie` laadt de lexostatus-definities, `reductie::peil` peilt op een eerder moment |
| `startstand` | grammen voor een lege kroniek |
| `kroniek` | append-only opslag, in het geheugen met een index per zaak, en herstel van een half geschreven regel |
| `controle` | de controles bij het opstarten |
| `synthese` | bronnen bevragen, samenvoegen met herkomst, en de controles erop |
| `origin` | wie een parameter levert volgens de wet (RFC-043): de controle bij het opstarten, de aanbodregel, het tijdvak en het besluitformulier |
| `transport` | intern en HTTP |
| `kanaal` | kanalen en rollen uit `proces.yaml`: de vorm van een login, de intake, de eigenaar, de controles |
| `gezag` | `namens` en `mandaten`: het gezag waarvoor een proces handelt, en de toets tegen de wet |
| `sessie` | sessies per rol |
| `toets` | parameters aan de engine, een of meer uitkomsten evalueren |
| `handeling` | de handelingen in een zaak (besluit, vervolg, feit): voorbereiden bij het laden, op proef, vastleggen, de stand per zaak en de rechtsbescherming, en de controles op behandeling |
| `rijen` | synthese per regel: een tabelveld wordt een array-parameter |
| `mogelijkheid` | wat het aanbod per tijdvak zegt (`portaal.aanbod`) |
| `voorbeelden` | de voorbeelden per handeling uit `voorbeelden` in `proces.yaml` |
| `api` | de routes: `api::cel` (de cel), `api::proces` (de router van een proces), `api::sessie`, `api::portaal`, `api::loket` en `api::behandeling` |
| `celclient` | hoe een proces de cel vraagt: zaak lezen, vastleggen, proefreductie, als typen |
| `datum` | momenten lezen, peildatum, jaartal en het `Tijdpunt` van een peil |
| `laden` | bestanden en mappen lezen, YAML valideren tegen zijn schema |
| `regelingen`, `formulier`, `schema` | laden en valideren |

## De engine en een losse uitkomst

De engine voert bij een gevraagde uitkomst het hele artikel uit, ook de
invoer uit andere artikelen en regelingen, en stopt bij de eerste waarde die
ontbreekt. Een toets is dus alleen te beoordelen als alles wat het artikel
aanraakt aanwezig is, ook feiten van de instantie zelf. Het proces vult dan
niets aan en meldt "niet te beoordelen: mist <parameter>". De test
`engine_eist_het_hele_artikel_bij_een_uitkomst` in `src/toets.rs` legt dit
gedrag vast.
