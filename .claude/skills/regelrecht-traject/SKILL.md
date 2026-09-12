---
name: regelrecht-traject
description: Voordeur voor het werken in een regelrecht-traject. Gebruik dit aan het begin van een traject, of bij twijfel waar te beginnen — het draagt de cyclus waar alle andere skills aan hangen (werkronde → uitspraak → verwerken → mijlpaal), zegt welke skill op welk moment aan zet is, en hoe een open punt met een escalatieladder door die cyclus reist. Routeert naar regelrecht-stelselanalyse (desk), regelrecht-audit-products (sessie) en regelrecht-verantwoording (de vorm van een claim). Casus-agnostisch. Tot 09-2026 heette deze skill regelrecht-dossier.
allowed-tools: Read, Glob, Grep, AskUserQuestion
---

# Regelrecht traject — de kapstok

Een traject is de eenheid waarin aan een vertaling wordt gewerkt. In de editor is het een
concreet ding: een eigen branch, een eigen corpus-configuratie met precies één schrijfbare
bron, een eigen annotaties-sidecar op die branch, eigen taken, één gedeelde pull request naar
de basisbranch, en leden met een rol. In de methode is het hetzelfde ding: de plek waar een
wet machine-leesbaar wordt gemaakt, de keuzes onderweg worden vastgelegd, en iemand die
daartoe bevoegd is ze bekrachtigt of verwerpt.

Deze skill is de voordeur. Hij doet zelf geen werk; hij draagt het **ritme** waaraan de
andere skills hangen, en zegt welke skill op welk moment aan zet is.

## De cyclus — één tabel

Alles in een traject beweegt in dezelfde cyclus. Elk moment heeft zijn eigen actor, en
op elk moment ontstaat een andere stand van een claim (de vorm waarin een
interpretatiekeuze telt — zie `regelrecht-verantwoording`).

| moment | wie handelt | wat hier dicht kan | claimstand die hier ontstaat | skill |
|---|---|---|---|---|
| **werkronde** — desk | model, analist | trede 0 model · 1 bron · 2 duiding | `voorgesteld` → `tijdelijk_vastgesteld` | `law-*`, `regelrecht-stelselanalyse` |
| **uitspraak** — asynchroon in de editor, of in een sessie | de bevoegde | trede 3 uitvoering · 4 jurist | `uitgesproken` | editor (`replying`-notitie) · `regelrecht-audit-products` |
| **verwerken** — desk | analist | — | corpus volgt; `gecorrigeerd` wordt een nieuw open punt | `regelrecht-stelselanalyse` |
| **mijlpaal** | de poort | — | `bewaakt`, of blokkade | `regelrecht-scenario-traces` + de poort |
| *buiten het traject* | bestuur, normsteller | trede 5 | uitstel met reden → volgende cyclus | — |

Drie regels volgen hieruit, en dit is de enige plek waar ze staan:

**1. Elke overgang vergt een andere actor, en sluiten vergt een bevoegde.** Daarom kan één
persoon een punt niet in zijn eentje van voorstel naar bekrachtiging duwen — dat is geen
afspraak maar een gevolg van wie welke stap zet. *Waar* de bevoegde spreekt is vrij:
asynchroon in de editor, of in een sessie. Dat de sessie lang de enige plek was, kwam doordat
de tooling geen bevoegde kende. Dat is een werkwijze om een gat heen, geen principe.

**2. Een cyclus zonder uitspraken verhoogt het zekerheidsniveau nooit.** Niet "zonder sessie":
de poort telt uitspraken. De sessie blijft het instrument voor wat asynchroon niet kan — samen
adversarial scenario's bouwen, doorpraten als het antwoord "het hangt ervan af" is, veel punten
in één keer sluiten met de juiste mensen aan tafel. Haar agenda is wat nog openstaat nádat het
asynchrone spoor zijn werk heeft gedaan.

**3. Een mijlpaal is een reductie op een peildatum**, geen document dat iemand schrijft. Daar
blokkeert de poort, en nergens anders. Tussen mijlpalen adviseert hij alleen; de verrijking
en de berekening lopen altijd door.

## De escalatieladder ís de cyclus

Een open punt heeft een trede: wie het mag sluiten. Dat is geen apart register maar de vraag
*op welk moment in de cyclus dit dicht kan*.

