# Regelrecht — zakkaart voor regelanalisten

*Eén A4. Waar begin ik · welke skill · hoe loopt een traject. (Methode bekend verondersteld.)*

## Bij twijfel: de voordeur
**Beschrijf wat je wilt → `regelrecht-traject` wijst de weg.** Je hoeft geen skill-namen te
onthouden. Eén vraag kan alleen jij beantwoorden: *heb je domeinkennis of buy-in nodig die je
nog niet hebt?* De rest is meetbaar.

## De cyclus — vier momenten, vier actoren

| moment | wie | wat gebeurt er | skill |
|---|---|---|---|
| **werkronde** | model + analist | tekst → YAML; aannames worden claims; de desk neemt ze over mét bevoegde en termijn | `law-interpret` · `regelrecht-stelselanalyse` |
| **uitspraak** | de bevoegde | bekrachtigt, corrigeert, weerlegt, of laat bewust open — asynchroon in de editor of in een sessie | editor · `regelrecht-audit-products` |
| **verwerken** | analist | het corpus volgt de uitspraak; *gecorrigeerd* wordt een nieuw open punt | `regelrecht-stelselanalyse` |
| **mijlpaal** | de poort | blokkeert op achterstallige punten; anders een reductie op een peildatum | `regelrecht-scenario-traces` + poort |

```
 werkronde ──▶ uitspraak ──▶ verwerken ──▶ mijlpaal ──▶ (volgende cyclus)
     ▲                            │
     └──── gecorrigeerd ──────────┘
```

> **Drie regels.** Elke overgang vergt een andere actor, en sluiten vergt een bevoegde.
> Een cyclus zonder uitspraken verhoogt het zekerheidsniveau nooit.
> Een mijlpaal is een reductie op een peildatum, geen document.

## Een claim — de vorm waarin een keuze telt

Zes vragen: **wat · door wie · op welke grond · wanneer · wie moet erover spreken · vóór wanneer.**
Kan een opmerking die niet beantwoorden, dan is het een opmerking. Het verworpen alternatief
hoort er verplicht bij. Vorm en werkstroom: `regelrecht-verantwoording`.

| stand | wie zet hem |
|---|---|
| `voorgesteld` | model |
| `tijdelijk_vastgesteld` | analist — rekent mee |
| `uitgesproken` | de bevoegde |
| `bewaakt` | poort |

## Een open punt — wie mag het sluiten?

| trede | wie | moment |
|---|---|---|
| 0 model · 1 bron · 2 duiding | de analist | werkronde |
| 3 uitvoering · 4 jurist | de bevoegde | uitspraak |
| 5 bestuur / normsteller | buiten het traject | uitstel mét reden → volgende cyclus |

De trede wordt **afgeleid** (artikel + `delegated_to` + soort vraag), niet gekozen.
Overschrijven mag, mét reden — en die overschrijving is zelf een bevinding.

## Soort bevinding — wat is de actie?

| label | actie |
|---|---|
| **modellering-fout** | desk fixt de YAML |
| **wetgevings-fout** | desk documenteert; normsteller is bevoegd |
| **engine-limitatie** | engine-issue |
| **acceptabele untranslatable** | een claim met een bevoegde; rekent door als werkhypothese |

Het label zegt wat het *is*. Wie het mag *sluiten* is de trede — een andere as.

## De poort

Tussen mijlpalen: **adviseert**. Op de mijlpaal: **blokkeert** — op invullingen zonder claim, op
claims zonder bevoegde of termijn, op punten over hun termijn zonder uitstel-met-reden.
Een poort die niets vangt is niet te onderscheiden van een die werkt; elke regel heeft een
negatieve test.

## Het bestandssysteem is de interface

Skills praten niet met de editor-api. Wat je achterlaat in de repo, ziet de editor: wet-YAML in
`regulation/`, scenario's in `scenarios/`, claims en uitspraken in
`annotations/{law_id}/annotations.yaml` op de trajectbranch — **append-only, nooit herbouwen.**

## Autonomie & veiligheid

`/loop` = lokale batch · `/schedule` = cloud-routine. Commits door routines gaan **alleen naar
private repos** (push-guard); bewust publiek pushen: `ALLOW_PUBLIC_PUSH=1`. Skills en
docs-site blijven casus-agnostisch — de leak-guard bewaakt het.

---
Canonieke flow: `regelrecht-traject/references/routing.md` · kapstok: `regelrecht-traject/SKILL.md`
