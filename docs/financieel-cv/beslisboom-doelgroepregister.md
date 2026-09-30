# Beslisboom: van persoon naar grond, recht en bedrag

Drie bomen. De eerste bepaalt of iemand in het doelgroepregister banenafspraak
komt en op welke grond. De tweede leidt van die grond naar de rechten. De derde
rekent die rechten om naar een bedrag.

Onderbouwing per stap staat in
[`doelgroepregister-categorieen.md`](doelgroepregister-categorieen.md).
Peildatums: Wfsv 2026-07-01, Wtl 2026-01-01, overige wetten 2026-07-01.

De volgorde van boom 1 is de IF-volgorde die het model hanteert in
`grond_opname_doelgroepregister`: a, b, c, d, e, f, lid 2, en pas daarna de
blijfgrond van lid 6. De eerst passende grond wint, ook als meer gronden van
toepassing zijn.

## Boom 1: Opname in het doelgroepregister banenafspraak

Bron ook als los bestand: [`beslisboom-doelgroepregister.mmd`](beslisboom-doelgroepregister.mmd).

```mermaid
flowchart TD
  classDef vraag fill:#e3f2fd,stroke:#1565c0,stroke-width:2px,color:#000;
  classDef opname fill:#c8e6c9,stroke:#388e3c,stroke-width:2px,color:#000;
  classDef geen fill:#ffcdd2,stroke:#c62828,stroke-width:2px,color:#000;
  classDef gat fill:#fafafa,stroke:#9e9e9e,stroke-width:2px,stroke-dasharray: 6 4,color:#616161;

  START(["Persoon met een arbeidsbeperking"])
  CH{"<b>Chapeau lid 1 en lid 6</b><br/>Heeft het college vastgesteld dat betrokkene<br/>uitsluitend in een beschutte omgeving<br/>mogelijkheden heeft?<br/><i>Pwet 10b lid 1</i>"}:::vraag
  UIT["<b>Geen opname</b><br/>Beschut werk sluit het register uit.<br/>Wel no-riskpolis via ZW 29b lid 2 f"]:::geen

  A{"<b>a</b><br/>Pwet-toeleiding of LKS op grond van 10d lid 2,<br/>plus UWV-vaststelling geen WML,<br/>of collegeloonwaarde onder WML"}:::vraag
  OA["<b>Opname, grond a</b><br/>pwet_lks_uwv_loonwaarde"]:::opname

  B{"<b>b</b><br/>Wsw-indicatie, of een nog geldende<br/>indicatiebeschikking Wsw artikel 11<br/>zoals dat luidde op 31-12-2014"}:::vraag
  OB["<b>Opname, grond b</b><br/>wsw"]:::opname

  C{"<b>c</b><br/>Recht op Wajong-arbeidsondersteuning<br/>of Wajong-uitkering"}:::vraag
  C2{"Duurzaam geen mogelijkheden<br/>tot arbeidsparticipatie?"}:::vraag
  C3{"Verricht betrokkene arbeid<br/>in dienstbetrekking?"}:::vraag
  OC["<b>Opname, grond c</b><br/>wajong"]:::opname

  D{"<b>d</b><br/>Voldoet aan een bij of krachtens AMvB<br/>vastgestelde indicatie"}:::vraag
  OD["<b>Opname, grond d</b><br/>amvb_38b_1_d"]:::opname

  E{"<b>e</b><br/>Pwet-toeleiding, plus UWV-vaststelling<br/>geen WML op eigen verzoek"}:::vraag
  OE["<b>Opname, grond e</b><br/>pwet_uwv_wml_eigen_verzoek"]:::opname

  F{"<b>f</b><br/>Was op of na 1-1-2013 een persoon<br/>onder b of c, en op 1-5-2015 niet meer"}:::vraag
  F2{"Uitzondering: viel onder c en heeft<br/>inmiddels duurzaam geen mogelijkheden?"}:::vraag
  OF["<b>Opname, grond f</b><br/>overgangsrecht_38b_1_f"]:::opname

  G{"<b>g</b><br/>Recht op IVA-uitkering, waarbij bij wijze<br/>van experiment loondispensatie is ingezet<br/><i>Wet SUWI artikel 82a lid 1</i>"}:::gat
  OG["<b>Opname, grond g</b><br/>ontbreekt in het model"]:::gat

  L2{"<b>lid 2</b><br/>UWV-oordeel: belemmering door ziekte of gebrek<br/>ontstaan voor het 18e jaar of tijdens de studie,<br/>en zonder voorziening geen WML"}:::vraag
  OL2["<b>Opname, grond lid 2</b><br/>jonggehandicapt_uwv_oordeel"]:::opname

  L6{"<b>lid 6 blijfgrond</b><br/>Voldeed eerder aan lid 1 of lid 2,<br/>en de opname in de registratie<br/>is nog niet geeindigd"}:::vraag
  OL6["<b>Opname, blijfgrond lid 6</b><br/>de eerdere grond blijft gelden"]:::opname

  NOP["<b>Geen opname</b><br/>grond_opname_doelgroepregister = geen"]:::geen

  START --> CH
  CH -- ja --> UIT
  CH -- nee --> A
  A -- ja --> OA
  A -- nee --> B
  B -- ja --> OB
  B -- nee --> C
  C -- nee --> D
  C -- ja --> C2
  C2 -- nee --> OC
  C2 -- ja --> C3
  C3 -- ja --> OC
  C3 -- nee --> D
  D -- ja --> OD
  D -- nee --> E
  E -- ja --> OE
  E -- nee --> F
  F -- nee --> G
  F -- ja --> F2
  F2 -- nee --> OF
  F2 -- ja --> G
  G -- ja --> OG
  G -- nee --> L2
  L2 -- ja --> OL2
  L2 -- nee --> L6
  L6 -- ja --> OL6
  L6 -- nee --> NOP
```

