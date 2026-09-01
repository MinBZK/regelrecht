---
name: roadmap
description: Onderhoudt de inhoud van de roadmap op /roadmap — werkpakketten toevoegen, wijzigen, verplaatsen of verwijderen, onderzoeksvragen schrijven en aan een sectie van het position paper koppelen, RFC's koppelen die het ontwerp beschrijven, en het outcome-mappingwerkblad invullen. Gebruik dit bij "voeg een werkpakket toe", "verplaats X naar de Hoe-fase", "zet er een onderzoeksvraag bij", of het bijwerken van fases en disciplines.
user-invocable: true
allowed-tools: Bash, Read, Grep, Glob, Edit, Write
---

# Roadmap

De roadmap op `regelrecht.rijks.app/roadmap` is een read-only weergave van
bestanden in deze repo. Er is geen beheerscherm en geen schrijf-API: wijzigen
is een pull request, net als bij de rest van de repo.

Deze skill gaat over de **inhoud**. Voor de pagina's, het schema en de
buildcontroles die eronder liggen: die staan beschreven in de code zelf, en de
redenering erachter in de commits van PR #1317.

## Waar de inhoud staat

```
docs/src/content/roadmap/werkpakketten/<uuid>.md   één bestand per werkpakket
docs/src/data/roadmap-config.json                  de fases en de disciplines
docs/src/data/outcome-mapping.json                 het outcome-mappingwerkblad
```

De bestandsnaam ís het `id` uit de frontmatter. Dat wordt bij de build
gecontroleerd, dus hernoem nooit het een zonder het ander.

## Een werkpakket toevoegen

Kopieer géén bestaand bestand. Dat is de manier waarop je een dubbel `id`
maakt: de bestandsnaam pas je aan, het `id` in de frontmatter vergeet je, en de
matrix toont dan twee kaarten die naar dezelfde pagina wijzen. De build vangt
het (`id "…" wordt door meer dan één bestand gebruikt`), maar je hebt dan al
gewerkt aan het verkeerde bestand.

### De UUID moet gegenereerd worden

**Schrijf nooit zelf een UUID op.** Een verzonnen UUID ziet er goed uit en komt
door het schema — dat toetst alleen de vorm — maar hij is niet uniform
getrokken. Een taalmodel dat er een "bedenkt" grijpt terug op patronen uit zijn
invoer en herhaalt cijferreeksen; de kans op een botsing met een bestaand id is
dan niet meer verwaarloosbaar. En een botsing is precies de fout die twee
kaarten naar dezelfde pagina laat wijzen. Laat een generator het doen:

```bash
node -e "console.log(require('node:crypto').randomUUID())"
```

Node is de veilige keuze omdat de docs-build er toch al op draait, en het geeft
op elk platform hetzelfde resultaat: kleine letters, versie 4. Alternatieven,
als je die liever hebt:

| Waar | Commando |
|---|---|
| Overal met Python | `python3 -c "import uuid; print(uuid.uuid4())"` |
| macOS, Linux met util-linux | `uuidgen \| tr 'A-Z' 'a-z'` |
| Windows PowerShell | `[guid]::NewGuid().ToString()` |

**Kleine letters, altijd.** `uuidgen` geeft op macOS hoofdletters terug, vandaar
de `tr` erachter; PowerShell geeft al kleine letters. Het schema accepteert
hoofdletters wel, maar de controle op bestandsnaam-is-id vergelijkt letterlijk,
en op macOS is het bestandssysteem hoofdletter-ongevoelig: lokaal lijkt dan
alles in orde terwijl git de afwijkende schrijfwijze vastlegt en de build bij
een ander valt. Alle negentien bestaande ids zijn kleine letters; houd dat zo.

Het bestand bevat alleen frontmatter, geen body:

