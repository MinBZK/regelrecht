# cel

Een proof of concept bij RFC-022: een runtime voor cellen en processen, met de
indeling van de positionpaper.

- Een **cel** legt feiten vast als chronolexogram in een eigen kroniek en
  reduceert die kroniek tot een lexostatus. Meer doet ze niet.
- Een **proces** handelt: het informeert (synthese van lexostatussen uit
  cellen), concludeert (de engine, het besluit) en vraagt een cel vast te
  leggen. Met een portaal laat het een indiening toetsen door een artikel (een
  `TOETS`) en indienen; met een behandeling ziet een behandelaar een
  werkvoorraad, opent een zaak, laat de engine een proefbesluit uitrekenen en
  neemt het besluit, dat de cel vastlegt als stage-decretogram.

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
rollen:                           # optioneel; zonder rollen geen login
  aanvrager: eherkenning          # het portaal
  behandelaar: medewerker         # werkvoorraad, zaak en besluit
portaal:                          # optioneel, vraagt rollen.aanvrager
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
    keuzes: {jaren_vanaf_nu: [0, 1]}   # als het artikel een tijdvak vraagt (Awb 4:2 lid 1)
  formulier: {pad: <pad>, scherm: <id>}   # optioneel
synthese:                         # optioneel, alleen met een portaal of een besluit
  - {cel: <cel-id>, lexostatus: <naam>, zaak: true}   # een lexostatus van de zaak
  - cel: <id van de bron-cel>
    url: <http://host:poort>      # optioneel; zonder url: intern transport
    lexostatus: <naam bij de bron>
    invoer: {<input van de bron>: {lexostatus: <lexostatus van de zaak of eerdere bron>, veld: <parameter of extra veld>}}
    parameters: [<naam>, ...]     # expliciet, geen wildcard
    extra_velden: [<naam>, ...]   # optioneel: invoer voor een latere bron
behandeling:                      # optioneel, vraagt rollen.behandelaar
  werkvoorraad: {cel: <cel-id>, lexostatus: <lijst-lexostatus>}
  besluit:
    regeling: <$id>               # optioneel: anders de beschikking van de actor
    uitkomsten: [<output>, ...]   # van een en hetzelfde artikel
    stand_bij_besluit:            # feiten van na het besluit: null of false
      <parameter>: null
    rijen:                        # synthese per regel (zie hieronder)
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
              <input>: {parameter: <naam>, als: eerste_dag_van_het_jaar}
            kolommen: {<naam bij de bron>: <kolom van de parameter>}
    vastleggen:                   # optioneel: waar het besluit terechtkomt
      cel: <cel-id>
      stroom: <$id>
      event: <event met zaak: volgt en een stage>
voorbeelden:                      # optioneel: standaardgegevens per handeling
  inloggen: [<pad>, ...]
  aanvraag: <pad>
  besluit: <pad>
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
| `GET /cellen/<id>/api/kroniek` | de grammen, elk met YAML |
| `GET /cellen/<id>/api/zaken/<zaakkenmerk>` | de grammen van één zaak, elk met YAML; de cel filtert, 404 als ze de zaak niet kent |
| `GET /cellen/<id>/api/lexostatus/<naam>?<input>=...` | een reductie; de inputs als query |
| `POST /cellen/<id>/api/lexostatus/<naam>/proef` | `{concept, inputs}`: de cel bouwt het gram van het concept in het geheugen en reduceert de kroniek mét dat gram; er wordt niets vastgelegd |
| `POST /cellen/<id>/api/grammen` | `{actor, stroom, event, intake, external, zaakkenmerk?, besluit?}`: de cel bouwt het gram, valideert het, controleert de actor en de zaak, en legt het vast (201); 409 als die stage al vastligt in de zaak |
| `GET /cellen/<id>/api/stroom` | de stroomdefinities van de cel, met hun hash |
| `GET /processen/<id>/api/voorbeelden` | de voorbeelden per handeling, zonder login |
| `POST /processen/<id>/api/eherkenning/login`, `GET .../sessie`, `POST .../logout` | alleen met portaal |
| `GET /processen/<id>/api/formulier` | alleen met portaal: de stroom en de formuliervelden |
| `POST /processen/<id>/api/aanvraag/toets` | alleen met portaal: proefreductie in de cel, synthese, engine |
| `POST /processen/<id>/api/aanvraag` | alleen met portaal: de cel legt het gram vast |
| `GET /processen/<id>/api/mogelijkheden` | alleen met portaal: wat het aanbod per tijdvak uit `aanbod.keuzes` zegt |
| `POST /processen/<id>/api/medewerker/login` (`{naam}`), `GET .../sessie`, `POST .../logout` | alleen met de rol behandelaar |
| `GET /processen/<id>/api/werkvoorraad` | behandelaar: de werkvoorraad, een lijst uit de cel |
| `GET /processen/<id>/api/zaken/<zaakkenmerk>` | behandelaar: de grammen van de zaak, het besluitformulier en een proefbesluit zonder oordelen |
| `POST /processen/<id>/api/zaken/<zaakkenmerk>/proefbesluit` | behandelaar: `{formulier}` naar een proefbesluit; niets wordt vastgelegd |
| `POST /processen/<id>/api/zaken/<zaakkenmerk>/besluit` | behandelaar: `{formulier}` naar een vastgelegd besluit (201), of een weigering (409) |

