# cel

Een proof of concept bij RFC-022: een runtime voor cellen. Een cel legt feiten
vast als chronolexogram in een eigen kroniek, reduceert die kroniek tot een
lexostatus, en kan met een portaal een indiening laten toetsen door een artikel
(een `TOETS`). De toets mag lexostatussen van andere cellen erbij halen
(synthese).

Een cel is configuratie, geen code: een map met een `cel.yaml`. De code noemt
geen casus; de tests draaien op de generieke fixtures in `tests/fixtures/`. De
docs-pagina `docs/src/content/docs/components/cel.md` beschrijft dezelfde
opzet, met de afwijkingen van RFC-022 en de open vragen.

## Starten

```bash
just cel          # runtime op :7170, frontend op :7171, op de fixture-cellen
```

## Configuratie

| Variabele | Betekenis |
|---|---|
| `CELLS_PATH` | Map met een submap per cel, elk met een `cel.yaml`. |
| `REGULATION_PATH` | Map met regelingen, gedeeld door alle cellen; elk YAML-bestand met `$id` en `articles` wordt geladen. |
| `DATA_DIR` | Map voor de kronieken: per cel een submap `<id>/`. |
| `CEL_PORT` | Poort, standaard 7170. De runtime luistert op `0.0.0.0`. |

```yaml
# <CELLS_PATH>/<map>/cel.yaml, schema schema/chronolex/v0.1.0/cel.json
id: <cel-id>                      # routes onder /cellen/<id>/api/
recording_actor: <actor>          # elke stroom van de cel heeft deze actor
stromen: [<pad>, ...]             # stroombestanden of mappen, relatief aan deze map
lexostatussen: <pad>              # lexostatus-definities
portaal:                          # optioneel
  stroom: <$id van de stroom>
  event: <event dat een indiening wordt>
  toets: {lexostatus: <naam>, regeling: <$id>, uitkomst: <output>}
  formulier: {pad: <pad>, scherm: <id>}   # optioneel
synthese:                         # optioneel, alleen met een portaal
  - cel: <id van de bron-cel>
    url: <http://host:poort>      # optioneel; zonder url: intern transport
    lexostatus: <naam bij de bron>
    invoer: {<input van de bron>: {lexostatus: <eigen toets-lexostatus>, veld: <parameter of extra veld>}}
    parameters: [<naam>, ...]     # expliciet, geen wildcard
startstand: <pad>                 # optioneel: grammen voor een lege kroniek
```

Een cel zonder `portaal` heeft geen eHerkenning- en aanvraagroutes, alleen een
kroniek en lexostatussen. Het formulierbestand levert alleen labels, soorten en
volgorde; het bepaalt nooit het gedrag.

## Routes

| Route | Doet |
|---|---|
| `GET /api/cellen` | de cellen, met per cel `portaal`, de lexostatussen (inputs, parameters, extra velden) en de synthese-bronnen |
| `GET /cellen/<id>/api/kroniek` | de grammen, elk met YAML |
| `GET /cellen/<id>/api/lexostatus/<naam>?<input>=...` | een reductie; de inputs als query |
| `POST /cellen/<id>/api/eherkenning/login`, `GET .../sessie`, `POST .../logout` | alleen met portaal |
| `GET /cellen/<id>/api/stroom` | alleen met portaal: de stroom en de formuliervelden |
| `POST /cellen/<id>/api/aanvraag/toets` | alleen met portaal: concept, reductie, synthese, engine |
| `POST /cellen/<id>/api/aanvraag` | alleen met portaal: het gram vastleggen |

Bij een cel met een portaal zijn kroniek en lexostatus alleen voor de
ingelogde KvK, en alleen over diens eigen grammen.

## De vier lagen

1. **Lexogram.** De regelingen onder `REGULATION_PATH`, ongewijzigd. Een
   artikel declareert welke parameters het nodig heeft.
2. **Stroomdefinitie** (`stroom`, schema `stream.json`). Welke feiten de cel
   vastlegt, door wie, in welke kroniek en op welke `grondslag` (een lijst; een
   artikelnummer mag een spatie hebben). Een veld bindt aan `$intake.*`, aan
   `$external.*` of is een constante. Een tabelveld declareert zijn kolommen.
   Een indiening van soort `aanvraag` heeft `fields.kern` (Awb 4:2 lid 1) en
   `fields.inhoud`. `niet_gereduceerd` noemt met reden de velden die geen
   afleiding of filter leest.
3. **Reductie tot lexostatus** (`reductie`, schema `lexostatus.json`). Een
   definitie beperkt de kroniek met `filter` en kiest met `kies: laatste` zo
   nodig een gram. Per parameter een afleiding:
   - op het gekozen gram: `veld`, `gevuld`, `gelijk`, `tabel` met `elke_regel`
     of `een_regel` (en `alleen_waar`), `moment`;
   - over de grammen die door een eigen `filter` komen: `bestaat: true`,
     `som: <veld>`, `kies: laatste` met `veld: <pad>` of met
     `bevat: {veld, waarde}`.

   Een filtersleutel is een veld van het gram zelf (`name`, `type`, `soort`,
   `zaakkenmerk`, `recording_actor`, `chronicle`) of een veldpad onder
   `fields`; `$x` komt uit de inputs. Geen gram is "nee" bij `bestaat` en
   `bevat`, en nul bij `som`: de cel spreekt alleen over haar eigen kroniek.
   Een waarde die er niet is (`veld` op een leeg veld, `kies` zonder gram)
   blijft weg; de cel vult nooit aan. `extra_velden` levert waarden die geen
   parameter zijn, zoals de invoer van een synthese-bron; ze gaan nooit naar de
   engine. `levert_aan` noemt artikelen van een afnemer waarvan de lexostatus
   parameters levert, als de afnemer een feit onder een eigen naam vraagt.
