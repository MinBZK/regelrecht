---
id: specificaties-ii-enrichment-overhaul-landen
titel: 'Specificaties II: de enrichment-overhaul landen'
faseId: wat
disciplineId: techniek
prioriteit: hoog
omvang: M
categorie: bet
capability: basis
capaciteit: conceptueel schrijver, engineer
toelichting: |-
  **Stand**: geland in acht stukken (#1448 tot en met #1453, en #1457), als
  schema v0.7.0 met `placement` op artikelen en `markings` als één kanaal dat
  `untranslatables` en `norm_gaps` vervangt (RFC-031). Open blijft de meting.

  Het ontwerp en de implementatie van een herziene enrichment-keten liggen klaar
  in één pull request, met vier documenten die van buiten naar binnen lopen: de
  werkvoorraad (welke artikelen verrijkt worden en in welke volgorde), de
  kwaliteit van één verrijking, hoe één stap draait, en waaraan het geheel
  gemeten wordt. Dit werkpakket brengt dat binnen, of neemt het anderszins over.

  **Norm gaps naast untranslatables**

  Het belangrijkste dat meekomt is schema v0.6.0, met `placement` op artikelen en
  `norm_gaps` als eigen kanaal naast `untranslatables`. Een norm die pas in een
  nog niet gevonden beleidsdocument wordt ingevuld is een gat in het corpus. Een
  norm die de taal niet kan uitdrukken is een tekortkoming van de taal. Die twee
  liepen door elkaar, en zolang dat zo is zegt geen enkele telling iets over waar
  de taal tekortschiet.

  **De meting deugt nog niet**

  Uit ronde 3 bleek dat het aantal bevindingen onbruikbaar is als maat: een model
  dat minder probeert scoort automatisch beter. De variant zonder contextbrief
  had minder bevindingen en legde nul cross-law bindingen tegen acht bij de
  variant met brief. Er moeten tellers naast de poorten komen voordat een
  volgende ronde iets bewijst.

  **Open punten uit de pull request**

  De overlappende laagnummering in het kwaliteitsdocument (ontwerplagen,
  contextlagen en de kwaliteitsladder delen cijfers zonder hetzelfde te
  betekenen), en de faalklassen die als letter worden aangehaald zonder dat die
  letters zijn toegekend.
volgorde: 1100
onderzoeksvragen:
  - Waaraan meten we de enricher, als het aantal bevindingen een model beloont
    dat minder probeert?
  - Hoeveel context heeft een verrijking nodig? De contextbrief maakte het werk
    zwaarder en liep tegen de timeout aan, en leverde tegelijk de cross-law
    bindingen op die zonder brief ontbraken.
  - vraag: >-
      Hoe controleren we wetten die door taalmodellen zijn vertaald naar code, en
      hoeveel menselijke inspanning is er per artikel nodig om deze vertaling
      juridisch verantwoord te kunnen adopteren?
    paper: sec:translation
onderzoek: loopt
bouw: wel
rfcs:
  - 26
  - 27
  - 28
  - 29
  - 33
afhankelijkVan: []
samenhangIds:
  - specificaties-i-documentatie-op-orde
  - specificaties-iii-gaten-vinden-met-de-enricher
  - referentie-casus-i
---