Een proces met rollen heeft een sessie per gebruiker (een cookie per proces);
wie als de andere rol inlogt, vervangt de sessie. De portaalroutes zijn alleen
voor de aanvrager (403 voor de behandelaar), de behandelroutes alleen voor de
behandelaar. Een cel kent geen login: haar routes zijn voor elke afnemer, er is
geen beveiligingscontext. Een aanvrager die een zaak wil volgen, moet die zaak
kennen (een gram van zijn KvK); dat controleert het proces.

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
   dragen: een besluit `BESLUIT`, een aanvraag `AANVRAAG`; alleen op een
   decretogram of een indiening.
3. **Reductie tot lexostatus** (`reductie`, schema `lexostatus.json`). Een
   definitie beperkt de kroniek met `filter` en kiest met `kies: laatste` zo
   nodig een gram. Per parameter een afleiding:
   - op het gekozen gram: `veld`, `jaar_van` (het jaartal van een datum),
     `gevuld`, `gelijk`, `tabel` met `elke_regel` of `een_regel` (en
     `alleen_waar`), `moment`;
   - over de grammen die door een eigen `filter` komen: `bestaat: true`,
     `som: <veld>`, `kies: laatste` met `veld: <pad>` of `jaar_van: <pad>` (en
     optioneel `geen_gram: <waarde>`, de lezing van afwezigheid) of met
     `bevat: {veld, waarde}`.

   Met `groepeer: zaakkenmerk` is de lexostatus een **lijst**: een regel per
   zaak waarvan ten minste een gram door `filter` komt, en met `zonder:
   {filter}` geen gram door dat filter. `kies` en de afleidingen werken per
   zaak; de afleidingen zijn kolommen, geen parameters. Een lijst gaat nooit
   naar de engine.

   Een filtersleutel is een veld van het gram zelf (`name`, `type`, `soort`,
   `stage`, `zaakkenmerk`, `recording_actor`, `chronicle`) of een veldpad onder
   `fields`; `$x` komt uit de inputs. Geen gram is "nee" bij `bestaat` en
   `bevat`, en nul bij `som`: de cel spreekt alleen over haar eigen kroniek.
   Een waarde die er niet is (`veld` op een leeg veld, `kies` zonder gram)
   blijft weg, tenzij de definitie met `geen_gram` zegt hoe zij het ontbreken
   van een gram leest (bijvoorbeeld null: niet gebeurd); de cel vult nooit aan. `extra_velden` levert waarden die geen
   parameter zijn, zoals de invoer van een synthese-bron; ze gaan nooit naar de
   engine. `levert_aan` noemt artikelen van een afnemer waarvan de lexostatus
   parameters levert, als de afnemer een feit onder een eigen naam vraagt.
4. **Het gram** (`kroniek`, schema `gram.json`). Een JSON-regel per gram in
   `DATA_DIR/<cel>/<chronicle>.jsonl`, alleen toevoegen. Een niet-ingevuld veld
   staat erin als `null`. Wat niet in de vorm van de stroom past, weigert de
   cel met 400 en het veldpad. Een gram uit de startstand draagt
   `herkomst: startstand`. Een gram van een besluit dat een proces nam draagt
   daarnaast `legal_character`, `decision_type`, `regulation`,
   `regulation_valid_from`, zo nodig `competent_authority`, `inputs` (elke
   parameter met haar waarde en herkomst) en `receipt`.

## Startstand