### Dezelfde boom als tabel

| Stap | Vraag | Ja | Nee |
|---|---|---|---|
| 0 | Chapeau: uitsluitend beschut werk vastgesteld door het college (Pwet 10b lid 1)? | Geen opname; einde | Naar stap a |
| a | Pwet-toeleiding of LKS 10d lid 2, plus UWV-vaststelling geen WML, of collegeloonwaarde onder WML? | Opname, grond a | Naar b |
| b | Wsw-indicatie of geldige oude indicatiebeschikking? | Opname, grond b | Naar c |
| c | Recht op Wajong-arbeidsondersteuning of -uitkering? | Naar c1 | Naar d |
| c1 | Duurzaam geen mogelijkheden tot arbeidsparticipatie? | Naar c2 | Opname, grond c |
| c2 | Verricht betrokkene arbeid in dienstbetrekking? | Opname, grond c | Naar d |
| d | Voldoet aan de AMvB-indicatie van 38b lid 1 d? | Opname, grond d | Naar e |
| e | Pwet-toeleiding plus UWV-vaststelling geen WML op eigen verzoek? | Opname, grond e | Naar f |
| f | Op of na 1-1-2013 persoon onder b of c, en op 1-5-2015 niet meer? | Naar f1 | Naar g |
| f1 | Viel onder c en heeft inmiddels duurzaam geen mogelijkheden? | Naar g | Opname, grond f |
| g | IVA-recht met experimentele loondispensatie (SUWI 82a)? | Opname, grond g — **ontbreekt in het model** | Naar lid 2 |
| lid 2 | UWV-oordeel jonggehandicapt met voorziening? | Opname, grond lid 2 | Naar lid 6 |
| lid 6 | Voldeed eerder aan lid 1 of 2, en registratie nog niet geëindigd? | Opname, blijfgrond | Geen opname |

## Boom 2: Van grond naar recht

De registratie zelf opent twee deuren. De overige instrumenten hangen aan de
grond eronder.

