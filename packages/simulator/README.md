# regelrecht-simulator

Testopstelling voor chronolexografie ([RFC-022](../../docs/src/content/rfcs/rfc-022.md)).
De crate simuleert een wereld van **cellen** en laat een scenario die wereld
optuigen en bevragen.

Deze eerste versie is bewust krap: één cel per scenario, geen verkeer tussen
cellen. Wat er wel al staat, is de grens — en die is met opzet een taalgrens en
geen afspraak.

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

En de vierde term, de **reductie**: de huidige vorm (één uitkomst van één eigen
regeling, per veld wint de laatste vastlegging) is een eerste benadering van wat
de paper en RFC-022 §4.1 bedoelen, namelijk filteren en aggregeren over
kronieken.

## Wat een cel is

Een cel is een *containment- en autonomiedomein*, en verder niets:

- ze houdt haar eigen **kronieken** (de feiten die zij zelf vastlegde);
- ze laadt haar eigen **regelingen**;
- ze **reduceert** over die eigen feiten tot een **lexostatus**: de
  rechtstoestand vanuit een gevraagd perspectief, op de feiten die zij kent.

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
```

Onbekende velden worden geweigerd, zodat een typfout niet stil verdwijnt. Elke
vraag heeft minstens één verwachting: een vraag zonder `expect` slaagt altijd en
zou als `ok` in het verslag komen, wat op bewijs lijkt en het niet is. De loader
weigert zo'n scenario.

### Kroniekstromen en tijd

Per stroom geldt: alleen vastleggingen met `op_moment <= ` het gevraagde moment
tellen mee, en van de rest wint per sleutelwaarde en per veld de laatste
vastlegging. Een vraag over een moment in het verleden levert dus het beeld van
toen. De stromen worden aan de engine aangeboden als databronnen, waar ze de
inputs van de eigen regelingen invullen. Twee stromen met dezelfde naam worden
geweigerd: de stroomnaam is tevens de naam van de databron, dus daar zou de
tweede de eerste stil schaduwen.

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