| trede | wie sluit | moment |
|---|---|---|
| 0 · model | de analist, met het corpus zelf | werkronde |
| 1 · bron | de analist, door te halen | werkronde |
| 2 · duiding | de analist, door te lezen — als werkhypothese, tot een bevoegde spreekt | werkronde |
| 3 · uitvoering | wie de regeling uitvoert | uitspraak |
| 4 · jurist | wie over de lezing gaat | uitspraak |
| 5 · bestuur / normsteller | buiten het traject | uitstel met reden; reist mee naar de volgende cyclus |

**De trede wordt afgeleid, niet gekozen.** Twee dingen bepalen hem, en het corpus weet ze
allebei: waar het punt aan hangt (het artikel, en bij een open term letterlijk
`open_terms[].delegated_to` — "wie mag deze term invullen"), en wat voor soort vraag het is
(feit → uitvoering; lezing → jurist; bevoegdheid of normconflict → jurist of bestuur;
ontbrekend beleid → de normsteller). Klopt de afleiding niet, dan mag je haar overschrijven
mét reden. Die overschrijvingen zijn zelf een bevinding: waar de afleiding faalt, zit een gat
in het model van bevoegdheid.

Een punt op trede 5 kan het traject niet sluiten. Het wordt een uitstel met een reden, en
het reist mee. Uitstel zonder reden bestaat niet — anders wordt de tussenstand de
parkeerplaats waar alles blijft liggen.

## Welke skill wanneer

- **`regelrecht-verantwoording`** — de vorm. Geen moment, maar iets dat je op élk moment pakt
  zodra een keuze wordt gemaakt of gesloten: wat, door wie, op welke grond, met welk
  alternatief, wie moet spreken, vóór wanneer.
- **`law-download` · `law-generate` · `law-reverse-validate`** (georkestreerd door
  `law-interpret`) — de machine-laan in de werkronde: verbatim tekst, `machine_readable`, en
  de aannames als claims in stand `voorgesteld`.
- **`regelrecht-stelselanalyse`** — de desk in de werkronde en bij het verwerken: meten,
  classificeren (soort bevinding), de bevoegde afleiden, claims overnemen als
  `tijdelijk_vastgesteld`, en na een uitspraak het corpus laten volgen.
- **`regelrecht-audit-products`** — de sessie: agenda uit wat wacht, uitspraken als
  vastlegging, geen verslag.
- **`regelrecht-scenario-traces`** — de andere gedeelde laag: casuïstiek vindbaar, ketens
  leesbaar en geassert. Wat een uitspraak vasthoudt, is een test of een trace; dat is de
  stand `bewaakt`.

## Triage — alleen wat niet af te leiden is

Vroeger stelde deze skill vier vragen. Drie ervan zijn meetbaar en horen dus niet aan een
mens gesteld te worden: of er een corpus is, of het gevalideerd is, en of een open punt
feitelijk dan wel oordeel is — dat volgt uit dekking, poorten en het soort vraag. Eén vraag
blijft, en die kan alleen jij beantwoorden:

**Heb je domeinkennis of buy-in nodig die je nog niet hebt?** Dan mag een verkennende
sessie vroeg, ook vóór het corpus af is. Dat is de enige sessie zonder poort ervoor.

Bij twijfel: `AskUserQuestion`, en pas dan routeren.

## Handoff — het bestandssysteem is de interface

Skills praten niet met elkaar en niet met de editor-api; ze laten bestanden achter in de
gedeelde repo, en de editor leest diezelfde bestanden. Wat elk moment achterlaat:

| moment | laat achter | waar |
|---|---|---|
| werkronde | wet-YAML, scenario's, claims | `regulation/…`, `…/scenarios/*.feature`, `annotations/{law_id}/annotations.yaml` op de trajectbranch |
| uitspraak | een `replying`-notitie met verdict en reden | dezelfde sidecar |
| verwerken | gewijzigde YAML, en de claim die zegt waarom | corpus + sidecar |
| mijlpaal | de uitkomst van de poort | de PR van het traject |

De annotaties-sidecar is het scharnier: wat een skill daar append-only in schrijft, ziet de
editor zonder tussenstap. Hoe dat schrijven precies moet — nooit het bestand herbouwen —
staat in `regelrecht-verantwoording`.

## Verder

- `references/routing.md` — de flow als diagram, en wat er in welke richting stroomt
- `references/zakkaart.md` — één A4 voor wie nieuw is
- `regelrecht-verantwoording/references/editor.md` — wat de editor al draagt, wat gereserveerd
  is, en wat ontbreekt