```mermaid
flowchart LR
  classDef grond fill:#e3f2fd,stroke:#1565c0,stroke-width:2px,color:#000;
  classDef reg fill:#c8e6c9,stroke:#388e3c,stroke-width:3px,color:#000;
  classDef recht fill:#ffe5e5,stroke:#c0392b,stroke-width:2px,color:#000;
  classDef uitsl fill:#ffcdd2,stroke:#c62828,stroke-width:2px,color:#000;

  GA["grond a of e<br/><i>Pwet-route</i>"]:::grond
  GB["grond b<br/><i>Wsw</i>"]:::grond
  GC["grond c<br/><i>Wajong</i>"]:::grond
  GD["grond d<br/><i>AMvB</i>"]:::grond
  GF["grond f<br/><i>overgangsrecht</i>"]:::grond
  G2["grond lid 2<br/><i>UWV-oordeel</i>"]:::grond
  BW["beschut werk<br/><i>Pwet 10b</i>"]:::uitsl
  WIAST["WIA-recht of<br/>minder dan 35 procent AO"]:::grond

  REG(["<b>Doelgroepregister banenafspraak</b><br/>Wfsv 38b"]):::reg

  LKV["<b>LKV doelgroep banenafspraak</b><br/>Wtl 2.10, 2.12, 2.13<br/><i>werkgever</i>"]:::recht
  NRP["<b>No-riskpolis</b><br/>ZW 29b<br/><i>werkgever</i>"]:::recht
  LKS["<b>Loonkostensubsidie</b><br/>Pwet 10c, 10d<br/><i>werkgever</i>"]:::recht
  BEG["<b>Begeleiding op de werkplek</b><br/>Pwet 10da<br/><i>werknemer</i>"]:::recht
  LDP["<b>Loondispensatie</b><br/>Wajong 2:20<br/><i>werkgever</i>"]:::recht
  JCW["<b>Jobcoaching en werkplekaanpassing</b><br/>WIA 35, Wajong 2:22, Pwet 10<br/><i>werknemer</i>"]:::recht
  PP["<b>Proefplaatsing</b><br/>WW 76a, WIA 37, Wajong 2:24, Pwet 8a<br/><i>werkgever</i>"]:::recht

  GA --> REG
  GB --> REG
  GC --> REG
  GD --> REG
  GF --> REG
  G2 --> REG
  BW -. "sluit uit" .-x REG

  REG == "2.10 lid 1" ==> LKV
  REG == "29b lid 2 e" ==> NRP

  GA -- "college stelt doelgroep vast" --> LKS
  GA --> BEG
  GA -- "8a lid 2 d" --> PP
  GA --> JCW
  GB -- "29b lid 2 b en d" --> NRP
  GC -- "2:20" --> LDP
  GC -- "29b lid 2 a" --> NRP
  GC -- "2:22 en 2:24" --> JCW
  GC --> PP
  BW -- "29b lid 2 f" --> NRP
  WIAST -- "29b lid 1 a en b" --> NRP
  WIAST -- "35 en 37" --> JCW
  WIAST --> PP
```

### Wat de registerstatus alleen oplevert

| Recht | Toetst op de registratie | Toetst op de grond eronder |
|---|---|---|
| LKV doelgroep banenafspraak | ja, Wtl 2.10 lid 1 | nee |
| No-riskpolis | ja, ZW 29b lid 2 e | ook, via lid 1 en lid 2 a, b, d en f |
| Loonkostensubsidie | nee | ja, Pwet 10c en 10d |
| Begeleiding op de werkplek | nee | ja, Pwet 10da |
| Loondispensatie | nee | ja, Wajong 2:20 |
| Jobcoaching en werkplekaanpassing | nee | ja, WIA 35, Wajong 2:22, Pwet 10 |
| Proefplaatsing | nee | ja, WW 76a, WIA 37, Wajong 2:24, Pwet 8a |

Een regelhulp die uitsluitend "u staat in het doelgroepregister" weet, kan
daarom twee instrumenten bepalen en vijf niet.

## Boom 3: Van recht naar bedrag

