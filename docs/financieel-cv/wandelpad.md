# Wandelpad door de Financieel CV-wetten

Educatieve route door de zes wetten plus AWB. **Niet** de runtime-flow
van de regelhulp (die is parallel/fan-out) maar een statische tour
voor uitleg: meest verbonden eerst, dan steeds verder de buitenring in.

```mermaid
flowchart TB
  classDef hub fill:#fff5f5,stroke:#c0392b,stroke-width:3px,color:#000;
  classDef stub fill:#e3f2fd,stroke:#1565c0,color:#000;
  classDef regeling fill:#f1f8e9,stroke:#558b2f,color:#000;
  classDef alone fill:#fff8e1,stroke:#f57f17,color:#000;
  classDef proces fill:#fce4ec,stroke:#7b1fa2,color:#000;
  classDef terminal fill:#eceff1,stroke:#37474f,color:#000;

  Start([Start van de wandeling]):::terminal
  Start ==> S1

  S1["<b>Stap 1 — NRP</b><br/>Ziektewet art. 29b<br/>━━━━━━━━<br/>het hart: 3 cross-law uitgangen"]:::hub

  S1 ==>|"lid 1.a, 1.b, 4"| S2
  S1 ==>|"lid 1.c, 1.d, 2.a, 2.c"| S4
  S1 ==>|"lid 2.e, 2.f"| S6

  subgraph WIA_C["Wet WIA-cluster"]
    direction TB
    S2["<b>Stap 2 — doelgroepstub</b><br/>Wet WIA art. 1<br/>UWV-vaststelling"]:::stub
    S3["<b>Stap 3 — JC + WPA</b><br/>Wet WIA art. 35<br/>lid 4 sluit Wajong/Pwet uit"]:::regeling
    S2 -.->|intra-law| S3
  end

  subgraph WAJ_C["Wajong-cluster"]
    direction TB
    S4["<b>Stap 4 — doelgroepstub</b><br/>Wajong art. 1:1<br/>UWV-vaststelling"]:::stub
    S5["<b>Stap 5 — LDP</b><br/>Wajong art. 2:20<br/>UWV-percentage via beleidsregel"]:::regeling
    S4 -.->|intra-law| S5
  end

  subgraph PWET_C["Participatiewet-cluster"]
    direction TB
    S6["<b>Stap 6 — doelgroepstub</b><br/>Pwet art. 1<br/>college-vaststelling"]:::stub
    S7["<b>Stap 7 — LKS</b><br/>Pwet art. 10c + 10d<br/>hoogte-formule met 70%-cap"]:::regeling
    S6 -.->|intra-law| S7
  end

  S8["<b>Stap 8 — LIV + LKV</b><br/>Wtl art. 2 + art. 3<br/>━━━━━━━━<br/>standalone, Belastingdienst<br/>(LIV per 2025 afgeschaft)"]:::alone
  S9["<b>Stap 9 — PP</b><br/>WW art. 76a<br/>━━━━━━━━<br/>standalone, UWV"]:::alone

  S3 ==> S10
  S5 ==> S10
  S7 ==> S10
  S8 ==> S10
  S9 ==> S10
  S1 ==> S10

  S10["<b>Stap 10 — AWB-schil</b><br/>art. 3:46 motiveringsplicht<br/>art. 6:7 bezwaartermijn 6 wkn<br/>━━━━━━━━<br/>firet automatisch op elke BESCHIKKING"]:::proces

  S10 ==> Eind([Eind van de wandeling]):::terminal
```

## Lezing van het diagram

**Kleurcodering:**

- 🟥 *hub* (rood, dik kader) — NRP als meest verbonden regeling, het natuurlijke startpunt
- 🟦 *stub* (blauw) — doelgroepvaststelling, pass-through naar UWV/college
- 🟩 *regeling* (groen) — uitwerking-artikel binnen een cluster
- 🟨 *alone* (geel) — regeling zonder cross-law verbindingen
- 🟪 *proces* (paars) — procesrechtelijke schil

**Lijntypen:**

- ════ dikke pijl — cross-law `input.source.regulation` (echte verwijzing naar andere wet)
- ╶╶╶╶ gestippeld — intra-law (binnen dezelfde wet, niet expliciet via source)

**Drie clusters zichtbaar:**

1. **De WIA/Wajong/Pwet-cluster** — drie wetten die elk twee rollen vervullen: doelgroepstub voor NRP én eigen regeling-uitwerking. NRP is hier de orchestrator.
2. **Wtl-cluster** — twee fiscale tegemoetkomingen, geen externe afhankelijkheden, Belastingdienst voert uit.
3. **WW-monade** — alleen PP, geen connecties, UWV voert uit.

Plus AWB als procedurele schil over alles.

## Niet hetzelfde als de runtime-flow

Dit diagram laat zien **hoe je het stelsel uitlegt**, niet **hoe de regelhulp werkt**. De regelhulp Financieel CV doet een fan-out: één invoerformulier, acht parallelle bevragingen, één overzicht terug. **Twee soorten gebruikers** kunnen de regelhulp gebruiken — werkgever en werknemer — met een licht andere uitkomst (de werknemer krijgt naast het overzicht ook een persoonlijke brief). Die runtime-flow ziet eruit als:

```mermaid
flowchart LR
  classDef gebruiker fill:#e3f2fd,stroke:#1565c0,color:#000;
  classDef regeling fill:#f1f8e9,stroke:#558b2f,color:#000;
  classDef orch fill:#fff5f5,stroke:#c0392b,stroke-width:2px,color:#000;
  classDef out fill:#fff8e1,stroke:#f57f17,color:#000;

  WG([Werkgever<br/>vult scenario in]):::gebruiker
  WN([Werknemer<br/>vult scenario in]):::gebruiker
  WG ==> Orch
  WN ==> Orch

  Orch[regelhulp Financieel CV<br/>orchestrator]:::orch

  Orch ==> NRP[NRP]:::regeling
  Orch ==> LIV[LIV]:::regeling
  Orch ==> LKV[LKV]:::regeling
  Orch ==> LKS[LKS]:::regeling
  Orch ==> LDP[LDP]:::regeling
  Orch ==> JC[JC]:::regeling
  Orch ==> WPA[WPA]:::regeling
  Orch ==> PP[PP]:::regeling

  NRP ==> Out
  LIV ==> Out
  LKV ==> Out
  LKS ==> Out
  LDP ==> Out
  JC ==> Out
  WPA ==> Out
  PP ==> Out

  Out[Aggregator:<br/>welke regelingen zijn van toepassing?]:::orch
  Out ==> OutWG([Werkgever krijgt<br/>overzicht regelingen]):::out
  Out ==> OutWN([Werknemer krijgt<br/>overzicht + persoonlijke brief]):::out
```

Voor de **werkgever** is de vraag: "Als ik deze persoon aanneem, op
welke regelingen krijg ik aanspraak?" Voor de **werknemer**: "Bij een
toekomstige werkgever, welke financiële voordelen kan ik bieden?" De
8 onderliggende regelingen blijven hetzelfde, alleen de presentatie
en de aanvullende brief verschillen.

Use-case in de workshop: eerst de wandeling tonen ("dit is hoe het stelsel zit"), daarna de fan-out ("zo werkt de regelhulp er bovenop, voor beide gebruikersgroepen"). Dat scheidt het juridisch begrip van het uitvoeringsbeleid.
