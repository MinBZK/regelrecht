# CSRD — stelsel en grondslagen

Het **startpunt** voor de regelrecht-modellering van CSRD. Toont op welk
**regulatory_layer-niveau** de inhoudelijke regels staan en welke
organisaties uitvoering en toezicht doen. Verwerkt de juristen-input
van 2026-05-13 (zie [jurist-input-2026-05-13.md](jurist-input-2026-05-13.md)).

## Stelsel-diagram

```mermaid
flowchart TB
  classDef richtlijn fill:#fff5e0,stroke:#c0392b,stroke-width:2px,color:#000;
  classDef verordening fill:#ffe6cc,stroke:#d35400,color:#000;
  classDef wijziging fill:#fce4ec,stroke:#c2185b,stroke-dasharray:5 5,color:#000;
  classDef wet fill:#fff8e1,stroke:#f57f17,color:#000;
  classDef beleid fill:#f3e5f5,stroke:#7b1fa2,color:#000;
  classDef uitvoer fill:#e3f2fd,stroke:#1565c0,color:#000;

  subgraph EU["EU-niveau"]
    direction LR
    R2013["Richtlijn 2013/34/EU<br/>Accounting Directive<br/>━━━━━━━━<br/>art. 1.3 — scope-criteria<br/>art. 19 bis — rapportage individueel<br/>art. 29 bis — geconsolideerd"]:::richtlijn
    R2022["Richtlijn 2022/2464/EU<br/>CSRD<br/>━━━━━━━━<br/>art. 5 lid 2.b — ingangsdatum<br/>(voegt art. 19 bis, 29 bis,<br/>29b toe aan 2013/34)<br/><i>amendeert 2013/34</i>"]:::wijziging
    OMNI["Omnibus 2026<br/>━━━━━━━━<br/>vereenvoudigingen<br/>drempels: 1000 werknemers,<br/>€450M omzet<br/><i>amendeert 2013/34</i>"]:::wijziging
    ESRS["Verordening (EU) 2023/2772<br/>ESRS-standaarden<br/>━━━━━━━━<br/>12 sector-agnostische<br/>(E1-E5, S1-S4, G1, ESRS 1+2)<br/><i>delegated act onder art. 29b</i>"]:::verordening
  end

  subgraph NL["NL-niveau (omzetting)"]
    direction LR
    BW["Boek 2 BW titel 9<br/>jaarrekening<br/>━━━━━━━━<br/>art. 391 e.v.<br/><i>omzetting 2013/34</i>"]:::wet
    WICSRD["Wet implementatie CSRD<br/>━━━━━━━━<br/>wijzigt Boek 2 BW<br/><i>omzetting 2022/2464</i>"]:::wet
  end

  subgraph UITV["Uitvoering & toezicht"]
    direction LR
    AFM["AFM<br/>━━━━━━━━<br/>toezicht beursgenoteerde<br/>ondernemingen"]:::uitvoer
    KVK["KVK<br/>━━━━━━━━<br/>deponering<br/>+ openbaarmaking"]:::uitvoer
    ACC["Externe accountant<br/>━━━━━━━━<br/>assurance op<br/>duurzaamheidsverslag"]:::uitvoer
    RJ["Raad voor de<br/>Jaarverslaggeving (RJ)<br/>━━━━━━━━<br/>interpretatie /<br/>RJ-uitingen"]:::beleid
  end

  %% EU-niveau amendments
  R2022 -.->|"voegt art. 19 bis,<br/>29 bis, 29b toe"| R2013
  OMNI -.->|"vereenvoudigt<br/>scope-drempels"| R2013
  R2013 -->|"art. 29b<br/>(toegevoegd door CSRD)<br/>— delegated act"| ESRS

  %% EU → NL omzetting
  R2013 ==>|"omzetting"| BW
  R2022 ==>|"omzetting"| WICSRD
  WICSRD -->|"wijzigt"| BW

  %% NL → Uitvoering
  BW --> AFM
  BW --> KVK
  ESRS --> ACC
  BW --> RJ
```

**Legenda:**