```yaml
---
id: <de uuid van hierboven>
titel: Korte titel van het werkpakket
faseId: wat
disciplineId: recht
prioriteit: ''
omvang: ''
categorie: ''
capability: ''
capaciteit: ''
toelichting: |-
  Twee of drie alinea's die uitleggen wat de opgave is. Markdown mag:
  **vet**, een lijst, een link.

  Een lege regel wordt alleen een alinea-einde onder `|`.
volgorde: 1000
onderzoek: ''
bouw: ''
rfcs: []
onderzoeksvragen: []
samenhangIds: []
---
```

### Wat er in de velden mag

`faseId` — een van `wat`, `wat-fase-2`, `hoe`, `waar`, `garantie`.
`disciplineId` — een van `techniek`, `recht`, `mensen`, `ethiek`,
`service-design`.

Beide staan in `roadmap-config.json`; een waarde die daar niet in staat laat de
build vallen met de naam van het werkpakket erbij.

`prioriteit` — `hoog`, `midden`, `laag`, of `''`.
`omvang` — `S`, `M`, `L`, `XL`, of `''`.
`categorie` — `lat`, `pivot`, `bet`, of `''`.
`capability` — `basis`, `ontwikkelen`, `simuleren`, `publiceren`, `analyseren`,
`implementeren`, `verifieren`, of `''`.

**Lege strings zijn normaal, geen tekortkoming.** Vijftien van de negentien
werkpakketten hebben geen prioriteit, zes geen categorie. De roadmap groeit door
eerst een titel en een plek vast te leggen en de rest later in te vullen. Vul
niets in om het vakje te vullen; een verzonnen prioriteit is slechter dan een
lege.

`toelichting` — markdown, als **letterlijke** blok-scalar (`|-`). Neem geen
`>-`: dat is een gevouwen scalar, en YAML plakt daarin een lege regel samen tot
één regelovergang. Remark leest dat als een zachte afbreking, niet als een
nieuwe alinea, dus je krijgt één doorlopende lap tekst — zonder dat de build
iets zegt. Alleen onder `|` telt een lege regel als alinea-einde, en alleen daar
wijst de bewerkknop naar de juiste regels in plaats van naar het hele veld.

`capaciteit` — vrije tekst, bijvoorbeeld "2 fte, 6 maanden". Geen enum, geen
getal; leeg laten mag.

`onderzoek` — `open`, `loopt`, `beantwoord`, of `''`.
`bouw` — `niet`, `deels`, `wel`, of `''`.

Dat zijn twee assen en met opzet geen één. Een vraag kan beantwoord zijn zonder
dat er iets gebouwd is, en er kan iets staan terwijl de vraag erachter nog open
is. Eén gecombineerde status zou in de helft van de gevallen een verkeerd beeld
geven. Beide mogen leeg blijven; de pagina toont dan "Nog niet bepaald".

`volgorde` — een getal dat de plek binnen één matrixcel bepaalt, laag eerst.
Dit veld is verplicht en heeft met opzet geen default: een ontbrekend veld zou
het werkpakket stilzwijgend bovenaan zetten. Gebruik stappen van 1000, dan kun
je er later tussen schuiven zonder alles te hernummeren.

`samenhangIds` — UUID's van andere werkpakketten. De build controleert of ze
bestaan. Dit is eenrichtingsverkeer: zet je A → B, dan verschijnt B niet
automatisch bij A. Zet 'm er handmatig bij als de relatie wederzijds is.

## Onderzoeksvragen

Een vraag is een gewone string, óf een mapping met een verwijzing naar het
position paper *Rules as Executed*:

```yaml
onderzoeksvragen:
  - >-
    Een vraag waar het paper niets over zegt blijft een gewone string.
  - vraag: >-
      Heeft een burger recht op de technische logbestanden van hoe een besluit
      tot stand is gekomen?
    paper: sec:traceaccess
```

Beide vormen mogen door elkaar in één lijst staan.

### De juiste sectie vinden

De ankers staan in de headings van het paper:

```bash
node -e "require('./docs/src/research/rules-as-executed.headings.json')
  .forEach(h => console.log(h.slug.padEnd(28), h.text))"
```

