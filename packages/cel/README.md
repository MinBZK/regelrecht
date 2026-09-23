# cel

Een proof of concept bij RFC-022: een runtime voor cellen. Een cel legt feiten
vast als chronolexogram in een eigen kroniek, reduceert die kroniek tot een
lexostatus, en kan met een portaal een indiening laten toetsen door een artikel
(een `TOETS`). De toets mag lexostatussen van andere cellen erbij halen
(synthese). Een behandelaar ziet een werkvoorraad (een lijst-lexostatus), opent
een zaak, laat de engine een proefbesluit uitrekenen zonder vast te leggen, en
neemt het besluit: dan legt de cel de uitkomst vast als stage-decretogram.

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
rollen:                           # optioneel; zonder rollen geen login
  aanvrager: eherkenning          # het portaal
  behandelaar: medewerker         # werkvoorraad, zaak en proefbesluit
portaal:                          # optioneel, vraagt rollen.aanvrager
  stroom: <$id van de stroom>
  event: <event dat een indiening wordt>
  toets: {lexostatus: <naam>, regeling: <$id>, uitkomst: <output>}
  formulier: {pad: <pad>, scherm: <id>}   # optioneel
synthese:                         # optioneel, alleen met een portaal of een besluit
  - cel: <id van de bron-cel>
    url: <http://host:poort>      # optioneel; zonder url: intern transport
    lexostatus: <naam bij de bron>
    invoer: {<input van de bron>: {lexostatus: <eigen toets-lexostatus>, veld: <parameter of extra veld>}}
    parameters: [<naam>, ...]     # expliciet, geen wildcard
behandeling:                      # optioneel, vraagt rollen.behandelaar
  werkvoorraad: <lijst-lexostatus>
  besluit:
    regeling: <$id>
    uitkomsten: [<output>, ...]   # van een en hetzelfde artikel
    lexostatussen: [<naam>, ...]  # eigen, met als enige input zaakkenmerk
    formulier:                    # oordelen van de behandelaar
      - {parameter: <naam>, label: <tekst>, groep: <tekst>}
    stand_bij_besluit:            # feiten van na het besluit: null of false
      <parameter>: null
    rijen:                        # synthese per regel (zie hieronder)
      - parameter: <array-parameter>
        tabel: {lexostatus: <eigen>, veld: <tabelveld>}
        kolommen: {<kolom van de tabel>: <kolom van de parameter>}
        bronnen:
          - cel: <id>
            url: <http://host:poort>   # optioneel; zonder url: intern
            lexostatus: <naam bij de bron>
            invoer:
              <input>: {kolom: <kolom van de regel>}
              <input>: {lexostatus: <eigen>, veld: <naam>}
              <input>: {parameter: <naam>, als: eerste_dag_van_het_jaar}
            kolommen: {<naam bij de bron>: <kolom van de parameter>}
    vastleggen:                   # optioneel: waar het besluit terechtkomt
      stroom: <$id>
      event: <event met zaak: volgt en een stage>
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
| `POST /cellen/<id>/api/medewerker/login` (`{naam}`), `GET .../sessie`, `POST .../logout` | alleen met de rol behandelaar |
| `GET /cellen/<id>/api/werkvoorraad` | behandelaar: de werkvoorraad, een lijst |
| `GET /cellen/<id>/api/zaken/<zaakkenmerk>` | behandelaar: de grammen van de zaak, het besluitformulier en een proefbesluit zonder oordelen |
| `POST /cellen/<id>/api/zaken/<zaakkenmerk>/proefbesluit` | behandelaar: `{formulier}` naar een proefbesluit; niets wordt vastgelegd |
| `POST /cellen/<id>/api/zaken/<zaakkenmerk>/besluit` | behandelaar: `{formulier}` naar een vastgelegd besluit (201), of een weigering (409) |

Een cel met rollen heeft een sessie per gebruiker (een cookie per cel); wie
als de andere rol inlogt, vervangt de sessie. Kroniek en lexostatus zijn dan
alleen voor wie is ingelogd: de aanvrager ziet de grammen van zijn KvK, de
behandelaar alle. De portaalroutes zijn alleen voor de aanvrager (403 voor de
behandelaar), de behandelroutes alleen voor de behandelaar.

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
   `herkomst: startstand`. Een gram van een besluit dat de cel zelf nam draagt
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

## Synthese per regel