- 🟥 **Richtlijn (rood)** — primaire EU-richtlijn (2013/34/EU)
- **Wijzigingsrichtlijn (roze, gestippeld)** — amendeert een eerdere richtlijn (CSRD 2022/2464 en Omnibus 2026 wijzigen beide 2013/34)
- 🟧 **Verordening (oranje)** — gedelegeerde Commissie-verordening (ESRS-standaarden)
- 🟨 **Wet (geel)** — Nederlandse omzettingswet
- 🟪 **Beleid (paars)** — interpretatieve uitingen (RJ)
- 🟦 **Uitvoeringsorganisatie (blauw)** — toezicht of administratieve afhandeling

**Lijntypes:**

- `══>` dikke pijl — omzetting (van EU naar NL) of toepassing (van wet naar uitvoerder)
- `-.->` gestippeld — amendement (wijzigingsrichtlijn)
- `-->` gewone pijl — afgeleide regelgeving of toepassings-relatie

## Welke regeling op welk niveau

| Niveau | Rol | Wat staat hier |
|--------|-----|----------------|
| **EU-richtlijn** (2013/34) | Hoofdregel | Scope-criteria, rapportage-verplichtingen, geconsolideerde rapportage |
| **EU-richtlijn (wijziging)** (2022/2464 CSRD, Omnibus 2026) | Amendement | Ingangsdata, drempelwaarde-aanpassingen, vereenvoudigingen |
| **EU-verordening (delegated act)** (2023/2772 ESRS) | Detaillering | Concrete rapportage-datapunten per standaard (~1000 totaal) |
| **NL-wet** (Boek 2 BW + Wet impl. CSRD) | Omzetting | Nederlandse uitwerking van EU-richtlijn — wat in NL geldend recht is |
| **Beleid** (RJ-uitingen) | Interpretatie | Hoe accountants/bedrijven de wet toepassen |
| **Uitvoering** (AFM, KVK, accountants) | Handhaving | Toezicht, deponering, assurance |

## Wat dit diagram laat zien voor het kick-off-gesprek

**Drie inzichten** die direct uit de structuur volgen:

1. **CSRD is geen op zichzelf staande wet** — het is een amendement op
   de Accounting Directive (2013/34/EU). Wie CSRD doorgrondt, leest
   noodzakelijk in 2013/34. Plus de geconsolideerde versie (datum
   2026-03-18) bevat de Omnibus-vereenvoudigingen al.

2. **De drempelwaarden zijn niet in de CSRD zelf gedefinieerd** —
   ze staan in 2013/34/EU art. 1 lid 3 en zijn door Omnibus 2026
   omhooggebracht naar 1000 werknemers en €450M omzet. Dit is precies
   een patroon van afschaffing/wijziging door latere wetswijziging.

3. **ESRS-standaarden zijn een aparte regulatory_layer** — niet de
   richtlijn maar een gedelegeerde Commissie-verordening (2023/2772).
   Dat onderscheid is belangrijk voor regelrecht-modellering: ESRS is
   geen "wet" maar `EU_VERORDENING` met andere wijzigingsritmes.

## Wat *niet* in dit diagram staat (bewust)

- **Niet-EU bedrijven** — third-country undertakings hebben aparte
  thresholds. Jurist heeft deze buiten beschouwing gelaten voor de
  eerste fase.
- **Sector-specifieke ESRS** — naast de 12 sector-agnostische
  standaarden komen er sector-specifieke (financiële instellingen,
  agri, mijnbouw, etc.). Niet voor de eerste slice.
- **NFRD-overgangsrecht** — de oude Non-Financial Reporting Directive
  (2014/95/EU) is door CSRD vervangen. Eventuele transitie-regelingen
  vallen buiten scope.
- **Sustainable Finance Disclosure Regulation (SFDR)** en
  **Taxonomieverordening** — buurregelingen die wel raakvlakken
  hebben (concept-definities zoals materialiteit), maar niet de scope
  van het CSRD-werk.

Zie [scope-bepaling.md](scope-bepaling.md) voor de wegkruis-tabel die
deze keuzes formaliseert met de jurist.