Sectie 12 is een onderzoeksagenda per discipline — `sec:agenda-legal`,
`sec:agenda-cs`, `sec:agenda-polsci`, `sec:agenda-philosophy`. Stelt het paper
de vraag daar zelf, wijs dan daarheen. Werkt een inhoudelijk hoofdstuk hem uit,
wijs dan naar dat hoofdstuk. Bij twijfel: het hoofdstuk, want daar staat een
antwoord in plaats van dezelfde vraag.

**Koppel niet wat niet past.** Drie van de drieënvijftig vragen hebben geen
sectie omdat ze te algemeen zijn ("Hoe navolgbaar is het?"). Een gedwongen
verwijzing kost de lezer een klik en levert niets op. Een verzonnen anker laat
de build vallen met het werkpakket en de vraag erbij.

## RFC's koppelen

`rfcs` is een lijst met RFC-nummers, als getal:

```yaml
rfcs:
  - 13
  - 21
```

De pagina toont ze onder "Ontwerp en implementatie", met de titel van de RFC en
zijn eigen `implementation`-waarde erbij. **Die waarde wordt niet overgenomen in
het werkpakket.** De RFC is het ding dat gebouwd wordt, dus die bezit dat feit;
een kopie hier zou een tweede waarheid zijn die niemand bijwerkt. Verandert een
RFC van `Not implemented` naar `Implemented`, dan verandert de roadmap mee
zonder dat je iets aanraakt.

Een nummer dat niet bestaat laat de build vallen (`RFC 99 bestaat niet`), met
het werkpakket erbij. Dezelfde RFC bij meerdere werkpakketten mag: RFC-013 hangt
nu aan drie. Twee keer hetzelfde nummer in één lijst levert één regel op, geen
twee.

### Welke RFC's nog nergens hangen

Elke RFC met `implementation: Implemented` hoort ergens in de roadmap terug te
komen: hij beschrijft werk dat gedaan is. Eén commando zegt welke dat nog niet
doen:

```bash
cd docs && node scripts/check-roadmap-rfcs.mjs
```

```
Roadmap-RFC-dekking: 5 van 15 geïmplementeerde RFC(s) hangen nog aan geen
enkel werkpakket:
  RFC-002  Bevoegdheid (Authority) in Machine-Readable Law
  …
```

Het draait ook mee in `just docs-a11y`, naast de andere `check-*`-scripts.

Dat is een melding en geen fout, met opzet. Een RFC die eerder landt dan de
roadmap-bijwerking is een redactionele achterstand, geen defect, en een poort
die daarop blokkeert zet de roadmap in de weg van het werk dat hij beschrijft.
Niet elke RFC hoort trouwens bij een werkpakket — RFC-000 gaat over het
RFC-proces zelf.

## Een werkpakket verplaatsen

Binnen een cel: pas `volgorde` aan. Naar een andere cel: pas `faseId` of
`disciplineId` aan, en geef het een `volgorde` die past tussen de buren in de
nieuwe cel.

## Een werkpakket verwijderen

Het bestand weggooien is niet genoeg: andere werkpakketten kunnen er via
`samenhangIds` naar wijzen, en die verwijzingen blijven achter. De build valt
daarop, en noemt elk bestand dat opgeruimd moet worden:

```
werkpakket 8faa572b-… (Controle en herstel): samenhangId "413459cd-…" bestaat niet
werkpakket 992fa816-… (Discretionaire ruimte): samenhangId "413459cd-…" bestaat niet
```

Kijk dus eerst wie er naar verwijst, dan weet je vooraf wat je aanpast:

```bash
grep -l '<uuid>' docs/src/content/roadmap/werkpakketten/*.md
```

Het bestand zelf staat ook in die uitkomst, want zijn eigen `id` staat erin;
de rest zijn de verwijzers.

De app die hier ooit stond ruimde die verwijzingen zelf op bij het verwijderen;
dat deed een server die er niet meer is. Nu doet de build het niet voor je, hij
zegt alleen waar het misgaat.

## Fases en disciplines wijzigen

Die staan in `roadmap-config.json`. Een fase heeft een `volgnummer` dat de
kolomvolgorde bepaalt; disciplines staan in de volgorde van het bestand.

