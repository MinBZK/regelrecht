# regelrecht-simulator

Testopstelling voor chronolexografie ([RFC-022](../../docs/src/content/rfcs/rfc-022.md)).
De crate simuleert een wereld van **cellen** en laat een scenario die wereld
optuigen en bevragen.

Deze versie is bewust krap, maar de grens staat er wel al — en die is met opzet
een taalgrens en geen afspraak.

Wetten zijn optioneel. Een cel met `laws: []` is een **bron-cel**: ze legt vast
en reduceert, zonder engine. Zie [Een bron-cel](#een-bron-cel).

Een cel met eigen wetten kan ook **besluiten**: ze voert een regeling uit en legt
de uitkomst vast als decretogram in haar eigen kroniek. Zie
[Het besluit-pad](#het-besluit-pad). Wat zo'n besluit aan **verplichtingen**
achterlaat, komt de klok later nakomen:
[Verplichtingen](#verplichtingen-wat-een-besluit-achterlaat).

Gaat een vraag over een celgrens, dan loopt hij langs de **veiligheidscontext**
van de vragende cel naar het **transport**, en dat is de enige weg. Zie
[Over een celgrens](#over-een-celgrens). Wat daar langs gaat is te meten met het
**observatielog**, een test-only instrument dat met opzet buiten de band staat:
[Het observatielog](#het-observatielog-buiten-de-band).

Het eigenlijke product zijn de **invarianten**. Een scenario declareert het
toegestane vraaggraf, de run levert het feitelijke, en de gate berekent er zelf uit
de celconfiguraties bij wat het recht van elke cel eigenlijk vraagt — elk verschil
laat het scenario falen. Zie
[De vijf invarianten](#de-vijf-invarianten-en-de-gate-eronder), inclusief de
vermelding die er altijd bij hoort: I1 tot en met I5 en het observatielog komen
**niet** uit RFC-022.

## De drie chronolexogrammen, en waar ze hier zitten

De paper onderscheidt drie soorten chronolexogram (RFC-022 §1.1–§1.2). Deze
crate kent ze alle drie bij naam; twee ervan ontstaan hier tijdens een run — de
kaart van paper naar code:

- **Lexogram** — generiek en compile-time: het recht zelf. Hier is dat elk
  versiebestand onder `laws`. De map met `valid_from`-versies van een regeling
  *is* de lexogram-kroniek; de engine kiest daarin op `op_moment`. Niemand
  noemde dat hier eerder zo, maar het is precies wat RFC-022 §1.1 beschrijft.
- **Executogram** — de vastgelegde vaststelling van een feit. Dat is een
  `ChronicleEvent`, en sinds de tijdlijn erin zit heeft die de vorm die
  RFC-022 §1.3 vraagt: zie [De executogram-vorm](#de-executogram-vorm).
- **Decretogram** — individueel en operationeel: het besluit. Dat legt een cel
  nu zelf vast, in haar eigen kroniek: zie [Het besluit-pad](#het-besluit-pad).

En de vierde term, de **reductie**: de huidige vormen (één uitkomst van één
eigen regeling, of een kroniekfilter dat per sleutelwaarde de laatste
vastlegging kiest of één veld optelt) zijn een eerste benadering van wat de paper
en RFC-022 §4.1 bedoelen, namelijk filteren en aggregeren over kronieken. Het
filteren staat er; van het aggregeren staat de som er, en verder nog niets.

## Wat een cel is

Een cel is een *containment- en autonomiedomein*, en verder niets:

- ze houdt haar eigen **kronieken** (de feiten die zij zelf vastlegde);
- ze laadt haar eigen **regelingen**;
- ze **reduceert** over die eigen feiten tot een **lexostatus**: de
  rechtstoestand vanuit een gevraagd perspectief, op de feiten die zij kent.

En één regel die bepaalt wat er in zo'n kroniek terecht mag komen:

> **Een kroniek bevat alleen wat de cel zelf overkwam.**

Een aanvraag die binnenkwam, een levering die ze ontving, een betaling die ze
deed, een besluit dat ze zelf nam. Wat een cel elders *opvroeg* hoort er niet
in: dat zou een schaduwboekhouding zijn van andermans feiten, en dan ligt het
totaalbeeld dat volgens RFC-022 nergens bestaat alsnog in één cel. Een
opgehaalde waarde die meedeed in een besluit hoort in het decretogram van dat
besluit, met bron en moment erbij — niet als los feit in een kroniek.

Daarom draagt elke vastlegging de naam van de cel zelf als `recording_actor`, en
weigert de loader een vastlegging op naam van een ander. Niet omdat een cel
nooit iets van buiten te weten krijgt, maar omdat ze dan vastlegt wat *haar*
overkwam: `intake: levering` zegt dat het geleverd werd.

## Een bron-cel

Een **bron-cel** is een cel met `laws: []`. Ze houdt kronieken en ze reduceert,
maar er draait geen engine in. Haar lexostatussen zijn filters over haar eigen
vastleggingen.

Dat is geen uitgeklede cel, het is de gewone vorm van bijna elke organisatie.
Een register, een vereniging, een betaalsysteem, een legacy-API: die voeren geen
machineleesbare wet uit en gaan dat morgen ook niet doen. RFC-022 §2 zegt
daarover precies één ding, en het alternatief "engine = cel" is er expliciet
afgewezen: **de engine is een component dat in een cel kán draaien, niet de cel
zelf.** Zolang een cel niet zonder engine kan bestaan, is die scheiding proza.
Daarom houdt [`Cell`](src/cell/mod.rs) haar engine in een `Option` — een cel
zonder wetten bouwt er geen, en `Cell::reduce` werkt er gewoon op.

Het is tegelijk de toets op het lexostatus-contract. Als "een gepubliceerde naam
met gedocumenteerde parameters, bevraagd op een moment" alleen werkt met een
engine erachter, dan is het contract een engine-interface met een andere naam.
Een consument kan aan een antwoord niet zien welke van de twee hij bevroeg, en
dat is het punt: de scenario's [`brp_broncel.yaml`](scenarios/brp_broncel.yaml)
en [`toeslagen_zorgtoeslag.yaml`](scenarios/toeslagen_zorgtoeslag.yaml) stellen
hun vragen op dezelfde manier.

Wat een bron-cel niet kan, is besluiten: daar hoort een eigen regeling bij, en
die heeft ze niet. Vastleggen en reduceren is genoeg om een cel te zijn — zie
[Het besluit-pad](#het-besluit-pad) voor wat een cel mét wetten er nog bij kan.

## Wat een cel níet is

Precies de drie dingen die er in de praktijk bij gedacht worden (RFC-022 §2):

- **Geen sleutels.** Ondertekening, trust material en de autorisatie waaronder
  een vraag beantwoord wordt, zitten in de veiligheidscontext, niet in de cel.
- **Geen bevoegd gezag.** `competent_authority` (RFC-002) is een juridisch feit
  van het besluit, geen eigenschap van de opslag. Een cel houdt kronieken van
  besluiten waarvoor een ander bevoegd is.
- **Geen synthese.** Een reductie raakt uitsluitend de eigen kronieken.
  Combineren over cellen heen doet een consument, nooit een cel.

Drie dingen dwingen dat af in code in plaats van in proza:

1. `Cell` houdt haar `ChronicleStore` in een **privéveld** zonder accessor. Er
   is geen `pub fn store()` en die komt er ook niet: dat een andere cel niet bij
   deze feiten kan, moet een compileerfout zijn.
2. De reductie mag alleen een regeling gebruiken die de cel zélf laadt. Een
   definitie die naar een vreemde regeling wijst, wordt geweigerd bij het
   optuigen van de cel — niet pas bij de eerste vraag.
3. `Cell` heeft **geen veld en geen parameter** voor een transport of een
   veiligheidscontext. Ze kan de grens dus niet bereiken; dat is een
   compileerfout en geen afspraak. Een test grept `src/cell/` erop, zodat ook een
   latere toevoeging die de weg zou openen meteen rood wordt. Dat geldt ook op het
   besluit-pad: een waarde *accepteren* van een andere cel kan wel, maar het
   ophalen gebeurt buiten de cel — zie
   [Accepteren in plaats van narekenen](#accepteren-in-plaats-van-narekenen-i5).

## De publieke ingang

Een consument heeft er precies één:

```rust,ignore
cell.reduce(lexostatus_naam, &params, op_moment) -> Result<Lexostatus>
```

Hij kiest een **gepubliceerde naam** en levert de **gedocumenteerde
parameters**. Een onbekende naam levert een fout die opsomt wat de cel wél
publiceert; een ontbrekende, onbekende of verkeerd getypeerde parameter wordt
geweigerd. Een eigen reductie meesturen kan niet — daar is geen parameter voor
(RFC-022 §4.1).

Lexostatus-definities zijn dan ook **data en geen Rust**: ze staan in de
cel-configuratie van het scenario. Een nieuwe lexostatus is een blok YAML, geen
nieuwe functie.

Gedocumenteerd geldt aan beide kanten. De `output` in de definitie stuurt de
evaluatie aan, maar begrenst het antwoord niet: de engine levert bij een
gevraagde uitkomst ook de uitkomsten die er causaal mee meekomen. Wat de cel
naar buiten geeft, staat daarom apart in `outputs`, en `Cell::reduce` laat de
rest weg. Berekend is niet gepubliceerd — anders krijgt een consument
`hoogte_zorgtoeslag` mee zonder dat de cel dat ooit beloofd heeft. Het
meegeleverde scenario publiceert die uitkomst dus expliciet. Een naam in
`outputs` die de regeling niet kent, wordt geweigerd bij het optuigen van de
cel, net als een reductie over een vreemde regeling.

### Twee reductievormen

```yaml
reduction:                        # wetsvorm: laat een eigen regeling rekenen
  regulation: wet_op_de_zorgtoeslag
  output: heeft_recht_op_zorgtoeslag
  parameters:
    bsn: $bsn

reduction:                        # kroniekfilter: lees een eigen kroniek
  chronicle: relaties
  key: bsn
  latest: true                    # de laatste vastlegging; mag weggelaten worden
  where:                          # optioneel, gelijkheid op velden
    partnerschap_type: HUWELIJK

reduction:                        # de som over een eigen kroniek
  chronicle: betalingen
  key: zaakkenmerk
  sum: bedrag                     # in plaats van `latest`, nooit samen
```

De configuratie kiest door te noemen wat ze bedoelt; er is geen `kind`-veld dat
herhaalt wat er al staat. Beide vormen in één reductie, of geen van beide, is een
fout die zegt welke twee vormen er zijn.

Het **kroniekfilter** levert per sleutelwaarde de laatste vastlegging op of vóór
`op_moment`. Drie dingen om te weten:

- De `key` is de naam van een **gedocumenteerde parameter**: de consument levert
  de waarde aan. Een sleutel die niet in `inputs` staat, wordt geweigerd — dan
  zou de vraag over geen enkel onderwerp gaan.
- `outputs` is hier **verplicht**. Bij de wetsvorm is er altijd één uitkomst die
  de lexostatus *is*; een filter heeft die niet, dus zonder `outputs` zou de cel
  haar hele vastlegging naar buiten geven zonder iets beloofd te hebben. Een
  output die in geen enkele vastlegging van de stroom voorkomt, wordt geweigerd
  bij het optuigen, net als een onbekende stroomnaam of een `where` op een veld
  dat de stroom niet kent. Zo'n filter zou anders stil "niets vastgesteld"
  antwoorden op elke vraag, en dat is niet te onderscheiden van een leeg
  verleden.
- `where` bepaalt **welke** vastleggingen meedoen, en pas van die groep wint de
  laatste. Een voorwaarde op een veld dat over tijd verandert levert dus de
  laatste vastlegging die eraan voldeed, niet de huidige stand — de lexostatus
  `laatste_huwelijk` in het bron-celscenario staat er om dat te laten zien. De
  waarden in `where` zijn **letterlijk**: `key` is de enige plek waar een
  kroniekfilter een parameter gebruikt, dus een `$naam` in `where` wordt bij het
  optuigen geweigerd in plaats van als tekst gezocht.

Het filter doet geen toestandsmerge: één vastlegging gaat er in haar geheel uit,
niet een record dat uit verschillende momenten is samengeraapt. Wat samen
vastgelegd is, blijft samen. Een gepubliceerd veld dat déze vastlegging niet
draagt, blijft uit het antwoord; de cel vult niets aan. Draagt ze er géén van,
dan is er niets vastgesteld — zie hieronder.

`sum: <veld>` is de andere kant: geen vastlegging maar één getal, opgeteld over
**alle** vastleggingen die op of vóór `op_moment` aan het filter voldoen. Dat is
de eerste aggregatie van de simulator, en ze bestaat voor één soort vraag: "hoeveel
is er tot nu toe betaald". Vier dingen leggen dat vast:

- `outputs` noemt precies het gesommeerde veld. Een som levert één waarde op, dus
  iets anders publiceren is een belofte die nooit nagekomen wordt, en dat wordt bij
  het optuigen geweigerd.
- `latest` en `sum` tegelijk is een fout: het zijn twee reducties, niet twee
  schrijfwijzen. `key` en `where` horen bij beide en betekenen hetzelfde.
- Nul vastleggingen is **niet** nul: dan is er niets vastgesteld. Een 0 zou "er is
  niets betaald" niet kunnen onderscheiden van "over deze zaak ligt hier niets".
- Een vastlegging zonder getal in dat veld is een fout en geen nul. Een som die
  zo'n vastlegging overslaat valt stil te laag uit, en dat is aan het getal niet
  te zien.

Dat dit een reductie is en geen teller, is het hele punt: "betaald tot nu toe" op
een moment in het verleden blijft exact hetzelfde antwoord geven nadat er meer
betaald is. Een saldo dat naast de kroniek wordt bijgehouden kan dat niet.

### "Niets vastgesteld" is een antwoord

Dit is het antwoord van een kroniekfilter dat niets aantreft; de wetsvorm levert
altijd de uitkomst die de engine berekent. Was er op `op_moment` geen feit, dan is
dat geen fout en geen lege map die op een antwoord lijkt, maar een eigen variant
met een reden:

```rust,ignore
match answer.outcome {
    LexostatusOutcome::Established(values) => /* … */,
    LexostatusOutcome::NotEstablished { reason } => /* … */,
}
```

De reden zegt welke stroom is nagekeken, met welke sleutelwaarde, op welk moment
en onder welk filter, zodat een consument kan zien waar hij moet kijken. Trof het
filter wél een vastlegging aan maar draagt die geen enkele gepubliceerde uitkomst,
dan zegt de reden dát, met het moment van die vastlegging erbij: dan moet de lezer
naar de velden kijken en niet naar de tijdas. Een scenario legt dit antwoord vast
met `expect_not_established: true`; dat is een volwaardige verwachting en sluit
`expect` uit.

`op_moment` is het moment waarop gevraagd wordt, altijd expliciet en nooit de
wandklok. Feiten die pas later in de cel zijn vastgelegd, bestaan voor dat
antwoord niet, en de engine kiest op datzelfde moment de regelingversie. Een
moment ná de klok van de wereld is een fout; zie [De tijdlijn](#de-tijdlijn).

## Het besluit-pad

De lus van chronolexografie is informeren → concluderen → **vastleggen**, en dit
is het derde deel. Een cel voert een eigen regeling uit en legt de uitkomst vast
als **decretogram** in haar eigen kroniek:

```rust,ignore
world.decide("toeslagen", "zorgtoeslag_vaststelling", &params, op_moment)?;
```

`Cell::decide` is `pub(crate)`, net als `Cell::record`: een consument kan een cel
niet laten besluiten, net zomin als hij in haar kroniek kan schrijven. Wat er
gebeurt, in deze volgorde:

1. de gedocumenteerde parameters worden gecontroleerd, precies zoals bij een vraag;
2. de inputs worden verzameld — uit de eigen kronieken zoals ze op `op_moment`
   waren, en uit de parameters van het besluit;
3. de **besluit-engine** voert de regeling uit op dat moment, dus op de wetsversie
   die toen gold;
4. de verplichtingen worden uitgerekend tot een schema van termijnen, op de
   uitkomsten waarop besloten is (zie
   [Verplichtingen](#verplichtingen-wat-een-besluit-achterlaat));
5. de uitkomst gaat als één gram de stroom `beschikkingen` in, met het schema erin.

Die stroom is **voorbehouden**, aan drie kanten:

- een configuratie die haar zelf declareert wordt geweigerd; ze wordt automatisch
  aangemaakt zodra een cel besluit-definities heeft;
- een `fixture` kan er niets in zetten. Zou een wereldbestand er een gram in
  mogen schrijven, dan lag er een "besluit" zonder receipt en zonder herkomst
  tussen de echte, en kon een reductie de twee niet onderscheiden — dan bewijst
  het kernscenario hieronder niets meer;
- een besluit kan er geen input uit halen (`from_chronicle: beschikkingen`).
  Daar liggen besluiten en geen feiten: een besluit leest geen besluit.

Alleen besluiten legt er iets in, en alleen een reductie haalt er iets uit.

### Twee paden, twee engines

Een cel met besluit-definities houdt **twee engine-instanties over precies dezelfde
wetten**. Dat is geen verdubbeling maar de plek waar het verschil tussen de twee
paden afdwingbaar wordt (RFC-022 §4.2):

| pad | engine | mag de celgrens over |
|---|---|---|
| `Cell::reduce` (publiek) | reduce-engine, **nooit** een `CellResolver` | nee — tier 1 en 2 |
| `Cell::decide` (intern) | besluit-engine, krijgt er een voor de duur van één besluit | ja — tier 3 |

Een cross-cel-pull vanuit een reductie is daarmee een *ontbrekende capability* en
geen afspraak: de engine van het reduce-pad heeft geen resolver, dus ze kan een
`source.regulation` die een andere cel aanwijst niet beantwoorden en levert geen
antwoord in plaats van een geraden antwoord. Dezelfde lexostatus over dezelfde
regeling faalt daar dus waar een besluit slaagt, en dat staat vast in
[`tests/accepteren.rs`](tests/accepteren.rs) — met een melding die zegt wat er aan
de hand is: *een reductie reikt niet buiten de eigen cel*.

### Wat een decretogram draagt

Het decretogram **is** het RFC-013 Execution Receipt, plus wat dat receipt niet
kan weten. Geen parallel formaat ernaast: een handgeschreven selectie zou bij elke
uitbreiding van RFC-013 stil achterlopen.

| veld | wat |
|---|---|
| `zaakkenmerk` | waaronder deze zaak terug te vinden is, uit het sjabloon van de definitie |
| `op_moment` | wanneer besloten is (het gram is een gewoon executogram) |
| `regulation` + `regulation_valid_from` | welke regeling, en **welke versie daarvan gold** |
| `competent_authority` | het bevoegd gezag dat de regeling noemt (RFC-002) |
| `legal_character` | wat de wet ervan maakt: `BESCHIKKING`, `TOETS`, … |
| de uitkomsten | de uitkomst die het besluit *is*, plus wat `outputs` erbij noemt |
| `inputs` | elke waarde waarop besloten is, **met haar herkomst** |
| `obligations` | het betalingsschema dat uit dit besluit volgt: per termijn een vervaldatum, een bedrag en een volgnummer |
| `receipt` | het volledige Execution Receipt |

De herkomst per input is geen versiering. Zonder haar staat er wel een waarde in
het gram, maar niet van wanneer ze was of wie haar leverde — en dan is
"accepteren in plaats van narekenen" niet van gokken te onderscheiden. Wat het
besluit ophaalde, gaat daarom ín het gram en **niet** als los feit in een
kroniek: een volgend besluit haalt het opnieuw op. Een kroniek bevat alleen wat de
cel zelf overkwam (zie [Wat een cel is](#wat-een-cel-is)).

Eén besluit is één gram. Wat tegelijk ontstaat, wordt samen vastgelegd (RFC-022
§1.2 — elk chronolexogram is *elementair*): het recht en het bedrag staan in
hetzelfde gram, niet in twee.

### Verplichtingen: wat een besluit achterlaat

Een beschikking die een bedrag toekent, laat iets achter dat later moet gebeuren.
Dat hoort bij het gram (RFC-022 §1.2), dus het schema wordt bij het besluit
uitgerekend en er in vastgelegd:

```yaml
obligations:
  - amount: $hoogte_zorgtoeslag      # een uitkomst van dít besluit
    payer: belastingdienst           # de cel die de verplichting draagt
    schedule: $betalingsritme        # ineens | kwartaal | maand, of een instelling
    from: '{jaar}-02-01'             # optioneel; standaard het moment van het besluit
```

- **`amount` is een uitkomst, geen bedrag.** Wat betaald moet worden komt uit de
  wet die het besluit uitvoert. Een letterlijk bedrag zou naast die uitkomst gaan
  leven, en dan zegt het gram twee dingen over hetzelfde geld.
- **Een ritme beschrijft één jaar**: `ineens` één termijn, `kwartaal` vier,
  `maand` twaalf. Elke termijn krijgt hetzelfde bedrag in hele eenheden en het
  restant gaat naar de laatste, dus de som van de termijnen is exact het
  toegekende bedrag. Dat is de eigenschap waarop "betaald tot nu toe" rust.
- **Het ritme mag een instelling zijn.** Een betalingsritme is doorgaans beleid en
  geen wet; `schedule: $betalingsritme` leest uit `settings` van het
  wereldbestand, zodat de besluit-definitie niet beweert dat de wet per kwartaal
  betaalt. Een instelling die niet bestaat of geen ritme noemt, sneuvelt bij het
  optuigen van de wereld.
- **`from` is een sjabloon over de gedocumenteerde parameters**, net als het
  zaakkenmerk, en wat het oplevert moet een datum zijn. Het mag niet vóór het
  besluit liggen: een termijn in het verleden zou bij het nakomen een betaling op
  een moment vastleggen dat al geweest is, en dan verandert het beeld van toen
  alsnog.

Nakomen doet de klok, niet het besluit. Op elke vervaldatum legt de **betalende**
cel een executogram vast in haar eigen stroom `betalingen` (`intake: betaling`,
met zaakkenmerk, bedrag, volgnummer en de verwijzing naar het decretogram), en de
**besluitende** cel een levering in de hare: *betaling ontvangen gemeld*. Twee
vastleggingen, elk in de kroniek van de cel die haar deed — beide kanten weten wat
er gebeurde, en niemand kopieert de staat van een ander.

`betalingen` is een naam van het platform, zoals `intake` platformvocabulaire is:
beide cellen declareren een stroom met die naam en `zaakkenmerk` als sleutel, en
een verplichting naar een cel die dat niet doet wordt bij het optuigen geweigerd —
niet op de eerste vervaldatum, halverwege de tijdlijn. Anders dan `beschikkingen`
is de stroom níet voorbehouden: een betaling is een gewoon feit, en een wereld mag
er een startstand in hebben staan.

**Eén termijn wordt één keer nagekomen.** Dezelfde zaak, hetzelfde besluit en
hetzelfde volgnummer is dezelfde termijn: de klok mag in zo kleine stappen
langskomen als ze wil, en een besluit dat wordt overgedaan levert geen tweede
betaling. Zonder die regel zou de som dubbel tellen, en dat is aan het getal niet
te zien.

Dat het besluit in die vergelijking staat en niet alleen het volgnummer, is de
andere helft: over één zaak worden meer besluiten genomen — een verlening en later
een vaststelling — en elk draagt een eigen schema dat bij termijn 1 begint. Op
zaak en volgnummer alléén zou de eerste termijn van het tweede besluit voor die
van het eerste doorgaan en stil wegvallen. Om dezelfde reden lopen de volgnummers
binnen één gram dóór over álle verplichtingen die het besluit oplegt, en beginnen
ze niet per verplichting opnieuw.

Het staat als scenario in
[`scenarios/toeslagen_verplichtingen.yaml`](scenarios/toeslagen_verplichtingen.yaml)
(vier kwartaaltermijnen, met de vraag over een eerder moment die ná een jaar nog
hetzelfde antwoordt) en
[`scenarios/toeslagen_verplichtingen_ritmes.yaml`](scenarios/toeslagen_verplichtingen_ritmes.yaml)
(hetzelfde bedrag `ineens` en per `maand` — het ritme bepaalt wanneer, niet hoeveel).

### Het zaakkenmerk moet bij precies één zaak horen

Het `zaakkenmerk` is waaronder een zaak terug te vinden is, dus twee zaken mogen
er nooit één worden. Twee regels bewaken dat:

- **Bij het optuigen**: twee verwijzingen die aan elkaar plakken worden
  geweigerd. `{jaar}{bsn}` geeft voor `2024` + `999993653` precies hetzelfde
  kenmerk als voor `20249` + `99993653`, en geen enkele waarde repareert dat.
- **Bij het besluit**: een parameterwaarde waarin een scheidingsteken van het
  sjabloon voorkomt wordt geweigerd. Bij `{jaar}/{bsn}` is `jaar = '2024/9'`
  met `bsn = '99993653'` niet te onderscheiden van `jaar = '2024'` met
  `bsn = '999993653'`.

Een sjabloon met één verwijzing heeft geen scheidingstekens en weigert dus niets:
wat ervóór en erachter staat ligt vast, dus `zorgtoeslag/{bsn}` blijft eenduidig
ook als de waarde zelf een `/` bevat.

Grammen van verschillende besluiten kunnen wél bewust één kenmerk delen — dat is
juist wat "de kroniek van een zaak" betekent. Ze houden elkaar uit elkaar met het
veld `besluit`, waar een kroniekfilter met `where:` op kan filteren.

### Twee besluiten op één moment

De dag is hier de fijnste korrel van de tijdas. Twee besluiten op dezelfde dag
over dezelfde zaak zijn daarop niet uit elkaar te houden; dan beslist de volgorde
van vastleggen, en een reductie levert het **laatstgenomen** besluit. Beide
grammen blijven staan — een kroniek groeit en vervangt niets — dus het eerste is
niet verdwenen, alleen niet meer het laatste woord van die dag.

### Terugzien is een reductie, nooit een herberekening

Het gram is een gewoon `ChronicleEvent` met `intake: eigen_besluit`, dus de
tijdreductie werkt er zonder speciale gevallen overheen. Een lexostatus die over
`beschikkingen` filtert levert het besluit terug zoals het toen vastgelegd is;
ligt er op het gevraagde moment nog geen gram, dan is het antwoord "niets
vastgesteld" — en geen som van vandaag.

Dat verschil is de hele reden dat een decretogram naast een lexogram bestaat, en
het staat als scenario in
[`scenarios/toeslagen_besluit.yaml`](scenarios/toeslagen_besluit.yaml): een
besluit op T1 onder wetsversie V1, een vraag op T2 waar V2 geldt en een
herberekening een ander bedrag zou geven, en een antwoord dat nog steeds het gram
van T1 is — met `regulation_valid_from: 2024-01-01` erin, zodat te zien is onder
welk recht besloten is. Het tegenscenario staat er direct naast: een nieuw besluit
op T2 levert wél V2.

De stroom `beschikkingen` gaat met opzet **niet** als databron naar de engine. Een
besluit is geen feit om op te rekenen; zou ze meedoen, dan kon een volgende
uitvoering stil op de uitkomst van een eerder besluit leunen, en dan is niet meer
te zeggen of er gerekend of overgeschreven is.

Welk van de twee een vraag oplevert, staat in het `reduction`-blok van de
lexostatus, en dat verschil is het lezen waard:

| `reduction` | wat de vraag oplevert |
|---|---|
| `chronicle: beschikkingen` | het **vastgelegde besluit** van toen, met de `regulation_valid_from` van toen; geen engine in zicht |
| `regulation: …` + `output: …` | de **rechtstoestand nu**, opnieuw berekend op de wetsversie die op `op_moment` gold |

Allebei zijn ze geldig en allebei zijn ze nodig: de tweede vorm is het lexogram
(*wat zegt het recht over deze feiten*), de eerste het decretogram (*wat is er
besloten*). Ze geven alleen niet hetzelfde antwoord zodra de wet of een feit
verandert, en dat is precies waarom ze naast elkaar bestaan. Een lexostatus die
naar het besluit heet te vragen maar de wetsvorm gebruikt, rekent dus nog steeds
— vandaar dat de naam en de vorm in een wereldbestand bij elkaar horen te passen.

### Wat van RFC-022 §1.2 hier wel en niet in zit

**Wel**: dat een decretogram een engine-uitkomst met een `legal_character` is en
het RFC-013 receipt haar lichaam; dat elk gram elementair is en co-ontstane
uitkomsten samen draagt; het `zaakkenmerk` als de sleutel waaronder de grammen van
één zaak een kroniek vormen; het moment.

**Niet**: de RFC-008-stages (BESLUIT, BEKENDMAKING, BEZWAAR — er is één soort gram
en geen stage-decretogrammen, dus "de huidige stap" bestaat hier niet); `modality`
(`is_intrekking_van`, `is_wijziging_van`); de afgeleide rechtsbeschermingsroute
(§3.3); `decision_type` als open vocabulaire; `extensions`; en de handtekening —
het gram wordt niet ondertekend, want er is geen sleutelmateriaal (zie
[Ondertekening is gesimuleerd](#ondertekening-is-gesimuleerd)).

## Over een celgrens

Een cel bevraagt nooit zelf een andere cel. De vraag gaat langs twee dingen die
een cel *niet* is (RFC-022 §2):

```rust,ignore
let transport = InProcessTransport::over(&cells);
let context = SecurityContext::new(Identity::for_cell("toeslagen"), &transport);
let signed = context.query("brp", "partnerschap", &params, op_moment)?;
```

- **`SecurityContext`** — identiteit, ondertekening, transportkeuze. Gebonden aan
  precies één cel, en de **enige** die het transport aanroept.
- **`CellTransport`** — de naad: `query(cel, lexostatus, params, op_moment)`.
  Exact de vorm van de publieke ingang van een cel en met opzet niets meer; een
  transport dat een reductie of een filter kon meesturen, zou de autonomie van de
  bevraagde cel omzeilen. In-process nu (`InProcessTransport`), HTTP later, en dat
  verschil mag `Cell` geen enkele wijziging kosten.

Wat de context teruggeeft is geen antwoord maar een **bewijsstuk**
(`SignedAnswer`): wie vroeg, ondertekend door welke identiteit, met welke
parameters, en wat de peer antwoordde. Dat is geen gemak. Een cel die een waarde
van een andere cel *accepteert* in plaats van narekent, legt precies dat in haar
decretogram vast (RFC-013 `accepted_values`), en het observatielog wil hetzelfde
weten.

Een vraag aan de eigen cel wordt geweigerd: voor eigen feiten is er een reductie.
Zonder die weigering zou een cel haar eigen kroniek als cross-cel-contact in het
log krijgen en daarmee het vraaggraf vervuilen.

### Accepteren in plaats van narekenen (I5)

Stelt een andere organisatie een feit vast, dan rekent deze cel het niet na. Ze
vraagt het op, legt vast bij wie ze het ophaalde en op welk moment, en rekent
daarmee verder. Dat is stap 4 van de RFC-009-beslisboom, en het is geen nuance: een
beschikking van een ander bevoegd gezag opnieuw uitrekenen is dat gezag overrulen.

Twee dingen kunnen zeggen dat een waarde van een ander komt, en ze lopen langs
dezelfde context, hetzelfde transport en hetzelfde log:

| wie zegt het | waar | wat je schrijft |
|---|---|---|
| de **besluit-definitie** | `inputs` van het besluit | `accept_from` + `lexostatus` + `field` (+ `params`) |
| de **wet** (tier 3) | `source.regulation` die een cel-id noemt | `accepts_from` op de cel: welke lexostatus bij welke uitkomst hoort |

Dat de tweede vorm een afspraak in de celconfiguratie nodig heeft, is geen
omissie. De wet noemt een cel-id en een uitkomstnaam; welke *gepubliceerde
lexostatus* daarbij hoort, is een afspraak tussen twee organisaties en geen recht,
dus ze staat niet in de wet. Diezelfde lijst is wat de engine haar cel-tier geeft:
alleen gedeclareerde cel-ids bereiken de resolver, dus een cel die er niet in staat
kan niet per ongeluk bevraagd worden — invariant I3 als capability. Een afspraak
die géén van de eigen wetten noemt, wordt bij het optuigen geweigerd, en een cel-id
dat een eigen regeling overschaduwt ook.

**Een cel haalt niets zelf op.** Ook op het besluit-pad niet: ze zegt wat ze nodig
heeft (`Cell::acceptance_requests`) en krijgt het aangereikt. Het ophalen gebeurt
in `accept.rs`, buiten `src/cell/`, want een veiligheidscontext en een transport
houdt een cel niet (RFC-022 §2) — en dat is een compileerfout plus een grep-poort
in [`tests/observation_log.rs`](tests/observation_log.rs), geen afspraak.

Wat er met de waarde gebeurt, en vooral wat er níet met haar gebeurt:

- ze gaat als parameter de besluit-engine in, en met bron, naam, moment en
  ondertekening het decretogram in (`InputOrigin::Accepted`; bij tier 3 zet de
  engine haar in `accepted_values` van het receipt);
- ze belandt **niet** in een kroniek en **niet** in het databronregister. Een
  volgend besluit vraagt opnieuw bij de bron. Anders zou er een
  schaduwboekhouding ontstaan die niet van eigen wetenschap te onderscheiden is;
- antwoordt de bron "niets vastgesteld", dan **valt het besluit om** met een
  melding die zegt welke input van welke cel ontbrak, en er wordt niets
  vastgelegd. Doorrekenen met een gat is erger dan geen besluit. De vraag zelf
  staat dan wél als contact vast: ze is gesteld en de peer heeft haar gezien, dus
  ze hoort in het vraaggraf — juist dít geval.

Het zaakkenmerk wordt met opzet ingevuld en gecontroleerd *vóór* de eerste vraag:
een besluit dat om zijn eigen kenmerk niet genomen kan worden, hoort een andere
organisatie niet te laten zien dat er iets over iemand werd opgevraagd.

Mist er een afspraak, dan zegt de melding dat ook. Noemt een wet een naam die de
cel niet laadt en die niet in `accepts_from` staat, dan bereikt de engine de cel
niet en heet dat daar een onbekende regeling — letterlijk waar, en het verkeerde
spoor: wie het leest gaat een corpusbestand zoeken dat er niet hoort te zijn. Het
besluit-pad noemt daarom beide uitwegen (laad de regeling, of leg de lexostatus in
`accepts_from` vast). Dat dit pas bij het besluit blijkt en niet bij het optuigen,
is geen slordigheid: een besluit-definitie mag zo'n input zelf aanleveren, en dan
komt de verwijzing nooit aan bod.

De scenario-runner rekent elk besluit af op de herkomst van zijn waarden — per
waarde `computed` of `accepted`, met bron — en dat is invariant I5 als gate:

```yaml
decide:
  - cell: toeslagen
    besluit: zorgtoeslag_vaststelling
    params: { bsn: '999993653' }
    op_moment: 2024-06-01
    expect_accepted:
      toetsingsinkomen: belastingdienst   # van die cel, en hier niet nagerekend
    expect_computed:
      - is_verzekerde                     # eigen feit, dus eigen werk
```

De gate draait bij élk besluit, ook zonder deze verwachtingen: een geaccepteerde
waarde moet naar een ánder wijzen dan de besluitende cel, en ze mag niet óók als
eigen uitkomst in hetzelfde gram staan — dan is ze alsnog nagerekend en is
"accepteren" een etiket.

De scenario's staan naast elkaar, en dat is de bedoeling:
[`toeslagen_accepteert_toetsingsinkomen.yaml`](scenarios/toeslagen_accepteert_toetsingsinkomen.yaml)
zet één besluit dat accepteert naast één dat hetzelfde getal uit de eigen kroniek
haalt — met twee verschillende cijfers, dus twee verschillende bedragen, want
zonder dat tegenbewijs bewijst het eerste niets.
[`toeslagen_accepteert_via_de_wet.yaml`](scenarios/toeslagen_accepteert_via_de_wet.yaml)
doet hetzelfde langs tier 3, met een testregeling uit
[`fixtures/regulation/`](fixtures/regulation/) in plaats van een corpuswet: geen
enkele echte wet hoort verbouwd te worden omdat een testopstelling iets wil laten
zien.

### Ondertekening is gesimuleerd

De ondertekening is een **placeholder**, en dat staat in de waarde zelf:
`GESIMULEERDE-ONDERTEKENING door cel:toeslagen`. Er zit geen sleutel achter en er
valt niets aan te verifiëren. `Signature::is_simulated()` is daarom waar, en de
test die dat vastlegt wordt rood op de dag dat er echt ondertekend wordt — wat de
bedoeling is. Echte sleutels en trust material (Blauwe Knop/FCID, FSC) zijn buiten
scope. Wat er wél is, is de vorm: een vraag die de grens over gaat draagt de
identiteit die hem stuurde, en het log laat zien dat en door wie er ondertekend is.

### Twee open vragen in de RFC, en wat wij hier kiezen

Beide keuzes zijn een **standpunt in een open vraag**, geen implementatiedetail.
Ze komen niet uit de RFC en horen niet als vaststaand gelezen te worden.

- **Open Question 4 — twee transports, of één mechanisme met twee soorten peer?**
  Wij lezen het als **één mechanisme met twee soorten peer**. Er is één
  `CellTransport`-trait; wie er aan de andere kant staat (een cel of een
  burger-client) is een eigenschap van de peer en niet van het mechanisme. Dat
  standpunt is te falsifiëren: zodra cel↔burger een veld, een methode of een eigen
  trait blijkt te vragen die cel↔cel niet gebruikt, was de andere lezing de juiste.
  Vandaag bestaat alleen de cel↔cel-kant, dus het bewijs is nog niet geleverd.
- **Open Question 2 — waaraan bindt de veiligheidscontext?** Onbeslist in de RFC.
  Hier: één context per cel, met één identiteit die de cel zélf is
  (`cel:toeslagen`). Geen medewerker, geen zaak, geen mandaat, geen autorisatie.
  Dat is de dunste vorm die de vraag openhoudt; komt er een fijnere binding, dan
  krijgt `Identity` velden en verandert er aan de aanroepers niets.

## Het observatielog (buiten de band)

`ObservationLog` legt per cross-cel-vraag vast: de vrager, de bevraagde cel, de
lexostatus, de parameters, het moment, de (gesimuleerde) ondertekening en wat er
terugkwam. Daarmee kan een run het feitelijke vraaggraf *laten zien* in plaats van
beweren dat het klopt.

Het log staat **niet in de RFC**. Die beweert autonomie, maar zegt nergens hoe je
die meet; dit is toegevoegd meetgereedschap. En het hoort nooit een
runtimecomponent te worden, om precies de reden die de opstelling wil bewijzen:
het log ziet álles, dus wie het houdt kent de unie van wat over de grenzen ging —
het totaalbeeld waarvan wij zeggen dat het nergens bestaat.

Drie regels, en de eerste twee zijn afgedwongen in plaats van afgesproken:

1. **Geen productiepad verwijst ernaar.** [`src/observation.rs`](src/observation.rs)
   wordt door geen enkel ander bestand in `src/` geïmporteerd en staat niet in de
   re-exports van de crate-wortel. Een test in
   [`tests/observation_log.rs`](tests/observation_log.rs) grept `src/` en wordt
   rood zodra iemand het toch doet.
2. **`Cell` komt er niet in voor.** Het log leest wat een vraag opleverde; het kan
   geen cel bevragen en geen kroniek bereiken. Ook dat is een grep-poort.
3. **Passief.** `record` geeft niets terug en kan niet falen. Een meetinstrument
   dat een run kan laten struikelen of een beslissing kan beïnvloeden, meet die
   run niet meer.

Het log bewaart het bewijsstuk van de veiligheidscontext ongewijzigd; er komt geen
kopie-met-andere-namen naast. Wat in het log staat, is exact wat over de grens
ging.

Daar hangt een prijs aan, en die hoort er expliciet bij te staan: **volledigheid
is niet afgedwongen.** De veiligheidscontext geeft haar bewijsstuk terug aan wie
vroeg, en of dat bewijsstuk in het log belandt beslist die aanroeper. Vandaag is
dat de scenario-runner, die elk bewijsstuk teruggeeft, dus voor een run klopt het
nu. Dat gold ook toen het besluit-pad ging accepteren: die vragen gaan over
dezelfde grens, dus `World::decide` geeft ze mee (`DecisionRecord::crossings`) en
de runner zet ze in het log. Was dat vergeten, dan was het log stil incompleet
geworden in plaats van rood — de gevaarlijke kant op, want het vraaggraf zou
schoner lijken dan het is.

De invarianten-gate rust hierop, en hij sluit één kant van die volledigheidseis
echt: een decretogram dat zegt een waarde van een andere cel geaccepteerd te
hebben zonder dat er een contact met die cel is vastgelegd, laat het scenario
falen (I2). De andere kant — een contact dat nergens wordt aangereikt — is niet te
meten door wie alleen de aangereikte contacten ziet, en blijft dus structureel.
Zie de sectie hieronder.

## De vijf invarianten, en de gate eronder

**I1 tot en met I5 en het observatielog komen niet uit RFC-022.** Dat hoort hier te
staan voordat er één invariant genoemd wordt, want de verleiding is groot om ze als
RFC-inhoud te lezen: ze staan in de taal van de RFC en ze gaan over wat de RFC
beweert. De RFC beweert autonomie; ze zegt nergens hoe je die meet. Dit is
toegevoegd meetgereedschap — van ons, falsifieerbaar, en te wijzigen zonder dat er
een RFC aan te pas komt.

| | wat de invariant zegt | hoe hij gehandhaafd wordt |
|---|---|---|
| **I1** | geen gedeelde state: een cel bezit haar kroniekstore privé | het **typesysteem**: geen `pub fn store()`, geen publiek veld, geen constructie die een andere cel erbij laat |
| **I2** | volledig waarneembaar: elk cross-cel-contact loopt langs veiligheidscontext → transport → log | structureel (één weg over de grens) plus een gate-toets: een geaccepteerde waarde zónder vastgelegd contact faalt |
| **I3** | een cel bevraagt alleen de cellen die haar eigen wetten of besluit-definities noemen | een **capability** (alleen gedeclareerde cel-ids bereiken de resolver) plus de gate, die het feitelijke gedrag toetst |
| **I4** | synthese gebeurt nooit in een cel | de reduce-engine heeft geen cel-resolver, plus de gate: combineren buiten een besluit faalt, en wat een besluit haalde moet in zijn gram staan |
| **I5** | narekenen versus accepteren | `check_provenance` over elk decretogram, plus `expect_accepted`/`expect_computed` per besluit — zie [Accepteren in plaats van narekenen](#accepteren-in-plaats-van-narekenen-i5) |

De gate leeft in [`src/invariant.rs`](src/invariant.rs) en draait bij **elke** run,
ook bij een scenario dat er niets over zegt. Een invariant die je moet aanzetten,
is een invariant die iemand vergeet.

### Het vraaggraf: gedeclareerd, feitelijk, toegestaan

Drie grafen, en het verschil tussen de derde en de eerste is de hele pointe.

- **gedeclareerd** — `query_graph` in het wereldbestand: welke cel welke andere
  cel mag bevragen, en op welke lexostatus. De lexostatus hoort erbij; "A mag B
  bevragen" is een blanco machtiging, en wat een cel publiceert zijn losse,
  gedocumenteerde namen (RFC-022 §4.1).
- **feitelijk** — afgeleid uit wat er werkelijk over de grenzen ging: de
  bewijsstukken die de veiligheidscontext afgaf, dezelfde regels die het
  observatielog bewaart.
- **toegestaan** — *berekend* uit de celconfiguraties: de `accept_from`-inputs van
  de besluit-definities van een cel, plus haar `accepts_from`-afspraken, waarmee
  een `source.regulation` uit een van haar eigen wetten bij een lexostatus van een
  peer uitkomt.

Wat de gate daarmee doet:

1. **feitelijk versus gedeclareerd, beide kanten op.** Een vraag die niet
   gedeclareerd is faalt, en een gedeclareerde vraag die uitbleef faalt óók. Die
   tweede is de belangrijkste: een niet-gedeclareerde vraag valt op, een vraag die
   stilletjes wegvalt niet. Zonder die kant blijft een scenario waarin het besluit
   verdwijnt groen terwijl het niets meer aantoont.
2. **feitelijk versus toegestaan (I3).** Een gestelde vraag buiten de definities
   faalt — **ook als het scenario haar declareert.** Zou een declaratie hier
   volstaan, dan was elke schending met één regel YAML te legaliseren en hield I3
   niets tegen dan slordigheid.
3. **gedeclareerd versus toegestaan.** Een declaratie waar geen definitie om
   vraagt, faalt op zichzelf, ook als de vraag nooit gesteld wordt. Een toegestaan
   graf dat ruimer is dan het recht van de cellen vraagt, vertelt een lezer iets
   anders dan er waar is — en zodra iemand er een vraag bij schrijft, ziet die
   vraag er gedeclareerd en dus in orde uit.

Elke melding is geschreven voor iemand die het scenario niet kent: welke
invariant, welke actoren, welk moment. Bijvoorbeeld:

```text
[FOUT] invarianten: 1 contact(en) over een celgrens, 1 tak(ken) in het vraaggraf
     I3 (definities): cel 'toeslagen' vroeg 'belastingdienst.toetsingsinkomen' op
     2024-06-01, maar haar eigen wetten en besluit-definities vragen daar niet om
     (haar definities vragen: toeslagen -> brp.partnerschap); een cel bevraagt
     alleen de cellen die haar definities noemen, ook als het scenario de vraag
     toestaat
```

De gate meldt zich ook als hij niets vond. Een gate die alleen bij een fout iets
zegt, is niet te onderscheiden van een gate die niet gedraaid heeft.

### I4: combineren mag, onzichtbaar combineren niet

I4 zegt niet dat een cel nooit twee organisaties mag bevragen. Het zegt dat het
combineren zichtbaar moet zijn. Twee toetsen, twee kanten van dezelfde naad:

- **een cel die buiten een besluit-pad meer dan één cel bevraagt, faalt.** Buiten
  een besluit is er geen gram, dus het totaalbeeld zou daar ontstaan zonder dat
  iemand het kan terugzien.
- **wat een besluit over de grens haalde, moet in zijn decretogram staan.** Een
  contact zonder bijbehorende geaccepteerde waarde is combineren dat het gram niet
  laat zien. En omgekeerd: een geaccepteerde waarde zonder contact betekent dat het
  contact nergens vastligt of dat de waarde ergens anders vandaan kwam — dat is de
  I2-toets uit de tabel hierboven.

Combineren bij een **consument** blijft legitiem (RFC-022 §4.1) en is voor de gate
onzichtbaar, omdat een consument geen celgrens overgaat: hij stelt twee gewone
vragen. Dat is geen gat maar het onderscheid zelf.

### De negatieve fixtures

Een gate die nooit rood wordt, is niet van een ontbrekende gate te onderscheiden.
In [`scenarios/negatief/`](scenarios/negatief/) staan daarom scenariobestanden die
met opzet één invariant schenden en die **moeten** falen:

| bestand | schending |
|---|---|
| `niet_gedeclareerde_call.yaml` | een besluit vraagt over de grens; het scenario declareert die vraag niet |
| `gedeclareerde_call_bleef_uit.yaml` | de declaratie is in orde, maar de vraag wordt nooit gesteld |
| `cel_bevraagt_cel_buiten_haar_definities.yaml` | een cel bevraagt een cel die haar definities niet noemen — en het scenario declareert die vraag wél (I3) |
| `declaratie_buiten_de_definities.yaml` | het vraaggraf staat een vraag toe waar geen enkele definitie om vraagt |
| `reductie_combineert_twee_cellen.yaml` | een cel legt buiten een besluit-pad twee celantwoorden bij elkaar (I4) |

Ze vallen buiten de gewone scenariosuite, want die verwacht van elk bestand dat het
groen is. [`tests/invarianten.rs`](tests/invarianten.rs) draait ze en rekent ze af
op twee dingen: dát ze falen, en waaróp. Dat tweede is het eigenlijke werk — een
fixture die om de verkeerde reden rood staat, bewijst niets over de invariant die
hij zegt te meten. Diezelfde test dwingt af dat elk bestand in die map een regel in
de tabel heeft en omgekeerd, zodat een fixture die niemand draait geen bestand is
dat stil niets doet.

De achterdeur waarlangs de twee gedrags-fixtures hun schending uitdrukken is
`query_via_transport`, de sonde over de naad. In de opstelling zelf kan een cel dit
niet: `Cell::reduce` krijgt nooit een cel-resolver, en over haar grens komt ze
alleen vanuit een besluit. Dat de schendingen alleen via de sonde te schrijven zijn,
is dus goed nieuws — maar een gate die de sonde zou overslaan, zou dat goede nieuws
niet kunnen aantonen, en zou bovendien in elk scenario een weg om I3 heen
openzetten. Daarom wordt de sonde aan dezelfde definities gehouden als een besluit.

Twee toetsen zijn niet als fixture te schrijven, omdat de opstelling ze onmogelijk
maakt: een gram dat accepteert zonder contact, en een contact dat het gram niet laat
zien. `tests/invarianten.rs` meet ze door echte artefacten verkeerd aan elkaar te
knopen — het gram van het ene besluit naast de contacten van het andere — in plaats
van een wereld te bouwen waarin het kan.

### Wat de gate niet doet

- **Hij kan een contact dat nergens wordt aangereikt niet zien.** Volledigheid van
  het meetinstrument is structureel, niet afgedwongen; zie
  [Het observatielog](#het-observatielog-buiten-de-band).
- **De contacten van een omgevallen besluit verdwijnen.** `World::decide` geeft bij
  een fout de contacten niet mee, en een omgevallen besluit breekt de run af. Dat
  is te verdedigen zolang die twee samen opgaan, en het staat als voorwaarde in
  `world.rs` — maar de vragen van een besluit dat niet lukte, staan niet in het
  graf.
- **Hij zegt niets over prestaties en draait niet over het hele corpus.** Dat zijn
  twee aparte vragen; deze gate is een eigenschap van een run.

## De tijdlijn

Tijd is een eigenschap van de **wereld**, niet van een vraag. Eén logische klok
per wereld — een datum, nooit de wandklok — en `World::advance(tot)` zet haar
vooruit:

```rust,ignore
let mut world = scenario.world(&regulation_root())?;   // klok op clock.start
world.advance(NaiveDate::from_ymd_opt(2024, 8, 1)...)?; // triggers gaan onderweg af
world.reduce("toeslagen", "toeslagpartnerschap", &params, moment)?;
```

Vier eigenschappen, en ze hangen samen:

- **De klok woont in de wereld, niet in een cel.** Een cel kent alleen de
  momenten die haar aangereikt worden. Zij houdt geen klok, net zoals ze geen
  sleutels en geen bevoegdheid houdt (RFC-022 §2).
- **`advance` loopt de triggers af in datumvolgorde**, en tijdens een trigger
  staat de klok op het moment van die trigger. Een vastlegging krijgt dus haar
  eigen datum als `op_moment`, niet de eindstand van de sprong. De lus is
  generiek — een gesorteerde lijst van `(datum, trigger)` — dus een soort erbij is
  een variant erbij en geen andere klok. Er zijn er twee: een `fixture` die bij
  het passeren wordt vastgelegd, en een **vervallende verplichting** die de cel
  die haar draagt laat betalen (zie
  [Verplichtingen](#verplichtingen-wat-een-besluit-achterlaat)).
- **Dat de wachtrij oplopend is, is een invariant en geen toestand.** Een besluit
  tijdens de run plant nieuwe vervaldata, en die gaan op datumpositie de rij in.
  Achteraan bijzetten zou een termijn die vóór een al wachtende trigger valt te
  laat of helemaal niet laten afgaan — en met alleen een fixture ná hen in de rij
  is dat niet te zien.
- **Een trigger voegt toe en wijzigt nooit een bestaand gram.** Daarom verandert
  het beeld van een eerder moment niet doordat de wereld verder loopt. Dat is de
  kernassertie, en ze staat als scenario in
  [`scenarios/toeslagen_tijdlijn.yaml`](scenarios/toeslagen_tijdlijn.yaml): een
  feit dat op T2 landt verandert de reductie op T2, terwijl dezelfde vraag over
  T1 exact hetzelfde antwoord blijft geven. Dezelfde vraag staat er twee keer in,
  vóór en ná de vastlegging — zonder dat paar is het geen assertie.
- **Vooruitkijken kan niet, terugkijken wel.** Een reductie op een moment ná de
  klok is een fout: wat na de klok gebeurt heeft nog niets vastgelegd, dus een
  antwoord zou een voorspelling zijn die zich voordoet als een reductie. Een
  moment ervóór levert het beeld van toen. De klok loopt zelf nooit terug.

De dag blijft de fijnste korrel. Twee vastleggingen op dezelfde dag ordent de
tijdas niet; dan beslist de volgorde van vastlegging, en de laatste wint.

Wie een scenario draait, hoeft `advance` niet zelf aan te roepen: de runner loopt
de vragen af in de volgorde van het bestand en zet de klok vooruit tot het moment
van de volgende vraag. Een vraag over een eerder moment kan altijd.

De vragen bepalen daarmee hoe ver de tijd loopt, dus een fixture met een datum
voorbij de laatste vraag gaat in die run niet af. Dat mag — "dit feit landt in
2030 en doet nu dus niet mee" is een geldige bewering — maar het verslag sluit af
met hoeveel vastleggingen bleven wachten, zodat een fixture die niemand ooit
bereikt geen stille regel in het bestand is.

## Het wereldbestand

Een wereld is één YAML-bestand. Het valt in twee helften uiteen, en die scheiding
is de kern van de opzet:

**De wereld zelf** — wat er is, en wat er gedaan kan worden:

| sleutel | wat |
|---|---|
| `clock` | waar de logische klok begint; verplicht |
| `cells` | de organisaties, elk met `laws`, `chronicles`, `lexostatus_definitions`, `besluit_definitions` (met `obligations`) en `accepts_from` |
| `settings` | casusdata die geen wet is, bijvoorbeeld een betalingsritme |
| `fixtures` | de startstand: vastleggingen met een moment |
| `actions` | wat een actor op de tijdlijn kan doen |
| `deadlines` | termijnen die waarschuwen als een feit ontbreekt |

**De stappen van een scenario** — wat er in déze run gebeurt, en wat dat moet
opleveren:

| sleutel | wat |
|---|---|
| `act` | een actor doet een actie, op een moment |
| `decide` | een cel besluit rechtstreeks, op een moment |
| `queries` | een consument bevraagt een cel |
| `query_via_transport` | een cel bevraagt een andere cel (een sonde) |
| `query_graph` | het toegestane vraaggraf: welke cel welke andere mag bevragen |
| `expect_warnings` | de termijnen die deze run moet melden |

Elke stap draagt zijn eigen verwachting. De assertie hoort bij het bestand, niet
bij Rust: een nieuw testgeval is een nieuw bestand.

De eerste helft is ook los te lezen — `WorldDefinition::load` — en dat is wat een
web-laag doet: één wereldbestand, en per sessie een verse `World` eruit. Een
scenariobestand draagt beide helften; de crate leest de eerste eruit met
`Scenario::definition()`.

```yaml
name: korte naam van het scenario
description: waarom dit scenario bestaat        # optioneel

clock:
  start: 2024-01-01                             # waar de klok begint; verplicht

settings:                                       # instellingen van deze wereld
  betalingsritme: kwartaal                      # waar een $naam naar verwijst

cells:
  - id: toeslagen                               # het cel-id
    laws:                                       # regelingen bij $id; de loader
      - wet_op_de_zorgtoeslag                   # laadt alle versies uit de map
      - regeling_standaardpremie

    chronicles:                                 # de eigen feiten van de cel
      - stream: relaties                        # naam van de kroniekstroom
        key: bsn                                # veld waarop gegroepeerd wordt
        events:                                 # mag leeg: zie `fixtures`
          - name: relatie_gewijzigd             # wat er gebeurde
            intake: levering                    # waarlangs het binnenkwam
            recording_actor: toeslagen          # wie vastlegde: de cel zelf
            grondslag: melding uit de brp       # op welke grondslag; mag leeg
            op_moment: 2023-01-01               # wanneer dit feit feit werd
            fields:                             # veldnamen = input-namen in de wet
              bsn: '999993653'
              partnerschap_type: HUWELIJK

    lexostatus_definitions:                     # wat de cel publiceert
      - name: toeslagpartnerschap
        doc: vrije toelichting                  # optioneel
        inputs:                                 # de gedocumenteerde parameters
          - name: bsn
            type: string                        # string | number | boolean
        outputs:                                # wat het antwoord mag dragen;
          - heeft_toeslagpartner                # leeg = alleen reduction.output
        reduction:                              # hoe de cel reduceert
          regulation: algemene_wet_inkomensafhankelijke_regelingen
          output: heeft_toeslagpartner
          parameters:
            bsn: $bsn                           # $naam verwijst naar een input;
                                                # alles zonder $ is letterlijk

      - name: partnerschap                      # een cel zonder wetten kan dit
        inputs:                                 # ook, zie Een bron-cel
          - name: bsn
            type: string
        outputs:                                # verplicht bij een kroniekfilter
          - partnerschap_type
        reduction:
          chronicle: relaties                   # een eigen stroom
          key: bsn                              # sleutel = naam van een input
          latest: true                          # de laatste vastlegging; mag weg

      - name: betaald_tot_nu_toe                # de som over een eigen stroom
        inputs:
          - name: zaakkenmerk
            type: string
        outputs:                                # precies het gesommeerde veld
          - bedrag
        reduction:
          chronicle: betalingen
          key: zaakkenmerk
          sum: bedrag                           # in plaats van `latest`

    besluit_definitions:                        # wat de cel kan besluiten
      - name: zorgtoeslag_vaststelling
        doc: vrije toelichting                  # optioneel
        regulation: wet_op_de_zorgtoeslag       # een eigen regeling
        output: heeft_recht_op_zorgtoeslag      # de uitkomst die het besluit ís
        outputs:                                # wat er in hetzelfde gram mee gaat
          - hoogte_zorgtoeslag
        zaakkenmerk: 'zorgtoeslag/{bsn}'        # {naam} = een gedocumenteerde
                                                # parameter; minstens één
        params:                                 # de gedocumenteerde parameters
          - name: bsn
            type: string
          - name: jaar
            type: string
        inputs:                                 # wat de cel de engine aanlevert
          bsn:
            param: bsn                          # uit de parameters van het besluit
          is_verzekerde:
            from_chronicle: inkomensleveringen  # uit een eigen stroom: de laatste
            field: is_verzekerde                # vastlegging op of vóór het moment
          toetsingsinkomen:                     # geaccepteerd van een andere cel
            accept_from: belastingdienst        # de cel die het vaststelt
            lexostatus: toetsingsinkomen        # de naam die zij publiceert
            field: toetsingsinkomen             # de uitkomst daarvan
            params:
              bsn: $bsn                         # $naam = parameter van dit besluit
        obligations:                            # wat er betaald moet worden
          - amount: $hoogte_zorgtoeslag         # een uitkomst van dit besluit
            payer: belastingdienst              # de cel die de verplichting draagt
            schedule: $betalingsritme           # ineens | kwartaal | maand, of
                                                # een $instelling
            from: '{jaar}-02-01'                # optioneel; standaard het moment
                                                # van het besluit

    accepts_from:                               # wat de wétten bij een cel halen
      - cell: brp                               # het cel-id uit source.regulation
        output: partnerschap                    # de uitkomst die de wet vraagt
        lexostatus: partnerschap                # wat daar gevraagd moet worden
        field: partnerschap_type                # en welke uitkomst de waarde is

fixtures:                                       # de startstand van de wereld
  - at: 2024-07-01                              # het moment van de vastlegging
    record:
      cell: toeslagen                           # bij wie het gram landt, en
      chronicle: relaties                       # in welke stroom
      name: relatie_gewijzigd
      intake: levering
      grondslag: melding uit de brp
      fields:
        bsn: '999993653'
        partnerschap_type: GEEN

actions:                                        # wat een actor kan doen
  - id: burger.aanvraag                         # waarmee de actie aangeroepen wordt
    actor: burger                               # de cel die haar doet
    label: Aanvraag indienen                    # wat een lezer ziet; casusdata
    doc: vrije toelichting                      # optioneel
    records:                                    # óf `records`, óf `decides`
      cell: burger                              # bij wie het gram landt
      chronicle: aanvragen                      # in welke stroom
      name: aanvraag_ingediend                  # hoe het gram heet
      intake: aanvraag                          # waarlangs het binnenkomt
      grondslag: Awir art. 15                   # optioneel
      fields:                                   # het formulier van de actie
        - name: bsn
          type: string                          # string | number | boolean
        - name: jaar
          type: number
      delivers_to:                              # optioneel: hetzelfde feit óók
        cell: toeslagen                         # bij de ontvanger
        chronicle: aanvragen
        intake: aanvraag                        # standaard `levering`
        name: aanvraag_ontvangen                # standaard dezelfde naam

  - id: toeslagen.toekenning
    actor: toeslagen
    label: Beslis op de aanvraag
    decides:                                    # start het besluit-pad van een cel
      cell: toeslagen
      besluit: zorgtoeslag_vaststelling         # het formulier is dat van dit
                                                # besluit (zijn `params`)
    available_when:                             # optioneel: pas als het verhaal
      cell: toeslagen                           # zover is
      chronicle: aanvragen
      field: jaar
      equals: 2024

deadlines:                                      # termijnen die waarschuwen
  - label: aanvraag ontvangen vóór 1 maart      # wat een lezer ziet; casusdata
    at: 2024-03-01                              # de dag waarop de termijn verstrijkt
    warn_if_missing:                            # het feit dat er dan hoort te liggen
      cell: toeslagen
      chronicle: aanvragen
      name: aanvraag_ontvangen

act:                                            # acties, elk op een moment
  - description: vrije omschrijving             # optioneel
    action: burger.aanvraag                     # de actie hierboven
    values:                                     # het ingevulde formulier
      bsn: '999993653'
      jaar: 2024
    op_moment: 2024-01-10                       # de klok gaat hier eerst naartoe
    expect:                                     # optioneel, bij een `decides`-actie
      heeft_recht_op_zorgtoeslag: true
    expect_accepted:                            # optioneel: herkomst per waarde
      toetsingsinkomen: belastingdienst
    expect_computed:                            # en wat hier wél vastgesteld is
      - is_verzekerde

query_graph:                                    # het toegestane vraaggraf
  - doc: waarom deze vraag mag                  # optioneel
    from: toeslagen                             # de cel die mag vragen
    to: belastingdienst                         # de cel aan wie
    lexostatus: toetsingsinkomen                # en waarover

decide:                                         # besluiten, elk op een moment
  - description: vrije omschrijving             # optioneel
    cell: toeslagen                             # de cel die besluit
    besluit: zorgtoeslag_vaststelling           # de definitie hierboven
    params:
      bsn: '999993653'
    op_moment: 2024-06-01                       # bepaalt welke feiten de cel
                                                # kent én welke wetsversie geldt
    expect:                                     # optioneel: wat het gram draagt
      heeft_recht_op_zorgtoeslag: true
    expect_accepted:                            # optioneel: herkomst per waarde
      toetsingsinkomen: belastingdienst         # van die cel, niet hier berekend
    expect_computed:                            # en wat hier wél vastgesteld is
      - is_verzekerde

queries:
  - description: vrije omschrijving             # optioneel
    cell: toeslagen
    lexostatus: toeslagpartnerschap
    params:
      bsn: '999993653'
    op_moment: 2025-01-01
    expect:                                     # wat het antwoord moet bevatten
      heeft_toeslagpartner: false               # uitkomsten die hier niet staan,
                                                # worden niet gecontroleerd

  - cell: toeslagen
    lexostatus: partnerschap
    params:
      bsn: '999993653'
    op_moment: 2020-01-01
    expect_not_established: true                # verwacht dat er op dit moment
                                                # niets vastgesteld was

query_via_transport:                            # vragen over een celgrens
  - description: vrije omschrijving             # optioneel
    from: toeslagen                             # de vragende cel; haar
                                                # veiligheidscontext ondertekent
    cell: brp                                   # de bevraagde cel
    lexostatus: partnerschap
    params:
      bsn: '999993653'
    op_moment: 2024-01-01
    expect:                                     # zelfde verwachtingen als bij
      partnerschap_type: HUWELIJK               # een gewone vraag

expect_warnings:                                # de termijnen die deze run miste
  - aanvraag ontvangen vóór 1 maart             # leeg (of afwezig) = geen enkele
```

`query_via_transport` is een **sonde, geen onderdeel van de opstelling**. In de
opstelling stelt een cel zo'n vraag uitsluitend vanuit haar besluit-pad: ze heeft
een input nodig die een andere organisatie vaststelt, en dan *accepteert* ze die —
zie [Accepteren in plaats van narekenen](#accepteren-in-plaats-van-narekenen-i5).
Wat deze stap overhoudt, is één vraag los kunnen stellen en op haar antwoord
asserteren zonder er een besluit omheen te bouwen: handig om de naad zelf te
beproeven, en verder niets.

Wat een sonde **niet** overslaat, is invariant I3. De gate houdt haar aan dezelfde
definities als een besluit, dus de vragende cel heeft een reden nodig om te vragen:
een eigen wet die de peer via `source.regulation` aanwijst, plus de
`accepts_from`-afspraak die zegt onder welke gepubliceerde naam de waarde daar te
halen is. Zonder die twee faalt het scenario, ook als de tak in `query_graph`
staat. Dat is met opzet — een sonde die buiten I3 viel, zou in elk scenario een weg
om de invariant heen openzetten — maar het betekent dat een nieuwe sonde niet
alleen een identiteit vraagt. Zie
[De vijf invarianten](#de-vijf-invarianten-en-de-gate-eronder).

De stappen lopen in deze volgorde: eerst de acties, dan de besluiten, dan de
vragen van een consument, dan die over een celgrens. Dat past bij wat ze zijn — een
actie en een besluit zijn gebeurtenissen op de tijdlijn, een vraag kijkt erop
terug — en het maakt niets onmogelijk: een vraag over een moment *vóór* een besluit
levert nog steeds het beeld van toen, want de reductie filtert zelf op
`op_moment`. De klok gaat vóór elke stap vooruit tot haar moment, zodat een
levering of een vervallen termijn die ertussen valt eerst landt.

Een wereldbestand gebruikt in de praktijk `act` óf `decide`. Mengen kan, en dan
gaan de acties voor; wie een moment kiest dat vóór de klok ligt, krijgt een fout
("de klok loopt niet terug") en geen stilte.

Een `act` en een `decide` mogen zonder `expect`, anders dan een vraag: ze leggen
iets vast, en bewijzen daarmee ook zonder verwachting iets — namelijk dat de
vragen erna iets te vinden hebben.

`query_graph` mag weg, en dan zegt het bestand iets: **er gaat niets over een
celgrens.** Zie [De vijf invarianten](#de-vijf-invarianten-en-de-gate-eronder) voor
wat ermee gebeurt.

Onbekende velden worden geweigerd, zodat een typfout niet stil verdwijnt. Elke
vraag heeft minstens één verwachting: een vraag zonder `expect` en zonder
`expect_not_established` slaagt altijd en zou als `ok` in het verslag komen, wat
op bewijs lijkt en het niet is. De loader weigert zo'n scenario, en ook een vraag
die beide verwachtingen tegelijk stelt — die kan nooit uitkomen. `clock.start` is
verplicht: een wereld zonder startmoment zou op de wandklok moeten terugvallen,
en dan is dezelfde run morgen een andere run.

### De executogram-vorm

Een vastlegging is een executogram, en die draagt de vorm van RFC-022 §1.3. Vier
vragen moeten per gram beantwoord zijn, plus één:

| veld | vraag |
|---|---|
| `name` + `fields` | **wat** is er vastgelegd |
| `recording_actor` | **door wie** — het cel-id; de celbeheerder, niet de `competent_authority`, want een executogram is een feit en geen besluit |
| `grondslag` | op **welke grondslag** — vrije tekst, mag leeg |
| `op_moment` | op **welk moment** |
| `intake` | **waarlangs** het de cel bereikte: `aanvraag`, `levering`, `betaling` of `eigen_besluit` |

Een vastlegging die alleen een veldwaarde en een datum draagt, laat de helft van
die vragen onbeantwoord — en zonder `intake` en `grondslag` valt van een feit in
een kroniek niet meer te zeggen hoe het daar kwam.

`intake` is hier een enum en geen vrije tekst, terwijl RFC-022 het vocabulaire
open houdt. Dat is een bewuste afwijking: een typfout in een kanaalnaam mag niet
stil doorgaan. Een kanaal erbij is één regel Rust — het is platformvocabulaire,
geen casusdata. De vier namen zijn ook niet die van het voorbeeld in de RFC
(`external_intake`): dit zijn de kanalen waarlangs een cel iets *overkomt*.
Aansluiten op een breder vocabulaire is later een hernoeming, geen herontwerp.

`recording_actor` moet de cel zijn die de stroom houdt; een vastlegging op naam
van een ander wordt geweigerd bij het optuigen. Zie [Wat een cel
is](#wat-een-cel-is): een kroniek is het eigen journaal van de cel.

In een `fixture` staat `recording_actor` niet, want dat is `record.cell`, en `at`
geeft het moment. Die twee kunnen niet uiteenlopen en hoeven dus niet twee keer
opgeschreven.

### Startstand: `fixtures`

Een startstand is data, geen opbouwcode. Een fixture is een vastlegging met een
moment:

- **op of vóór `clock.start`** — staat bij het optuigen al in de kroniek; de klok
  staat op `start`, dus wat toen al gebeurd was, is gebeurd;
- **erna** — is een trigger, en landt zodra `advance` die datum passeert.

Een fixture wordt bij het optuigen getoetst zoals ze bij het vastleggen getoetst
wordt: een onbekende cel, een onbekende stroom of een ontbrekend sleutelveld
faalt daar, ook als haar datum nog jaren weg is. Een typfout in een startstand
hoort niet halverwege een tijdlijn op te duiken, en of dat gebeurt mag niet
afhangen van hoe ver die datum weg ligt.

Eenzelfde feit kan dus twee kanten op geschreven worden — als `events` in de
celconfiguratie of als `fixture` met een `at` — en dat is geen dubbelop. Het
eerste is wat de cel al bijhield toen de wereld begon, het tweede is wat er
tijdens de run gebeurt.

### Acties: wat een actor kan doen

Een actie is de aansturing van de wereld, en ze is **data**. `World::act(id,
waarden)` voert er één uit, op de stand van de klok — een actie draagt geen eigen
moment, want dat is wat haar van een `fixture` onderscheidt: een startstand staat
op een datum, een actor doet iets op het moment dat de wereld staat.

Twee vormen, en precies één per actie:

- **`records`** legt een executogram vast in een eigen kroniek van de actor. Het
  formulier (`fields`) zegt wat de actor invult en van welk type; de waarden
  worden de velden van het gram. Staat er `delivers_to`, dan landt hetzelfde feit
  óók bij de ontvanger — als **tweede gram in zijn eigen kroniek**, op zijn eigen
  naam, met zijn eigen kanaal. Dat is de eerlijke vorm van een aanvraag: de
  aanvrager weet wat ze indiende, de ontvanger weet wat hem geleverd is, en geen
  van beide leest de kroniek van de ander. Aan jezelf leveren wordt geweigerd —
  dat zou hetzelfde gram twee keer in dezelfde kroniek zetten.
- **`decides`** start het besluit-pad van een cel. Het formulier is dan **dat van
  het besluit**: de `params` die de besluit-definitie al documenteert. Een tweede
  lijst in de actie zou daarvan gaan afwijken.

`available_when` is één simpele voorwaarde: *er ligt in kroniek X van cel Y een
feit waarin veld Z de waarde W heeft.* Daarmee kan een actie wachten tot het
verhaal zover is — beslissen pas als er een aanvraag ligt — zonder dat die
volgorde in Rust komt te staan. Het is met opzet geen tweede reductietaal: er komt
geen waarde naar buiten, alleen ja of nee. Kan een actie nu niet, dan is dat een
leesbare weigering die zegt wát er nog niet ligt, en het beeld van de wereld toont
haar met diezelfde reden erbij.

Een actie ontsnapt niet aan de invarianten. Lokt ze een besluit uit dat een waarde
van een andere cel accepteert, dan gaat dat contact over een celgrens en hoort de
tak in `query_graph` te staan — precies zoals bij een `decide`. De gate leest álle
besluiten van een run, of ze door een actie zijn uitgelokt of rechtstreeks genomen:
zouden die twee uit elkaar vallen, dan zou een actie een weg om I3 heen openen.
Zie [De vijf invarianten](#de-vijf-invarianten-en-de-gate-eronder).

Alles wat een actie belooft, wordt bij het optuigen getoetst: bestaat de actor,
bestaat de cel, houdt ze de stroom, kan die stroom haar sleutelveld uit het
formulier krijgen, bestaat het besluit, en gaat de voorwaarde over een veld dat
bestaat. Een actie die pas bij de eerste klik omvalt, is een typfout die op het
verkeerde moment boven water komt. De stroom met decretogrammen
(`beschikkingen`) is geen doel voor een actie: daar ontstaat een gram door te
besluiten.

### Termijnen: waarschuwen zonder te blokkeren

Een `deadline` zegt wanneer een feit er hoort te liggen. Passeert de klok die dag
en ligt het er niet, dan komt er een **waarschuwing** naast de wereld — en verder
niets. De aanvraag kan alsnog binnenkomen, en ze landt gewoon.

Dat is een standpunt en geen gemak. De wet zegt wat de termijn was, niet dat er
daarna niets meer mag; wat er met een te late aanvraag gebeurt, is een besluit van
het bestuursorgaan en geen eigenschap van de simulator. Een termijn die de actie
tegenhoudt, zou de opstelling laten beweren dat een te late aanvraag niet bestaat.

De waarschuwing valt op het moment dat de termijn passeert, en niet aan het eind
van een run. Dat moet ook: een feit dat er een dag later wél ligt, lag er op de
termijn niet, en een berekening achteraf zou nooit waarschuwen. `at` is daarom een
vaste datum en geen sjabloon over `settings` — de termijnen staan bij het optuigen
in de wachtrij, en een instelling die later wijzigt zou een termijn moeten
verplaatsen die misschien al gepasseerd is.

Een scenario rekent erop af met `expect_warnings`. Die lijst wordt **altijd**
vergeleken, ook als hij niet in het bestand staat: dan is de verwachting "geen
enkele". Een waarschuwing die niemand verwachtte, hoort een run te laten falen in
plaats van stil in het verslag te belanden.

### De wereld besturen

Vijf ingangen, en samen zijn ze wat een web-laag nodig heeft:

| aanroep | wat |
|---|---|
| `World::from_definition(&definition, corpus)` | tuig een wereld op uit een wereldbestand |
| `World::act(id, waarden)` | voer een actie uit op de stand van de klok |
| `World::advance(tot)` | zet de klok vooruit en laat de triggers onderweg afgaan |
| `World::update_settings(wijzigingen)` | wijzig de instellingen |
| `World::reset()` | terug naar de startstand |
| `World::snapshot()` | het beeld van alles wat er staat |

`act` en `advance` leveren `Events`: welke grammen erbij kwamen, welke besluiten
genomen zijn en welke termijnen verstreken. Dat is geen tweede waarheid naast de
kronieken — alles erin ligt óók in de cel waar het hoort — maar het verslag van
één stap.

`reset` is **opnieuw beginnen**, niet terugdraaien. Dat verschil is de hele reden
dat het kan: een kroniek groeit en wijzigt nooit, dus "terug" bestaat niet; wat wél
bestaat is een verse wereld uit hetzelfde bestand. Ook de instellingen gaan terug
naar wat het bestand zegt — wie ze wijzigde, wijzigde de wereld en niet het
bestand.

### Instellingen komen vast te staan

`update_settings` weigert een instelling te wijzigen die **al door een besluit
gebruikt is**. Een besluit legt vast waarop besloten is; een ritme dat er achteraf
onder vandaan geschoven wordt, laat het gram iets anders zeggen dan er gebeurd is,
en dan is een decretogram niet meer terug te lezen. Wie het toch wil wijzigen,
begint een nieuwe wereld.

Wat "gebruikt" betekent, komt uit de besluit-definitie en niet uit het gram: het
gram draagt het uitgerekende schema, en daaruit is niet meer te zien of het ritme
uit een instelling kwam of letterlijk in de definitie stond. Welke instellingen
vaststaan, staat in het beeld van de wereld (`locked_settings`) — zodat een lezer
kan zien welke knop nog om kan zonder het te hoeven proberen.

Verder is `update_settings` alles-of-niets, en het toetst de nieuwe stand met
dezelfde controle als bij het optuigen: een instelling die geen ritme is, valt hier
en niet bij het eerstvolgende besluit dat erop leunt.

### Het beeld van de wereld

`World::snapshot()` levert één doorsnede van alles wat er staat, en dat is het
**contract naar een frontend** (`serde`-`Serialize`; `tests/fixtures/snapshot.json`
is het vastgepinde voorbeeld):

| sleutel | wat |
|---|---|
| `clock` | waar de logische klok staat |
| `settings` + `locked_settings` | wat geldt, en wat vast staat en waardoor |
| `cells` | per cel haar `laws`, wat ze publiceert, wat ze kan besluiten, en haar kronieken |
| `cells[].chronicles[].grams` | elk gram met zijn soort (`lexogram`/`decretogram`/`executogram`), moment, kanaal, grondslag en velden |
| `…grams[].fields[].origin` | de herkomst per waarde |
| `actions` | elke actie met haar formulier, en of ze nu kan |
| `crossings` | wat er over een celgrens ging |
| `warnings` | de termijnen die verstreken zonder dat het feit er lag |

Drie dingen om bij stil te staan:

**Geen casusnamen.** Er staat geen naam in het contract die bij één casus hoort.
Elk label komt uit het wereldbestand, dus een andere casus is een ander bestand en
geen andere frontend.

**Herkomst per waarde.** Bij een executogram is de herkomst de vastlegging zelf:
langs welk kanaal, op welke grondslag. Bij een decretogram valt ze uiteen, en dat
is het hele punt van invariant I5 — een input die van een andere cel
**geaccepteerd** is, draagt bron, lexostatus, moment en ondertekening; een uitkomst
draagt de regeling die haar berekende; een vast veld van het gram draagt dat het
dat is. Een geaccepteerde waarde hoort niet op een berekende te lijken. Een gram
dat een cel zelf vastlegde over haar eigen vaststelling — een bron-cel zonder
engine — draagt geen receipt, en dan is de herkomst van elke waarde erin de
vastlegging, want er heeft geen uitvoering gedraaid.

**Het receipt gaat niet mee.** Een decretogram draagt het volledige RFC-013
Execution Receipt, en dat draagt wandkloktijd. Een beeld dat per run verschilt is
geen contract, dus het receipt blijft in de kroniek waar het hoort; wat een lezer
eraan had, is de herkomst hierboven. Het `lexogram` in de lijst met gram-soorten
komt in geen enkele kroniek voor: de wet is generiek en van niemand in bijzonder,
en welk recht een cel laadt staat in `cells[].laws`. De variant staat er zodat een
lezer één vocabulaire voor alle drie de grammen heeft.

Het beeld is een **inspectiebeeld**, net zoals het observatielog een meetinstrument
is. De wereld bezit de cellen en zij maakt het; een cel kan het niet opvragen en
kan er dus niet de kroniek van een ander mee lezen. `crossings` is het materiaal
van dat log, en een lezer hoort het als zodanig te labelen: wie deze lijst houdt,
kent de unie van wat over de grenzen ging — precies het totaalbeeld waarvan geen
enkele cel er een heeft.

### Kroniekstromen en tijd

Per stroom geldt: alleen vastleggingen met `op_moment <= ` het gevraagde moment
tellen mee, en van de rest wint per sleutelwaarde en per veld de laatste
vastlegging. Dit is de weg naar de engine; een kroniekfilter kiest één hele
vastlegging en merge't niets (zie
[Twee reductievormen](#twee-reductievormen)). Een vraag over een moment in het
verleden levert dus het beeld van toen. De stromen worden aan de engine
aangeboden als databronnen, waar ze de inputs van de eigen regelingen invullen.
Twee stromen met dezelfde naam worden geweigerd: de stroomnaam is tevens de naam
van de databron, dus daar zou de tweede de eerste stil schaduwen.

Dat "per veld wint de laatste vastlegging" is een **bewuste vereenvoudiging**,
geen eigenschap om trots op te zijn. Het is een toestandsmerge: de velden van
verschillende vastleggingen, op verschillende momenten, worden over elkaar
gelegd tot één record dat als vastlegging nooit bestaan heeft. Dat gebeurt omdat
de engine records als databron wil. De paper legt de nadruk op het omgekeerde —
niet de resulterende toestand opslaan, maar de procesrelatieve vaststelling ("op
moment T heeft actor X vastgesteld dat …") en bij hergebruik expliciet
herinterpreteren. Wat samen ontstond hoort samen te blijven; wat apart ontstond
hoort niet stil samengevoegd te worden. De vastlegging draagt inmiddels wél
`recording_actor`, `grondslag`, `intake`, een naam en een moment (zie [De
executogram-vorm](#de-executogram-vorm)); wat nog mist is een reductie die
daarover filtert en aggregeert in plaats van alleen te overschrijven. De
gegevens zijn er dus al voordat de reductie ze gebruikt.

Het kroniekfilter van een bron-cel doet dat al niet: dat kiest één vastlegging en
geeft die in haar geheel terug. Die vorm kan hier omdat er geen engine tussen zit
die records wil.

De dag is de fijnste korrel van de tijdas. Twee vastleggingen op hetzelfde
`op_moment` vallen daar niet uit elkaar te houden; dan beslist de volgorde in het
bestand, en de laatste wint. Wie ze wél wil ordenen heeft een fijnere tijdas
nodig, geen andere schrijfvolgorde.

Feiten die in de echte wereld van een andere organisatie komen, staan hier als
binnengekomen feit in de eigen kroniek. Dat blijft zo, ook nu er een transport is:
een **reductie** kan geen andere cel bereiken, want de cel houdt geen transport en
`Cell::reduce` heeft er geen weg naartoe. Over de grens gaat alleen wat langs de
veiligheidscontext gaat, en die wordt vandaag alleen door een scenario-stap
aangeroepen (zie [Over een celgrens](#over-een-celgrens)).
Vraag je een moment op waarop een binnengekomen feit nog niet vastlag, dan faalt
de reductie: bij een input met een `source` naar een regeling die deze cel niet
laadt met "Law not found", en anders met "Variable not found". Beide zeggen
hetzelfde — de cel reikt niet buiten zichzelf, en levert dus geen antwoord in
plaats van een geraden antwoord.

## Een scenario draaien

```bash
# los, met verslag op de terminal; exitcode 1 als een verwachting niet uitkwam.
# Zonder argument draait het scenario hieronder.
just simulate packages/simulator/scenarios/toeslagen_zorgtoeslag.yaml

# als test: draait elk bestand in scenarios/ en controleert alle verwachtingen
# (zit ook in `just test`)
cd packages && cargo test -p regelrecht-simulator
```

Het corpus wordt standaard naast de crate gezocht (`corpus/regulation`);
`REGULATION_PATH` overschrijft dat. Vindt een cel haar regeling daar niet, dan
wordt nog in [`fixtures/regulation/`](fixtures/regulation/) gekeken: een handvol
**testregelingen** die een eigenschap van de opstelling aantonen en geen recht
weergeven. Het corpus gaat voor, dus een fixture kan nooit een echte regeling
overschaduwen.

## Wat hier nog niet staat

**Een verplichting kent geen rente, verrekening of terugvordering.** Een termijn
vervalt en wordt betaald; wat er gebeurt als er te laat, te veel of niet betaald
wordt, staat er niet. Een terugvordering is in deze opzet een gewoon besluit met
een eigen verplichting, en dat is nog nergens uitgewerkt. Een verplichting kan ook
niet gewijzigd of ingetrokken worden: het schema staat in het gram, en een gram
verandert niet.

**Een voorwaarde op een actie is één gelijkheid.** `available_when` kijkt naar één
veld in één kroniek van één cel. Er is geen "en", geen "of", geen "ligt er iets"
zonder waarde, en geen voorwaarde over een gram-naam. Wat er nu kan, is genoeg voor
"het verhaal is zover"; een voorwaarde die meer nodig heeft, is een aanwijzing dat
het wereldbestand een stap mist.

**Een gemiste termijn heeft geen gevolg in de wereld.** Ze komt in de lijst met
waarschuwingen en verder niets: geen gram, geen verval van een recht, geen
herinnering die op een termijn afgaat. Wat een bestuursorgaan met een te late
aanvraag doet, is een besluit en dus een besluit-definitie.

**Onbetrouwbaar gedrag tussen cellen bestaat niet.** Een bron-cel is er altijd, ze
antwoordt meteen, en haar antwoord is nooit verouderd of in tegenspraak met dat van
een ander. Accepteren is dus alleen uitgewerkt voor het geval dat goed gaat plus het
geval "niets vastgesteld"; het moeilijkste deel van elk decentraal systeem staat
nog open. Zo ook **echte ondertekening** en de RFC-009-modi als configuratie: de
ondertekening in een geaccepteerde waarde is de placeholder uit
[Ondertekening is gesimuleerd](#ondertekening-is-gesimuleerd).

Aan de kant van de celgrens ontbreekt verder **autorisatie**: de
veiligheidscontext kent identiteit en ondertekening, en beslist nog niets over wat
mag. De invarianten-gate draait wel — zie
[De vijf invarianten](#de-vijf-invarianten-en-de-gate-eronder) voor wat hij toetst
en, even belangrijk, wat hij niet kan zien.

Ook een **HTTP-transport** is er niet; dat is het punt van de trait. Komt het er,
dan is dat een tweede implementatie naast `InProcessTransport` en geen wijziging in
`Cell` — en als dat laatste wél nodig blijkt, was de naad op de verkeerde plek
gelegd. Asynchrone intake met een echte tijdlijn volgt apart. De indeling
anticipeert erop: de reductielogica woont in [`src/cell/`](src/cell/) en niet in de
scenario-runner, zodat een latere `packages/cell` een verplaatsing is en geen
herschrijving.
