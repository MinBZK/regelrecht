# De poort — wat blokkeert, en wanneer

De methode staat of valt bij één regel: **overslaan mag, stil overslaan niet.**
Zonder dwang loopt elk register vol met punten die niemand meer sluit; met de
verkeerde dwang staat het werk stil op een moment dat de analist nog aan het
denken is.

Vandaar de scheiding: **blokkeren op de mijlpaal, adviseren daarbuiten.**

## Twee momenten

| moment | wat de poort doet |
|---|---|
| **tijdens het werk** | meldt wat er ontbreekt, houdt niets tegen. De verrijking rekent door met een werkhypothese; dat is de bedoeling |
| **bij een mijlpaal** | blokkeert. Een mijlpaal is een reductie op een peildatum, en die kan niet sluiten met punten die hun uitspraak nog missen |

De dwang zit dus op het geheel, niet op de losse run. Dat is ook waar hij hoort:
een statusniveau van een corpus is een uitspraak over de hele vertaling.

## De vijf regels

1. **Een ingevulde open term zonder claim bestaat niet.** De invulling wijst naar
   een claim met grond, alternatief en bevoegde — of hij is niet verantwoord.
2. **`tijdelijk_vastgesteld` zonder bevoegde of zonder termijn bestaat niet.**
   Anders is de tussenstand meteen de parkeerplaats.
3. **Sluiten vergt een andere actor dan vaststellen.** Eén persoon duwt een punt
   niet in zijn eentje van voorstel naar bekrachtiging.
4. **Openheid is een bewering, geen afwezigheid.** Een leeg antwoordblok telt niet
   als antwoord.
5. **Achterstallige punten blokkeren de mijlpaal** — tenzij er een uitstel met een
   reden ligt.

Regel 1 tot en met 4 adviseren buiten de mijlpaal; alle vijf blokkeren hem.

## Een poort die niets vangt, is niet te onderscheiden van een die werkt

Beide zijn groen. Elke regel hoort daarom een negatieve test te hebben: één geval
dat rood moet worden en één dat groen blijft. Zonder dat tweede geval kun je een
poort "repareren" door hem alles te laten doorlaten.

Twee valkuilen uit de praktijk, allebei kosteloos te vermijden:

- **een verdict op stderr.** Een poort die zijn uitkomst naar stderr schrijft en in
  een `| grep` hangt, meldt niets op stdout en faalt of slaagt om de verkeerde
  reden.
- **een pipe zonder `pipefail`.** `script | tail -1` levert de exitcode van `tail`
  — altijd nul. Het rood van het script erachter valt weg. Een poort waarvan het
  falen niet doorkomt, is geen poort.

## Wat de poort niet moet doen

**Niet raden op naam.** Zolang de invulling niet naar haar claim wijst, kan een
controle alleen namen vergelijken, en dat faalt in beide richtingen: een
naamsvariant met streepjes wordt gemist, en een punt dat een woord terloops noemt
wordt ten onrechte aangewezen. Rapporteer die gelijkenis dan als *hint* — "er is
mogelijk een punt, en er wijst niets naar" — en niet als groen. Het verschil
tussen wat een mens vindt en wat een machine vindt, is precies de meting.

**Niet alles groen maken.** Een fix die de uitkomst verbetert door de controle te
verzwakken, is geen fix. Meet vooraf wat je verwacht, en vergelijk daarna.