`startstand.jsonl` heeft per regel `stroom`, `name`, `op_moment`,
`herkomst: startstand`, `fields` en optioneel `zaakkenmerk`. De rest volgt uit
de stroom. De velden moeten precies die van het event zijn. De runtime zet de
startstand in de kroniek als elke kroniek van de cel leeg is, en daarna nooit
meer. Zo'n gram is geplaatst, niet berekend: er is geen engine-trace bij.

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
en `veld`) of uit de samengevoegde parameters (`parameter`), zo nodig omgezet met
`als: eerste_dag_van_het_jaar` (de tegenhanger van de afleiding `jaar_van`).
Een bron levert een kolom uit haar `parameters` of haar `extra_velden`.
Ontbreekt een invoer, is een bron onbereikbaar, of levert ze de waarde niet,
dan blijft die kolom weg; `mist` noemt welke. Er wordt niets aangevuld.

De toets kent hetzelfde blok onder `portaal.toets.rijen`. Daar komt de tabel
uit de proefreductie van het concept (de toets-lexostatus) of uit een bron die
haar doorgeeft; de toets bouwt de rijen op vóór de engine, zoals het besluit.
Een rijen-blok levert alleen aan de uitvoering waar het staat: de rijen van
de toets gelden in de controle op herkomst voor de toets, die van het besluit
voor het besluit.

## Proefbesluit

`behandeling.besluit` zegt welke uitkomsten van welk artikel het besluit zijn
en waar elke parameter vandaan komt, uit precies een bron: een lexostatus van
de zaak (een synthese-bron met `zaak: true`, gevraagd aan de cel), een andere
synthese-bron (met de invoer uit die lexostatus), het besluitformulier (oordelen van de behandelaar: de parameters met origin
`OORDEEL`, met het label na "Naam:" in hun omschrijving; herkomst
`behandelaar`) of de stand bij besluit (feiten van na het besluit, zoals de
bekendmaking, als null of false; herkomst `stand_bij_besluit`). Het
proefbesluit voert het artikel uit op de datum van vandaag. Het antwoord heeft
de uitkomsten als elk een waarde heeft, anders "niet te nemen: mist X"; per
parameter de herkomst; en `niet_geleverd`: elke parameter die de aanroeper van
het artikel moet leveren en die geen bron leverde, met de omschrijving uit de
regeling. Een aanroep binnen dezelfde regeling zonder `parameters` deelt de
parameters; een aanroep van een andere regeling krijgt alleen wat
`parameters` meegeeft. Niets wordt vastgelegd.

## Het besluit vastleggen

`POST /processen/<id>/api/zaken/<zaakkenmerk>/besluit` rekent hetzelfde uit en
laat de cel de uitkomst vastleggen, als `behandeling.besluit.vastleggen` zegt
waar. Het gram is een stage-decretogram van dat event: `zaak: volgt` met het
zaakkenmerk van de zaak, en de uitkomsten als velden. Daarbij komt wat het
besluit tot besluit maakt: `legal_character` en `decision_type` uit `produces`
van het artikel, `regulation` en `regulation_valid_from`, `inputs` met per
parameter haar waarde en haar herkomst (RFC-013 `accepted_values`), en een
`receipt` met de geladen regelingen en de stromen van de cel, met een
SHA-256 over beide. Het proces stelt dit samen (het draait de engine); de
hashes van de stromen komen van de cel. De cel bouwt het gram uit haar stroom
en legt het vast met `POST /cellen/<cel>/api/grammen`.

Het bevoegd gezag komt uit de regeling (het artikel, anders de regeling zelf)
en wordt getoetst tegen de `actor` van het proces: gelijk betekent
vastleggen, een ander gezag betekent weigeren, en noemt de regeling er geen,
dan legt de cel vast met een waarschuwing en zonder `competent_authority`.

Drie dingen leiden tot een weigering met 409 en zonder gram: het proefbesluit
is niet compleet ("niet te nemen: mist X"), de cel weigert omdat er al een
gram met stage `BESLUIT` in de zaak ligt (het wijzigen van een besluit valt
buiten deze stap), of de wet wijst een ander gezag aan. Of een stage
vastlegbaar is, beslist de cel: een zaak doorloopt elke stage één keer
(RFC-022 par. 1.2), en de cel toetst dat onder hetzelfde slot als het
schrijven, zodat twee gelijktijdige besluiten er niet allebei door komen. Het gram wordt voor het vastleggen tegen
`gram.json` gevalideerd.

## Controles bij het opstarten

Faalt er een, dan start de runtime niet. Een melding over een cel begint met
`cel '<id>':`, een over een proces met `proces '<id>':`.

Per cel:

1. Celdefinitie, stromen en lexostatus-definities valideren tegen hun schema;
   de lexostatussen horen bij deze cel en elke stroom heeft haar
   `recording_actor`.
2. Een afleiding wijst naar iets wat bestaat: een parameter van een artikel uit
   de grondslag van een event dat haar filter aanwijst (of uit `levert_aan`), en
   veldpaden van dat event. Een afleiding op het gekozen gram vraagt `kies`.
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
   vastlegt (die van het portaal en die van het besluit).