Een artikel kan een tabel als parameter vragen waarvan de indiener maar een
deel invult; de rest stelt de instantie zelf vast, per regel, uit registers
van andere cellen. `rijen` zegt welk tabelveld van een eigen lexostatus de
regels levert, welke kolom onder welke naam meegaat, en welke bron per regel
met welke invoer wordt bevraagd. Per regel gaat de cel langs de bronnen, in
volgorde, zodat een bron een kolom kan gebruiken die een eerdere leverde. De
invoer komt uit de regel (`kolom`), uit een eigen lexostatus (`lexostatus` en
`veld`) of uit de samengevoegde parameters (`parameter`), zo nodig omgezet met
`als: eerste_dag_van_het_jaar` (de tegenhanger van de afleiding `jaar_van`).
Een bron levert een kolom uit haar `parameters` of haar `extra_velden`.
Ontbreekt een invoer, is een bron onbereikbaar, of levert ze de waarde niet,
dan blijft die kolom weg; `mist` noemt welke. Er wordt niets aangevuld.

## Proefbesluit

`behandeling.besluit` zegt welke uitkomsten van welk artikel het besluit zijn
en waar elke parameter vandaan komt, uit precies een bron: een eigen
lexostatus van de zaak, een synthese-bron (met de invoer uit die eigen
lexostatus), het besluitformulier (oordelen van de behandelaar, herkomst
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

`POST /cellen/<id>/api/zaken/<zaakkenmerk>/besluit` rekent hetzelfde uit en
legt de uitkomst vast, als `behandeling.besluit.vastleggen` zegt waar. Het
gram is een stage-decretogram van dat event: `zaak: volgt` met het
zaakkenmerk van de zaak, en de uitkomsten als velden. Daarbij komt wat het
besluit tot besluit maakt: `legal_character` en `decision_type` uit `produces`
van het artikel, `regulation` en `regulation_valid_from`, `inputs` met per
parameter haar waarde en haar herkomst (RFC-013 `accepted_values`), en een
`receipt` met de geladen regelingen en de stromen van de cel, met een
SHA-256 over beide.

Het bevoegd gezag komt uit de regeling (het artikel, anders de regeling zelf)
en wordt getoetst tegen de `recording_actor` van de cel: gelijk betekent
vastleggen, een ander gezag betekent weigeren, en noemt de regeling er geen,
dan legt de cel vast met een waarschuwing en zonder `competent_authority`.

Drie dingen leiden tot een weigering met 409 en zonder gram: het proefbesluit
is niet compleet ("niet te nemen: mist X"), er ligt al een gram met stage
`BESLUIT` in de zaak (het wijzigen van een besluit valt buiten deze stap), of
de wet wijst een ander gezag aan. Het gram wordt voor het vastleggen tegen
`gram.json` gevalideerd.

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
6. Synthese: alleen met een portaal of een besluit; elke invoer komt uit een
   veld van de toets-lexostatus; elke parameter is een parameter van het
   artikel van de toets of het besluit, of van een artikel dat een van beide
   transitief aanroept (via `source`); een parameter komt uit maar een bron.
7. De startstand past in de stromen van de cel.
8. Een lijst (`groepeer`) wijst alleen events met een zaak aan, ook in
   `zonder`, en `zonder` wijst een event aan. Haar kolommen hoeven geen
   parameter te zijn en botsen niet met die van andere lexostatussen. Het
   portaal en het besluit gebruiken geen lijst.
9. Rollen en behandeling: een portaal vraagt de rol aanvrager (en omgekeerd),
   een behandeling de rol behandelaar; de werkvoorraad is een lijst; de
   uitkomsten van het besluit komen uit een artikel; de lexostatussen van het
   besluit hebben als enige input `zaakkenmerk`; elke parameter uit het
   formulier, de stand bij besluit of een rijen-definitie moet de aanroeper
   van het artikel leveren; een parameter komt uit maar een bron.
10. Synthese per regel: de tabel komt uit een lexostatus van het besluit die
   haar levert; elke kolomnaam komt uit maar een plek (de tabel of een bron);
   een invoer `kolom` wijst een kolom aan die ervoor gevuld wordt; een bron is
   een andere cel. Waar het besluit wordt vastgelegd: een bestaand event met
   `zaak: volgt` en een stage, waarvan de `$external`-sleutels precies de
   uitkomsten van het besluit zijn.
11. Een lexostatus levert iets: ten minste een afleiding of een extra veld.

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
| `eherkenning` | nep-login (KvK, gemachtigde, machtiging `volledig`) |
| `sessie` | sessies per rol, en de nagebootste medewerkerslogin |
| `toets` | parameters aan de engine, een of meer uitkomsten evalueren |
| `besluit` | het proefbesluit op een zaak, het vastleggen ervan, en de controles op rollen en behandeling |
| `rijen` | synthese per regel: een tabelveld wordt een array-parameter |
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
