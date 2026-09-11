# regelrecht-simulator

Testopstelling voor chronolexografie ([RFC-022](../../docs/src/content/rfcs/rfc-022.md)).
De crate simuleert een wereld van **cellen** en laat een scenario die wereld
optuigen en bevragen.

Deze eerste versie is bewust krap: één cel per scenario, geen verkeer tussen
cellen. Wat er wel al staat, is de grens — en die is met opzet een taalgrens en
geen afspraak.

Wetten zijn optioneel. Een cel met `laws: []` is een **bron-cel**: ze legt vast
en reduceert, zonder engine. Zie [Een bron-cel](#een-bron-cel).

## De drie chronolexogrammen, en waar ze hier zitten

De paper onderscheidt drie soorten chronolexogram (RFC-022 §1.1–§1.2). Deze
crate kent ze alle drie bij naam, maar legt er nog maar één zelf vast — de kaart
van paper naar code:

- **Lexogram** — generiek en compile-time: het recht zelf. Hier is dat elk
  versiebestand onder `laws`. De map met `valid_from`-versies van een regeling
  *is* de lexogram-kroniek; de engine kiest daarin op `op_moment`. Niemand
  noemde dat hier eerder zo, maar het is precies wat RFC-022 §1.1 beschrijft.
- **Executogram** — de vastgelegde vaststelling van een feit. Dat is wat een
  `ChronicleEvent` wil worden: een veldwaarde op een moment. Wat er nog aan
  ontbreekt is de procesrelatieve kant (wie stelde het vast, op welke
  grondslag, in welke intake).
- **Decretogram** — individueel en operationeel: het besluit. Dat wordt hier
  nog nergens vastgelegd; zie [Wat hier nog niet staat](#wat-hier-nog-niet-staat).

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

Wat een bron-cel (nog) niet kan, is besluiten: daar hoort een regeling bij, een
bevoegd gezag en een vastgelegd decretogram. Vastleggen en reduceren is genoeg
om een cel te zijn.

## Wat een cel níet is

Precies de drie dingen die er in de praktijk bij gedacht worden (RFC-022 §2):

- **Geen sleutels.** Ondertekening, trust material en de autorisatie waaronder
  een vraag beantwoord wordt, zitten in de veiligheidscontext, niet in de cel.
- **Geen bevoegd gezag.** `competent_authority` (RFC-002) is een juridisch feit
  van het besluit, geen eigenschap van de opslag. Een cel houdt kronieken van
  besluiten waarvoor een ander bevoegd is.
- **Geen synthese.** Een reductie raakt uitsluitend de eigen kronieken.
  Combineren over cellen heen doet een consument, nooit een cel.

Twee dingen dwingen dat af in code in plaats van in proza:

1. `Cell` houdt haar `ChronicleStore` in een **privéveld** zonder accessor. Er
   is geen `pub fn store()` en die komt er ook niet: dat een andere cel niet bij
   deze feiten kan, moet een compileerfout zijn.
2. De reductie mag alleen een regeling gebruiken die de cel zélf laadt. Een
   definitie die naar een vreemde regeling wijst, wordt geweigerd bij het
   optuigen van de cel — niet pas bij de eerste vraag.

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
antwoord niet, en de engine kiest op datzelfde moment de regelingversie.

## Scenarioformaat

Een scenario is één YAML-bestand met twee blokken: `cells` (de wereld) en
`queries` (de vragen plus wat ze moeten opleveren). De assertie hoort bij het
scenario, niet bij Rust: een nieuw testgeval is een nieuw bestand.

```yaml
name: korte naam van het scenario
description: waarom dit scenario bestaat        # optioneel

cells:
  - id: toeslagen                               # het cel-id
    laws:                                       # regelingen bij $id; de loader
      - wet_op_de_zorgtoeslag                   # laadt alle versies uit de map
      - regeling_standaardpremie

    chronicles:                                 # de eigen feiten van de cel
      - stream: relaties                        # naam van de kroniekstroom
        key: bsn                                # veld waarop gegroepeerd wordt
        events:
          - op_moment: 2023-01-01               # wanneer dit feit feit werd
            fields:                             # veldnamen = input-namen in de wet
              bsn: '999993653'
              partnerschap_type: HUWELIJK
          - op_moment: 2024-07-01               # latere vastlegging wint
            fields:
              bsn: '999993653'
              partnerschap_type: GEEN

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
```

Onbekende velden worden geweigerd, zodat een typfout niet stil verdwijnt. Elke
vraag heeft minstens één verwachting: een vraag zonder `expect` en zonder
`expect_not_established` slaagt altijd en zou als `ok` in het verslag komen, wat
op bewijs lijkt en het niet is. De loader weigert zo'n scenario, en ook een vraag
die beide verwachtingen tegelijk stelt — die kan nooit uitkomen.

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
hoort niet stil samengevoegd te worden. De echte vorm kan pas als een
vastlegging `recording_actor`, `grondslag`, `intake` en een moment draagt
(RFC-022 §1.3) en de reductie daarover filtert en aggregeert in plaats van
alleen te overschrijven.

Het kroniekfilter van een bron-cel doet dat al niet: dat kiest één vastlegging en
geeft die in haar geheel terug. Die vorm kan hier omdat er geen engine tussen zit
die records wil.

De dag is de fijnste korrel van de tijdas. Twee vastleggingen op hetzelfde
`op_moment` vallen daar niet uit elkaar te houden; dan beslist de volgorde in het
bestand, en de laatste wint. Wie ze wél wil ordenen heeft een fijnere tijdas
nodig, geen andere schrijfvolgorde.

Feiten die in de echte wereld van een andere organisatie komen, staan hier als
binnengekomen feit in de eigen kroniek. Zolang er geen transport tussen cellen
is, is dat het eerlijke model: de cel kan niets ophalen wat ze niet zelf heeft.
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
`REGULATION_PATH` overschrijft dat.

## Wat hier nog niet staat

**De cel legt nooit zelf iets vast.** Haar kroniekstore wordt bij het optuigen
gevuld en daarna alleen gelezen; alles hierboven is de leeshelft. In
chronolexografie is vastleggen juist de *productieve* activiteit, en de lus is
informeren → concluderen → **vastleggen**. Een reductie die
`heeft_recht_op_zorgtoeslag: true` oplevert is een besluit, en dat hoort als
decretogram (BESCHIKKING, met moment en `zaakkenmerk`) in de eigen kroniek te
landen (RFC-022 §1.2), zodat een latere vraag *op dat moment* het besluit
terugvindt in plaats van het opnieuw uit te rekenen onder een mogelijk andere
wetsversie. Precies dat verschil — decretogram tegenover lexogram — is wat deze
opstelling wil laten zien, en zonder vastleggen valt het niet te demonstreren;
"een besluit van een andere bevoegde organisatie accepteren" heeft er evengoed
een vastgelegd besluit voor nodig. De eerstvolgende stap is daarom een
**vastleg-pad naast `reduce`**, alleen aanroepbaar door de cel zelf: het is
haar eigen kroniek, dus geen consument mag erin schrijven, net zomin als eruit
lezen.

Verkeer tussen cellen (`CellTransport`), de veiligheidscontext, het
observatielog, meerdere cellen in één scenario en asynchrone intake met een
echte tijdlijn volgen apart. De indeling anticipeert erop: de reductielogica
woont in [`src/cell/`](src/cell/) en niet in de scenario-runner, zodat een
latere `packages/cell` een verplaatsing is en geen herschrijving.
