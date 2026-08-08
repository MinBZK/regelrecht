---
name: issue-labels
description: Labelt en plaatst een nieuw issue in deze repo, met de maat, het onderdeel, de blokkade, en of het in een milestone hoort. Gebruik dit bij elk issue dat je aanmaakt, en bij het bijwerken van een issue dat kaal is blijven staan.
allowed-tools: Bash, Read
---

# Issue-labels en milestones

Een issue krijgt zijn labels bij het aanmaken, niet later. Wie ze later zet moet
opnieuw uitzoeken wat er aan de hand was, en dat gebeurt dus niet: van de open
issues staat een tiende kaal, en dat zijn vrijwel allemaal issues die een agent
heeft aangemaakt. Deze skill is de maatstaf waarmee je ze meteen zet.

## Haal eerst de actuele waarden op

```bash
export GH_CONFIG_DIR=$HOME/.config/gh
gh label list --limit 200
gh api repos/MinBZK/regelrecht/milestones --jq '.[] | "\(.title) — \(.description)"'
```

Dit document beschrijft de assen en waar de grens tussen twee waarden ligt. De
waarden zelf staan in de repo en veranderen; type ze nooit uit je geheugen over.
`gh issue create` weigert de hele aanroep op één onbekend label, dus een
verkeerd gespeld label kost je het issue, niet alleen het label.

## Maat

Precies één per issue, kaal en zonder voorvoegsel: XS, S, M, L of XL. De maat
schat de omvang van het werk. Een fout die productie plat legt en in één regel
te repareren is, is XS.

De verdeling over de open issues is XS 6, S 12, M 52, L 28, XL 8 en tien zonder.
Daar lees je twee dingen in af. M is waar alles belandt wat niet is nagedacht en
onderscheidt daarom nauwelijks; en geen maat is geen eindtoestand maar een issue
die nog niemand heeft bekeken.

**XS.** De reparatie staat vast en past in één bestand. Er valt niets meer te
beslissen, alleen te doen: een export weghalen die niemand meer leest, een
dependency-regel verplaatsen, een waarde hergebruiken die al in scope staat, een
alinea corrigeren die aantoonbaar onjuist is. De beschrijving mag lang zijn; het
gaat om het werk, niet om de uitleg.

**S.** Eén onderdeel, een handvol bestanden, en de oplossing staat al in het
issue. Het gesprek gaat nog over het uitvoeren en niet meer over het wat: een
poort die te smal staat, een vlag die aan moet, een uitzondering die eraf kan.

**M.** De richting is duidelijk, het antwoord niet. Werk binnen één onderdeel,
plus de test en de documentatie eromheen. Kies M pas nadat je de twee buren hebt
langsgelopen: staat de reparatie er feitelijk al (dan S), of moet er eerst iets
beslist worden voordat de eerste regel geschreven kan worden (dan L)? M is goed
als beide antwoorden nee zijn. Het is fout als je niet gekeken hebt.

**L.** Meerdere onderdelen tegelijk, of een ontwerpbeslissing die vooraf gaat
aan het werk. Ook de inventarisaties horen hier: één issue dat een hele klasse
plekken in de repo opsomt, waarbij elke regel klein is en de omvang in het
aantal zit.

**XL.** Past niet in één pull request. Een nieuw vermogen van de engine, een
heel rechtsgebied vertalen, een compliancetraject. XL zegt daarmee ook dat het
opknippen nog moet gebeuren; het is geen maat waar je aan gaat werken.

## Onderdeel

De `comp:`-labels zeggen welk onderdeel het raakt. Kies op waar de reparatie
landt, niet op waar het is opgevallen. Twee labels mag wanneer het werk
aantoonbaar aan beide kanten moet landen; een verschil in uitkomst tussen engine
en editor is er echt twee. Drie is een teken dat het niet is uitgezocht: of het
issue is te groot en hoort opgeknipt, of één kant is de echte en de andere twee
zijn de plek waar je het zag.

