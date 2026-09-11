# frontend-chrono-poc

De frontend van de chronolexografie-testopstelling: één pagina die tekent wat de
wereld aanbiedt, en die het woord van geen enkele casus kent.

- **links** een kolom per cel, met per kroniek de grammen in tijdsvolgorde,
  gekleurd per soort (lexogram, decretogram, executogram); een decretogram klapt
  uit en toont per waarde waar ze vandaan komt;
- **rechts** de bediening: de acties die de wereld nu aanbiedt (gegroepeerd per
  actor, met het formulier uit de actie zelf), de instellingen, een vraag aan een
  cel, en het observatielog;
- **onderaan** de tijdlijn: waar de klok staat, elk moment waarop iets ligt als
  punt, en "spoel vooruit tot".

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
| opgave bij de actie | de parameter |
| berekend | de uitgevoerde regeling |
| vast veld van het besluit | — |

Plus de wetsversie waaronder besloten is en de verplichtingen die eruit volgen.

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
just chrono-poc-check   # tests, ontwerpsysteem-imports en bundel
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

## Wat de app van de server verwacht

| route | waarvoor |
|---|---|
| `GET /api/world` | het beeld: klok, cellen met kronieken, instellingen, acties, observatielog, waarschuwingen |
| `POST /api/actions/{id}` | een actie uitvoeren, met de velden van het formulier als body |
| `POST /api/advance` | `{ "until": "jjjj-mm-dd" }` |
| `PUT /api/settings` | instellingen wijzigen |
| `POST /api/reset` | terug naar de startstand |
| `GET /api/cells/{cel}/lexostatus/{naam}` | een reductie opvragen; "niets vastgesteld" is een gewoon antwoord met status 200 |

Een antwoord op een wijziging mag het nieuwe beeld zelf zijn of het onder
`snapshot` / `world` dragen; draagt het er geen, dan haalt de app het beeld
opnieuw op. Wat er nieuw is leidt de app af uit twee opeenvolgende beelden, niet
uit een gebeurtenissenlijst — zo leunt de frontend op één vorm, en dat is het
beeld dat ook in `packages/simulator` vastligt.