4. **Het gram** (`kroniek`, schema `gram.json`). Een JSON-regel per gram in
   `DATA_DIR/<cel>/<chronicle>.jsonl`, alleen toevoegen. Een niet-ingevuld veld
   staat erin als `null`. Wat niet in de vorm van de stroom past, weigert de
   cel met 400 en het veldpad. Een gram uit de startstand draagt
   `herkomst: startstand`.

## Startstand

`startstand.jsonl` heeft per regel `stroom`, `name`, `op_moment`,
`herkomst: startstand`, `fields` en optioneel `zaakkenmerk`. De rest volgt uit
de stroom. De velden moeten precies die van het event zijn. De runtime zet de
startstand in de kroniek als elke kroniek van de cel leeg is, en daarna nooit
meer. Zo'n gram is geplaatst, niet berekend: er is geen engine-trace bij.

## Synthese en transport

De toets reduceert eerst het concept tot de eigen lexostatus. Daarna vraagt ze
elke synthese-bron om haar lexostatus, met de invoer uit de eigen lexostatus,
via `/cellen/<cel>/api/lexostatus/<naam>`. Het transport (`transport`) is intern
als de bron-cel in dezelfde runtime draait (dezelfde aanroep door de router,
zonder netwerk) en HTTP als de bron een `url` heeft. Een bron krijgt drie
seconden. Van het antwoord neemt de toets alleen de parameters uit
`parameters`. Het antwoord van de toets noemt per parameter de herkomst (de
eigen lexostatus, of cel en lexostatus met het transport) en per bron de
status (`bevraagd`, `onbereikbaar`, `fout`, `niet_bevraagd`). Niets uit de
synthese wordt vastgelegd. Is een bron onbereikbaar en de uitkomst daardoor
niet te beoordelen, dan zegt de toets "niet te beoordelen: bron <id>
onbereikbaar", en vult ze niets aan.

## Controles bij het opstarten

Per cel; faalt er een, dan start de runtime niet, en elke melding begint met
`cel '<id>':`.

1. Celdefinitie, stromen en lexostatus-definities valideren tegen hun schema;
   de lexostatussen horen bij deze cel en elke stroom heeft haar
   `recording_actor`.
2. Een afleiding wijst naar iets wat bestaat: een parameter van een artikel uit
   de grondslag van een event dat haar filter aanwijst (of uit `levert_aan`), en
   veldpaden van dat event. Een afleiding op het gekozen gram vraagt `kies`.
3. Geen weesveld: elk veld wordt door een afleiding of filter gelezen, of
   staat in `niet_gereduceerd`.
4. Een parameter krijgt maar een afleiding.
5. Het portaal wijst naar een bestaand event, een lexostatus die een gram
   kiest, en een uitkomst van een artikel uit de grondslag van het event.
6. Synthese: alleen met een portaal; elke invoer komt uit een veld van de
   toets-lexostatus; elke parameter is een parameter van het artikel van de
   toets of van een artikel dat het transitief aanroept (via `source`); een
   parameter komt uit maar een bron.
7. De startstand past in de stromen van de cel.

Of een bron bereikbaar is en de lexostatus met die parameters en inputs
aanbiedt, controleert de runtime na het starten via `GET /api/cellen` bij de
bron. Een probleem daar is een waarschuwing, geen weigering: de bron mag later
komen.

## Modules

| Module | Taak |
|---|---|
| `runtime` | cellen laden en controleren, kronieken openen, router over alle cellen |
| `cel` | een cel uit haar map laden |
| `config` | omgeving en `cel.yaml` |
| `stroom` | stroomdefinitie laden en valideren, gram bouwen uit intake en external |
| `reductie` | lexostatus-definities laden, kroniek reduceren tot lexostatus |
| `startstand` | grammen voor een lege kroniek |
| `kroniek` | append-only opslag |
| `controle` | de controles bij het opstarten |
| `synthese` | bronnen bevragen, samenvoegen met herkomst, en de controles erop |
| `transport` | intern en HTTP |
| `eherkenning` | nep-login (KvK, gemachtigde, machtiging `volledig`) en sessies |
| `toets` | parameters aan de engine, een uitkomst evalueren |
| `api` | de routes van een cel |
| `regelingen`, `formulier`, `schema` | laden en valideren |

## De engine en een losse uitkomst

De engine voert bij een gevraagde uitkomst het hele artikel uit, ook de
invoer uit andere artikelen en regelingen, en stopt bij de eerste waarde die
ontbreekt. Een toets is dus alleen te beoordelen als alles wat het artikel
aanraakt aanwezig is, ook feiten van de instantie zelf. De cel vult dan niets
aan en meldt "niet te beoordelen: mist <parameter>". De test
`engine_eist_het_hele_artikel_bij_een_uitkomst` in `src/toets.rs` legt dit
gedrag vast.
