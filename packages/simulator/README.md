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
oplegt en tussen wie, staat in de regeling die het uitvoert; welke cel ze nakomt,
in het wereldbestand — en de klok komt ze later na:
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
  besluiten waarvoor een ander bevoegd is. Ook wie de cel *zegt te zijn* — haar
  **identiteit** — houdt ze niet zelf: die bewering is wat een ondertekening
  straks moet bewijzen, en woont daarom in de veiligheidscontext. Ze komt bij
  een besluit van buiten mee en wordt dan naast de wet gelegd: zie
  [Wie mag besluiten](#wie-mag-besluiten).
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

Een gedocumenteerde parameter heeft een van vier typen: `string`, `number`,
`boolean` of `date`. Ze gelden overal waar een parameter gedocumenteerd staat —
een lexostatus, een besluit, het formulier van een actie — want het is dezelfde
belofte, en hij wordt op één plek gebonden (`ParameterType::bind`).

`date` is de enige die meer doet dan naar het soort van de waarde kijken. Op de
draad is een datum een string in ISO-notatie (`jjjj-mm-dd`), en dat blijft zo:
het is de notatie die de engine leest en die een kroniek vastlegt. Wat het type
toevoegt is de **toets** — de waarde gaat bij het binden door `NaiveDate`, dus
`01-12-2026` en `2024-02-30` lopen hier stuk, met een melding die zegt welke
notatie wél gelezen wordt. Ook `2026-1-1` loopt stuk, al leest `NaiveDate` die
vorm gewoon: de engine eist verderop de canonieke vorm met nullen vooraan, omdat
een niet-canonieke datum onder `>`/`<` chronologisch vergelijkt en onder `EQUALS`
op de tekst. Zonder dat type is een datum gewoon tekst en valt
dezelfde fout drie lagen verderop in een regeling die er een datum van probeert
te maken, in het Engels van de engine en zonder dat er nog iets van het
formulier bekend is. De Nederlandse schrijfwijze blijft iets van het scherm: de
frontend toont dd-mm-jjjj en stuurt ISO.

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

Beide vormen leggen naast hun uitkomst vast **hoe** ze eraan kwamen; zie
[Hoe het antwoord tot stand kwam](#hoe-het-antwoord-tot-stand-kwam).

### Hoe het antwoord tot stand kwam

Elk antwoord draagt een blok `reductie`: welke gegevens de cel gelezen heeft en
hoe ze die tot dit ene antwoord heeft teruggebracht. Dat kan hier en alleen hier
— een reductie leest uitsluitend de eigen kronieken van de eigen cel (RFC-022
§4.1), dus de cel kan precies zeggen wát ze las. Een consument die dit naast de
uitkomst legt, loopt de reductie na zonder de cel binnen te gaan.

```json
"reductie": {
  "vorm": {
    "soort": "kroniekfilter",
    "chronicle": "beschikkingen",
    "key": "zaakkenmerk",
    "key_value": "zorgtoeslag/999993653",
    "where": {},
    "regel": {"regel": "laatste"},
    "op_moment": "2026-12-01"
  },
  "grammen": [
    {
      "cell": "toeslagen",
      "chronicle": "beschikkingen",
      "id": "toeslagen|beschikkingen|0",
      "kind": "decretogram",
      "name": "zorgtoeslag_toekenning",
      "volgnummer": 0,
      "op_moment": "2026-04-01",
      "regulation_valid_from": "2026-01-01",
      "bijdrage": null
    }
  ]
}
```

Wat er per vorm in staat:

- **kroniekfilter**: de stroom, de sleutel met de waarde uit de vraag, de `where`
  en de regel (`laatste`, of `som` met het gesommeerde veld), plus elk gelezen
  gram. Bij een som staat elk meegeteld gram erin met zijn `bijdrage`: een som
  waarvan alleen de uitkomst te zien is, valt niet na te rekenen.
- **wetsvorm**: de regeling met de **versie** die op dat moment gold
  (`regulation_valid_from`), de gevraagde uitkomst, en per input van die
  uitvoering waar hij vandaan kwam — uit de vraag (`parameter`), uit een eigen
  kroniek (met het gram dat hem droeg), of berekend door een andere regeling die
  de cel zelf laadt. De grammen staan er net zo goed bij: welk gram van een
  kroniekstroom de engine te zien kreeg, weet zij niet — zij ziet per onderwerp
  één record, want de tijdreductie is er dan al overheen gegaan.
- **niets vastgesteld** is geen derde vorm maar hetzelfde kroniekfilter zonder
  één gram, met `gemist` erbij: hoeveel grammen er in die stroom lagen, en
  hoeveel daarvan afvielen op het moment, op de sleutel of op de voorwaarden.
  Zonder die telling is "hier ligt niets over deze zaak" niet te onderscheiden
  van "hier ligt niets".

Elk gram is een **verwijzing** en geen kopie — dezelfde `id` waarmee het
journaal naar een gram wijst (`<cel>|<kroniek>|<plek>`), terug te vinden in het
[beeld van de wereld](#het-beeld-van-de-wereld). En elke verwijzing noemt de cel
die het antwoord gaf: een uitleg die een andere cel noemt, zou een weg
beschrijven die deze opstelling niet heeft, en `tests/invarianten.rs` meet dat
over elk scenario.

Er staat geen wandkloktijd in. Wat hier staat, hangt alleen van de kronieken en
van het gevraagde moment af, en daarmee is de uitleg net zo goed een contract als
het antwoord zelf.

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
4. de verplichtingen die het uitgevoerde artikel declareert, worden uitgerekend
   tot een schema van termijnen, op de uitkomsten die de uitvoering opleverde
   (zie [Verplichtingen](#verplichtingen-wat-een-besluit-achterlaat));
5. de uitkomst gaat als één gram de stroom `beschikkingen` in, met het schema erin.

Ketst het besluit af op een voorwaarde, dan slaat stap 4 over — zie
[Een weigering is ook een besluit](#een-weigering-is-ook-een-besluit).

Die stroom is **voorbehouden**, aan drie kanten:

- een configuratie die haar zelf declareert wordt geweigerd; ze wordt automatisch
  aangemaakt zodra een cel besluit-definities heeft;
- een `fixture` kan er niets in zetten. Zou een wereldbestand er een gram in
  mogen schrijven, dan lag er een "besluit" zonder receipt en zonder herkomst
  tussen de echte, en kon een reductie de twee niet onderscheiden — dan bewijst
  het kernscenario hieronder niets meer;
- een besluit kan er geen input uit halen (`from_chronicle: beschikkingen`).
  Daar liggen besluiten en geen feiten: een besluit leest geen besluit — niet
  vermomd als eigen feit, althans. Zie de vierde inputvorm hieronder voor de
  benoemde vorm waarin het wél mag.

Alleen besluiten legt er iets in, en alleen een reductie haalt er iets uit.

### Het `chronolex`-blok

Twee van de stappen hierboven leunen op iets wat het **uitvoerende artikel** zegt
en waar het schema van de wet niets over vastlegt: wanneer het besluit een
afwijzing is, en wat het oplegt. Beide staan onder `produces.extensions`, in de
namespace `chronolex`:

```yaml
produces:
  legal_character: BESCHIKKING
  decision_type: TOEKENNING
  extensions:
    chronolex:
      afwijzing_wanneer: {...}     # wanneer dit besluit een afwijzing is
      verplichtingen: [...]        # wat dit besluit achterlaat
```

`extensions` is per namespace ondoorzichtig — RFC-022 §3.2 wijst het aan als de
haak waar een platform zijn eigen aanvullingen legt, en het law-model draagt het
blok ongewijzigd mee zonder het uit te leggen. Wie een namespace leest, bezit
hem: `chronolex` is die van deze opstelling, en een andere namespace
(`blauwe_knop`, of wat er nog komt) blijft hier **ongelezen en ongemoeid**. Een
wet die er een draagt, laadt gewoon.

Binnen `chronolex` is de sleutellijst **gesloten**, en dat is de enige plek waar
dat kan. Het JSON-schema zet geen `additionalProperties: false` op `produces`,
dus `verplichtignen` met een typfout valideert zoals het staat; de opstelling is
de enige lezer en dus de enige die het kan weigeren. Twee dingen worden daarom
geweigerd bij het **optuigen van de cel**, met een melding die regeling, versie,
artikel en de bekende sleutels noemt:

- een `chronolex`-waarde die geen blok met sleutels is (een lijst, een getal);
- een sleutel die niet in de lijst hierboven staat.

Bij het optuigen en niet bij de eerste aanvrager, omdat er tussen die twee
momenten niets meer te zien is: een artikel dat niets oplegt en een artikel met
een typfout leveren allebei een beschikking zonder verplichting, en een
regeling die denkt te weigeren wijst dan stil niemand af. Eén lezer voor het hele
blok (`src/cell/extensions.rs`) houdt dat vast: het optuigen, het schema van het
decretogram en het besluit zelf lezen alle drie door diezelfde struct, zodat er
geen pad is waarop het blok wél gelezen wordt en de strengheid niet.

De twee sleutels staan hieronder: `afwijzing_wanneer` in
[Een weigering is ook een besluit](#een-weigering-is-ook-een-besluit),
`verplichtingen` in
[Verplichtingen](#verplichtingen-wat-een-besluit-achterlaat). Een sleutel erbij
is één regel in de struct — en daarmee meteen bekend bij alle drie de lezers.

### Een weigering is ook een besluit

Awb 1:3 lid 2: een beschikking omvat ook **de afwijzing van de aanvraag**. Een
besluit dat afketst is dus geen mislukte uitvoering en geen bedrag nul — er hoort
een gram te liggen. Wannéér dat zo is, zegt de **wet**: het artikel dat de
aansturende uitkomst voortbrengt declareert het bij zijn `produces`.

```yaml
# in het lexogram, op het artikel dat de uitkomst voortbrengt
produces:
  legal_character: BESCHIKKING
  decision_type: TOEKENNING            # het type als het besluit doorgaat
  extensions:
    chronolex:
      afwijzing_wanneer:
        heeft_recht_op_zorgtoeslag: false
```

**In het lexogram en niet in het wereldbestand.** Wanneer een besluit een
afwijzing is, hangt aan de uitkomst die het artikel voortbrengt en geldt voor
elke cel die dat artikel uitvoert. Zou een besluit-definitie het mogen zetten,
dan konden twee uitvoerders dezelfde wet verschillend laten weigeren zonder dat
er aan de wet iets te zien was. Een `afwijzing_wanneer` in een besluit-definitie
wordt daarom bij het optuigen geweigerd, met een melding die naar het blok in de
regeling wijst.

De namespace waarin dat staat en wat ze verder kent, staat in
[Het `chronolex`-blok](#het-chronolex-blok): de sleutellijst is gesloten, dus een
typfout in `afwijzing_wanneer` weigert bij het optuigen in plaats van de
weigering stil uit te zetten.

`afwijzing_wanneer` noemt per **boolean-uitkomst** de waarde die tot afwijzing
leidt. Drie toetsen bij het optuigen van de cel, alle drie om te voorkomen dat de
regel er staat zonder iets te doen: het blok moet een toewijzing van uitkomst
naar `true`/`false` zijn, de naam moet een uitkomst zijn die de regeling kent, en
die uitkomst moet onder elke geladen versie een ja-of-nee zijn — op een bedrag
raakt de voorwaarde nooit vervuld. Meer dan één voorwaarde is een **of**: elke
vervulde is op zichzelf genoeg, en ze komen alle vervulde in het gram te staan.

Een voorwaarde hoeft geen uitkomst te zijn die het besluit *vastlegt*: de cel
vraagt haar bij de uitvoering gewoon mee op, en de grond komt met haar artikel in
`afwijzingsgrond` te staan. Wat het gram onder de uitkomsten draagt, blijft wat
de besluit-definitie in `output` en `outputs` noemt.

Is er een voorwaarde vervuld, dan legt de cel één gram vast als altijd — hetzelfde
rechtskarakter (`BESCHIKKING`, want een weigering is er een), dezelfde vaste
velden, het receipt, en de uitkomsten die de regeling wél leverde — met precies
drie verschillen:

- `decision_type` is `AFWIJZING`;
- `afwijzingsgrond` noemt elke vervulde voorwaarde: de uitkomst, de waarde die
  afwees, en het **artikel** dat die uitkomst voortbrengt. Een afwijzing zonder
  haar grond is een besluit zonder motivering;
- er zijn **geen verplichtingen**. Ook niet als het uitvoerende artikel er een
  oplegt: een weigering belooft niets, dus er valt niets in te roosteren en er
  vervalt geen termijn.

Wijst het besluit *niet* af, dan draagt het gram het besluittype dat hetzelfde
`produces` voor de gewone afloop noemt (`decision_type`, bijvoorbeeld
`TOEKENNING`) — de waarde die het platform daar toch al las voor de
BESCHIKKING-toets. Zegt de regeling er niets over, dan staat er `null`: dat is een
gat in die regeling en geen uitnodiging om het hier in te vullen.

Verder blijft alles wat over een decretogram geldt gelden. De reductie over
`beschikkingen` vindt een afwijzing als elk ander besluit — `decision_type` is een
gewoon veld waarover een lexostatus mag publiceren — en `from_decretogram` leest
er een veld uit terug zoals uit elk gram: draagt het gram het veld, dan komt de
waarde eruit, en anders faalt het volgende besluit met dezelfde melding als
altijd. Een weigering is geen gat in de kroniek.

In het [schema van het decretogram](#het-schema-van-het-decretogram-wat-komt-uit-de-wet)
komen `decision_type` en `afwijzingsgrond` daarmee als **lexogramvelden** te
staan, met het artikel dat ze declareert erbij.

In een scenario is er één verwachting bijgekomen: `expect` mag naast de
uitkomsten ook op `decision_type` slaan. Dat is nodig ook —
`heeft_recht_op_zorgtoeslag: false` is evengoed de uitkomst van een besluit dat
het platform níet als afwijzing kent, dus zonder die assertie bewijst een
scenario over een weigering niets over de vorm van het gram.
`scenarios/toeslagen_afwijzing.yaml` speelt het geheel af: een aanvrager zonder
recht, een gram met `AFWIJZING`, een lege betalingsstroom ondanks een
kwartaalverplichting in de definitie, en een volgend besluit dat die afwijzing
gewoon terugleest. De celconfiguratie daarin is die van elk ander
besluit-scenario — de wet is veranderd, de uitvoerder niet.

Wat hier **niet** in zit: een afwijzingsgrond die niet als boolean-uitkomst in een
regeling staat (die hoort eerst in de YAML); buiten behandeling stellen (Awb 4:5)
en horen vóór afwijzing (Awb 4:7). En `decision_type` is hier geen open
vocabulaire — het gram draagt wat de regeling zegt, of `AFWIJZING`.

### De vier inputvormen van een besluit

`inputs` zegt wat de cel de engine aanlevert, op de naam van een parameter of
input van de regeling. Waar die waarde vandaan komt, staat erbij — en dat is
telkens precies één van deze vier:

| vorm | velden | waar de waarde vandaan komt |
|---|---|---|
| parameter | `param` | de gedocumenteerde parameters van dit besluit |
| eigen kroniek | `from_chronicle` + `field` | de laatste vastlegging in die eigen stroom op of vóór het moment, gezocht op het sleutelveld van de stroom (tier 1) |
| geaccepteerd | `accept_from` + `lexostatus` + `field` (+ `params`) | een andere cel stelt haar vast; deze cel rekent haar niet na (tier 2, invariant I5) |
| eerder besluit | `from_decretogram` + `field` | het laatste gram van dát besluit over **dezelfde zaak**, op of vóór het moment |

De laatste is de smalle uitzondering op "een besluit leest geen besluit", en ze
is smal op drie manieren:

- **Benoemd.** Er staat een besluitnaam, geen kroniekstroom. `from_chronicle:
  beschikkingen` blijft geweigerd: dan zou een gram als eigen feit binnenkomen en
  was niet meer te zien dat er teruggelezen is.
- **Aan de eigen zaak vast.** De sleutel is het zaakkenmerk van het **lopende**
  besluit, ingevuld uit het eigen sjabloon. Er is geen parameter waarmee een
  besluit de zaak van een ander kan aanwijzen. Ligt er geen gram van dat besluit
  met dat kenmerk op of vóór dit moment, dan faalt het besluit — *geen eerder
  besluit '…' voor zaak '…' op of vóór …* — en wordt er niets vastgelegd. Dat is
  geen "niets vastgesteld": een vaststelling zonder de verlening waarop ze
  terugslaat, hoort niet met een gat verder te rekenen.
- **Zichtbaar in het gram.** De herkomst is een eigen vorm
  (`eerder_besluit`, met het besluit, de zaak en het moment), naast eigen
  kroniek, parameter en geaccepteerd. Wie het gram leest, ziet dát er een eerder
  besluit is teruggelezen; het is geen eigen feit en geen herberekening — het
  bedrag komt uit het gram zoals het toen is vastgelegd, onder het recht dat toen
  gold.

`field` mag een uitkomst van dat gram zijn, een van zijn vaste velden, of een van
de inputs waarop het rekende; wat een gram draagt staat vast zodra de definities
er zijn, dus een typfout valt bij het optuigen en niet bij de eerste zaak. Die
twee lagen — het gram zelf en zijn inputs — worden allebei gelezen, en een naam
die in béide zit wordt geweigerd: dan wijst `field` twee waarden aan, en welke van
de twee gepakt wordt is geen leesregel die iemand bij het schrijven voor ogen had.
Hernoem dan de input, of lees een veld dat maar één ding kan zijn.

In `accept_from.params` staat naast `$parameter` en letterlijke tekst één
ingebouwde verwijzing: **`$zaakkenmerk`**, het ingevulde kenmerk van het lopende
besluit. Daarmee is "wat is er op déze zaak betaald?" een vraag die een besluit
kan stellen zonder dat het kenmerk via een vrije parameter langs de aanroeper
loopt. Er is geen algemeen `{…}`-sjabloon in `params`, en bij het optuigen wordt
geweigerd wat de naam dubbelzinnig zou maken: een definitie zonder
zaakkenmerk-sjabloon, en een definitie die zelf een parameter `zaakkenmerk`
documenteert.

De publieke wereld speelt de twee samen af: `zorgtoeslag_vaststelling` voert de
**Awir** uit (art. 19, vaststelling na afloop van het berekeningsjaar), leest het
toegekende bedrag terug uit haar eigen toekenning en accepteert van de betalende
cel wat er als voorschot is uitbetaald. Het slotbedrag is het verschil — de
verrekening van art. 24, tweede lid.

`scenarios/toeslagen_nabetaling.yaml` doet hetzelfde over een **testregeling**
(`fixtures/regulation/test_nabetaling`) en blijft daarvoor staan: dat scenario
toetst de twee inputvormen zelf — teruglezen is geen eigen feit, accepteren is
geen narekening — met een regeling die niets anders doet dan het verschil
uitrekenen. Zo hangt die toets niet aan de inhoud van een echte wet, en de
publieke wereld laat zien hoe de vorm er in het recht uitziet.

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
| `stage` | de stap van de procedure waarin dit gram ontstond (RFC-008): in een decretogram altijd `BESLUIT`, want dat *is* het besluit. De bekendmaking legt een eigen gram met `BEKENDMAKING` — zie [De bekendmaking](#de-bekendmaking-een-tweede-gram-op-dezelfde-zaak) |
| `op_moment` | wanneer besloten is (het gram is een gewoon executogram) |
| `regulation` + `regulation_valid_from` | welke regeling het besluit *is*, en **welke versie daarvan gold** |
| `executed_regulations` | álle regelingen die deze uitvoering aanriep, elk met de versie die toen gold; die van het besluit vooraan. Méér dan `regulation` alleen — een uitvoeringsregeling die een bedrag levert hoort er even goed bij — en minder dan `scope.loaded_regulations` uit het receipt, dat ook een versie noemt die niet gold en een regeling die deze uitvoering niet raakte |
| `competent_authority` | het bevoegd gezag dat de regeling noemt (RFC-002): dat van het artikel dat de aansturende uitkomst voortbrengt, anders dat van het document; `null` als ze er geen noemt |
| `besloten_door` | de identiteit van de cel die besloot — zie [Wie mag besluiten](#wie-mag-besluiten) |
| `legal_character` | altijd `BESCHIKKING`: dat is wat een decretogram is (RFC-022 §1.2), en een besluit over iets anders wordt bij het optuigen geweigerd |
| `decision_type` | wélk besluit dit is: `AFWIJZING` zodra een afwijzingsvoorwaarde vervuld was, anders wat het uitvoerende artikel aanwijst (`produces.decision_type`), en `null` als de regeling zwijgt — zie [Een weigering is ook een besluit](#een-weigering-is-ook-een-besluit) |
| `afwijzingsgrond` | de vervulde afwijzingsvoorwaarden: per stuk de uitkomst, de waarde die afwees en het artikel dat haar voortbrengt. Leeg bij elk besluit dat niet afwees |
| de uitkomsten | de uitkomst die het besluit *is*, plus wat `outputs` erbij noemt |
| `inputs` | wat de besluit-definitie zelf aanleverde, **met herkomst per waarde**: uit een eigen kroniek (met het moment van die vastlegging), uit een parameter, of geaccepteerd van een andere cel |
| `chronicle_sources` | de eigen kronieken die als databron klaarstonden, elk met haar stand op het moment van het besluit: aantal grammen en een hash erover (RFC-022 §1.3). Wat de engine daaruit las, staat in de trace van het receipt |
| `obligations` | het betalingsschema dat uit dit besluit volgt: per termijn een vervaldatum, een bedrag, een volgnummer, de betalende cel, de `grondslag` uit het lexogram en de herkomst (`lexogram`: regeling, versie, artikel). Leeg bij een afwijzing, ook als het lexogram er een oplegt |
| `wacht_op_bekendmaking` | de verplichtingen die het lexogram met `vanaf: bekendmaking` oplegt: bedrag, partijen, ritme, grondslag en volgnummer staan er, de vervaldatum niet — die volgt uit de bekendmaking. Leeg bij elk besluit waarvan de termijnen meteen vervielen |
| `receipt` | het volledige Execution Receipt, **met de uitvoeringstrace** (`results.trace`) |

### Het schema van het decretogram: wat komt uit de wet?

Bij het optuigen rekent elke besluit-definitie uit **welke velden** het gram dat
ze voortbrengt zal dragen, en wie elk veld declareert. Dat schema staat in het
beeld van de wereld (`cells[].besluiten[].schema`) en in de frontend, en het
bestaat vóórdat er één besluit genomen is: het is de vorm van het gram en geen
samenvatting van wat er al ligt.

Per veld staan er een `name`, een `type` (de engine-typen — `string`, `number`,
`boolean`, `date`, `amount`, `object`, `array` — met de `unit` uit het `type_spec`
van de wet) en een `herkomst`:

| herkomst | wat het zegt | `gat` |
|---|---|---|
| `lexogram` | een regeling declareert dit veld; met `regulation`, `valid_from` en `article` | nee |
| `beleid` | hetzelfde, maar de regeling draagt `regulatory_layer: UITVOERINGSBELEID` | nee |
| `wereldbestand` | het wereldbestand zegt het, en geen enkele regeling | **ja** |
| `platform` | elk decretogram draagt het, ongeacht welke wet er draait | nee |

**Waarom `wereldbestand` een gat is.** RFC-022 zegt dat normatieve inhoud in het
**lexogram** hoort: de wet zegt wat er vastgesteld wordt, en de uitvoering voert
uit. Wat in `besluit_definitions` staat — welke uitkomsten samen één gram vormen
en het zaakkenmerk-sjabloon — is normatief én staat in de configuratie van deze
opstelling. Een andere organisatie die dezelfde wet uitvoert, zou het opnieuw
moeten verzinnen en zou er iets anders van kunnen maken, zonder dat één regeling
verandert. Dat is precies wat het lexogram hoort te voorkomen, dus die velden
dragen `gat: true` — niet als foutmelding, maar als **meting**: het is de lijst
die korter hoort te worden.

**En zo wordt hij korter.** De verplichtingen (`obligations[0]`, `obligations`)
stonden op die lijst en staan er niet meer: het artikel dat de beschikking
voortbrengt, declareert ze zelf in `produces.extensions.chronolex`, dus ze dragen
`lexogram` met de regeling, de versie en het artikel erbij (zie
[Verplichtingen](#verplichtingen-wat-een-besluit-achterlaat)). Ook de twee
partijen staan daar: het artikel zegt wie schuldenaar is en wie schuldeiser, en het
schema schrijft de standaarden uit voor wie ze weglaat. Wat het wereldbestand er nog
over zegt, is wélke cel de schuldenaar nakomt — uitvoering, en geen norm.

Twee dingen die het schema met opzet níet doet. Het spreekt zich niet uit over
`platform`-velden: dat een gram zijn eigen moment, zijn eigen receipt en de stand
van de kronieken waarop het leunde draagt, is geen norm die een wet had moeten
stellen. En het verzwijgt niet waar het platform zijn waarde *leest*: bij
`competent_authority` en `legal_character` staat het lexogram erbij — het artikel
dat de aansturende uitkomst voortbrengt, of het document als de regeling het daar
declareert (RFC-002-volgorde, dezelfde die het besluit-pad toepast).

**`decision_type` en `afwijzingsgrond` zijn lexogramvelden.** Anders dan
`legal_character`, dat altijd `BESCHIKKING` is omdat het platform elke andere
waarde weigert, kiest het platform niets in wát een besluit is: het artikel zegt
met `decision_type` wat er uitkomt als het besluit doorgaat, en met
`extensions.chronolex.afwijzing_wanneer` wanneer het een afwijzing wordt (zie
[Een weigering is ook een besluit](#een-weigering-is-ook-een-besluit)). Beide
velden wijzen daarom naar het artikel dat het zegt. Zwijgt de regeling erover —
geen `decision_type` en geen afwijzingsvoorwaarde — dan draagt het gram `null`
respectievelijk een lege lijst, en zijn het platformvelden zonder lexogram. Geen
gat: dat een wet geen weigering kent, is geen norm die ze had moeten stellen.

Het schema staat op de **nieuwste geladen versie** van de regeling; de versie
staat er daarom bij. Een besluit over een ouder moment landt op een oudere versie
en kan een ander schema hebben — wat er werkelijk gold, zegt het gram zelf met
`regulation_valid_from`.

### De trace zit in het gram

Het besluit-pad voert uit **met trace**, en die trace gaat ongewijzigd mee in het
receipt. Dat is het verschil met een reductie: zonder haar staat er wel wát er
uitkwam, maar niet langs welke artikelen, en dan is een besluit na te rekenen maar
niet na te lopen (RFC-013).

Twee eigenschappen maken haar bruikbaar als bewijs:

- **Elke rekenstap noemt haar regeling en haar artikel.** De engine schrijft ze op
  terwijl ze het artikel uitvoert (`PathNode::regulation` / `PathNode::article`);
  ze worden niet achteraf uit de naam van een uitkomst afgeleid, want zo'n
  reconstructie kan afwijken van wat er werkelijk liep.
- **Er staan geen looptijden in.** De engine kán per node meeschrijven hoe lang
  die stap duurde; het besluit-pad zet dat uit (`TraceBuilder::new_untimed`). Een
  looptijd verschilt per run, dus hij zou het gram per run anders maken en de hash
  eroverheen waardeloos. De wandkloktijd van de uitvoering staat één keer in het
  receipt, gelabeld als wat ze is; verder draagt de trace geen tijd. Twee
  uitvoeringen met dezelfde invoer leveren daardoor dezelfde trace, tot op de
  hash — en dat staat vast in
  [`tests/trace.rs`](tests/trace.rs).

**Waarom in het gram en niet ernaast.** Een trace is groot genoeg om de vraag te
stellen, dus ze is gemeten. De zorgtoeslag-toekenning van de publieke wereld komt
als JSON uit op ruim **13 kB** gram plus receipt, waarvan de trace ruim 8 kB. Het
zwaarste besluit dat deze opstelling nu draait — één dat drie regelingen aanraakt
— komt uit op ongeveer **37 kB**, waarvan de trace ruim 23 kB.

Dat is ruim onder de grens (zo'n 200 kB per gram) waarboven het de moeite waard
wordt de trace apart in de kroniek te leggen met alleen een hash in het gram.
Zolang dat zo is, wint het gram: een besluit dat zijn eigen onderbouwing draagt,
kan er niet van gescheiden raken, en er is geen tweede plek die mee moet
verhuizen. Groeit een casus daar overheen, dan is de scheiding alsnog te maken —
het gram draagt dan de hash, en waar de trace dan ligt hoort in dit stuk te komen
staan.

De herkomst per input is geen versiering. Zonder haar staat er wel een waarde in
het gram, maar niet van wanneer ze was of wie haar leverde — en dan is
"accepteren in plaats van narekenen" niet van gokken te onderscheiden. Wat het
besluit ophaalde, gaat daarom ín het gram en **niet** als los feit in een
kroniek: een volgend besluit haalt het opnieuw op. Een kroniek bevat alleen wat de
cel zelf overkwam (zie [Wat een cel is](#wat-een-cel-is)).

Eén besluit is één gram. Wat tegelijk ontstaat, wordt samen vastgelegd (RFC-022
§1.2 — elk chronolexogram is *elementair*): het recht en het bedrag staan in
hetzelfde gram, niet in twee.

### Wie mag besluiten

Drie regels, en ze horen in deze volgorde gelezen te worden:

1. **De wet bepaalt wie het bevoegd gezag is.** Dat staat in de regeling
   (`competent_authority`, RFC-002) en nergens anders. Het platform leest het uit
   het law-model; een celconfiguratie kan zichzelf geen gezag toebedelen.
   RFC-002 legt het op het **artikel** — één wet kan meer dan één gezag kennen —
   dus het artikel dat de aansturende uitkomst voortbrengt gaat voor; het
   document is de terugvaloptie voor een artikel dat zelf zwijgt.
2. **De cel beweert wie zij is.** `identity: <naam>` op de cel in het
   wereldbestand, standaard haar cel-id. Die naam landt niet in de cel maar in
   haar **veiligheidscontext** (`Identity`, RFC-022 §2): het is dezelfde
   identiteit die elke vraag over de grens ondertekent, en de wereld reikt haar
   bij een besluit aan. Eén register, zodat wie tekent en wie besluit nooit
   twee namen worden.
3. **De bewering geldt voor nu als waar.** Er wordt niets bewezen: er is geen
   sleutelmateriaal en de ondertekening is gesimuleerd (zie
   [Ondertekening is gesimuleerd](#ondertekening-is-gesimuleerd)). Bewijs is
   later werk; wat er nu gebeurt, is de toets tegen de wet.

Bij een besluit legt het platform die twee naast elkaar, op genormaliseerde tekst
— witruimte eromheen en kasus tellen niet mee, en verder wordt er niets
geïnterpreteerd. Een afkorting is een andere naam, en of die dezelfde organisatie
aanwijst is een vraag over het recht; die hoort in de wet of in het wereldbestand
beantwoord te worden en niet hier geraden. Drie uitkomsten:

| de regeling | wat er gebeurt |
|---|---|
| wijst deze identiteit aan | het besluit gaat door; het gram draagt `competent_authority` (uit de regeling) en `besloten_door` (de identiteit) |
| wijst een ander aan | het besluit wordt **geweigerd** en er wordt niets vastgelegd: *cel 'toeslagen' beweert 'Belastingdienst' te zijn, maar regeling 'wet\_op\_de\_zorgtoeslag' wijst 'Dienst Toeslagen' aan als bevoegd gezag* |
| wijst niemand aan | het besluit gaat door met `competent_authority: null`, en de wereld **waarschuwt**: *regeling '…' declareert geen bevoegd gezag* |

Dat de derde regel doorgaat en niet weigert, is een keuze: een regeling die geen
gezag declareert is een gat in díe regeling, en de opstelling dichtzetten voor
iets waar de besluitende cel niets aan kan doen, zet de speeltuin dicht. Maar er
viel dan niets te toetsen, en dat is iets anders dan een geslaagde toets — dus
het staat in het gram (`competent_authority: null`), in het verslag, in het beeld
van de wereld en in het gram-detail van de frontend.

De weigering valt **vóór de eerste vraag over een celgrens**, om dezelfde reden
als bij het zaakkenmerk: een vraag is bij de bevraagde organisatie een
gebeurtenis, en die hoort niet te vallen voor een besluit dat toch niet genomen
kan worden.

`competent_authority` mag in de wet een `#`-verwijzing zijn naar een uitkomst van
de regeling zelf (`competent_authority: '#bevoegd_gezag'`, zoals in
`wet_op_de_zorgtoeslag`). Die wordt opgelost vóór de vergelijking — anders zou de
toets op de tekst `#bevoegd_gezag` gaan en was geen enkele cel ooit bevoegd. Komt
zo'n verwijzing nergens op uit — geen actie zet die uitkomst op een letterlijke
naam — dan is dat een **fout** en geen zwijgende wet: de regeling zegt iets wat
niet te lezen is, en doorgaan alsof ze niets zegt zou de toets stil uitzetten.

Dat `besloten_door` naast `competent_authority` in het gram staat, is met opzet:
ze zijn gelijk zodra er een gezag is, maar ze zeggen verschillende dingen — het
ene is wat de wet aanwijst, het andere wie er feitelijk besloot. Wie er later een
handtekening onder zet, heeft dan een veld om die aan te hangen.

De scenario's: [`besluit_zonder_bevoegd_gezag.yaml`](scenarios/besluit_zonder_bevoegd_gezag.yaml)
voor het ontbrekende gezag, en
[`scenarios/geweigerd/`](scenarios/geweigerd/) voor de weigering — die laatste
kan geen gewoon scenario zijn, want een besluit dat omvalt breekt de run af. Ze
worden afgerekend in [`tests/bevoegd_gezag.rs`](tests/bevoegd_gezag.rs), op
dezelfde manier als de negatieve fixtures van de gate: niet alleen *dat* ze
falen, maar waaróp.

### Verplichtingen: wat een besluit achterlaat

Een beschikking die een bedrag toekent, laat iets achter dat later moet gebeuren.
Wát dat is, staat in het **lexogram**: in het artikel dat de beschikking
voortbrengt, onder `produces.extensions.chronolex` — de namespaced haak die
RFC-022 §3.2 daarvoor aanwijst, met de gesloten sleutellijst uit
[Het `chronolex`-blok](#het-chronolex-blok). Wíe het nakomt, staat in het
**wereldbestand**. Die scheiding is de hele paragraaf.

```yaml
# in de regeling, op het artikel dat de sturende uitkomst voortbrengt
produces:
  legal_character: BESCHIKKING
  decision_type: TOEKENNING
  extensions:
    chronolex:
      verplichtingen:
        - soort: betaling                  # de enige soort die te declareren is
          bedrag: $hoogte_zorgtoeslag      # een uitkomst van dít artikel
          ritme: $betalingsritme           # ineens | kwartaal | maand, of $instelling
          schuldenaar: '#bevoegd_gezag'    # optioneel; dit is de standaard
          schuldeiser: $bsn                # optioneel; standaard de parameter uit
                                           # het zaakkenmerk
          richting_bij_negatief: omkeren   # optioneel; zonder dit is een negatief
                                           # bedrag een fout
          vanaf: '{jaar}-02-01'            # optioneel; standaard het moment van het besluit
          grondslag: Wet op de zorgtoeslag art. 2 jo. Awir art. 16 jo. art. 22
```

```yaml
# in het wereldbestand, bij de cel die betaalt
- id: belastingdienst
  komt_na:
    - Dienst Toeslagen                     # de naam namens wie zij nakomt
  chronicles:
    - stream: betalingen
      key: zaakkenmerk
      gebeurtenissen:                      # het schema dat het platform vraagt
        - name: betaling_gedaan            # wat zíj vastlegt: zij betaalde
          intake: betaling
          fields: [...]                    # zie De executogram-vorm
        - name: betaling_gemeld            # bij de besluitende cel: het is gemeld
          intake: levering
          fields: [...]
```

Beide kanten van een nagekomen verplichting leggen vast, elk in haar eigen
kroniek: de betaler dat zij betaalde (`betaling_gedaan`), de besluitende cel dat
het haar gemeld is (`betaling_gemeld`). Twee grammen en niet één gedeelde staat.
Het schema van die stroom is niet vrij: het platform declareert welke velden die
twee dragen, en het optuigen weigert een cel die ze niet dekt — zie [Het
gebeurtenisschema van een
stroom](#het-gebeurtenisschema-van-een-stroom).

**Waarom in de wet en niet in het wereldbestand.** Dát er 80% voorschot betaald
wordt en dat het slotbedrag ineens komt, schrijft het recht voor (Wpp art. 60 lid
2, Awb 4:86 lid 2). Zou dat in de besluit-definitie van één uitvoerder staan, dan
konden twee uitvoerders van dezelfde regeling een ander betalingsschema hanteren
zonder dat de wet verschilt — en dan zou het wereldbestand normatieve inhoud
dragen. Een besluit-definitie die nog zelf `obligations` draagt, wordt daarom bij
het optuigen geweigerd, met een melding die naar het blok hierboven wijst. Een
BESCHIKKING-artikel **zonder** blok is geen fout: niet elke beschikking kent een
bedrag toe.

- **`bedrag` is een uitkomst, geen bedrag.** Wat betaald moet worden komt uit de
  wet zelf: een uitkomst van dít artikel, of een uitkomst die het uitvoerende
  besluit erbij vastlegt (`outputs`). Een letterlijk bedrag zou naast die uitkomst
  gaan leven, en dan zegt het gram twee dingen over hetzelfde geld. Wat het besluit
  zelf niet publiceert, gaat wél als gevraagde uitkomst mee de uitvoering in.
- **Een ritme beschrijft één jaar**: `ineens` één termijn, `kwartaal` vier,
  `maand` twaalf. Elke termijn krijgt hetzelfde bedrag in hele eenheden en het
  restant gaat naar de laatste, dus de som van de termijnen is exact het
  toegekende bedrag. Dat is de eigenschap waarop "betaald tot nu toe" rust.
- **Het ritme mag een instelling zijn.** Een betalingsritme is doorgaans beleid en
  geen wet; `ritme: $betalingsritme` leest uit `settings` van het wereldbestand,
  zodat de regeling niet beweert dat de wet per kwartaal betaalt. Een instelling
  die niet bestaat of geen ritme noemt, sneuvelt bij het optuigen van de wereld —
  en zodra een besluit haar gebruikt heeft, staat ze vast (zie
  [Instellingen komen vast te staan](#instellingen-komen-vast-te-staan)).
- **`vanaf` is een sjabloon over de gedocumenteerde parameters** van het besluit
  dat het artikel uitvoert, net als het zaakkenmerk, en wat het oplevert moet een
  datum zijn. Het mag niet vóór het besluit liggen: een termijn in het verleden zou
  bij het nakomen een betaling op een moment vastleggen dat al geweest is, en dan
  verandert het beeld van toen alsnog.
- **`vanaf: bekendmaking` is geen datum maar een gebeurtenis.** Een besluit dat
  niet bekendgemaakt is, werkt niet (Awb 3:40), dus er valt niets in te
  roosteren. De verplichting komt dan met bedrag en partijen in het besluit te
  staan onder `wacht_op_bekendmaking`, en het schema ontstaat pas bij de
  bekendmaking — met als eerste vervaldag de **uiterste betaaldatum** die de
  algemene wet daar uitrekent (Awb 4:87), onder de naam `uiterste_betaaldatum`.
  Zolang die bekendmaking uitblijft komt de klok niets na, en er wordt geen dag
  verzonnen om toch maar iets te kunnen inroosteren. Levert de stage die datum
  niet, dan valt de bekendmaking om met de melding dat de wet haar hoort te
  geven. Zie [De bekendmaking](#de-bekendmaking-een-tweede-gram-op-dezelfde-zaak).
- **`grondslag` is verplicht en vrije tekst.** Een verplichting zonder grondslag is
  een bedrag zonder wet. Ze reist mee tot in elke termijn van het gram, samen met
  de herkomst (`lexogram`: regeling, versie en artikel), zodat wie een betaling
  terugleest niet alleen ziet dát er betaald moest worden maar ook waarom en
  waaruit.
- **`schuldenaar` en `schuldeiser` zijn namen uit het recht, geen cellen.** Een
  verplichting is een rechtsverhouding tussen twee partijen; wie ze zijn wijst de
  wet aan. Twee vormen: `'#bevoegd_gezag'` — het `competent_authority` van het
  artikel, en anders dat van het document, met dezelfde resolutie als bij het
  besluit zelf — of `$parameter`, een gedocumenteerde parameter van het besluit
  waarvan de waarde de partij benoemt. Een kale naam mag niet: die zou één
  organisatie in de wet vastspijkeren, en dan legt dezelfde regeling in een andere
  wereld de verplichting bij iemand die er niets mee te maken heeft.
- **Laat je ze weg, dan gelden de standaarden**: schuldenaar is het bevoegd gezag
  en schuldeiser is de parameter waarmee het zaakkenmerk de zaak identificeert —
  een beschikking die een bedrag toekent, laat het bestuursorgaan betalen aan de
  partij over wie de zaak gaat. Die standaard wordt bij het optuigen **expliciet
  gemaakt** en staat in elke termijn van het gram: een gram dat de partijen
  verzwijgt omdat ze "vanzelf spreken", laat een lezer ze later raden. Wijst het
  zaakkenmerk niet precies één parameter aan (`{jaar}/{bsn}`), dan valt er niets
  te leiden en weigert het optuigen de verplichting — met de melding dat
  `schuldeiser: $parameter` eronder hoort.

**Een negatief bedrag bestaat niet.** Valt het bedrag onder nul — een vaststelling
lager dan het betaalde voorschot — dan is dat geen negatieve betaling. Juridisch is
het een **terugvordering** (Awb 4:57): een verplichting de andere kant op, waarbij
de partij moet betalen. Dat is een andere rechtsverhouding en geen minteken, dus de
wet moet hem aanwijzen. Declareert de verplichting `richting_bij_negatief: omkeren`,
dan wisselen schuldenaar en schuldeiser, wordt het bedrag positief en draagt de
termijn `soort: terugvordering`. Zonder die declaratie valt het besluit om en wordt
er **niets** vastgelegd, met een melding die de declaratie noemt: een gram met een
negatieve termijn erin zou een betaling beloven die niemand kan doen, en een gram dat
er eenmaal ligt verandert niet meer. `terugvordering` is zelf niet te declareren —
ze ontstaat uit de richting, niet uit een woord in de wet.

**Wie er feitelijk betaalt, komt uit het wereldbestand.** De wet noemt een naam; het
wereldbestand zegt welke **cel** er onder die naam nakomt. Twee wegen, in deze
volgorde:

1. **`komt_na`**: een cel noemt de namen waarvoor zij betalingsverplichtingen
   nakomt — de naam zoals de wet het gezag aanwijst (`competent_authority`,
   RFC-002), niet een cel-id. Zo betaalt een betaalsysteem namens een
   bestuursorgaan dat zelf geen cel is. Het lexogram kan die cel niet noemen — een
   regeling weet niet hoe iemand zijn uitvoering heeft ingericht — dus de enige
   naam die beide kanten kennen is die van de partij. Hetzelfde lexogram laat
   daardoor in een ander wereldbestand een andere cel betalen zonder dat het recht
   verschilt.
2. **De identiteit van een cel**: is de schuldenaar een partij die zélf als cel
   meedoet (haar `identity` is die naam), dan betaalt die cel. Zo komt de aanvrager
   een terugvordering na zonder dat iemand daar een binding voor opschrijft.

`komt_na` gaat vóór, en dat moet ook: een bestuursorgaan dat als cel besluit, laat
doorgaans een ander systeem voor haar betalen, en dat systeem zegt dat zo. Twee
cellen die dezelfde naam nakomen worden bij het optuigen geweigerd; dragen twee
cellen dezelfde `identity`, dan wijst die weg niemand aan — kiezen zou een betaling
bij een willekeurige organisatie laten landen.

**Geen cel is geen fout.** Kent de wereld voor de schuldenaar geen cel, dan roostert
de klok de termijn wél in en komt hem niet na: het gram draagt haar met `betaler:
null`, en het beeld toont haar als **openstaand**. Een terugvordering op een burger
is een echte verplichting, ook in een wereld waarin die burger niet meedoet — wat de
wet oplegt hangt niet af van wie er in deze opstelling een systeem heeft. Wat het
optuigen wél weigert, is een gebonden cel zonder stroom `betalingen`, voor zover ze
vooruit te weten is: een schuldenaar die het bevoegd gezag is staat dan al vast, een
`$parameter` krijgt pas bij het besluit een waarde. De besluitende cel mag zichzelf
nakomen; ze heeft die stroom hoe dan ook nodig, want zij legt vast dat het haar
gemeld is.

Omdat het schema aan het **artikel** hangt en niet aan de besluit-definitie,
krijgen twee besluiten die op dezelfde uitkomst van hetzelfde artikel gaan
hetzelfde schema. Dat is de bedoeling — en het is meteen de reden dat een
verlening en een vaststelling niet hetzelfde artikel horen uit te voeren. In de
publieke wereld doen ze dat dan ook niet: de toekenning voert Wet op de
zorgtoeslag art. 2 uit en legt het voorschot in termijnen op (Awir art. 16 jo.
art. 22), de vaststelling voert Awir art. 19 uit en legt het slotbedrag ineens
op, ná verrekening van wat er als voorschot betaald is (art. 24, tweede lid).
Zouden beide op art. 2 staan, dan legde de wereld het volle bedrag twee keer op
— niet omdat het schema aan het artikel hangt, maar omdat "vaststellen" dan
niets anders was dan hetzelfde nog eens uitrekenen.

Eén besluit legt nooit iets op, wat het artikel ook zegt: een **afwijzing**. Een
weigering belooft niets, dus er valt niets in te roosteren — zie
[Een weigering is ook een besluit](#een-weigering-is-ook-een-besluit).

Nakomen doet de klok, niet het besluit. Op elke vervaldatum legt de **betalende**
cel een executogram vast in haar eigen stroom `betalingen` (`intake: betaling`, met
zaakkenmerk, bedrag, volgnummer, de soort, de twee partijen en de verwijzing naar het
decretogram), en de **besluitende** cel een levering in de hare: *betaling ontvangen
gemeld*. Twee vastleggingen, elk in de kroniek van de cel die haar deed — beide
kanten weten wat er gebeurde, en niemand kopieert de staat van een ander. Het gram
heet naar de soort, dus een terugvordering ligt er als *terugvordering* en niet als
een betaling met een verhaal eromheen. Dat de twee partijen in het feit zelf staan is
het punt: een som over deze stroom gaat anders over bedragen waarvan de richting
alleen elders staat, en `betaald_tot_nu_toe` van een cel telt precies op wat zíj
betaalde.

Die verwijzing naar het decretogram is een **adres** en niet alleen een
omschrijving:

| veld | wat |
|---|---|
| `besluit` | de besluit-definitie die het gram voortbracht |
| `besluit_cel` | de cel in wiens kroniek dat gram ligt |
| `besluit_kroniek` | haar stroom — altijd `beschikkingen`, en toch opgeschreven |
| `besluit_gram` | de plek van dat gram in die stroom, geteld vanaf nul |
| `besluit_op_moment` | het moment van dat besluit |
| `volgnummer` | welke termijn van het schema dit is |

Cel, kroniek en plek samen zijn precies de verwijzing waarmee het beeld van de
wereld een gram aanwijst (`<cel>|<kroniek>|<plek>`, zie
[Het journaal](#het-journaal-wie-deed-wat-en-wat-veranderde-er)). Daarmee komt
een lezer van de betaling bij het besluit — en dus bij de **uitvoeringstrace** in
zijn receipt — zonder de kroniek van de besluitende cel af te lopen. Twee van de
drie opschrijven en de derde laten raden zou die verwijzing laten leunen op een
afspraak buiten het gram.

De plek wordt ingevuld **nadat** het decretogram vastligt: het is de plek waar het
gram terechtkwam en niet de plek waar het naar verwachting terecht zou komen.

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
hetzelfde antwoordt),
[`scenarios/toeslagen_verplichtingen_ritmes.yaml`](scenarios/toeslagen_verplichtingen_ritmes.yaml)
(hetzelfde bedrag `ineens` en per `maand`, elk uit een eigen artikel — het ritme
bepaalt wanneer, niet hoeveel) en
[`scenarios/toeslagen_terugvordering.yaml`](scenarios/toeslagen_terugvordering.yaml)
(een vaststelling lager dan het voorschot: de aanvrager-cel betaalt terug aan het
bevoegd gezag, en de twee sommen lopen uiteen zonder saldo). Wat er gebeurt als de
wet over een negatief bedrag zwijgt, staat als fixture in
[`scenarios/geweigerd/negatief_bedrag_zonder_omkeren.yaml`](scenarios/geweigerd/negatief_bedrag_zonder_omkeren.yaml)
en wordt afgerekend in [`tests/verplichtingen.rs`](tests/verplichtingen.rs).

### De bekendmaking: een tweede gram op dezelfde zaak

Een besluit nemen is niet hetzelfde als een besluit laten werken. Awb 3:40: een
besluit treedt niet in werking voordat het is **bekendgemaakt**. RFC-008 maakt
daar een **stage** van — een beschikking doorloopt een procedure, en BESLUIT en
BEKENDMAKING zijn twee stappen daarin — en RFC-022 §1.2 zegt wat dat in een
kroniek betekent: elke stage legt haar **eigen elementaire gram** op hetzelfde
zaakkenmerk.

Zo staat het hier ook. Na de bekendmaking liggen er in `beschikkingen` twee
grammen over dezelfde zaak, met hetzelfde `besluit` erin en één veld dat ze uit
elkaar houdt:

| | het besluit | de bekendmaking |
|---|---|---|
| `stage` | `BESLUIT` | `BEKENDMAKING` |
| draagt | de uitkomsten, de inputs met hun herkomst, het receipt, het schema van de verplichtingen | de dag van de bekendmaking, de uitkomsten van de hooks op die stage met hun artikel, en de termijnen die nu pas gaan lopen |
| ontstaat door | `Cell::decide` | `Cell::bekendmaken` |

**Geen veld dat erbij komt, maar een gram dat erbij komt.** Het besluit-gram
wijzigen zou de eerste regel van een kroniek breken: ze groeit en verandert niets.
Wie op een moment tussen de twee vraagt, ziet daarom het besluit wél en de
bekendmaking niet — en dat is precies wat een reductie hoort te zeggen over een
besluit dat nog niet werkt.

**Wat de bekendmaking toevoegt, komt uit de algemene wet.** De uiterste
betaaldatum (Awb 4:87), de bezwaartermijn (6:7 jo. 6:8) en de
rechtsmiddelenclausule (3:45) hangen aan de bekendmaking en niet aan de
uitvoerende regeling. Ze komen hier dan ook niet uit die regeling: het zijn
**hooks** — artikelen die zich met `hooks.applies_to` op een rechtskarakter en een
stage aanbieden (RFC-007/RFC-008) — en de engine vuurt ze af als ze de stage
uitvoert. De uitvoerende regeling weet er niets van, en dat is de winst: zou elke
uitvoeringsregeling haar eigen uiterste betaaldatum uitrekenen, dan zouden die
per regeling uiteen gaan lopen zonder dat er aan het recht iets verandert.

In het gram staat per uitkomst wélk artikel haar voortbracht (`hooks`, met
regeling, versie en artikelnummer). Een uiterste betaaldatum zonder die herkomst
is niet te onderscheiden van een datum die de uitvoerder zelf bedacht.

**De stage rekent niet opnieuw.** Wat de engine uitvoert, doet ze op de inputs
zoals ze in het besluit-gram staan en op het **moment van het besluit** — onder
het recht van toen, op de feiten van toen. Wat er van vandaag is, is de dag van de
bekendmaking, en die gaat als parameter mee (`bekendmaking_datum`, naast
`competent_authority` uit het gram). Zou de stage op vandaag rekenen, dan kon een
bekendmaking een ander bedrag opleveren dan het besluit dat ze bekendmaakt.

Om diezelfde reden gaat er bij een bekendmaking **niets over een celgrens**: wat
er van een ander geaccepteerd is, staat al in het gram. Er is geen brug, geen
resolver en geen vraag.

`from_decretogram` leest standaard het gram van de stage `BESLUIT` terug: een
vaststelling die naar de verlening terugkijkt, kijkt naar het besluit en niet naar
de stage die erna kwam. Een reductie over `beschikkingen` kan met `where:` op
`stage` filteren en zo allebei de vragen stellen — *wat is er besloten* en *is het
bekendgemaakt, en wat geldt er sindsdien*.

```yaml
# in het wereldbestand, bij de cel die besloot
actions:
  - id: uitvoerder.bekendmaking
    actor: uitvoerder
    label: Maak het besluit bekend
    publishes:
      cell: uitvoerder                 # de cel die besloot
      besluit: toekenning              # de besluit-definitie

lexostatus_definitions:
  - name: bekendmaking
    inputs:
      - name: zaakkenmerk
        type: string
    outputs:
      - uiterste_betaaldatum           # uit de wet, langs de hook
      - bezwaartermijn_einddatum
    reduction:
      chronicle: beschikkingen
      key: zaakkenmerk
      latest: true
      where:
        stage: BEKENDMAKING            # het gram van déze stage
```

**Twee keer bekendmaken wordt geweigerd**, en niet als voorzichtigheid: twee
bekendmakingen van hetzelfde besluit zouden twee verschillende uiterste
betaaldata op één zaak opleveren, en dan zegt de kroniek twee dingen over
dezelfde verplichting. Of de actie kan, wordt afgeleid uit de kronieken zelf —
zie [Acties](#acties-wat-een-actor-kan-doen).

Wat er **niet** in zit: de stages vóór het besluit (AANVRAAG, BEHANDELING — in
deze opstelling gaat daar een `records`-actie aan vooraf), BEZWAAR en alles
daarna, en het opnieuw bekendmaken van een gewijzigd besluit. De procedure van de
testregeling `fixtures/regulation/test_awb_procedure` kent daarom precies twee
stages: wat er staat, is wat het platform ook werkelijk uitvoert.

Het scenario staat in
[`scenarios/bekendmaking.yaml`](scenarios/bekendmaking.yaml) (een besluit op T1,
een bekendmaking op T2, en een vraag ertussenin die het besluit wél ziet en de
bekendmaking niet), met
[`scenarios/bekendmaking_blijft_uit.yaml`](scenarios/bekendmaking_blijft_uit.yaml)
als tegenscenario. De vorm van het gram wordt afgerekend in
[`tests/bekendmaking.rs`](tests/bekendmaking.rs).

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

**Wel**: dat een decretogram een engine-uitkomst met `legal_character:
BESCHIKKING` is en het RFC-013 receipt haar lichaam — een besluit-definitie over
een toets of een waardebepaling wordt bij het optuigen geweigerd; dat elk gram
elementair is en co-ontstane uitkomsten samen draagt; het `zaakkenmerk` als de
sleutel waaronder de grammen van één zaak een kroniek vormen; het moment; en uit
§1.3 dat de eigen kronieken die aan de uitvoering bijdroegen met inhoud en versie
in het gram staan (`chronicle_sources`). En dat een beschikking ook de **afwijzing**
van de aanvraag omvat (Awb 1:3 lid 2): het gram draagt een `decision_type`, en een
besluit dat afketst legt er een vast met `AFWIJZING` en zonder verplichtingen.

En sinds de bekendmaking een eigen gram is: de **stages** van RFC-008. Elk gram
draagt de stap waarin het ontstond (`stage`), en een zaak kan er meer dan één
dragen — het besluit en zijn bekendmaking, op hetzelfde zaakkenmerk (zie
[De bekendmaking](#de-bekendmaking-een-tweede-gram-op-dezelfde-zaak)).

**Niet**: de stages vóór het besluit (AANVRAAG, BEHANDELING) en die erna
(BEZWAAR, beroep) — de procedure loopt hier van BESLUIT tot BEKENDMAKING en niet
verder; `modality`
(`is_intrekking_van`, `is_wijziging_van`); de afgeleide rechtsbeschermingsroute
(§3.3); `decision_type` als open vocabulaire; `extensions` **op het gram zelf**
(die op een `produces` in het lexogram wordt wél gelezen — zie
[Een weigering is ook een besluit](#een-weigering-is-ook-een-besluit)); en de handtekening —
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
  precies één cel, en de **enige** die het transport aanroept. De identiteit
  draagt twee namen: het **cel-id** — het adres, waarop het transport de peer
  vindt en waarop het vraaggraf gaat — en de **naam** waaronder de cel zich
  uitgeeft (`identity:` in het wereldbestand, standaard het id). Die tweede is
  de bewering die bij een besluit naast de wet komt te liggen, zie
  [Wie mag besluiten](#wie-mag-besluiten), en ze staat hier en niet op de cel:
  het is wat een ondertekening straks bewijst. Eén register voor "wie is dit",
  want twee zouden bij de eerste echte ondertekening uiteen gaan lopen.
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
dat een eigen regeling overschaduwt ook. Breder nog: een cel-id dat de `$id` is
van een regeling die *welke cel dan ook* in de wereld laadt, valt bij het optuigen
van de wereld — RFC-022 §4.2 maakt daar onvoorwaardelijk een laadfout van, want
een vraag aan die cel zou door die regeling beantwoord worden zonder spoor.

**Een cel haalt niets zelf op.** Ook op het besluit-pad niet: ze zegt wat ze nodig
heeft (`Cell::acceptance_requests`) en krijgt het aangereikt. Het ophalen gebeurt
in `accept.rs`, buiten `src/cell/`, want een veiligheidscontext en een transport
houdt een cel niet (RFC-022 §2) — en dat is een compileerfout plus een grep-poort
in [`tests/observation_log.rs`](tests/observation_log.rs), geen afspraak.

Wat er met de waarde gebeurt, en vooral wat er níet met haar gebeurt:

- ze gaat als parameter de besluit-engine in, en met bron, bevoegd gezag van die
  bron, naam, moment en ondertekening het decretogram in (`InputOrigin::Accepted`;
  bij tier 3 zet de engine haar in `accepted_values` van het receipt);
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
waarde `computed`, `accepted` of teruggelezen, met bron — en dat is invariant I5
als gate:

```yaml
decide:
  - cell: toeslagen
    besluit: zorgtoeslag_vaststelling
    params: { bsn: '999993653' }
    op_moment: 2025-01-15
    expect_accepted:
      verleende_voorschotten: belastingdienst   # van die cel, niet nagerekend
    expect_read_back:
      toegekende_tegemoetkoming: zorgtoeslag_toekenning  # uit een eigen ouder gram
    expect_computed:
      - slotbedrag                              # hier uitgerekend, dus eigen werk
```

`expect_read_back` staat naast de andere twee en niet erin: een teruggelezen
waarde komt uit de eigen kroniek, maar de cel heeft haar hier niet vastgesteld —
ze is overgeschreven uit een ouder gram, onder het recht dat toen gold. Ze telt
daarom niet mee als `expect_computed`; wie haar daar toch noemt, krijgt rood.

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
  Hier: één context per cel, met één identiteit — het cel-id als adres
  (`cel:toeslagen`) en de naam waaronder de cel zich uitgeeft als bewering. Het
  wereldbestand is de plek waar die context aan de cel gebonden wordt. Geen
  medewerker, geen zaak, geen mandaat, geen autorisatie. Dat is de dunste vorm
  die de vraag openhoudt; komt er een fijnere binding, dan krijgt `Identity`
  velden en verandert er aan de aanroepers niets.

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
| **I5** | narekenen versus accepteren | `check_provenance` over elk decretogram, plus `expect_accepted`/`expect_read_back`/`expect_computed` per besluit — zie [Accepteren in plaats van narekenen](#accepteren-in-plaats-van-narekenen-i5) |

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

Naast die map staat [`scenarios/geweigerd/`](scenarios/geweigerd/), met dezelfde
opzet maar voor iets anders: scenario's die de gate niet eens halen omdat ze
eerder omvallen — vandaag het besluit van een cel die niet het bevoegd gezag is
(zie [Wie mag besluiten](#wie-mag-besluiten)). Ze worden gedraaid en afgerekend in
[`tests/bevoegd_gezag.rs`](tests/bevoegd_gezag.rs).

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
  achter de schuldenaar laat betalen — of, als deze wereld die cel niet kent, blijft
  openstaan (zie
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
| `cells` | de organisaties, elk met `identity`, `laws`, `komt_na`, `chronicles`, `lexostatus_definitions`, `besluit_definitions` en `accepts_from` |
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
| `expect_warnings` | de waarschuwingen die deze run moet melden: gemiste termijnen, en regelingen zonder bevoegd gezag |

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
    identity: Dienst Toeslagen                  # wie de cel zegt te zijn; mag
                                                # weg, dan is het het cel-id
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
            type: string                        # string | number | boolean | date
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

    komt_na:                                    # de namen waarvoor deze cel
      - Dienst Toeslagen                        # betalingen nakomt; een cel komt
                                                # haar eigen `identity` vanzelf na

    besluit_definitions:                        # wat de cel kan besluiten
      - name: zorgtoeslag_besluit               # een voorbeeld, geen echte
        doc: vrije toelichting                  # definitie: het toont elke vorm
                                                # die er is, niet één besluit
        regulation: wet_op_de_zorgtoeslag       # een eigen regeling
        output: heeft_recht_op_zorgtoeslag      # de uitkomst die het besluit ís
        outputs:                                # wat er in hetzelfde gram mee gaat
          - hoogte_zorgtoeslag
        zaakkenmerk: 'zorgtoeslag/{bsn}'        # {naam} = een gedocumenteerde
                                                # parameter; minstens één
                                                # (wannéér dit besluit afwijst
                                                # staat in de regeling, niet hier)
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
              zaakkenmerk: $zaakkenmerk         # de zaak van dít besluit
          toegekend_bedrag:                     # uit een eerder besluit van deze
            from_decretogram: zorgtoeslag_toekenning  # cel over dezelfde zaak
            field: hoogte_zorgtoeslag           # een uitkomst of input van dat gram
                                                # wat er betaald moet worden staat
                                                # in het lexogram van de regeling,
                                                # niet hier; zie Verplichtingen

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
          type: string                          # string | number | boolean | date
        - name: jaar
          type: number
        - name: ondertekend_op
          type: date                            # ISO op de draad: jjjj-mm-dd
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
      besluit: zorgtoeslag_besluit              # het formulier is dat van dit
                                                # besluit (zijn `params`)
                                                # of ze nu kan, volgt uit de
                                                # inputs van dat besluit

  - id: toeslagen.bekendmaking
    actor: toeslagen
    label: Maak het besluit bekend
    publishes:                                  # de volgende stage van de
      cell: toeslagen                           # procedure (RFC-008); geen
      besluit: zorgtoeslag_besluit              # formulier, en of ze nu kan
                                                # volgt uit de kronieken

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
    expect_read_back:                           # optioneel: uit een eigen ouder
      toegekend_bedrag: zorgtoeslag_toekenning  # gram teruggelezen
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
    besluit: zorgtoeslag_besluit                # de definitie hierboven
    params:
      bsn: '999993653'
    op_moment: 2024-06-01                       # bepaalt welke feiten de cel
                                                # kent én welke wetsversie geldt
    expect:                                     # optioneel: wat het gram draagt
      heeft_recht_op_zorgtoeslag: true
    expect_accepted:                            # optioneel: herkomst per waarde
      toetsingsinkomen: belastingdienst         # van die cel, niet hier berekend
    expect_read_back:                           # optioneel: uit een eigen ouder
      toegekend_bedrag: zorgtoeslag_toekenning  # gram teruggelezen
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
gaan de acties voor. Een `act` op een moment dat de klok al voorbij is, levert een
fout ("de klok loopt niet terug") en geen stilte: een actie gebeurt op de stand
van de klok en draagt haar moment niet mee het gram in, dus zou ze anders op een
andere dag landen dan het bestand noemt. Een `decide` draagt zijn moment zelf en
mag daarmee wél terugkijken; het gram krijgt dan de dag die er staat.

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

### Het gebeurtenisschema van een stroom

Een kroniekstroom kan naast `stream` en `key` een lijst `gebeurtenissen` dragen:
per gebeurtenisnaam de velden met hun type, het kanaal en de grondslag.

```yaml
- stream: betalingen
  key: zaakkenmerk
  gebeurtenissen:
    - name: betaling_gedaan
      intake: betaling
      grondslag: Algemene wet bestuursrecht, art. 4:89
      fields:
        - name: zaakkenmerk
          type: string
        - name: bedrag
          type: amount
```

Dit is het typeschema van een executogram, en dat is **generiek en
compile-time**: het geldt voor elke vastlegging van die naam, niet voor één
casus. Het hoort dus data te zijn en geen Rust — anders weet alleen de code wat
een betaling draagt, kan er geen tweede soort bij zonder een nieuwe versie, en
valt een typfout in een fixture pas op als de tijdlijn erlangs komt.

De typen zijn die van een gedocumenteerde parameter — `string`, `number`,
`boolean`, `date` — met `amount` erbij voor een bedrag. Eén stelsel voor beide,
want een veld dat via het formulier van een actie in een kroniek belandt gaat
door allebei de toetsen.

Wat het schema afdwingt:

- **onbekende naam** — een gebeurtenis die er niet in staat, hoort in een andere
  stroom of is een typfout;
- **ontbrekend veld** — wat gedeclareerd is, moet erin staan; hiermee valt ook
  een typfout in een veldnaam, want dan ontbreekt het gedeclareerde veld;
- **verkeerd type** — met één uitzondering: `null` komt overal doorheen. Dat is
  een uitspraak over het veld en geen ontbrekend feit (RFC-036) — het register
  zegt dat er geen partner is.

Een veld dat het schema *niet* noemt, is geen bezwaar: het schema is een
**ondergrens**. Een besluit legt zijn eigen uitkomsten in het gram, en die volgen
uit de regeling die het uitvoert; zou het schema ze ook moeten opsommen, dan
stond de wet twee keer opgeschreven.

De toets valt zo vroeg mogelijk: bij een `fixture` en bij het formulier van een
`action` bij het **optuigen** (beide staan dan al in het bestand), en bij een
levering op het moment van **vastleggen**. En `grondslag` op een gebeurtenis is
de default voor grammen van die naam: een gram dat er zelf geen draagt, krijgt
die van het schema — als veld in de kroniek, niet als weergave. Dat een aanvraag
op Awir art. 15 berust, geldt voor elke aanvraag, en dan hoort het één keer
opgeschreven te staan.

Een stroom **zonder** `gebeurtenissen` blijft toegestaan en wordt niet getoetst:
een kroniek van een organisatie die er nooit een schema bij schreef, is nog
steeds een kroniek, en de toets hoort erbij te komen doordat iemand hem
opschrijft.

Eén stroom is de uitzondering. `betalingen` is platformvocabulaire (zie [Verplichtingen: wat een besluit
achterlaat](#verplichtingen-wat-een-besluit-achterlaat)): het
platform declareert zelf wat `betaling_gedaan` en `betaling_gemeld` dragen —
zaakkenmerk, bedrag, volgnummer, besluit, schuldenaar en schuldeiser — en een
cel die een verplichting nakomt of oplegt moet een stroom houden waarvan het
schema dat dekt. Doet ze dat niet, dan weigert het optuigen met een melding die
de ontbrekende velden noemt; anders zou dat pas op de eerste vervaldatum blijken.
Declareert het artikel `richting_bij_negatief: omkeren`, dan kan er ook een
verplichting de andere kant op uit komen: dan vraagt het optuigen er
`terugvordering_gedaan` en `terugvordering_gemeld` bij, want die namen belanden in
dezelfde stroom en zouden anders pas bij de eerste omkering stranden.

Waar deze declaraties uiteindelijk wonen — in het wereldbestand of in eigen
bestanden naast de regelingen — is een later besluit over de plek van de
declaraties. Vandaag staan ze in het wereldbestand; verplaatsen is dan een
verhuizing en geen herontwerp.

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

Drie vormen, en precies één per actie:

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
- **`publishes`** maakt het laatste besluit van een cel **bekend**: de volgende
  stage van de procedure die de algemene wet voor een beschikking declareert. Zij
  heeft geen formulier — wat er bekendgemaakt wordt ligt al in de kroniek, en
  wannéér het gebeurt is de stand van de klok. Zie
  [De bekendmaking](#de-bekendmaking-een-tweede-gram-op-dezelfde-zaak).

Elk veld van zo'n formulier mag voorgevuld staan met wat de wereld al weet; zie
[Voorinvulling](#voorinvulling-wat-de-wereld-al-weet).

**Of een actie nu kan, staat nergens in het wereldbestand.** Het volgt uit de
definitie van het besluit dat ze start. Een besluit zegt daar al welke inputs het
uit welke eigen kroniek leest (`from_chronicle`) en welk eerder besluit over
dezelfde zaak het terugleest (`from_decretogram`); levert elk van die feiten op de
stand van de klok een waarde op — voor de parameters zoals het formulier ze
voorgevuld aanbiedt — dan kan de actie, en anders niet. Er is dus geen voorwaarde
om te schrijven en geen die uit de pas kan lopen met de inputs waarover ze gaat.

Kan een besluit nu niet, dan noemt de reden het **feit** dat ontbreekt: welke cel,
welke kroniekstroom, welk onderwerp en op welke dag er gezocht is. Het beeld van de
wereld draagt die reden bij de actie, en `act` weigert met dezelfde tekst — gewogen
op de waarden die de invuller verstuurt en niet op de voorinvulling, want wie een
ander onderwerp invult, vraagt om een besluit over díe zaak.

Een `records`-actie kan altijd. Zij *is* het feit; haar laten wachten tot er iets
ligt zou betekenen dat een actor niet kan vastleggen wat hem overkwam.

Een **bekendmaking** wordt langs dezelfde lijn afgeleid, uit dezelfde soort
droogloop maar over een andere kroniek: ligt er in `beschikkingen` een gram van de
stage `BESLUIT` waarvoor nog geen gram van de stage `BEKENDMAKING` bestaat? Zo ja,
dan kan ze; zo nee, dan noemt de reden het gram dat **ontbreekt** ("er ligt geen
gram van de stage BESLUIT") of het gram dat er **al ligt** (met de zaak en de dag
waarop ze bekendgemaakt is). Ook dat is een vraag die de cel bij zichzelf
beantwoordt.

Wat níet meetelt is een input die van een **andere organisatie** geaccepteerd wordt
(`accept_from`). Daar zou een vraag over een celgrens voor nodig zijn, en het beeld
van de wereld wordt bij elke stap opgevraagd — dan zou het openslaan van een scherm
verkeer opleveren dat niemand vroeg (invariant I1). Of de ander iets vastgesteld
heeft, weegt het besluit zelf, op het moment dat het genomen wordt. De check die
hier wél gebeurt, leest uitsluitend in de kronieken van de besluitende cel zelf en
staat dus niet in het vraaggraf en niet in het observatielog.

Een actie ontsnapt niet aan de invarianten. Lokt ze een besluit uit dat een waarde
van een andere cel accepteert, dan gaat dat contact over een celgrens en hoort de
tak in `query_graph` te staan — precies zoals bij een `decide`. De gate leest álle
besluiten van een run, of ze door een actie zijn uitgelokt of rechtstreeks genomen:
zouden die twee uit elkaar vallen, dan zou een actie een weg om I3 heen openen.
Zie [De vijf invarianten](#de-vijf-invarianten-en-de-gate-eronder).

Alles wat een actie belooft, wordt bij het optuigen getoetst: bestaat de actor,
bestaat de cel, houdt ze de stroom, kan die stroom haar sleutelveld uit het
formulier krijgen, en bestaat het besluit. Een actie die pas bij de eerste klik
omvalt, is een typfout die op het verkeerde moment boven water komt. De stroom met
decretogrammen (`beschikkingen`) is geen doel voor een actie: daar ontstaat een
gram door te besluiten.

### Voorinvulling: wat de wereld al weet

Een formulier hoort niet te vragen wat de wereld al weet. Elk veld — een
`fields`-veld van een `records`-actie, en een `params`-veld van een besluit dat
een actie start — mag daarom een `prefill` dragen:

```yaml
fields:
  - name: bsn
    type: string
    prefill: $last:brp.relaties.bsn   # de laatst vastgelegde waarde
  - name: jaar
    type: number
    prefill: 2024                     # een letterlijke waarde
  - name: ondertekend_op
    type: date                        # zonder prefill: de klok
  - name: gemeld_op
    type: date
    prefill: $clock                   # of met zoveel woorden
```

Drie vormen, en ze staan in het **wereldbestand**: een voorinvulling is
casusdata, net als het label van de actie. Er staat geen Rust die weet dat een
BSN bestaat.

| vorm | wat er komt te staan |
|---|---|
| een letterlijke waarde | die waarde, met haar soort — `2024` is een getal en niet de tekst `"2024"` |
| `$clock` | de stand van de logische klok, als `jjjj-mm-dd` |
| `$last:<cel>.<kroniek>.<veld>` | de laatste waarde die dat veld in die kroniek kreeg, op of vóór de klok |

Een **datumveld zonder `prefill`** krijgt de klok. Een actie draagt geen eigen
moment — ze gebeurt op de stand van de wereld — dus elke andere datum is een
correctie die de invuller bewust maakt, en de klok overtypen is nooit het werk.
Staat er wél een `prefill`, dan wint die: een opgave in het bestand is een keuze.

`$last` levert **niets** als er nog niets ligt, en dat is een antwoord en geen
fout: het veld staat dan leeg en de actie kan gewoon. Zo volgt een voorinvulling
de kroniek in plaats van er een tweede kopie van te zijn — de besluit-parameter
die naar de ontvangen aanvraag wijst, is leeg vóór de aanvraag en gevuld erna.
Waar de verwijzing naar wijst wordt bij het optuigen getoetst, maar alleen tot en
met de **kroniek**: welke velden een stroom kent, blijkt uit wat erin ligt, en een
kroniek die pas tijdens de run gevuld wordt, kent er bij het optuigen nog geen.

Een `$`-woord dat geen van beide verwijzingen is, wordt geweigerd in plaats van
als letterlijke tekst doorgegeven. Anders staat er straks `$clok` in een kroniek,
en niemand die het merkt. Om dezelfde reden gaat een voorinvulling die nu al een
waarde ís — een letterlijke waarde of `$clock` — bij het optuigen door de
typetoets van haar veld: `2024` in een `string`-veld valt daar, en niet pas als
iemand op "uitvoeren" drukt over een waarde die hij nooit getypt heeft. Waar een
`$last` op uitkomt is bij het optuigen nog niet bekend; die staat gewoon in het
veld en valt bij het versturen door dezelfde toets als wat de invuller zelf typt.

Een `$last` mag naar een **andere cel** wijzen dan die het formulier draagt — het
aanvraagformulier van de burger stelt het BSN voor dat het register al kent — en
dat is geen celgrensoverschrijding. De wereld leest hier haar eigen cellen zoals
ze dat voor het beeld ook doet, buiten de veiligheidscontext en het transport om:
er komt geen `crossing` van en geen regel in het observatielog, want er wordt
geen vraag gesteld. Wat eruit komt is een suggestie op een scherm; een feit wordt
het pas als de invuller het verstuurt, en dan legt de cel het op eigen naam vast.

Een voorinvulling is een **voorstel**, geen feit. De invuller kan er iets anders
van maken, en wat vastgelegd wordt is wat hij verstuurt; aan het gram is later
niet te zien wat er voorgesteld stond. Het beeld van de wereld draagt de
voorinvulling dan ook per actie en **opgelost** (`actions[].prefill`) — de
verwijzing blijft in het bestand, want wie haar naar buiten zou sturen, laat de
frontend zelf in de kronieken zoeken.

Wat er met opzet níet in zit, is een berekening: dit is geen tweede
reductietaal. Een voorinvulling wijst iets aan dat er al ligt, of ze noemt een
waarde. Wie een bedrag uit de wet wil voorstellen, laat de wet rekenen.

De `inputs` van een lexostatus dragen dezelfde parametervorm, maar daar is geen
formulier: een vraag aan een cel komt van een consument en niet uit een invulveld.

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

Een gemiste termijn is niet de enige waarschuwing. Ze dragen allemaal hun
`soort`, zodat een lezer — en de frontend — ze uit elkaar kan houden in plaats
van ze op hun veldnamen te moeten herkennen:

| `soort` | waar ze over gaat | waaraan `expect_warnings` haar herkent |
|---|---|---|
| `gemiste_termijn` | een termijn verstreek zonder dat het feit er lag | het `label` uit het wereldbestand |
| `geen_bevoegd_gezag` | er is besloten onder een regeling die geen bevoegd gezag declareert (zie [Wie mag besluiten](#wie-mag-besluiten)) | `regeling '<$id>' declareert geen bevoegd gezag` |

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
| `cells[].lexostatussen` | per gepubliceerde naam haar `doc`, de parameters met hun type, de uitkomsten, en — bij een kroniekfilter — de `key`: de stroom en het sleutelveld waarop gereduceerd wordt |
| `cells[].besluiten` | per besluit zijn `doc`, het **zaakkenmerk-sjabloon** en de kroniek waarin de decretogrammen landen |
| `cells[].besluiten[].schema` | het [schema van het decretogram](#het-schema-van-het-decretogram-wat-komt-uit-de-wet): per veld het type en wie het declareert, met `gat: true` waar geen enkel lexogram het dekt |
| `cells[].chronicles[].grams` | elk gram met zijn soort (`lexogram`/`decretogram`/`executogram`), moment, kanaal, grondslag en velden |
| `…grams[].fields[].origin` | de herkomst per waarde |
| `actions` | elke actie met haar formulier, en of ze nu kan |
| `actions[].prefill` | de [voorinvulling](#voorinvulling-wat-de-wereld-al-weet) per veld, opgelost op de stand van de klok |
| `crossings` | wat er over een celgrens ging |
| `warnings` | de termijnen die verstreken zonder dat het feit er lag |
| `journal` | het [journaal](#het-journaal-wie-deed-wat-en-wat-veranderde-er): één regel per gebeurtenis, in volgorde van ontstaan |

Drie dingen om bij stil te staan:

**Wat een cel belooft, staat erbij.** Een consument die een lexostatus vraagt,
moet precies de gedocumenteerde parameters meegeven; wat die zijn stond tot nu toe
alleen in het wereldbestand, en een vrager moest het dus al weten om het te kunnen
vragen. Nu staat het in het beeld: de toelichting, de parameternamen met hun type,
en of een parameter de **sleutel** van een kroniek is. Wie dat laatste weet, kan de
kroniek in hetzelfde beeld erbij pakken en zien welke waarden er nu onder die
sleutel liggen — en het zaakkenmerk-sjabloon bij de besluiten zegt welke *vorm* zo'n
sleutel heeft. Het blijft een inspectiebeeld: er staat niets in wat niet al bij het
optuigen vastlag.

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
eraan had, is de herkomst hierboven. Nakijken kan wél, **op verzoek en per gram**:
`World::gram_receipt(cel, kroniek, plek)` geeft het receipt van één decretogram
(zie hieronder). Het `lexogram` in de lijst met gram-soorten
komt in geen enkele kroniek voor: de wet is generiek en van niemand in bijzonder,
en welk recht een cel laadt staat in `cells[].laws`. De variant staat er zodat een
lezer één vocabulaire voor alle drie de grammen heeft.

Het beeld is een **inspectiebeeld**, net zoals het observatielog een meetinstrument
is. De wereld bezit de cellen en zij maakt het; een cel kan het niet opvragen en
kan er dus niet de kroniek van een ander mee lezen. `crossings` is het materiaal
van dat log, en een lezer hoort het als zodanig te labelen: wie deze lijst houdt,
kent de unie van wat over de grenzen ging — precies het totaalbeeld waarvan geen
enkele cel er een heeft.

### Het receipt van één gram

`World::gram_receipt(cel, kroniek, plek)` geeft het RFC-013 Execution Receipt van
één decretogram. Alleen lezen, net als het beeld en een reductie; `plek` is de
plek in de kroniek geteld vanaf nul, in precies de volgorde waarin het beeld de
grammen geeft.

Waarom dit er naast het beeld staat: een decretogram *ís* het receipt (RFC-022
§1.2), maar het beeld kan het niet dragen (wandkloktijd), en daarmee was van
buitenaf niet te zien dát het gram het draagt. Deze weg maakt dat na te kijken
zonder het beeld te vervuilen — wie er niet om vraagt, ziet het niet.

Wat eruit komt is het receipt zoals het gram het draagt, met drie dingen die het
ruwe veld niet heeft (`src/receipt.rs`):

| veld | wat het toevoegt |
|---|---|
| `gram` | van welk gram dit het receipt is: cel, kroniek, plek, naam, besluit, zaakkenmerk en het moment in de *logische* tijd |
| `accepted_values` | de vereniging van beide acceptatiewegen (`accept_from` en een cel-bron van de wet), per waarde met de bron-cel, het **bevoegd gezag dat die bron noemde**, de lexostatus, het moment, het zaakkenmerk en de ondertekening |
| `timestamp` | de wandkloktijd, met erbij dat het dát is en geen moment in de logische tijd van de wereld |

De **uitvoeringstrace** komt mee in `results.trace`, precies zoals het gram haar
draagt (zie [De trace zit in het gram](#de-trace-zit-in-het-gram)). Het
Grammen-tabblad van de frontend zet haar neer als inklapbare boom, per stap met
de regeling, het artikel, de bewerking en de uitkomst.

Elke andere sectie gaat ongewijzigd door, dus een RFC-013 die morgen een sectie
toevoegt staat hier morgen in beeld. Wijst de vraag naar een gram dat geen
decretogram uit het besluit-pad is, dan is het antwoord
`SimulatorError::GramWithoutReceipt` — "er is er geen" en niet een leeg receipt,
want er heeft nooit een uitvoering gedraaid.

Het gezag van de bron komt uit het antwoord zelf: publiceert de bevraagde
lexostatus een uitkomst `competent_authority`, dan legt `InputOrigin::Accepted`
hem vast bij het accepteren. Zegt zij er niets over, dan staat er `null` — een gat
bij de bron, en geen reden om het cel-id onder een andere naam te herhalen: een
adres is geen gezag.

### Het journaal: wie deed wat, en wat veranderde er

De kronieken zeggen wat er per cel **ligt**. Het journaal zegt hoe het zover kwam:
één regel per gebeurtenis, in de volgorde waarin ze ontstond. Een gebeurtenis is
een actie van een actor (`records` of `decides`), een trigger van de klok (een
startstand die passeert, een vervallen verplichting, een verstreken termijn), of
een vraag die over een celgrens ging.

| veld | wat |
|---|---|
| `seq` | de plek in het journaal, vanaf 0 — waarnaar `parent` verwijst |
| `moment` | het moment in de logische tijd |
| `actor` | `actor` (een actie), `cell` (een cel die zelf besloot of vroeg) of `klok` |
| `kind` | `vastlegging`, `besluit`, `betaling`, `termijn` of `vraag` |
| `description` | korte omschrijving, in de woorden van het wereldbestand |
| `grams` | verwijzingen naar de grammen die erdoor ontstonden: cel, kroniek, gram-id (`<cel>|<kroniek>|<plek>`) |
| `changes` | wat er aan de stand van de zaak veranderde, per betrokken cel |
| `accepted` | de waarden die dit besluit van een andere cel accepteerde |
| `executed` | bij een `besluit`: wat er uitgevoerd is — zie hieronder |
| `question` | het contact zelf, bij een `vraag`-regel — dezelfde vorm als in `crossings`, mét het antwoord en de uitleg waarop het berust |
| `parent` | de regel die deze uitlokte; een cross-cel-vraag hangt onder haar besluit |

Het is **geen tweede administratie**. Een regel wijst naar grammen die in een cel
liggen en draagt er geen kopie van. Het enige dat er staat en nergens in een gram
ligt, is het verschil in de stand van de zaak — en dat is een meting.

#### Wat een besluit uitvoerde

Een besluitregel noemt niet alleen dát er besloten is, maar ook wat er gebeurd
is. `executed` draagt drie dingen, alle drie uit het decretogram waar de regel
naar wijst — het gram legt ze vast onder `executed_regulations`, `inputs` en de
uitkomsten zelf:

| veld | wat |
|---|---|
| `regulations` | de uitgevoerde regelingen, met de `valid_from` van de versie die op het moment van het besluit gold; de regeling van het besluit vooraan |
| `inputs` | de waarden waarop gerekend is: naam, waarde, en de herkomst zoals het gram haar opschreef (`parameter`, `eigen_kroniek`, `eerder_besluit`, `geaccepteerd`) |
| `outputs` | de uitkomsten die het besluit vastlegde: naam en waarde |

`regulations` is méér dan de regeling waarop het besluit gaat: een uitvoering kan
er meer aanroepen — een uitvoeringsregeling die een bedrag levert, een kaderwet
die een begrip invult (RFC-007) — en zonder die is niet te zien onder welk recht
een bedrag tot stand kwam. Het is ook **minder** dan `scope.loaded_regulations`
uit het receipt: daar staat elke versie in die de cel geladen heeft, ook een
versie die op dit moment niet gold en een regeling die deze uitvoering niet
geraakt heeft.

Eén grens: een aanroep die de engine bewust oversloeg — een verplichte parameter
noemde niemand, dus de regeling draaide niet en de input werd een afwezigheid
(RFC-036) — is in de herkomst van een input niet te onderscheiden van een aanroep
die wél draaide, en staat er dus ook in.

Een geaccepteerde input draagt in haar herkomst de cel en de lexostatus waarmee
ze opgehaald is; daarmee is ze te koppelen aan de `vraag`-regel die onder dit
besluit hangt, en die regel draagt het antwoord zoals de andere cel het gaf, met
de [uitleg](#hoe-het-antwoord-tot-stand-kwam) waarop het berust. De frontend
doet precies dat.

#### Statusindicatoren zijn casusdata

Welke reducties "de stand van de zaak" van een cel dragen, staat per cel in het
wereldbestand:

```yaml
    status_indicators:
      - lexostatus: zorgtoeslagbeschikking   # een gepubliceerde naam van deze cel
        label: toekenningspositie            # wat een lezer ziet
        params:
          zaakkenmerk: $zaakkenmerk          # uit een veld van de gebeurtenis
```

`$veld` wijst een veld van de gebeurtenis aan — het zaakkenmerk van een besluit,
de bsn van een feit — en alles zonder `$` is een letterlijke waarde. Draagt een
gebeurtenis dat veld niet, dan gaat de indicator er niet over en wordt hij niet
gereduceerd: een betaling zegt niets over een partnerschap. Dat de lexostatus
bestaat en dat de parameters precies kloppen, valt bij het optuigen en niet bij
de eerste meting — een indicator die stil nooit iets oplevert, is niet van "er
gebeurde niets" te onderscheiden.

Voor elke gebeurtenis reduceert de wereld de indicatoren van de **geraakte**
cellen vóór en ná, op het moment van de gebeurtenis, en zet het verschil in de
regel: `niets vastgesteld → 25000`, `0 → 49294`. Geen verschil is geen regel.

#### De meting is geen celgrens-verkeer

De vóór/ná-reductie loopt langs `Cell::reduce` — de publieke ingang van de cel —
buiten de veiligheidscontext en het transport om, precies zoals de
[voorinvulling](#voorinvulling-wat-de-wereld-al-weet) van een formulier dat doet.
Er wordt dus geen vraag over een grens gesteld: er komt **geen `crossing`** van,
geen regel in het observatielog, en de invarianten-gate ziet er niets van. De
wereld meet hier haar eigen opstelling, zoals ze ook het beeld van alle kronieken
maakt; een meting die zichzelf als verkeer laat tellen zou het vraaggraf
vervuilen met vragen die het recht niet stelt.

Een reductie die niet lukt, levert geen meting en geen fout. Dat is de juiste
kant op: een indicator waarvan de reductie buiten haar eigen cel zou reiken loopt
daar stuk (de cel krijgt geen resolver mee), en dan hoort er geen regel te komen
in plaats van een gebeurtenis om te vallen.

#### Eén bron

`ScenarioRun::report()` schrijft dit journaal op als verhaal, bovenaan het
verslag, en de frontend toont dezelfde regels. Het wordt niet twee keer afgeleid:
de wereld houdt het bij op de plek waar de gebeurtenissen ontstaan, en `journal`
in het beeld en `ScenarioRun::journal` zijn dezelfde lijst.

### Kroniekstromen en tijd

Per stroom geldt: alleen vastleggingen met `op_moment <= ` het gevraagde moment
tellen mee, en van de rest wint per sleutelwaarde de laatste vastlegging — **in
haar geheel**. Dit is de weg naar de engine, en het is dezelfde regel als die
van het kroniekfilter (zie [Twee reductievormen](#twee-reductievormen)): een
bron-cel en een engine horen over dezelfde kroniek hetzelfde te zien. Een vraag
over een moment in het verleden levert dus het beeld van toen. De stromen worden
aan de engine aangeboden als databronnen, waar ze de inputs van de eigen
regelingen invullen. Twee stromen met dezelfde naam worden geweigerd: de
stroomnaam is tevens de naam van de databron, dus daar zou de tweede de eerste
stil schaduwen.

In haar geheel, en niet veld voor veld — dat is geen detail. Een toestandsmerge
zou de velden van verschillende vastleggingen, op verschillende momenten, over
elkaar leggen tot één record dat als vastlegging nooit bestaan heeft. De paper
legt de nadruk op het omgekeerde: niet de resulterende toestand opslaan, maar de
procesrelatieve vaststelling ("op moment T heeft actor X vastgesteld dat …"), en
RFC-022 zegt het de engine na — de reductie redeneert over *vaststellingen op
momenten*, niet over een toestand van de wereld. Wat samen ontstond blijft samen;
wat apart ontstond wordt niet stil samengevoegd. Een veld dat de laatste
vastlegging niet draagt, is op dat moment dus niet vastgesteld, ook als een
eerdere het wél droeg. Wie dat veld toch nodig heeft, legt het opnieuw vast — en
dan staat er ook bij wanneer en waarlangs.

Wat een besluit uit die stromen las, is na te lopen: het gram draagt per stroom
haar stand op dat moment (`chronicle_sources`), zie
[Wat een decretogram draagt](#wat-een-decretogram-draagt).

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

**Een afwijzing is de enige uitweg naast toewijzen.** Buiten behandeling stellen
(Awb 4:5) en horen vóór afwijzing (Awb 4:7) bestaan hier niet, een grond die niet
als boolean-uitkomst in een regeling staat kan niet afwijzen, en `decision_type`
is geen open vocabulaire: het gram draagt wat de regeling aanwijst, of `AFWIJZING`.
Zie [Een weigering is ook een besluit](#een-weigering-is-ook-een-besluit).

**Een verplichting kent geen rente en geen verzuim.** Een termijn vervalt en wordt
betaald; wat er gebeurt als er te laat of niet betaald wordt, staat er niet — geen
rente, geen aanmaning, geen dwangbevel (Awb 4:97 e.v.). Verrekenen gebeurt wél, maar
als **regel in de wet** en niet als iets dat het platform met een schema doet: Awir
art. 19 trekt de verleende voorschotten van de vastgestelde tegemoetkoming af en legt
alleen het slotbedrag op. Komt dat slotbedrag onder nul, dan is er een richting nodig:
`richting_bij_negatief: omkeren` maakt er een terugvordering van (zie
[Verplichtingen](#verplichtingen-wat-een-besluit-achterlaat)), en zonder die
declaratie valt het besluit om. Awir art. 19 declareert haar nog niet, dus een
vaststelling die lager uitkomt dan het voorschot loopt daar vast in plaats van terug
te vorderen — de grondslag daarvoor (art. 24, derde lid) staat nog niet in dat blok.

Twee besluiten op hetzelfde artikel leggen allebei het volle schema op: het tweede
verrekent niet met het eerste, en een terugvordering is een eigen verplichting naast
het voorschot en geen correctie erop. Een verplichting kan ook niet gewijzigd of
ingetrokken worden: het schema staat in het gram, en een gram verandert niet — en dat
is meteen waarom de vaststelling in de publieke wereld ná de laatste voorschottermijn
staat. Een vaststelling zet de nog openstaande termijnen van het voorschot niet stop:
die vervallen gewoon door, en het slotbedrag komt er dan bovenop. Wat de wereld
verrekent is daarom wat er op dat moment betaald is, niet wat er verleend is.

**Beschikbaarheid kijkt alleen naar de eigen feiten van een besluit.** Een besluit
dat alles van een ander accepteert, heet dus altijd mogelijk, ook als die ander nog
niets heeft vastgesteld — dat blijkt pas bij het besluit zelf. Dat is geen
omissie maar de prijs van invariant I1: een check die het wél zou weten, zou een
vraag over een celgrens moeten stellen bij elk beeld van de wereld. Wie een actie
op zo'n feit wil laten wachten, laat het als levering in de eigen kroniek van de
besluitende cel landen; dan is het een eigen feit en telt het gewoon mee.

**De procedure loopt van besluit tot bekendmaking, en niet verder.** De stages
ervóór (AANVRAAG, BEHANDELING) zijn hier gewone acties en geen stage van de
engine, en alles ná de bekendmaking — bezwaar, beroep, het opnieuw bekendmaken
van een gewijzigd besluit — bestaat niet. Een besluit kan dus wel bekendgemaakt
worden en daarna niets meer; wat een belanghebbende ertegen kan beginnen, staat
alleen als tekst in het gram (`bezwaar_bij`, `bezwaar_termijn_weken`) en is geen
stap die deze wereld kent. Zie
[De bekendmaking](#de-bekendmaking-een-tweede-gram-op-dezelfde-zaak).

**Een hook kan geen letterlijke getypeerde waarde aan een ander artikel
meegeven.** `source.parameters` draagt alleen tekst en `$verwijzingen`: een
getal, een booleaanse waarde of een datum die er letterlijk in staat, wordt een
tekst — en het JSON-schema staat er ook niets anders toe. Een artikel dat een
ander artikel "zes weken" wil aanreiken, kan dat dus niet langs die weg. De
testregeling hierboven rekent daarom op wat de **stage** binnenkrijgt
(`bekendmaking_datum`, `competent_authority`) en op een eigen uitkomst voor het
aantal weken (art. 6:7), die de andere artikelen als gewone input ophalen.
Getypeerde `source.parameters` zouden een wijziging van het schema vragen, en
dat is een eigen beslissing.

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