3. Het portaal wijst naar een bestaand event van die cel, dat alleen
   `$intake`-paden leest die het portaal levert, een lexostatus die dat event
   leest en een gram kiest (geen lijst, alleen input `zaakkenmerk`), en een
   uitkomst van een artikel uit de grondslag van het event. Het aanbod noemt een
   bestaande uitkomst en een termijn uit hetzelfde artikel, en leunt alleen op
   wat vooraf vaststaat: elke parameter van zijn artikel heeft origin `KANAAL`
   of `REGISTER`, of `BELANGHEBBENDE` met grondslag Awb 4:2 lid 1 (het
   tijdvak, met `aanbod.keuzes`).
4. Synthese: alleen met een portaal of een besluit; elke invoer komt uit een
   veld van de toets-lexostatus of een lexostatus van de zaak, of van een
   eerdere bron die het doorgeeft; elke parameter is een parameter van het
   artikel van de toets, het besluit of het aanbod, of van een artikel dat een
   van die transitief aanroept (via `source`); een parameter komt uit maar een
   bron; een gewone bron is een andere cel dan die van het proces.
5. Rollen en behandeling: een portaal vraagt de rol aanvrager (en omgekeerd),
   een behandeling de rol behandelaar; de werkvoorraad is een lijst; een bron
   van de zaak vraagt een behandeling; de uitkomsten van het besluit komen uit
   een artikel; de lexostatussen van de zaak hebben als enige input
   `zaakkenmerk`; elke parameter uit het formulier, de stand bij besluit of een
   rijen-definitie moet de aanroeper van het artikel leveren; een parameter
   komt uit maar een bron. Zonder `regeling` vindt het proces zijn besluit via
   de `actor`: de enige beschikking waarvoor die het bevoegd gezag is.
6. Synthese per regel: de tabel komt uit een lexostatus van de zaak of een
   bron die haar levert; elke kolomnaam komt uit maar een plek (de tabel of een
   bron); een invoer `kolom` wijst een kolom aan die ervoor gevuld wordt; een
   bron is een andere cel. Waar het besluit wordt vastgelegd: een bestaand
   event met `zaak: volgt` en een stage, waarvan de `$external`-sleutels
   precies de uitkomsten van het besluit zijn.
7. Als synthese en besluit kloppen: elke parameter die de aanroeper van de
   toets, het aanbod of het besluit moet leveren, heeft een leverancier die bij
   zijn geldende origin past (RFC-043; zie `origin`). Zonder leverancier start
   de runtime niet, behalve bij `required: false`: dan is het een waarschuwing.
   Een parameter zonder origin, en een `BELANGHEBBENDE`-parameter zonder
   `required: false`, geven ook een waarschuwing. `origins` in uitvoeringsbeleid
   van de actor overschrijft de origin uit de wet; twee botsende
   overschrijvingen zijn een fout.
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
| `reductie` | lexostatus-definities laden, kroniek reduceren tot lexostatus |
| `startstand` | grammen voor een lege kroniek |
| `kroniek` | append-only opslag |
| `controle` | de controles bij het opstarten |
| `synthese` | bronnen bevragen, samenvoegen met herkomst, en de controles erop |
| `origin` | wie een parameter levert volgens de wet (RFC-043): de controle bij het opstarten, de aanbodregel, het tijdvak en het besluitformulier |
| `transport` | intern en HTTP |
| `eherkenning` | nep-login (KvK en persoon; bevoegdheid komt uit het handelsregister) |
| `sessie` | sessies per rol, en de nagebootste medewerkerslogin |
| `toets` | parameters aan de engine, een of meer uitkomsten evalueren |
| `besluit` | het proefbesluit op een zaak, het vastleggen ervan, en de controles op rollen en behandeling |
| `rijen` | synthese per regel: een tabelveld wordt een array-parameter |
| `api` | de routes van een cel en van een proces |
| `regelingen`, `formulier`, `schema` | laden en valideren |

## De engine en een losse uitkomst

De engine voert bij een gevraagde uitkomst het hele artikel uit, ook de
invoer uit andere artikelen en regelingen, en stopt bij de eerste waarde die
ontbreekt. Een toets is dus alleen te beoordelen als alles wat het artikel
aanraakt aanwezig is, ook feiten van de instantie zelf. Het proces vult dan
niets aan en meldt "niet te beoordelen: mist <parameter>". De test
`engine_eist_het_hele_artikel_bij_een_uitkomst` in `src/toets.rs` legt dit
gedrag vast.
