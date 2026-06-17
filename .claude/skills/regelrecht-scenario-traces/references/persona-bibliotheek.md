# Persona-bibliotheek — benoemde, herbruikbare leaf-bundels

Een **persona** is een casus met een naam: een benoemde bundel leaf-deltas op de
casus-assen (`casus-assen.md`). De persona is de bron; scenario's zijn afgeleiden die
een persona inzetten en hooguit één ding variëren. Eén definitie, hergebruikt — niet
dezelfde leaf-bundel telkens met de hand opnieuw.

## Wat een persona vastlegt

- **Naam** in mensentaal — hoe een jurist de casus zou noemen.
- **Assen-coördinaat** — de waarde op elke relevante as (de rest staat op baseline).
- **Leaf-deltas** — de concrete parameters die afwijken van de baseline (de machine-vorm
  van het coördinaat).
- **Bedoeld onderscheid** — wat deze persona test dat een buur-persona niet test. Dit is
  cruciaal voor twin-persona's (zie onder).

Leg dit vast in `templates/persona-bibliotheek.md`, bij het corpus.

## Expressievormen (kies naar runner-mogelijkheden)

1. **Scenario Outline + Examples** — de meest portable vorm: één scenario-skelet, één
   rij per persona in de Examples-tabel. De tabel ís de persona-bibliotheek en draait
   in elke Gherkin-runner zonder extra code. Geschikt als de personas hetzelfde
   keten-pad delen en alleen op invoer/uitkomst verschillen.
2. **Custom step `Given persona "<naam>"`** — een stap die de leaf-bundel uit een
   fixtures-bestand laadt. Het scenario leest dan in mensentaal; de leaf-vertaling staat
   op één plek. Vereist een step-definitie in de runner.
3. **Fixtures-bestand** — een los databestand (persona → leaf-deltas) dat zowel de
   custom step als analyse-tooling (casus-matrix, coverage) kan inlezen. De meest
   herbruikbare vorm; combineer met (2).

Begin bij de vorm die de bestaande runner zonder nieuwe code aankan (meestal de
Scenario Outline), en stap pas op naar fixtures + custom step als de bibliotheek groeit.

## Baseline + delta-discipline

Houd een expliciete **baseline** (alle leafs op hun neutrale waarde) en laat elke
persona alleen zijn **deltas** zetten. Voordelen: een lezer ziet meteen wat deze casus
*bijzonder* maakt; en je voorkomt dat een vergeten leaf stilletjes meelift uit een
vorige casus. Documenteer welke waarde de runner voor een niet-gezette leaf aanneemt.

## Typering bewaken

Als de runner getypeerde waarden via een aparte tabel-stap parset en quoted waarden als
string behandelt, leg dat dan vast bij de persona-vorm: booleans/getallen via de
typerende stap, identifiers als string. Een type-mismatch hier geeft een runner-fout of,
erger, een stil-verkeerd geïnterpreteerde waarde. (Dit is runner-eigenschap, geen
casus-inhoud — leg de concrete regel vast in het corpus, niet hier.)

## Twin-persona's — het onzichtbare onderscheid zichtbaar maken

Soms geven twee juridisch verschillende casussen identieke leaf-invoer, omdat het
onderscheidende kenmerk geen leaf in het model is. Dan ziet het model het verschil niet
— en jij leest eroverheen. Maak zulke gevallen expliciet:

- Definieer ze als **twin-persona's**: identiek behalve op precies één as-waarde (het
  bedoelde onderscheid).
- Valt het onderscheid buiten elke bestaande as → dat is een **bevinding**, geen
  scenario. Het model mist een dimensie. Route het via de 4-weg-classificatie van
  `regelrecht-stelselanalyse` (modellering-fout vs wetgevings-fout vs acceptabel).
- Documenteer bij beide twins expliciet dat ze dezelfde invoer delen en waaróm de
  uitkomst (niet) hoort te verschillen.

Twin-persona's voeden ook de keten-sensitiviteitstest (`keten-checkpoints.md`): één
as-verschil dat door één ketenschakel loopt, maakt die schakel falsifieerbaar.

## Anti-patronen

- **Copy-paste-casus** — dezelfde leaf-bundel in tien scenario's overgetypt. Eén
  persona-definitie hergebruiken.
- **Naamloze casus** — een scenario met alleen leaf-deltas en geen persona-naam/assen.
  Onvindbaar.
- **Impliciet onderscheid** — twee casussen die "eigenlijk anders zijn" zonder dat het
  verschil in een as of een bevinding is vastgelegd.
