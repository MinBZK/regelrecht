---
name: regelrecht-verantwoording
description: >
  Legt vast wie een interpretatiebeslissing nam, op welke grond, met welk
  alternatief, en wie erover moet spreken voordat het punt dicht kan. Gebruik dit
  wanneer een open term wordt ingevuld, een open norm een lezing krijgt, of een
  bepaling meerdere kanten op kan — dus bij elke verrijking die niet uit de tekst
  volgt. Dossier-agnostisch. Voor het afdwingen ervan bij een mijlpaal: zie
  references/poort.md.
allowed-tools: Read, Glob, Grep, Bash, Edit, Write
user-invocable: true
---

# Verantwoording — de vorm waarin een lezing telt

Een verrijking die een open term invult, kiest een lezing. Vandaag verdwijnt die
keuze: de invulling komt in het corpus, de reden staat hoogstens in proza ernaast,
en na een maand is niet meer te zien wie het besloot of waarom.

Deze skill legt de vorm vast waarin zo'n keuze wél telt, en de standen die zij
doorloopt. Hij bouwt geen nieuw register: een claim woont bij datgene waar zij
over gaat.

## Wanneer je hem pakt

- de verrijking vult een `open_term` of kiest tussen twee lezingen van een bepaling
- een `untranslatable` krijgt alsnog een invulling
- een analist neemt een voorstel van een model over, of verwerpt het
- de bevoegde spreekt zich uit over een eerder vastgelegde lezing

Niet: een gewone opmerking bij een tekst. Zie *Wat géén claim is*, onderaan.

## Het contract — zes vragen

Een opmerking telt pas als claim wanneer zij deze zes beantwoordt. Kan zij dat
niet, dan is het een opmerking en verder niets.

| | vraag | waar het vandaan komt |
|---|---|---|
| 1 | **wat** is vastgesteld | de lezing zelf |
| 2 | **door wie** | de actor die de stand zette |
| 3 | **op welke grond** | wettekst, wetsgeschiedenis, systematiek, uitvoeringspraktijk |
| 4 | **wanneer** | het moment, niet de peildatum van een bestand |
| 5 | **wie moet erover spreken** | de bevoegde — zie hieronder |
| 6 | **vóór wanneer** | een mijlpaal, geen datum |

Vraag 1 tot en met 4 zijn de vastleggingselementen; git levert er drie van als je
per claim commit. Vraag 5 en 6 kan git niet weten en zijn dus altijd velden.

**Waarom het alternatief erbij hoort.** Een lezing zonder haar verworpen
alternatief is een bewering, geen oordeel. De bevoegde kan niets bekrachtigen als
hij niet ziet waartegen gekozen is, en het burger-effect van de keuze is pas te
wegen als beide kanten er staan. Noem per alternatief het gevolg.

## Vier standen, en elke overgang vergt een andere actor

| stand | wie zet hem | betekent |
|---|---|---|
| `voorgesteld` | model | de verrijking stelde een lezing voor; geen mens keek ernaar |
| `tijdelijk_vastgesteld` | analist | overgenomen mét grond, alternatief en benoemde bevoegde; **rekent mee** |
| `uitgesproken` | de bevoegde | bekrachtigd, gecorrigeerd, weerlegd, of bewust onbeslist gelaten |
| `bewaakt` | poort | verwerkt, en een test of trace houdt het vast |

Dat elke overgang een andere actor vergt, is de dragende regel: één persoon kan
een punt niet in zijn eentje van voorstel naar bekrachtiging duwen.

**De verrijking stopt nooit op een open rechtsvraag.** Er komt een werkende versie
uit, met een werkhypothese, en de keten rekent door. Wat wél afgedwongen wordt is
dat er een uitspraak komt vóór het punt dicht kan, en dat een mijlpaal niet sluit
met achterstallige punten. Uitstel mag, maar is zelf een vastlegging met een reden
— anders wordt `tijdelijk_vastgesteld` de nieuwe parkeerplaats.

## Wie is de bevoegde?

Leid af, declareer niet. Twee dingen bepalen het, en het corpus weet ze allebei:

1. **waar de claim aan hangt** — het artikel, en daarmee het orgaan dat erover gaat.
   Voor een open term staat dat er letterlijk: `open_terms[].delegated_to` ("wie mag
   deze term invullen").
2. **wat voor soort vraag het is** — feit → uitvoering; lezing → jurist;
   bevoegdheid of normconflict → jurist of bestuur; ontbrekend beleid → de
   normsteller.

Klopt de afleiding niet, dan mag je haar overschrijven, mét reden. Die
uitzonderingen zijn het interessantste rapport dat de methode oplevert: waar de
afleiding faalt, zit een gat in het model van bevoegdheid.

## Waar de claim woont

**Het anker beslist.** Bouw geen tweede administratie.

- gaat de claim over **wettekst** → een stand-off notitie bij die tekst
  (RFC-005/RFC-018). Het citaat-anker overleeft hernummering, de resolver meldt
  *found · ambiguous · orphaned*, en de notitie reist mee in een federatie.
- gaat de claim over het **model** — een dode parameter, een output die niemand
  leest, een open term die nergens aan hangt — dan is er geen citaat om aan te
  hangen. Die hoort bij het model, in de YAML.

Een taak is een verwijzing ("X moet uitspraak doen over claim Y"), geen
bewaarplaats. Een overzicht of dashboard is een reductie. Niets is twee keer
canoniek.

## De invulling wijst naar haar verantwoording

Het sluitstuk, en op dit moment het gat. Een `implements`-blok dat een open term
vult, hoort naar de claim te wijzen:

```yaml
implements:
  - law: wet_op_de_zorgtoeslag
    article: '2'
    open_term: standaardpremie
    gelet_op: ...
    verantwoording: <verwijzing naar de claim>   # ← nog niet in het schema
```

Zonder dat veld kan een controle alleen op naam raden, en raden faalt in beide
richtingen: een dossier dat `waarde-motorvoertuig` heet wordt niet gevonden bij een
open term `waarde_motorvoertuig`, en een punt dat een woord terloops noemt wordt
ten onrechte als de verantwoording aangewezen.

Zolang het veld er niet is: leg de claim vast als notitie bij de tekst, en noteer
in de `gelet_op`-beschrijving naar welke notitie hij verwijst. Dat is een
werkhypothese van de methode zelf — met dezelfde plicht: hij moet dicht.

## Wat géén claim is

- een opmerking bij een tekst zonder gevolg voor de uitvoering
- een leeg antwoordveld. **Openheid is een bewering, geen afwezigheid.** Een
  antwoordblok waarvan elke sleutel leeg is, leest voor elke controle als
  beantwoord en is het niet. Zet de stand expliciet.
- een verwijzing naar een register-id uit één traject. Die bestaat nergens anders.

## Verder

- `references/poort.md` — de vijf regels die een mijlpaal blokkeren, en hoe je ze draait
- `references/vorm.md` — de vastleggingsvorm veld voor veld, met een voorbeeld
- `regelrecht-stelselanalyse` — de deskcyclus waarin deze claims ontstaan
- `regelrecht-audit-products` — de sessie waarin de bevoegde zich uitspreekt
