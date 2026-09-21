---
title: Grammen
description: De drie chronolexogrammen van RFC-022 als JSON-schema, met per soort een voorbeeld uit de publieke wereld en de betekenis van elk veld.
lang: nl
---

[RFC-022](/rfcs/rfc-022) neemt de drie soorten vastlegging uit de
[Chronolexografie-position paper](https://chronolexografie.nl/position-paper/)
over: het **lexogram**, het **decretogram** en het **executogram**. De
chronolexografie-testopstelling (`packages/simulator`) maakt ze, en deze pagina
beschrijft hoe ze eruitzien.

De vorm ligt vast in drie JSON-schema's naast het wetsschema:

| schema | wat het beschrijft |
|---|---|
| [`schema/v0.6.0/gram.json`](https://github.com/MinBZK/regelrecht/blob/simulator-poc/schema/v0.6.0/gram.json) | één gram, met drie varianten op `kind` |
| [`schema/v0.6.0/chronicle.json`](https://github.com/MinBZK/regelrecht/blob/simulator-poc/schema/v0.6.0/chronicle.json) | één kroniekstroom-definitie: naam, sleutel en gebeurtenisschema ([RFC-022 §1.3](/rfcs/rfc-022#13-executograms-chronicles-directory)) |
| [`schema/v0.6.0/world-snapshot.json`](https://github.com/MinBZK/regelrecht/blob/simulator-poc/schema/v0.6.0/world-snapshot.json) | het beeld van de wereld, het antwoord van `GET /api/world` |

De schema's zijn geen tweede opschrijving naast de code maar een toets erop:
elk gram uit elk scenario, elk beeld uit de tests van de HTTP-laag en elke
kroniekstroom in de scenario- en wereldbestanden valideert ertegen
(`packages/simulator/tests/json_schemas.rs`). Loopt de code weg van het schema,
dan wordt die test rood.

De voorbeelden hieronder komen uit de **publieke wereld**
(`packages/simulator/worlds/publieke_wereld.yaml`): een aanvraag voor
zorgtoeslag, de toekenning door Dienst Toeslagen, de betalingen in termijnen en
de vaststelling na de definitieve aanslag. De burgerservicenummers zijn
fictief.

## Wat een gram draagt

Een gram is een vastlegging in de kroniek van een cel. Elk gram heeft dezelfde
omslag:

| veld | betekenis |
|---|---|
| `kind` | welk van de drie grammen dit is: `lexogram`, `decretogram` of `executogram` |
| `name` | wat er gebeurde, in de woorden van de cel |
| `intake` | het kanaal waarlangs het feit de cel bereikte: `aanvraag`, `levering`, `betaling` of `eigen_besluit` |
| `recording_actor` | wie vastlegde: het id van de cel zelf. De celbeheerder, niet het bevoegd gezag ([RFC-022 §1.3](/rfcs/rfc-022#13-executograms-chronicles-directory)) |
| `grondslag` | op welke grondslag het gram vastgelegd is; vrije tekst, mag leeg zijn |
| `op_moment` | het moment waarop het feit in deze cel feit werd, de as waarop een reductie ordent ([RFC-022 §4.1](/rfcs/rfc-022#41-lexostatus-chronolexoreductie-and-chronolexosynthese)) |
| `fields` | de vastgelegde velden, elk als `{ value, origin }` |

Dat zijn de vier vragen die RFC-022 §1.3 per vastlegging beantwoord wil zien:
*wat* (`name` en `fields`), *door wie* (`recording_actor`), *op welke grondslag*
(`grondslag`) en *op welk moment* (`op_moment`). `intake` voegt er *waarlangs*
aan toe.

`kind` volgt uit `intake`: een gram dat de cel via `eigen_besluit` vastlegde, is
een decretogram; elk ander is een executogram. Het zijn geen twee velden die uit
de pas kunnen lopen.

### De herkomst van een waarde

Elk veld draagt naast zijn waarde een `origin` met een etiket `herkomst`. Bij een
executogram is dat altijd hetzelfde; bij een decretogram valt het uiteen, en
juist daar is het nodig om te zien dat een waarde die van een andere cel is
**geaccepteerd** niet op een berekende waarde lijkt.

| `herkomst` | betekenis | draagt verder |
|---|---|---|
| `recorded` | de cel legde dit feit zelf vast | `intake`, `grondslag` |
| `computed` | een uitkomst die de regeling van het besluit berekende | `regulation` |
| `besluit_input` | een input waarop het besluit rekende | `recorded_origin`: uit een eigen kroniek (`eigen_kroniek`), uit een parameter (`parameter`), geaccepteerd van een andere cel (`geaccepteerd`, met ondertekening en contactnummer), teruggelezen uit een eerder besluit over dezelfde zaak (`eerder_besluit`), of, in het gram van een latere stage zoals de bekendmaking, gelezen uit het gram van hetzelfde besluit: een uitkomst ervan (`besluit_uitkomst`) of een input ervan (`besluit_input`), elk met `besluit`, `besluit_gram` en `field` |
| `besluit` | een vast veld van het decretogram zelf | niets |

### Het receipt staat er niet in

Een decretogram **is** het RFC-013 Execution Receipt plus wat dat receipt niet
kan weten. In de kroniek staat het receipt dus in het gram. In het beeld van de
wereld staat het er niet: het draagt wandkloktijd, en een beeld dat per run
verschilt is geen contract. Het is per gram apart op te vragen via
`GET /api/cells/{cel}/chronicles/{stroom}/grams/{n}/receipt`.

## Lexogram

Een lexogram is een versie van een regeling: het recht zelf, generiek en
voorbereid vóór de uitvoering ([RFC-022 §1.1](/rfcs/rfc-022#11-lexograms-already-exist)).
In de publieke wereld is `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2024-01-01.yaml`
een lexogram, en de reeks `valid_from`-versies van die map is de lexogramkroniek
van de Wet op de zorgtoeslag.

Een lexogram hoort bij geen enkele cel, dus het ligt in geen enkele kroniek. De
inhoud is het regelingsbestand, en dat valideert tegen `schema.json` (zie
[Law Format](/concepts/law-format)). `gram.json` noemt de variant zodat één
vocabulaire alle drie de grammen dekt; `world-snapshot.json` weigert een
kroniek of journaalregel die er toch een draagt. Welke lexogrammen een cel laadt, staat in
het beeld bij `cells[].laws`, en welke versie een besluit uitvoerde, staat in het
decretogram (`regulation`, `regulation_valid_from`, `executed_regulations`).

## Decretogram

Een decretogram is een besluit dat de cel zelf nam: een uitkomst van de engine
met `legal_character: BESCHIKKING`
([RFC-022 §1.2](/rfcs/rfc-022#12-decretograms-decision_type-as-an-open-vocabulary)).
De cel legt het vast in haar kroniek `beschikkingen`.

In de publieke wereld besluit Dienst Toeslagen op 1 maart 2024 over de aanvraag.
Het gram (ingekort; het volledige gram staat in
`packages/simulator/tests/fixtures/snapshot.json`):

```json
{
  "kind": "decretogram",
  "name": "zorgtoeslag_toekenning",
  "intake": "eigen_besluit",
  "recording_actor": "toeslagen",
  "grondslag": "wet_op_de_zorgtoeslag (versie 2024-01-01)",
  "op_moment": "2024-03-01",
  "fields": {
    "zaakkenmerk": { "value": "zorgtoeslag/999993653", "origin": { "herkomst": "besluit" } },
    "stage": { "value": "BESLUIT", "origin": { "herkomst": "besluit" } },
    "regulation": { "value": "wet_op_de_zorgtoeslag", "origin": { "herkomst": "besluit" } },
    "regulation_valid_from": { "value": "2024-01-01", "origin": { "herkomst": "besluit" } },
    "competent_authority": { "value": "Dienst Toeslagen", "origin": { "herkomst": "besluit" } },
    "legal_character": { "value": "BESCHIKKING", "origin": { "herkomst": "besluit" } },
    "decision_type": { "value": "TOEKENNING", "origin": { "herkomst": "besluit" } },
    "chronicle_sources": {
      "value": [
        {
          "chronicle": "aanvragen",
          "op_moment": "2024-03-01",
          "grams": 1,
          "content_hash": "sha256:5334c0f80ccf8444015ade39117d2541f77b564b8d44c2842cf91f5740f7cafd"
        }
      ],
      "origin": { "herkomst": "besluit" }
    },
    "obligations": {
      "value": [
        {
          "vervaldatum": "2024-03-01",
          "bedrag": 49294,
          "volgnummer": 1,
          "soort": "betaling",
          "schuldenaar": "Dienst Toeslagen",
          "schuldeiser": "999993653",
          "betaler": "belastingdienst",
          "schedule": "kwartaal",
          "ritme_herkomst": { "herkomst": "wereldbestand", "instelling": "betalingsritme" },
          "grondslag": "Wet op de zorgtoeslag art. 2 jo. Awir art. 16 jo. art. 22 (uitbetaling in termijnen)",
          "lexogram": { "regulation": "wet_op_de_zorgtoeslag", "regulation_valid_from": "2024-01-01", "artikel": "2" }
        }
      ],
      "origin": { "herkomst": "besluit" }
    },
    "hoogte_zorgtoeslag": {
      "value": 197178.01,
      "origin": { "herkomst": "computed", "regulation": "wet_op_de_zorgtoeslag" }
    },
    "toetsingsinkomen": {
      "value": 81000,
      "origin": {
        "herkomst": "besluit_input",
        "recorded_origin": {
          "herkomst": "geaccepteerd",
          "cell": "belastingdienst",
          "competent_authority": "Belastingdienst",
          "lexostatus": "toetsingsinkomen",
          "field": "toetsingsinkomen",
          "op_moment": "2024-03-01",
          "asked_by": "cel:toeslagen",
          "signature": "GESIMULEERDE-ONDERTEKENING door cel:toeslagen",
          "contact": 1
        }
      }
    }
  }
}
```

De uitkomsten van de regeling (`heeft_recht_op_zorgtoeslag`,
`hoogte_zorgtoeslag`) staan er met herkomst `computed`, de inputs waarop gerekend
is (`bsn`, `is_verzekerde`, `toetsingsinkomen`) met herkomst `besluit_input`.
Het toetsingsinkomen is niet hier berekend maar geaccepteerd van de
Belastingdienst, en dat is aan het gram te zien.

### De vaste velden van het besluit

Een gram uit het besluit-pad draagt `stage` met herkomst `besluit`. Bij de stage
`BESLUIT` horen deze vaste velden; ze zijn er altijd, ook als ze leeg zijn:

| veld | betekenis |
|---|---|
| `zaakkenmerk` | waaronder de zaak terug te vinden is, ingevuld uit het sjabloon van de besluit-definitie. Alle stage-grammen van één besluit delen het ([RFC-022 §1.2](/rfcs/rfc-022#12-decretograms-decision_type-as-an-open-vocabulary)) |
| `besluit` | de naam van de besluit-definitie die het gram voortbracht |
| `stage` | de stap van de procedure (RFC-008): hier altijd `BESLUIT` |
| `regulation` | de `$id` van de regeling die het besluit *is* |
| `regulation_valid_from` | welke versie van die regeling gold op `op_moment`; `null` als de versie geen datum draagt |
| `executed_regulations` | álle regelingen die de uitvoering aanriep, elk met de versie die gold; die van het besluit vooraan |
| `competent_authority` | het bevoegd gezag dat de regeling noemt (RFC-002), of `null` |
| `besloten_door` | de identiteit van de cel die besloot |
| `legal_character` | altijd `BESCHIKKING`: dat is wat een decretogram is |
| `decision_type` | wélk besluit: `AFWIJZING` zodra een afwijzingsvoorwaarde vervuld was, anders wat het artikel aanwijst, of `null`. Een open vocabulaire ([RFC-022 §1.2](/rfcs/rfc-022#12-decretograms-decision_type-as-an-open-vocabulary)) |
| `afwijzingsgrond` | de vervulde afwijzingsvoorwaarden, elk met uitkomst, waarde en artikel; leeg als het besluit niet afwees |
| `hook_niet_uitgevoerd` | de hooks die vuurden maar niet draaiden omdat een input ontbrak |
| `obligations` | het betalingsschema: per termijn vervaldatum, bedrag, volgnummer, soort, schuldenaar, schuldeiser, betalende cel, ritme met herkomst, grondslag en het artikel dat de verplichting oplegt. Leeg bij een afwijzing |
| `wacht_op_bekendmaking` | de verplichtingen die pas bij de bekendmaking gaan lopen: alles staat vast behalve de vervaldatum |
| `niets_te_betalen` | de verplichtingen waarvan het bedrag op nul uitkwam |
| `chronicle_sources` | de eigen kronieken die als databron klaarstonden, elk met haar stand op het moment van het besluit: aantal grammen en een hash erover ([RFC-022 §1.3](/rfcs/rfc-022#13-executograms-chronicles-directory)) |

### De bekendmaking

De bekendmaking van een besluit (Awb 3:40 jo. 3:41) is een eigen decretogram met
stage `BEKENDMAKING` op hetzelfde zaakkenmerk, met de naam
`<besluit>_bekendmaking`. Het herhaalt de uitkomsten van het besluit niet, maar
draagt `besluit_op_moment` en `besluit_gram` (waar het besluit ligt),
`bekendgemaakt_door`, de hooks die bij deze stage vuurden (`hooks`), de
uitkomsten die de eigen regeling bij deze stage levert (`stage_uitkomsten`, en
wat ze niet leverde: `stage_uitkomst_niet_geleverd`), de termijnen die bij de
bekendmaking ingeroosterd zijn (`obligations`), en `termijnen_vervallen_door` als
er intussen een besluit in de plaats kwam. De publieke wereld maakt niets bekend;
de scenario's `packages/simulator/scenarios/bekendmaking*.yaml` wel.

### Een eigen vaststelling zonder engine

Een bron-cel heeft geen wetten en geen engine, maar kan wel een eigen
vaststelling vastleggen. In de publieke wereld doet de Belastingdienst dat met
de aanslag (`aanslag_vastgesteld`). Zo'n gram is een decretogram (`intake:
eigen_besluit`), maar het draagt geen `stage` en geen vaste velden: elke waarde
erin heeft herkomst `recorded`, want er heeft geen uitvoering gedraaid.

## Executogram

Een executogram is de vastlegging van een feit dat de cel overkwam: een
aanvraag, een levering, een betaling
([RFC-022 §1.3](/rfcs/rfc-022#13-executograms-chronicles-directory)). Het is geen
uitkomst van een regeling. Elk veld erin heeft herkomst `recorded`.

In de publieke wereld dient de burger op 10 januari 2024 een aanvraag in. Dat
zijn twee grammen: de burger legt in haar eigen kroniek vast wat zij indiende,
en Dienst Toeslagen legt in de zijne vast wat hem geleverd is. Die van Dienst
Toeslagen:

```json
{
  "kind": "executogram",
  "name": "aanvraag_ontvangen",
  "intake": "aanvraag",
  "recording_actor": "toeslagen",
  "grondslag": "levering door cel 'burger' met actie 'burger.aanvraag'",
  "op_moment": "2024-01-10",
  "fields": {
    "bsn": {
      "value": "999993653",
      "origin": {
        "herkomst": "recorded",
        "intake": "aanvraag",
        "grondslag": "levering door cel 'burger' met actie 'burger.aanvraag'"
      }
    },
    "jaar": { "value": 2024, "origin": { "herkomst": "recorded", "intake": "aanvraag", "grondslag": "…" } },
    "ondertekend_op": { "value": "2024-01-09", "origin": { "herkomst": "recorded", "intake": "aanvraag", "grondslag": "…" } }
  }
}
```

Wat een executogram van een bepaalde naam minstens draagt, zegt het
**gebeurtenisschema** van zijn stroom, en dat is data in het wereldbestand en
geen code:

```yaml
- stream: aanvragen
  key: bsn
  gebeurtenissen:
    - name: aanvraag_ontvangen
      intake: aanvraag
      grondslag: Algemene wet inkomensafhankelijke regelingen, art. 15
      fields:
        - name: bsn
          type: string
        - name: jaar
          type: number
        - name: ondertekend_op
          type: date
```

Het gram hierboven noemt een eigen grondslag (de levering), en die gaat voor;
een gram zonder eigen grondslag krijgt die van het schema.

`chronicle.json` beschrijft zo'n definitie: `stream` (de naam), `key` (het veld
waarop de stroom groepeert en waarop een reductie zoekt) en `gebeurtenissen`
(per naam het kanaal, de grondslag en de velden met hun type: `string`,
`number`, `amount`, `boolean` of `date`). Het gebeurtenisschema is een
ondergrens: een gram mag meer dragen. Een stroom zonder `gebeurtenissen` wordt
niet getoetst. RFC-022 §1.3 zet deze definities in een eigen map `chronicles/`
naast het corpus, met een schets die er anders uitziet (`$id`,
`recording_actor`, `chronicle` en `events` met verwijzingen als
`$external.amount_cents`). In de testopstelling staan ze vandaag in het
wereldbestand, onder `cells[].chronicles[]`, en `chronicle.json` legt die vorm
vast. De cel die de stroom houdt is daar de `recording_actor`, dus een eigen
veld is niet nodig. Waar de definities uiteindelijk komen te staan, ligt nog
niet vast.

### De betaling

Een betaling is een executogram dat het platform zelf vastlegt als een termijn
uit een besluit vervalt. De cel die betaalt legt `betaling_gedaan` vast in haar
kroniek `betalingen` (kanaal `betaling`), de cel die besloot krijgt
`betaling_gemeld` (kanaal `levering`). Bij een negatief slotbedrag, als het
artikel dat toestaat, keert de verplichting om en heten de grammen
`terugvordering_gedaan` en `terugvordering_gemeld`.

Die vier namen zijn een vaste woordenschat van het platform, geen vrije keuze
van een wereldbestand: `gram.json` herkent een betaling aan haar naam
(`^(betaling|terugvordering)_(gedaan|gemeld)$`) en eist dan de velden hieronder.
Een executogram onder een van die namen dat ze niet draagt, valideert niet.

De eerste termijn van de toekenning, bij de Belastingdienst (waarden zonder
herkomst):

| veld | waarde | betekenis |
|---|---|---|
| `zaakkenmerk` | `zorgtoeslag/999993653` | de zaak van het besluit, en de sleutel van de stroom `betalingen` |
| `bedrag` | `49294` | het bedrag van de termijn |
| `volgnummer` | `1` | het nummer van de termijn binnen de verplichting, vanaf 1 |
| `soort` | `betaling` | `betaling` of `terugvordering` |
| `schuldenaar` | `Dienst Toeslagen` | de partij die nakomt: een naam uit het recht, geen cel |
| `schuldeiser` | `999993653` | de partij aan wie nagekomen wordt |
| `besluit` | `zorgtoeslag_toekenning` | het besluit waaruit de termijn volgt |
| `besluit_cel` | `toeslagen` | de cel die besloot |
| `besluit_kroniek` | `beschikkingen` | de kroniek waarin dat besluit ligt |
| `besluit_gram` | `0` | de plek van het besluit in die kroniek, vanaf nul |
| `besluit_op_moment` | `2024-03-01` | het moment van het besluit |

`besluit_cel`, `besluit_kroniek` en `besluit_gram` samen vormen de verwijzing
`<cel>|<kroniek>|<plek>` waarmee het beeld een gram aanwijst. Daarmee komt een
lezer van de betaling bij het besluit, en via het besluit bij het receipt en de
uitvoeringstrace: het besluit is na te lopen (RFC-013). `gram.json` eist de drie
samen of geen van drieën.

## Het beeld van de wereld

`world-snapshot.json` beschrijft het antwoord van `GET /api/world` (en het beeld
in de antwoorden van de routes die de wereld veranderen): de stand van de klok,
de instellingen en welke daarvan vastliggen, per cel wat ze publiceert, wat ze
kan besluiten en haar kronieken met hun grammen, de acties met formulier en
beschikbaarheid, elk contact over een celgrens, de waarschuwingen en het
journaal. Voor de grammen verwijst het naar `gram.json`, voor de
gebeurtenisschema's naar `chronicle.json`.
