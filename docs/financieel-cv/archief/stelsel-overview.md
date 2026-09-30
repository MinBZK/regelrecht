# Financieel CV — stelsel en grondslagen

Voor de kick-off met het regelhulp Financieel CV-team. In de stijl van
het HHNK-kwijtschelding-stelsel (workshop 2026-04-23): per Financieel
CV-regeling laten zien op welk **regulatory_layer-niveau** de
inhoudelijke regels staan, en welke organisatie de uitvoering doet.

```mermaid
flowchart TB
  classDef wet fill:#fff5e0,stroke:#c0392b,stroke-width:2px,color:#000;
  classDef amvb fill:#ffe6cc,stroke:#d35400,color:#000;
  classDef mr fill:#fce4ec,stroke:#c2185b,color:#000;
  classDef beleid fill:#f3e5f5,stroke:#7b1fa2,color:#000;
  classDef uitvoer fill:#e3f2fd,stroke:#1565c0,color:#000;
  classDef proces fill:#e0f2f1,stroke:#00695c,color:#000;

  subgraph WET["WET (Rijk — formele wet)"]
    direction LR
    ZW["Ziektewet<br/>art. 29b<br/>BWBR0001888"]:::wet
    WIA["Wet WIA<br/>art. 35 + 36<br/>BWBR0019057"]:::wet
    WAJ["Wajong<br/>art. 2:20<br/>BWBR0008657"]:::wet
    WTL["Wtl<br/>art. 2 + 3<br/>BWBR0037522"]:::wet
    PWET["Participatiewet<br/>art. 10c + 10d<br/>BWBR0015703"]:::wet
    WW["WW<br/>art. 76a<br/>BWBR0004045"]:::wet
    WFSV["Wfsv<br/>art. 38b<br/>doelgroepregister banenafspraak<br/>BWBR0017745"]:::wet
  end

  subgraph LAGERE["Lagere regelgeving (Rijk)"]
    direction LR
    AMVB_REI["AMVB<br/>Reïntegratiebesluit<br/>BWBR0018394<br/><i>onder Wet WIA art. 35 lid 5</i>"]:::amvb
    MR_LIV["MIN. REGELING<br/>Regeling LIV<br/><i>uurloongrenzen jaarlijks</i><br/><i>onder Wtl art. 3.1 lid 4</i>"]:::mr
    MR_WGL["MIN. REGELING<br/>Regeling werkgevers-<br/>lastenvergoeding<br/><i>onder Pwet art. 10d lid 4</i>"]:::mr
    BR_LDP["BELEIDSREGEL<br/>UWV-Beleidsregels<br/>Loondispensatie<br/><i>onder Wajong art. 2:20</i>"]:::beleid
    AMVB_38B["AMVB<br/>indicatie doelgroepregister<br/><i>onder Wfsv 38b.1.d + 38b.3</i><br/><i>(nog niet geharvest)</i>"]:::amvb
  end

  subgraph PROCES["Procesrecht (overkoepelend)"]
    AWB["Algemene Wet Bestuursrecht<br/>art. 3:46 + 6:7<br/>BWBR0005537"]:::proces
  end

  subgraph UITVOERING["Uitvoeringsorganisaties"]
    direction LR
    UWV["UWV<br/>━━━━━━━<br/>NRP, JC, WPA<br/>(art. 35 / 29b)<br/>LDP-percentage<br/>PP-toestemming"]:::uitvoer
    BD["Belastingdienst<br/>━━━━━━━<br/>LIV, LKV<br/>(loonaangifte<br/>aggregatie)"]:::uitvoer
    GEM["Gemeente<br/>(college b&w)<br/>━━━━━━━<br/>LKS<br/>(art. 10c, 10d)<br/>loonwaarde-<br/>vaststelling"]:::uitvoer
    RVO["RVO<br/>━━━━━━━<br/>regelhulp<br/>Financieel CV<br/>(orchestrator)"]:::uitvoer
  end

  %% AMvB / MR / BR onder WET
  WIA -- "lid 5 AMvB-grondslag" --> AMVB_REI
  WTL -- "art. 3.1 lid 4 jaarlijkse aanpassing" --> MR_LIV
  PWET -- "art. 10d lid 4 vergoeding werkgeverslasten" --> MR_WGL
  WAJ -- "art. 2:20 lid 1 'naar evenredigheid'" --> BR_LDP
  WFSV -- "38b.1.d + 38b.3 AMvB-grondslag" --> AMVB_38B

  %% Wfsv 38b is de GEDEELDE GRONDSLAG: eenmaal gemodelleerd, cross-law
  %% aangeroepen. NRP en LKV hangen hun banenafspraak-status hieraan op.
  WFSV == "doelgroepregister &rarr; lid 2.e" ==> ZW
  WFSV == "doelgroepregister &rarr; categorie c" ==> WTL

  %% Uitvoering consumeert WET + lagere regelgeving
  ZW --> UWV
  WIA --> UWV
  AMVB_REI --> UWV
  WAJ --> UWV
  BR_LDP --> UWV
  WW --> UWV
  WFSV --> UWV

  WTL --> BD
  MR_LIV --> BD

  PWET --> GEM
  MR_WGL --> GEM

  UWV -. "outputs" .-> RVO
  BD  -. "outputs" .-> RVO
  GEM -. "outputs" .-> RVO

  %% AWB hooks fire op elke BESCHIKKING (alle 7 regelingen produceren BESCHIKKING TOEKENNING)
  AWB -. "hook BESCHIKKING" .-> UWV
  AWB -. "hook BESCHIKKING" .-> BD
  AWB -. "hook BESCHIKKING" .-> GEM
```

