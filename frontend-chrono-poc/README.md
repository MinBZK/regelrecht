# frontend-chrono-poc

De frontend van de chronolexografie-testopstelling: één pagina die tekent wat de
wereld aanbiedt, en die het woord van geen enkele casus kent.

- **bovenaan** de bediening, over de volle breedte: de acties die de wereld nu
  aanbiedt (gegroepeerd per actor, met het formulier uit de actie zelf), de
  instellingen, een vraag aan een cel — op een moment naar keuze, tot aan de
  klok —, alle grammen op een rij, en het observatielog;
- **daaronder** het [journaal](#het-journaal-is-de-hoofdweergave): één verhaal in
  tijdsvolgorde van wie wat deed en wat dat veranderde;
- **daaronder** een kolom per cel, met per kroniek de grammen in tijdsvolgorde,
  gekleurd per soort (lexogram, decretogram, executogram); een decretogram klapt
  uit en toont per waarde waar ze vandaan komt;
- **onderaan** de tijdlijn: waar de klok staat, elk moment waarop iets ligt als
  punt, en "spoel vooruit tot".

Boven en onder, niet links en rechts: een kroniekrij draagt een naam, een moment,
een kanaal en een grondslag, en in een halve pagina paste geen enkele cel nog
heel. De bediening is smal van zichzelf en kan de volle breedte hebben zonder er
iets mee te doen; de cellen krijgen de rest. Past het rijtje kolommen niet naast
elkaar, dan schuift het **binnen zijn eigen paneel** opzij (`nldd-collection`
met `layout="horizontal-scroll"` en een vaste `item-width`) en nooit de pagina.

## Het journaal is de hoofdweergave

Het journaal vertelt het verhaal: één regel per gebeurtenis, in de volgorde
waarin ze ontstond. Per regel het moment, wie het deed (een actor, een cel, of de
klok), wat het was, en — het punt van de hele weergave — wat het aan de **stand
van de zaak** veranderde: `toekenningspositie: niets vastgesteld → …`,
`betaald: 0 → 49294`. Een regel klapt uit naar de grammen die erdoor ontstonden
(klikbaar: ze openen het gram zoals de cel het in haar eigen kroniek toont, met
de herkomst van elke waarde), naar wat een besluit van een ander accepteerde in
plaats van na te rekenen, en naar elk verschil apart.

Een vraag die over een celgrens ging, staat ingesprongen onder het besluit dat
haar uitlokte: los gelezen is ze een vraag zonder aanleiding.

De kolommen per cel, het grammenpaneel en het observatielog zijn hier de
**details** van. Wie alleen de kolommen ziet, ziet wat er ligt en niet wat er
gebeurde — en dat was precies wat er miste.

Drie dingen die deze app hier níet doet:

- **rekenen.** Het verschil tussen "was" en "is" is door de wereld gemeten, op de
  kronieken zelf, vóór en ná de gebeurtenis. Deze app heeft geen kroniek en zou
  het niet kunnen;
- **een casus kennen.** Welke reducties de stand van een zaak dragen, staat als
  `status_indicators` in het wereldbestand; deze app leest de labels die daaruit
  komen;
- **een tweede administratie voeren.** Een regel wijst naar grammen die in het
  beeld staan (`<cel>|<kroniek>|<plek>`) en draagt er geen kopie van.

Filteren kan op actor en op cel. De tijdlijn onderaan hangt eraan vast: een punt
aanklikken houdt de regels van die dag over en klapt ze open; de knop erboven
brengt je terug naar het hele verhaal. Wat er na een actie of na vooruitspoelen
bij kwam, draagt dezelfde "nieuw"-markering als in de kolommen — en daarom is de
groene balk over wat een stap opleverde weg zodra het journaal hem zelf toont.

## Alle grammen op een rij

Het tabblad **Grammen** legt elk gram van elke cel chronologisch naast elkaar:
moment, cel, kroniek, type, naam, kanaal en grondslag, te filteren op cel en op
type, en per rij uitklapbaar naar het ruwe gram als JSON.

Dat is — net als het observatielog — een leesbeeld van de opstelling: geen cel
kan dit overzicht opvragen, en er is niets in dit tabblad dat iets verandert. Het
komt uit hetzelfde beeld als de kolommen; er wordt niets bij opgehaald en niets
uit weggelaten. Eén ding zit er daarom niet in: het **receipt** van een besluit.
`packages/simulator/src/snapshot.rs` laat dat bewust uit het beeld omdat het
wandkloktijd draagt en een beeld dat per run verschilt geen contract is.

Een decretogram heeft daarom in dit tabblad een tweede uitklap, **Receipt**, en
die is het enige in deze app dat níet uit het beeld komt: hij haalt het receipt
van dat ene gram op bij
`GET /api/cells/{cel}/chronicles/{stroom}/grams/{n}/receipt`, pas als je hem
opendoet. Erin staan de secties van RFC-013 leesbaar, de geladen regelingen met
hun hash, en de geaccepteerde waarden als tabel met de bron-cel en het bevoegd
gezag dat die bron noemde. De tijdstempel staat er met het label dat zegt wat het
is: wandkloktijd, niet de logische tijd van de wereld.

## Geen casus in deze app

Elke naam die op het scherm komt — een cel, een kroniek, een actie, een
instelling, een gram — staat in het beeld dat de server geeft
(`GET /api/world`). Een andere wet met andere organisaties is een ander
wereldbestand en dezelfde frontend. Staat er ooit een naam uit een casus in deze
map, dan zit hij op de verkeerde plek.

## De herkomst van een waarde

Het punt van de opstelling is dat een geaccepteerde waarde niet op een berekende
lijkt (invariant I5). Een uitgeklapt decretogram zegt daarom per veld:

| herkomst | wat erbij staat |
|---|---|
| geaccepteerd van een andere cel | de cel, de lexostatus, de uitkomst, het moment, wie vroeg en de (gesimuleerde) ondertekening |
| uit de eigen kroniek | de kroniek, het veld en het moment van vastlegging |
| uit een eerder besluit over deze zaak | het besluit, de zaak en het moment van dat besluit |
| opgave bij de actie | de parameter |
| berekend | de uitgevoerde regeling |
| vast veld van het besluit | — |

Plus de wetsversie waaronder besloten is en de verplichtingen die eruit volgen.

En één ding dat er juist níet staat: declareert de regeling geen bevoegd gezag
(`competent_authority: null`), dan meldt het uitgeklapte gram dat er **niet te
toetsen viel wie mocht besluiten**. Dat hoort bij het gram waar het over gaat en
niet in de lijst verstreken termijnen elders op de pagina: een besluit waarvan de
wet niemand aanwees, is iets anders dan een gemiste termijn. Wees er niet te snel
overheen — het is de afwezigheid van een toets, niet de uitkomst ervan.

## Een tweede besluit is een keuze, geen ongeluk

Ligt er over de zaak in het formulier al een decretogram van dit besluit, dan
zegt de kaart dat ("al besloten op …") en vraagt ze om bevestiging voordat ze het
nog eens doet. De knop blijft bruikbaar — een tweede besluit over dezelfde zaak
is juist het verhaal van deze opstelling — maar er komt een decretogram bij, en
dat hoort een klik te zijn die iemand bedoelde. Welke zaak het is leest de app af
aan de parameters van het besluit zoals het gram ze vastlegde, en niet uit het
zaakkenmerk-sjabloon: dat sjabloon staat wel in het beeld, maar het **invullen**
ervan is werk van de cel, met een weigering eraan vast voor een waarde waarin een
scheidingsteken voorkomt. Een tweede plek die het kenmerk samenstelt zou daarvan
af kunnen wijken.

## Het zaakkenmerk komt niet uit het niets

Een vraag aan een cel vraagt om precies de parameters die de cel documenteert, en
één daarvan is moeilijker dan de rest: het **zaakkenmerk**. Een BSN weet de
vrager; een zaakkenmerk ontstaat pas bij een besluit, uit een sjabloon in het
wereldbestand (`zorgtoeslag/{bsn}`), en het is daarna de sleutel waarop de kroniek
met beschikkingen groepeert. Wie dat niet weet, typt er iets en krijgt "niets
vastgesteld" terug — een geldig antwoord op een vraag over een zaak die niet
bestaat.

Het tabblad **Lexostatus** maakt die keten zichtbaar, en alles ervan komt uit het
beeld:

- het formulier is dat van de gekozen definitie: één veld per gedocumenteerde
  parameter, met haar type. De cel accepteert precies deze namen, dus ze hoeven
  niet geraden te worden;
- de `doc` van de definitie staat erboven als toelichting;
- bij de parameter die de **sleutel** van de reductie is, staat de kroniek en de
  vorm erbij ("sleutel van kroniek `beschikkingen`, vorm `zorgtoeslag/{bsn}`").
  Die vorm komt van de besluiten van diezelfde cel, en staat daarom ook als tag in
  haar kolom;
- het veld biedt de kenmerken aan die er **nu in die kroniek liggen**
  (`nldd-combo-box` met `allow-custom`). Vrije invoer blijft: een vraag over een
  zaak die er nog niet is, is een geldige vraag, en dat antwoord te zien krijgen
  is precies wat deze opstelling wil laten zien.

En het kenmerk zelf staat waar het ontstaat: in het journaal en in het tabblad
Grammen draagt een gram dat er een heeft zijn zaak onder de naam ("zaak
zorgtoeslag/999993653"), en in een uitgeklapt decretogram staat het als eerste
veld.

## Het observatielog is een meetinstrument

Het log staat náást de opstelling, niet erin, en de UI zegt dat: geen enkele cel
kan dit overzicht opvragen, en in een echte deployment bestaat het niet.

## Alles uit het ontwerpsysteem

De hele UI bestaat uit `nldd-*`-webcomponenten van `@nldd/design-system` en zijn
CSS-tokens. Er is geen eigen componentbibliotheek en geen CSS die een component
nabouwt; `css/main.css` bevat alleen het document zelf (box-sizing, hoogte, het
basisfont-token).

Componenten worden **per entry point** geïmporteerd, in
`src/nldd-components.js`. Dat bestand is gegenereerd:

```bash
npm run nldd:imports -w frontend-chrono-poc   # bijwerken
npm run nldd:check -w frontend-chrono-poc     # in sync? (ook een CI-stap)
```

## Draaien

```bash
just dev-chrono-poc     # Vite op 0.0.0.0:7250, /api + /health naar de server (8000)
just build-chrono-poc   # de bundel in dist/, die de server statisch uitdeelt
just chrono-poc         # bundel + de server die hem serveert
just chrono-poc-e2e     # de bewijsronde in een echte browser (e2e/)
just chrono-poc-check   # tests, ontwerpsysteem-imports, bundel en bewijsronde
```

`API_PORT` verzet de proxy naar een andere serverpoort, `VITE_PORT` de
browserpoort.

## Tests

`vitest` met `happy-dom` en `@vue/test-utils`. De tests draaien op de
snapshot-fixture van de simulator zelf
(`packages/simulator/tests/fixtures/snapshot.json`, ingelezen door
`src/testing/worldFixture.js`) en niet op een kopie ernaast: het beeld is het
contract tussen de wereld en deze app, en twee exemplaren zouden stil uit elkaar
lopen.

Daarnaast staat in [`e2e/`](e2e) de bewijsronde door de publieke wereld: de
echte server, een echte Chromium, en veertig beweringen onder de check-id's
waaronder die ronde eerder met de hand liep. Die suite draait op een gebouwde
bundel en heeft dus iets anders te zeggen dan de vitest-tests hierboven: niet of
een component het juiste tekent bij een gegeven beeld, maar of de opstelling als
geheel doet wat ze belooft. Zie
[`packages/chrono-poc-web/README.md`](../packages/chrono-poc-web/README.md#de-bewijsronde-in-de-browser).

## Wat de app van de server verwacht

| route | waarvoor |
|---|---|
| `GET /api/world` | het beeld: klok, cellen met kronieken, instellingen, acties, observatielog, waarschuwingen |
| `POST /api/actions/{id}` | een actie uitvoeren, met de velden van het formulier als body |
| `POST /api/advance` | `{ "until": "jjjj-mm-dd" }` |
| `PUT /api/settings` | instellingen wijzigen |
| `POST /api/reset` | terug naar de startstand |
| `GET /api/cells/{cel}/lexostatus/{naam}` | een reductie opvragen, met `op_moment` het moment van de vraag; "niets vastgesteld" is een gewoon antwoord met status 200 |

Een antwoord op een wijziging mag het nieuwe beeld zelf zijn of het onder
`snapshot` / `world` dragen; draagt het er geen, dan haalt de app het beeld
opnieuw op. Wat er nieuw is leidt de app af uit twee opeenvolgende beelden, niet
uit een gebeurtenissenlijst — zo leunt de frontend op één vorm, en dat is het
beeld dat ook in `packages/simulator` vastligt.
