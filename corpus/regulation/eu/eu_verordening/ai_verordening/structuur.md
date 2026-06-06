# AI-verordening (EU) 2024/1689 — interne structuur

Stelsel-graaf van de opbouw van de verordening, gegenereerd uit het corpus-bestand
[`2024-08-01.yaml`](./2024-08-01.yaml) (bron: EUR-Lex, authentieke NL-tekst).
113 artikelen in 13 hoofdstukken (16 afdelingen) + 13 bijlagen.

## Hoofdstukken en afdelingen

```mermaid
flowchart LR
  AIV(["AI-verordening (EU) 2024/1689"]):::root

  AIV --> HI["<b>H.I</b> ALGEMENE BEPALINGEN<br/>art. 1–4"]:::chap
  AIV --> HII["<b>H.II</b> VERBODEN AI-PRAKTIJKEN<br/>art. 5"]:::chap
  AIV --> HIII["<b>H.III</b> AI-SYSTEMEN MET EEN HOOG RISICO<br/>art. 6–49"]:::chap
  HIII --> HIIIS1["Afd. 1 Classificatie van AI-systemen als AI-sy…<br/>art. 6–7"]:::sec
  HIII --> HIIIS2["Afd. 2 Eisen voor AI-systemen met een hoog ris…<br/>art. 8–15"]:::sec
  HIII --> HIIIS3["Afd. 3 Verplichtingen van aanbieders en gebrui…<br/>art. 16–27"]:::sec
  HIII --> HIIIS4["Afd. 4 Aanmeldende autoriteiten en aangemelde …<br/>art. 28–39"]:::sec
  HIII --> HIIIS5["Afd. 5 Normen, conformiteitsbeoordeling, certi…<br/>art. 40–49"]:::sec
  AIV --> HIV["<b>H.IV</b> TRANSPARANTIEVERPLICHTINGEN VOOR AANBIEDERS E…<br/>art. 50"]:::chap
  AIV --> HV["<b>H.V</b> AI-MODELLEN VOOR ALGEMENE DOELEINDEN<br/>art. 51–56"]:::chap
  HV --> HVS1["Afd. 1 Classificatieregels<br/>art. 51–52"]:::sec
  HV --> HVS2["Afd. 2 Verplichtingen voor aanbieders van AI-m…<br/>art. 53–54"]:::sec
  HV --> HVS3["Afd. 3 Verplichtingen van aanbieders van AI-mo…<br/>art. 55"]:::sec
  HV --> HVS4["Afd. 4 Praktijkcodes<br/>art. 56"]:::sec
  AIV --> HVI["<b>H.VI</b> MAATREGELEN TER ONDERSTEUNING VAN INNOVATIE<br/>art. 57–63"]:::chap
  AIV --> HVII["<b>H.VII</b> GOVERNANCE<br/>art. 64–70"]:::chap
  HVII --> HVIIS1["Afd. 1 Governance op Unieniveau<br/>art. 64–69"]:::sec
  HVII --> HVIIS2["Afd. 2 Nationale bevoegde autoriteiten<br/>art. 70"]:::sec
  AIV --> HVIII["<b>H.VIII</b> EU-DATABANK VOOR AI-SYSTEMEN MET EEN HOOG RIS…<br/>art. 71"]:::chap
  AIV --> HIX["<b>H.IX</b> MONITORING NA HET IN DE HANDEL BRENGEN, INFOR…<br/>art. 72–94"]:::chap
  HIX --> HIXS1["Afd. 1 Monitoring na het in de handel brengen<br/>art. 72"]:::sec
  HIX --> HIXS2["Afd. 2 Delen van informatie over ernstige inci…<br/>art. 73"]:::sec
  HIX --> HIXS3["Afd. 3 Handhaving<br/>art. 74–84"]:::sec
  HIX --> HIXS4["Afd. 4 Rechtsmiddelen<br/>art. 85–87"]:::sec
  HIX --> HIXS5["Afd. 5 Toezicht, onderzoek, handhaving en moni…<br/>art. 88–94"]:::sec
  AIV --> HX["<b>H.X</b> GEDRAGSCODES EN RICHTSNOEREN<br/>art. 95–96"]:::chap
  AIV --> HXI["<b>H.XI</b> BEVOEGDHEIDSDELEGATIE EN COMITÉPROCEDURE<br/>art. 97–98"]:::chap
  AIV --> HXII["<b>H.XII</b> SANCTIES<br/>art. 99–101"]:::chap
  AIV --> HXIII["<b>H.XIII</b> SLOTBEPALINGEN<br/>art. 102–113"]:::chap

  classDef root fill:#0b5fff,color:#fff,stroke:#0b5fff;
  classDef chap fill:#e8f0ff,stroke:#0b5fff,color:#11264d;
  classDef sec fill:#f5f8ff,stroke:#9bb8ff,color:#11264d;
```

## Bijlagen en de artikelen waarop ze betrekking hebben

Bijlagen zijn in het corpus als pseudo-artikelen opgenomen (`number: 'Bijlage III'`),
omdat het schema (nog) geen aparte `annexes`-sectie kent. De stippellijnen tonen het
artikel dat naar de bijlage verwijst (afgeleid uit de bijlage-ondertitel).

```mermaid
flowchart LR
  BI["<b>Bijlage I</b><br/>Lijst van harmonisatiewetgeving van de Unie"]:::anx
  BII["<b>Bijlage II</b><br/>Lijst van in artikel 5, lid 1, eerste alinea, pun…"]:::anx
  BII -.->|"bedoeld in"| ART5["art. 5"]:::art
  BIII["<b>Bijlage III</b><br/>In artikel 6, lid 2, bedoelde AI-systemen met een…"]:::anx
  BIII -.->|"bedoeld in"| ART6["art. 6"]:::art
  BIV["<b>Bijlage IV</b><br/>Technische documentatie als bedoeld in artikel 11…"]:::anx
  BIV -.->|"bedoeld in"| ART11["art. 11"]:::art
  BV["<b>Bijlage V</b><br/>EU-conformiteitsverklaring"]:::anx
  BVI["<b>Bijlage VI</b><br/>Conformiteitsbeoordelingsprocedure op basis van i…"]:::anx
  BVII["<b>Bijlage VII</b><br/>Conformiteit op basis van een beoordeling van het…"]:::anx
  BVIII["<b>Bijlage VIII</b><br/>Informatie die moet worden verstrekt bij de regis…"]:::anx
  BVIII -.->|"bedoeld in"| ART49["art. 49"]:::art
  BIX["<b>Bijlage IX</b><br/>Informatie die moet worden ingediend bij de regis…"]:::anx
  BIX -.->|"bedoeld in"| ART60["art. 60"]:::art
  BX["<b>Bijlage X</b><br/>Wetgevingshandelingen van de Unie over grootschal…"]:::anx
  BXI["<b>Bijlage XI</b><br/>Technische documentatie als bedoeld in artikel 53…"]:::anx
  BXI -.->|"bedoeld in"| ART53["art. 53"]:::art
  BXII["<b>Bijlage XII</b><br/>Transparantie-informatie als bedoeld in artikel 5…"]:::anx
  BXII -.->|"bedoeld in"| ART53["art. 53"]:::art
  BXIII["<b>Bijlage XIII</b><br/>In artikel 51 bedoelde criteria voor de aanwijzin…"]:::anx
  BXIII -.->|"bedoeld in"| ART51["art. 51"]:::art

  classDef anx fill:#fff3e0,stroke:#ff9800,color:#5a3d00;
  classDef art fill:#eafaf0,stroke:#2e9e5b,color:#114a2a;
```