```mermaid
flowchart TD
  classDef stap fill:#e3f2fd,stroke:#1565c0,stroke-width:2px,color:#000;
  classDef bedrag fill:#c8e6c9,stroke:#388e3c,stroke-width:2px,color:#000;
  classDef gat fill:#fafafa,stroke:#9e9e9e,stroke-width:2px,stroke-dasharray: 6 4,color:#616161;

  subgraph LKVB["Loonkostenvoordeel, per kalenderjaar"]
    direction TB
    L1["Werkgever heeft verzoek gedaan<br/>in de loonaangifte<br/><i>Wtl 2.1</i>"]:::stap
    L2["Bereken per categorie waar recht op bestaat"]:::stap
    LA["banenafspraak<br/>1,01 euro per verloond uur<br/>maximaal 2.000 euro<br/><i>Wtl 2.13</i>"]:::stap
    LB["arbeidsgehandicapte<br/>3,05 euro per verloond uur<br/>maximaal 6.000 euro<br/>ten hoogste 3 jaar<br/><i>Wtl 2.9 en 2.8</i>"]:::stap
    LC["herplaatsen<br/>3,05 euro per verloond uur<br/>maximaal 6.000 euro<br/>ten hoogste 1 jaar<br/><i>Wtl 2.17 en 2.16</i>"]:::stap
    L3["Anticumulatie: neem het hoogste bedrag.<br/>Bij gelijke hoogte de eerstgenoemde in de wet<br/><i>Wtl 4.1 lid 3</i>"]:::stap
    L4["hoogte_lkv_per_jaar_eurocent"]:::bedrag
    L1 --> L2 --> LA --> L3
    L2 --> LB --> L3
    L2 --> LC --> L3
    L3 --> L4
  end

  subgraph LKSB["Loonkostensubsidie, per maand"]
    direction TB
    K1["WML plus vakantiebijslag<br/><i>parameter, WML niet gemodelleerd</i>"]:::gat
    K2["min de vastgestelde loonwaarde<br/>plus vakantiebijslag<br/><i>parameter, methode niet gemodelleerd</i>"]:::gat
    K3["bruto_subsidie_eurocent_per_maand"]:::stap
    K4["Cap op 70 procent van<br/>WML plus vakantiebijslag<br/><i>Pwet 10d lid 4</i>"]:::stap
    K5["Normbasis 36 uur per week"]:::stap
    K6["Naar evenredigheid van de<br/>overeengekomen arbeidsduur"]:::stap
    K7["hoogte_lks_eurocent_per_maand<br/><i>exclusief werkgeverslastenvergoeding</i>"]:::bedrag
    K1 --> K3
    K2 --> K3
    K3 --> K4 --> K5 --> K6 --> K7
  end

  subgraph NRPB["Ziekengeld no-riskpolis, per dag"]
    direction TB
    N1["Dagloon<br/><i>Dagloonbesluit niet gemodelleerd</i>"]:::gat
    N2["70 procent van het dagloon<br/><i>ZW 29b lid 5</i>"]:::stap
    N3["Eerste 52 weken, op verzoek van de werkgever:<br/>100 procent van het dagloon, ten hoogste het loon<br/>dat de werkgever verschuldigd zou zijn<br/><i>ZW 29b lid 6</i>"]:::stap
    N4["Bij een Wsw artikel 7-overeenkomst:<br/>dagloon min het naar werkdagen herleide<br/>subsidiebedrag<br/><i>ZW 29b lid 7</i>"]:::stap
    N5["Ziekengeld<br/><i>ontbreekt als output</i>"]:::gat
    N1 --> N2 --> N3 --> N4 --> N5
  end

  subgraph LDPB["Loondispensatie"]
    direction TB
    P1["UWV vermindert de beloningsaanspraak<br/>naar evenredigheid van de arbeidsprestatie<br/><i>Wajong 2:20 lid 1</i>"]:::stap
    P2["Een lager overeengekomen beding is nietig<br/><i>lid 2</i>"]:::stap
    P3["Bedrag<br/><i>ontbreekt als output</i>"]:::gat
    P1 --> P2 --> P3
  end
```

### Waar de boom vastloopt

| Knooppunt | Ontbreekt | Gevolg |
|---|---|---|
| WML plus vakantiebijslag | Wet minimumloon met artikel 15 en de indexerings-AMvB | De hele LKS-berekening rust op een handmatig ingevoerd bedrag |
| Loonwaarde | Besluit loonkostensubsidie Participatiewet | De methode achter het getal is onbekend |
| Dagloon | Dagloonbesluit werknemersverzekeringen | Ziekengeld, WIA- en WW-uitkering leveren geen bedrag |
| Evenredige vermindering | Besluit loondispensatie Wajong | Loondispensatie levert een recht en geen bedrag |

Alle vier staan als tekst in het corpus en hebben geen `machine_readable`-blok.

## Gebruik in de juristsessie

Boom 1 is de toets waar de jurist het over gaat hebben: onderdeel g ontbreekt,
de AMvB onder onderdeel d bestaat in het model niet, en de volgorde a tot en
met lid 2 bepaalt welke grond in de beschikking komt te staan wanneer meer
gronden van toepassing zijn. Die volgorde volgt uit de modellering en staat
niet als zodanig in de wet.

Boom 2 is de toets op de verwachting van de gebruiker: wie in het register
staat, heeft daarmee nog geen loonkostensubsidie, geen loondispensatie en geen
jobcoach.

Boom 3 is de toets op wat de regelhulp vandaag kan tonen: twee van de zeven
bedragen zijn te berekenen, vier knooppunten ontbreken.
