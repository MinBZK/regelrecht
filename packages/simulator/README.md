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
[Het besluit-pad](#het-besluit-pad).

Gaat een vraag over een celgrens, dan loopt hij langs de **veiligheidscontext**
van de vragende cel naar het **transport**, en dat is de enige weg. Zie
[Over een celgrens](#over-een-celgrens). Wat daar langs gaat is te meten met het
**observatielog**, een test-only instrument dat met opzet buiten de band staat:
[Het observatielog](#het-observatielog-buiten-de-band).

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
eigen regeling, of één kroniekfilter dat per sleutelwaarde de laatste
vastlegging kiest) zijn een eerste benadering van wat de paper en RFC-022 §4.1
bedoelen, namelijk filteren en aggregeren over kronieken. Het filteren staat er
nu; aggregeren (som, telling) nog niet.

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
  latest: true                    # de enige modus; mag weggelaten worden
  where:                          # optioneel, gelijkheid op velden
    partnerschap_type: HUWELIJK
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
4. de uitkomst gaat als één gram de stroom `beschikkingen` in.

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
[Ondertekening is gesimuleerd](#ondertekening-is-gesimuleerd)). Verplichtingen
die uit een besluit volgen horen ook bij §1.2 en staan er nog niet.

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
  vastgelegd. Doorrekenen met een gat is erger dan geen besluit.

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

De invarianten-gate die het gedeclareerde vraaggraf met het feitelijke vergelijkt
(I3) staat er nog niet. Dit is het instrument waar die op gaat rusten, en daar
hoort die volledigheidseis dan ook thuis.

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
  generiek — een gesorteerde lijst van `(datum, trigger)` — zodat vervallende
  verplichtingen en gemiste termijnen er later naast passen. Vandaag bestaat er
  één soort: een `fixture` die bij het passeren wordt vastgelegd.
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

Een wereld is één YAML-bestand: `clock` (de tijdlijn), `cells` (wie er zijn),
`fixtures` (de startstand), `decide` (welke cel wanneer waarover besluit) en twee
soorten vraag: `queries` (een consument bevraagt een cel) en
`query_via_transport` (een cel bevraagt een andere cel). Elk draagt zijn eigen
verwachting. De assertie hoort bij het bestand, niet bij Rust: een nieuw
testgeval is een nieuw bestand.

```yaml
name: korte naam van het scenario
description: waarom dit scenario bestaat        # optioneel

clock:
  start: 2024-01-01                             # waar de klok begint; verplicht

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
          latest: true                          # de enige modus; mag weg

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
```

`query_via_transport` is een **sonde, geen onderdeel van de opstelling**. In de
opstelling stelt een cel zo'n vraag uitsluitend vanuit haar besluit-pad: ze heeft
een input nodig die een andere organisatie vaststelt, en dan *accepteert* ze die —
zie [Accepteren in plaats van narekenen](#accepteren-in-plaats-van-narekenen-i5).
Wat deze stap overhoudt, is één vraag los kunnen stellen en op haar antwoord
asserteren zonder er een besluit omheen te bouwen: handig om de naad zelf te
beproeven, en verder niets.

De stappen lopen in deze volgorde: eerst de besluiten, dan de vragen van een
consument, dan die over een celgrens. Dat past bij wat ze zijn — een besluit is
een gebeurtenis op de tijdlijn, een vraag kijkt erop terug — en het maakt niets
onmogelijk: een vraag over een moment *vóór* een besluit levert nog steeds het
beeld van toen, want de reductie filtert zelf op `op_moment`. De klok gaat vóór
elke stap vooruit tot haar moment, zodat een levering die ertussen valt eerst
landt.

Een `decide` mag zonder `expect`, anders dan een vraag: hij legt iets vast, en
bewijst daarmee ook zonder verwachting iets — namelijk dat de vragen erna iets te
vinden hebben.

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

**Een besluit trekt nog geen verplichtingen na zich aan.** Een beschikking die
een bedrag toekent, brengt in de echte wereld betalingen voort met een
vervaldatum; die horen bij het decretogram (RFC-022 §1.2) en worden bij het
verstrijken van de tijd executogrammen. De trigger-lus in de wereld is er al op
gebouwd, maar er is nog geen soort trigger voor.

**Een besluit wordt nog niet door een actie uitgelokt.** Het scenario zegt in
`decide` wie wanneer waarover besluit; er is nog geen `World::act` waarmee een
actor (een aanvraag indienen, een opgave doen) een cel aan het werk zet. Dat is
de reden dat `decide` een stap in het bestand is en geen gevolg van iets anders.

**Onbetrouwbaar gedrag tussen cellen bestaat niet.** Een bron-cel is er altijd, ze
antwoordt meteen, en haar antwoord is nooit verouderd of in tegenspraak met dat van
een ander. Accepteren is dus alleen uitgewerkt voor het geval dat goed gaat plus het
geval "niets vastgesteld"; het moeilijkste deel van elk decentraal systeem staat
nog open. Zo ook **echte ondertekening** en de RFC-009-modi als configuratie: de
ondertekening in een geaccepteerde waarde is de placeholder uit
[Ondertekening is gesimuleerd](#ondertekening-is-gesimuleerd).

Aan de kant van de celgrens ontbreken verder de **invarianten-gate** die het
gedeclareerde vraaggraf uit het scenario vergelijkt met het feitelijke uit het
observatielog (I3) — de herkomstgate van I5 draait wel, bij elk besluit — en
**autorisatie**: de veiligheidscontext kent identiteit en ondertekening, en beslist
nog niets over wat mag.

Ook een **HTTP-transport** is er niet; dat is het punt van de trait. Komt het er,
dan is dat een tweede implementatie naast `InProcessTransport` en geen wijziging in
`Cell` — en als dat laatste wél nodig blijkt, was de naad op de verkeerde plek
gelegd. Asynchrone intake met een echte tijdlijn volgt apart. De indeling
anticipeert erop: de reductielogica woont in [`src/cell/`](src/cell/) en niet in de
scenario-runner, zodat een latere `packages/cell` een verplaatsing is en geen
herschrijving.