## Lezing van het diagram

**Verticaal stelsel (Nederlandse rechtsleer):**

1. **Wet** — door de Staten-Generaal vastgestelde formele wet. Hier
   staan de inhoudelijke aanspraken (recht op NRP, hoogte LKS-formule,
   etc.). Rood-omrand.

2. **AMvB / Min. regeling / Beleidsregel** — gedelegeerde regelgeving
   die door de wet wordt opgeroepen via `open_term`-clausules ("bij
   ministeriële regeling", "bij of krachtens algemene maatregel van
   bestuur", "naar evenredigheid"). In onze YAML's gemodelleerd als
   `open_terms` met `delegation_type` en `legal_basis`.

3. **Uitvoeringsorganisaties** — de partijen die de regeling
   feitelijk toepassen op individuele cases (UWV, Belastingdienst,
   gemeente). Geen wetgever — wel beslissingsdragers. Output =
   beschikking.

4. **Regelhulp / orchestrator** — RVO bouwt geen nieuwe juridische
   regels, maar bevraagt de 8 onderliggende regelingen voor één
   werkgever-werknemer-scenario. Modelleerbaar als
   `regulatory_layer: UITVOERINGSBELEID`.

5. **Procesrecht (AWB)** — niet een aparte regeling maar een
   procedurele schil rond elke beschikking. In onze YAML's via
   `hooks` op `legal_character: BESCHIKKING`.

## Welke regeling op welk niveau

| Regeling | Inhoud zit in | Lagere regelgeving | Wie voert uit |
|----------|---------------|---------------------|---------------|
| **NRP**  | Ziektewet 29b lid 1, 2, 4 | (geen — directe wet)  | UWV |
| **PP**   | WW 76a lid 1-4            | Lid 5 — open MR (geen actuele) | UWV |
| **LIV**  | Wtl 3.1 + 3.2             | Regeling LIV (uurloongrenzen) | Belastingdienst |
| **LKV**  | Wtl 2.1 + categorieën     | (bedragen direct in wet)        | Belastingdienst |
| **LKS**  | Pwet 10c + 10d            | Min. regeling werkgeverslasten (10d.4) | Gemeente |
| **LDP**  | Wajong 2:20 lid 1 + 2     | UWV-Beleidsregels loondispensatie | UWV |
| **JC**   | Wet WIA 35 lid 1, 2.d, 4  | Reïntegratiebesluit (AMvB) | UWV |
| **WPA**  | Wet WIA 35 lid 1, 2.c, 4 + 36 | Reïntegratiebesluit (AMvB) | UWV |
| _(grondslag)_ **Doelgroepregister banenafspraak** | Wfsv 38b lid 1 a–f, 2, 6 | AMvB-indicatie 38b.1.d + 38b.3 (nog niet geharvest) | UWV |

### De banenafspraak zit niet in een eigen wet

Het **doelgroepregister banenafspraak** is bij ons niet als losse "Wet
banenafspraak" gemodelleerd, maar op de plek waar de wet het regelt: **Wfsv
artikel 38b** (BWBR0017745). Eén keer gemodelleerd, door de andere regelingen
**cross-law aangeroepen** — de **NRP** (Ziektewet 29b lid 2.e) en de **LKV**
(Wtl, categorie banenafspraak) hangen hun banenafspraak-status hieraan op.

Wat 38b in onze YAML vangt:

- **Opname-gronden 38b.1 a–f** (Pwet-LKS-toeleiding, WSW-indicatie,
  Wajong-arbeidsondersteuning, AMvB-indicatie, eigen-verzoek-WML-route), plus
  **38b.2** (UWV-oordeel jonggehandicapt) en **38b.6** (blijf-grond).
- Kern-outputs: `behoort_tot_doelgroepregister_banenafspraak`,
  `datum_opname_doelgroepregister` + `grond_opname` (relevant voor de
  LKV-driejaarstermijn), en `vaststelling_door` (UWV).
- Een expliciete uitsluiting: **beschut werk (Pwet 10b) hoort níét in het
  register** — dat is een apart traject, geen arbeidsbeperkte in de zin van 38b.

## Open delegaties (`open_term`-blokken in YAML)

Per regeling welke gedelegeerde regelgeving door onze
machine_readable als `open_term` is opgeroepen, en wat de wet daar
zegt:

- **PP — `nadere_regels_uitvoering_proefplaatsing`** (WW 76a lid 5):
  "Bij ministeriële regeling kunnen nadere regels worden gesteld
  omtrent de uitvoering van het eerste tot en met vierde lid." Geen
  actuele regeling bekend — placeholder in YAML.
- **LIV — `liv_uurloongrenzen_per_jaar`** (Wtl 3.1 lid 4): "Bij het
  begin van het kalenderjaar worden de bedragen ... bij regeling van
  Onze Minister ... gewijzigd." Hardgecodeerd voor 2024 als literals
  (1433, 1491, 49, 96000), met open_term-placeholder voor toekomstige
  jaaroverzichten.
- **LKS — `werkgeverslastenvergoeding_eurocent`** (Pwet 10d lid 4):
  "vermeerderd met een bij ministeriële regeling vastgestelde
  vergoeding voor werkgeverslasten". Niet uitgewerkt in deze YAML.
- **LDP — `dispensatiepercentage`** (Wajong 2:20 lid 1, BR-niveau):
  "vermindert het UWV ... naar evenredigheid". Het percentage zelf is
  uitvoeringsbeleid van UWV, niet uit de wet af te leiden.
- **JC + WPA — `nadere_regels_voorzieningen_artikel_35`** (Wet WIA
  art. 35 lid 5): "Bij of krachtens algemene maatregel van bestuur
  kunnen nadere regels worden gesteld." Verwijst naar
  Reïntegratiebesluit (BWBR0018394), nog niet als
  `implements`-relatie geharvest.

## Wat dit diagram laat zien voor de kick-off

1. **Het stelsel is verstrengeld over zes wetten + AWB.** Dat is de
   reden dat één regelhulp (RVO) bestaansrecht heeft: anders moet de
   werkgever bij elke uitvoeringsorganisatie apart aankloppen.
2. **De delegatie loopt via vier regulatory_layers.** Onze YAML's
   modelleren dat correct via `open_term` (top-down) en — waar
   gewenst — `implements` (bottom-up).
3. **Drie verschillende uitvoeringsorganisaties** dekken de 8
   regelingen. Voor de regelhulp betekent dat: drie verschillende
   sleutels, drie verschillende formats, drie verschillende
   beslistermijnen.
4. **AWB is universeel.** Elke beschikking erft motiveringsplicht +
   bezwaartermijn. Voor de regelhulp betekent dat: één
   bezwaarsjabloon volstaat in eerste instantie, AWB-hooks in onze
   YAML's vangen dit automatisch op.