Een fase of discipline verwijderen kan alleen als geen enkel werkpakket er nog
naar wijst — anders faalt de build. Zoek eerst wie er hangt:

```bash
grep -l 'faseId: hoe' docs/src/content/roadmap/werkpakketten/*.md
```

**Een categorie toevoegen aan `CATEGORIEEN` in `docs/src/lib/roadmap.ts` is
geen inhoudelijke wijziging maar een codewijziging**, en er hoort een regel bij
in `docs/src/styles/roadmap.css`. Zonder die regel rendert het filtervakje wel,
maar blijven die kaarten verborgen zodra er gefilterd wordt. `assertFilterRules`
laat de build daarop vallen en zegt welke regel ontbreekt; volg die melding.

## Het outcome-mappingwerkblad

`docs/src/data/outcome-mapping.json` voedt `/roadmap/outcome-mapping`. Het staat
grotendeels leeg: gevuld zijn `mission`, de vier boundary partners en de
eerste outcome challenge. De rest staat op lege strings. Aanvullen is gewoon
het JSON-bestand bewerken, maar kijk eerst wat er staat: overschrijven is net
zo makkelijk als aanvullen.

Eén regel maakt het lastiger dan het eruitziet: **de arrays zijn positioneel**.
Index i van `outcomeChallenges`, `progressMarkers` en `strategyMaps` hoort bij
`boundaryPartners[i]`, en `organizationalPractices` loopt gelijk op met de acht
praktijken in de code. Een vijfde boundary partner toevoegen betekent dus alle
vier de arrays uitbreiden; doe je er één, dan valt de build met het veld erbij:

```
too_small … "outcomeChallenges"
```

Dat is opzet: zonder die controle zou de pagina de gegevens van de ene partner
onder de naam van een andere zetten, en dat zie je aan niets.

## Verifiëren

```bash
just docs-build     # schema + de controles hieronder; dit is de echte poort
just docs           # dev-server op :4321, om het na te kijken
```

De controles zitten in de paginasjablonen, dus onder de dev-server slaan ze pas
aan zodra je `/roadmap` echt opvraagt. Vertrouw op `docs-build`.

`docs-build` faalt met een leesbare melding bij:

- een onbekende `faseId` of `disciplineId`
- een `samenhangId` dat nergens heen wijst
- twee bestanden met hetzelfde `id`, of een bestandsnaam die niet het `id` is
- een `paper:`-anker dat niet in het paper staat
- een RFC-nummer in `rfcs` dat niet bestaat
- een ontbrekende of foute waarde volgens het zod-schema

Raak je ook de pagina's aan, draai dan `just docs-a11y` (duurt ~10 minuten en
draait ook in CI).

### Wat de build níét controleert, met opzet

Onvolledige inhoud is geen fout. Een werkpakket zonder prioriteit, een cel
zonder werkpakketten, een vraag zonder papersectie: dat is de stand van het
werk, niet een defect. Een poort die daarover klaagt zou bij elke commit
afgaan en daarmee genegeerd worden — en hij zou het toevoegen van een
half-uitgewerkt werkpakket blokkeren, wat juist de manier is waarop deze
roadmap groeit.

Wil je weten waar de roadmap onaf is, kijk dan zelf:

```bash
grep -L 'prioriteit: [a-z]' docs/src/content/roadmap/werkpakketten/*.md
```

## Twee dingen om te weten

**De pagina staat bewust nergens gelinkt.** Niet in de navigatie, niet op de
landingspagina, en hij is uitgesloten van de zoekindex. Hij is wel gewoon
publiek bereikbaar, op zowel `regelrecht.rijks.app` als
`docs.regelrecht.rijks.app`. Wil je hem gaan linken, dan is dat het moment om
de inhoud publicatierijp te maken: er staan nu werktitels en lege velden in.

**Er is geen ondersteuning voor meerdere papers.** Het veld heet `paper` en het
anker wordt getoetst aan dat ene paper. Komt er een tweede, dan is dat een
codewijziging, geen inhoudelijke.