Twee grenzen volgen niet uit de mappenstructuur en moet je hier lezen:

- **De verrijkingslogica krijgt `comp:enricher`**, ook al staat de code in
  `packages/pipeline`. `comp:pipeline` gaat over de wachtrij en het
  job-mechaniek zelf, dus timeouts, statuskolommen en workers, en niet over wat
  een verrijking vaststelt.
- **`comp:editor` dekt zowel `frontend/` als `packages/editor-api`.** De editor
  is één product met een Vue-voorkant en een Rust-achterkant, en een bevinding
  daar hangt zelden aan één van beide.

Raakt een issue geen enkel onderdeel, bijvoorbeeld iets repo-breeds, een proces
of een compliancevraag, dan blijft hij zonder `comp:`. Dat is een geldige
uitkomst en geen omissie.

## Blokkade

Zet een `blocked:`-label alleen wanneer er op dit moment een blokkade is, en
laat het label zeggen aan welke kant hij zit: op ander werk in deze repo, of op
iets buiten ons. Dat onderscheid telt omdat we het eerste zelf naar voren kunnen
halen en het tweede niet. Haal het label weg zodra de blokkade weg is; een
achtergebleven `blocked:` verstopt werk dat klaarligt om opgepakt te worden.

## Bijdrage van buiten

`contribution:` is voor issues die hun oorsprong buiten het team hebben. Zet het
niet op werk dat wij zelf hebben bedacht en dat toevallig door iemand van buiten
wordt gedaan.

## Wat je nooit met de hand zet

- **`dbot:` en `mbot:`** zet een bot zelf, bij het aanmaken van zijn eigen issue
  of pull request. Een handmatig exemplaar liegt over de herkomst en komt
  terecht in filters die op botwerk zijn gebouwd.
- **`deploy:preview`** is een schakelaar op een pull request die de
  preview-uitrol aanzet. Het benoemt geen onderwerp, en op een issue doet het
  niets.

## Milestones

Haal ze op met het `gh api`-commando hierboven. Het zijn er nu drie, en elke
milestone heeft een eindstreep, want hij sluit wanneer zijn issues gesloten
zijn.

Een issue hoort in een milestone wanneer die milestone niet af kan zonder dat
issue. Dat is de hele toets. Een issue dat er inhoudelijk bij past maar er niet
voor nodig is hoort er niet in, want dan schuift de eindstreep mee met de
belangstelling.

Er past er maar één op een issue. Een milestone is daarmee ongeschikt om een
onderwerp mee bij te houden. Onderwerpen horen op labels, want die kun je
stapelen en die eindigen nooit. Staat een XL in een milestone, dan is dat bijna
altijd het signaal dat hij eerst opgeknipt moet worden.

Geen milestone is de normale toestand.

## Vorm van het issue

Zo kort als het probleem toelaat, meestal een paar regels. Eén probleem per
issue. Geen sectiekoppen, geen opsomming van acceptatiecriteria, geen
samenvattende slotzin die herhaalt wat er net stond. Schrijf wat er mis is, waar
het staat en wat het gevolg is, en houd daarna op.

De reden is dat een issue hier op naam staat van de eigenaar van de repo en moet
lezen als iets wat hij zelf heeft getypt. Lengte mag alleen uit de zaak komen,
uit een meting, een tabel met cijfers of een stuk code, en nooit uit structuur
die er overheen is gelegd.

Een titel in de vorm `type(scope): onderwerp` is gebruikelijk waar het issue
concreet werk benoemt, maar niet verplicht; een issue is geen commit.

## Aanmaken

```bash
export GH_CONFIG_DIR=$HOME/.config/gh
gh issue create -R MinBZK/regelrecht \
  --title "fix(editor): ..." \
  --body "..." \
  --label "S,comp:editor" \
  --milestone "Editor & Engine rekenen gelijk"
```

Een issue dat al bestaat en kaal is blijven staan werk je met dezelfde toets bij
via `gh issue edit <nr> --add-label "M,comp:engine"`.
