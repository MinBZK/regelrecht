# Financieel CV — sessieoverzicht

Branch: `feat/financieel_cv_RVO`. Sessieduur: zes uur. Voor de demo op
dinsdag aan de jurist.

## Scope

Acht regelingen uit de regelhulp [Financieel
CV](https://regelhulpenvoorbedrijven.nl/financieelcv/), gericht op
werkgevers/werknemers met afstand tot de arbeidsmarkt. **Eén regeling
grondig, zeven illustratief.**

| Acroniem | Regeling                       | Status   | Wet                                                                                | BWB-ID       | Hoofdartikel    | Peildatum  |
|----------|--------------------------------|----------|------------------------------------------------------------------------------------|--------------|-----------------|------------|
| **NRP**  | **No-riskpolis**               | **full** | Ziektewet                                                                          | BWBR0001888  | 29b lid 1, 2, 4 | 2025-01-01 |
| LIV      | Lage-inkomensvoordeel          | skeleton | Wet tegemoetkomingen loondomein (Wtl)                                              | BWBR0037522  | 3.1             | 2024-01-01 |
| LKV      | Loonkostenvoordeel             | skeleton | Wet tegemoetkomingen loondomein (Wtl)                                              | BWBR0037522  | 2.1             | 2024-01-01 |
| LKS      | Loonkostensubsidie             | skeleton | Participatiewet                                                                    | BWBR0015703  | 10c             | 2025-01-01 |
| LDP      | Loondispensatie                | skeleton | Wet arbeidsongeschiktheidsvoorziening jonggehandicapten (Wajong)                   | BWBR0008657  | 2:20            | 2025-01-01 |
| JC       | Jobcoaching                    | skeleton | Wet werk en inkomen naar arbeidsvermogen (Wet WIA)                                 | BWBR0019057  | 35.1, 35.2.d    | 2025-01-01 |
| WPA      | Werkplekaanpassingen           | skeleton | Wet werk en inkomen naar arbeidsvermogen (Wet WIA)                                 | BWBR0019057  | 35.1, 35.2.c    | 2025-01-01 |
| PP       | Proefplaatsing                 | skeleton | Werkloosheidswet (WW)                                                              | BWBR0004045  | 76a.1           | 2024-01-01 |

### Toelichting op peildata

- **LIV** is per 2025-01-01 afgeschaft voor nieuwe dienstverbanden
  (Wet 36458). Peildatum 2024-01-01 voor Wtl gekozen zodat zowel LIV als
  LKV in beeld blijven voor de Financieel CV-graphdemo.
- **WW** harvest met 2025-01-01 faalde (geen versie beschikbaar op die
  datum); 2024-01-01 gebruikt — artikel 76a is sinds die datum niet
  inhoudelijk gewijzigd.
- **Pwet** 2025-01-01 nieuw geharvest naast bestaande 2022-03-15. Het
  oude bestand bevat machine_readable voor bijstand (art. 7, 8, 11, 18,
  21-24, 43, 69) en is ongemoeid gelaten.

## Hoofduitkomsten

| Regeling | Hoofduitkomsten                                                                |
|----------|--------------------------------------------------------------------------------|
| NRP      | `heeft_recht_op_no_risk_polis`, `duur_no_risk_polis_jaren`, `voldoet_aan_lid_{1,2,4}` |
| LIV      | `heeft_recht_op_liv`, `hoogte_liv_per_jaar` (skeleton: 0)                      |
| LKV      | `heeft_recht_op_lkv`, `categorie_lkv`, `hoogte_lkv_per_jaar` (skeleton: 0)     |
| LKS      | `heeft_recht_op_lks`, `loonwaarde_percentage` (skeleton: 0)                    |
| LDP      | `heeft_recht_op_loondispensatie`                                               |
| JC       | `heeft_recht_op_jobcoaching`                                                   |
| WPA      | `heeft_recht_op_werkplekaanpassing`                                            |
| PP       | `mag_proefplaatsing_aangaan`, `duur_proefplaatsing_weken` (=26)                |

## Cross-law structuur (graph)

De law dependency graph in de editor (zie commit `c7ded67`) leest
`articles[].machine_readable.execution.input[].source.regulation` en
toont een edge per cross-law referentie. De volgende edges ontstaan in
de huidige branch:

```
ziektewet (NRP)
  → wet_werk_en_inkomen_naar_arbeidsvermogen   (lid 1.a, 1.b, 4)
  → wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten   (lid 1.c, 1.d, 2.a, 2.c)
  → participatiewet                            (lid 2.e, 2.f)

wet_tegemoetkomingen_loondomein (LIV, LKV)
  → participatiewet                            (LKV banenafspraak)

participatiewet (LKS)
  → participatiewet                            (intra-law: 10c gebruikt doelgroepstub uit art. 1)

wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten (LDP)
  → wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten (intra-law)

wet_werk_en_inkomen_naar_arbeidsvermogen (JC, WPA)
  → wet_werk_en_inkomen_naar_arbeidsvermogen (intra-law)

werkloosheidswet (PP)   — geen cross-law edge
```

Output-leaf-knopen die de graph rendert (filtert standaard
`wet_naam`, `bevoegd_gezag`, `datum_inwerkingtreding` weg):

- ziektewet: `heeft_recht_op_no_risk_polis`, `duur_no_risk_polis_jaren`,
  `voldoet_aan_lid_1`, `voldoet_aan_lid_2`, `voldoet_aan_lid_4`,
  `is_wia_uitkeringsgerechtigd`, `is_wia_min_35_arbeidsongeschikt`,
  `heeft_voortgezet_wia_recht`, `heeft_arbeidsbeperking_wia`,
  `is_wajong_gerechtigd`, `is_jonggehandicapt_schoolverlater`,
  `is_banenafspraak_doelgroep`, `is_pwet_loonkostensubsidie`,
  `is_beschut_werk`, `loonwaarde_lager_dan_minimumloon`
- wtl: `heeft_recht_op_liv`, `hoogte_liv_per_jaar`,
  `heeft_recht_op_lkv`, `categorie_lkv`, `hoogte_lkv_per_jaar`
- pwet (2025): doelgroepstub-outputs + `heeft_recht_op_lks`,
  `loonwaarde_percentage`
- wajong: `is_wajong_gerechtigd`, `is_jonggehandicapt_schoolverlater`,
  `heeft_recht_op_loondispensatie`
- wet_WIA: `is_wia_uitkeringsgerechtigd` e.a. + `heeft_recht_op_jobcoaching`,
  `heeft_recht_op_werkplekaanpassing`
- ww: `mag_proefplaatsing_aangaan`, `duur_proefplaatsing_weken`

### Overzichtsplaatje (alle 7 wetten samen, artikel-niveau)

De editor-graph rendert steeds vanuit één wortel-wet en walkt
transitief naar dependencies. Voor één totaalbeeld van Financieel CV
op artikel-niveau (vergelijkbaar met de editor-graph maar dan
statisch en compleet):

```mermaid
flowchart LR
  classDef regeling fill:#fff5f5,stroke:#c0392b,color:#000;
  classDef bron fill:#f0f6ff,stroke:#2980b9,color:#000;
  classDef proces fill:#fff9e6,stroke:#b58900,color:#000;

  subgraph ZW["Ziektewet (NRP) — BWBR0001888"]
    ZW_29b["Art. 29b lid 1, 2, 4<br/>━━━━━━━<br/>heeft_recht_op_no_risk_polis<br/>duur_no_risk_polis_jaren<br/>voldoet_aan_lid_1 / lid_2 / lid_4"]
  end

  subgraph WTL["Wtl (LIV + LKV) — BWBR0037522"]
    WTL_31["Art. 3.1 + 3.2 (LIV)<br/>━━━━━━━<br/>heeft_recht_op_liv<br/>hoogte_liv_per_jaar_eurocent<br/>gemiddeld_uurloon_eurocent<br/>voldoet_aan_uurloongrens<br/>voldoet_aan_minimum_verloonde_uren"]
    WTL_21["Art. 2.1 + 2.7/9/13/17 (LKV)<br/>━━━━━━━<br/>heeft_recht_op_lkv<br/>categorie_lkv (4 cat.)<br/>bedrag_per_uur_eurocent<br/>maximum_per_jaar_eurocent<br/>hoogte_lkv_per_jaar_eurocent"]
  end

  subgraph PW["Participatiewet — BWBR0015703"]
    PW_1["Art. 1 (doelgroep-stub)<br/>━━━━━━━<br/>is_banenafspraak_doelgroep<br/>is_pwet_loonkostensubsidie<br/>is_beschut_werk<br/>loonwaarde_lager_dan_minimumloon"]
    PW_10c["Art. 10c + 10d (LKS)<br/>━━━━━━━<br/>heeft_recht_op_lks<br/>bruto_subsidie_eurocent_per_maand<br/>maximum_subsidie_eurocent_per_maand<br/>hoogte_lks_eurocent_per_maand"]
  end

  subgraph WAJ["Wajong — BWBR0008657"]
    WAJ_11["Art. 1:1 (doelgroep-stub)<br/>━━━━━━━<br/>is_wajong_gerechtigd<br/>is_jonggehandicapt_schoolverlater"]
    WAJ_220["Art. 2:20 (LDP)<br/>━━━━━━━<br/>heeft_recht_op_loondispensatie<br/>beding_lagere_beloning_is_nietig"]
  end

  subgraph WIA["Wet WIA — BWBR0019057"]
    WIA_1["Art. 1 (doelgroep-stub)<br/>━━━━━━━<br/>is_wia_uitkeringsgerechtigd<br/>is_wia_min_35_arbeidsongeschikt<br/>heeft_voortgezet_wia_recht<br/>heeft_arbeidsbeperking_wia"]
    WIA_35["Art. 35.1+2 (JC + WPA)<br/>━━━━━━━<br/>heeft_recht_op_jobcoaching<br/>heeft_recht_op_werkplekaanpassing<br/>artikel_35_van_toepassing<br/>voldoet_aan_basisvoorwaarden_lid_1"]
  end

  subgraph WW["Werkloosheidswet (PP) — BWBR0004045"]
    WW_76a["Art. 76a lid 1-5 (PP)<br/>━━━━━━━<br/>mag_proefplaatsing_aangaan<br/>max_duur_proefplaatsing_maanden (=6)<br/>voldoet_aan_lid_3_voorwaarden<br/>ww_uitkering_blijft_bestaan"]
  end

  subgraph AWB["AWB — BWBR0005537 (procedure-hooks)"]
    AWB_346["Art. 3:46<br/>motivering_vereist"]
    AWB_67["Art. 6:7<br/>bezwaartermijn_weken"]
  end

  %% NRP cross-law sources (echte input.source.regulation edges)
  ZW_29b -- "lid 1.a, 1.b, lid 4" --> WIA_1
  ZW_29b -- "lid 1.c, 1.d, 2.a, 2.c" --> WAJ_11
  ZW_29b -- "lid 2.e, 2.f" --> PW_1

  %% AWB hooks fire op elke BESCHIKKING (alle 7 regelingen produceren BESCHIKKING TOEKENNING)
  ZW_29b -. "BESCHIKKING-hook" .-> AWB_346
  ZW_29b -. "BESCHIKKING-hook" .-> AWB_67

  class ZW_29b,WTL_31,WTL_21,PW_10c,WAJ_220,WIA_35,WW_76a regeling
  class WIA_1,WAJ_11,PW_1 bron
  class AWB_346,AWB_67 proces
```

**Legenda:**

- 🟥 *regeling* — uitkomst-artikel uit de Financieel CV-regelhulp
- 🟦 *bron* — doelgroepvaststelling-stub (pass-through naar UWV/college)
- 🟨 *proces* — AWB-hook die automatisch firet op elke BESCHIKKING
- ──── solide pijl — cross-law of intra-law `input.source.regulation`
- ╶╶╶╶ gestippeld — AWB-hook (fires niet via `source` maar via
  `hooks` declarations)

**Wat valt op:**

- NRP (Ziektewet 29b) is het meest verbonden, met drie cross-law uitgangen
  naar WIA, Wajong en Pwet — dat is de doelgroepafbakening uit de wet.
- LIV (Wtl 3.1) staat geheel los: geen cross-law, geen graph-edge —
  dat klopt, het is een puur loon-uurgrenzen-instrument.
- PP (WW 76a) staat los om dezelfde reden — voorwaarden zitten alleen
  in WW zelf.
- AWB-hooks zijn hier zichtbaar gemaakt, maar bestaan voor elke
  BESCHIKKING in het corpus. NRP is BESCHIKKING TOEKENNING dus de hooks
  triggeren tijdens elke uitvoering.

**Wat ontbreekt bewust** (skeleton-status, jurist-input nodig):

- Hoogteberekeningen (LIV, LKV, LKS, NRP-ziekengeld 70% × dagloon).
- Cumulatie- en uitsluitingsregels tussen NRP, LKV, LKS, LDP.
- Wsw-doelgroep (BWBR0008903) — nu als directe parameter
  `is_wsw_werknemer`, niet als cross-law node.

### Detail-zoom: alleen NRP (Ziektewet artikel 29b)

Hetzelfde diagram maar nu volledig uitgepakt voor NRP: elke
afzonderlijke input uit een bron-wet als eigen edge, en de interne
lid-logica zichtbaar.

```mermaid
flowchart LR
  classDef input fill:#f0f6ff,stroke:#2980b9,color:#000;
  classDef param fill:#f5f5f5,stroke:#7f8c8d,color:#000,stroke-dasharray: 3 3;
  classDef gate fill:#fff5f5,stroke:#c0392b,color:#000;
  classDef output fill:#fde2e4,stroke:#a02020,color:#000,font-weight:bold;
  classDef proces fill:#fff9e6,stroke:#b58900,color:#000;

  subgraph WIA["Wet WIA — art. 1 doelgroepstub"]
    WIA_out1["is_wia_uitkeringsgerechtigd<br/>(lid 1.a)"]
    WIA_out2["is_wia_min_35_arbeidsongeschikt<br/>(lid 1.b)"]
    WIA_out3["heeft_voortgezet_wia_recht<br/>(lid 4)"]
  end

  subgraph WAJ["Wajong — art. 1:1 doelgroepstub"]
    WAJ_out1["is_jonggehandicapt_schoolverlater<br/>(lid 1.c, 1.d)"]
    WAJ_out2["is_wajong_gerechtigd<br/>(lid 2.a, 2.c)"]
  end

  subgraph PW["Participatiewet — art. 1 doelgroepstub"]
    PW_out1["is_banenafspraak_doelgroep<br/>(lid 2.e)"]
    PW_out2["is_pwet_loonkostensubsidie<br/>(lid 2.e alt.)"]
    PW_out3["is_beschut_werk<br/>(lid 2.f)"]
  end

  subgraph PARAMS["Parameters bij NRP-aanvraag"]
    P_bsn["bsn"]
    P_wsw["is_wsw_werknemer<br/>(lid 2.b, 2.d)"]
  end

  subgraph ZW["Ziektewet artikel 29b — NRP"]
    direction TB
    LID1["voldoet_aan_lid_1<br/>OR(a, b, c+d)"]
    LID2["voldoet_aan_lid_2<br/>OR(a, b, c, e, f)"]
    LID4["voldoet_aan_lid_4<br/>= heeft_voortgezet_wia_recht"]
    OUT1["heeft_recht_op_no_risk_polis<br/>= OR(lid_1, lid_2, lid_4)"]
    OUT2["duur_no_risk_polis_jaren<br/>IF lid_2 → 0; lid_1 → 5; lid_4 → 5"]

    LID1 --> OUT1
    LID2 --> OUT1
    LID4 --> OUT1
    LID1 --> OUT2
    LID2 --> OUT2
    LID4 --> OUT2
  end

  subgraph AWB_HOOKS["AWB — fires op BESCHIKKING TOEKENNING"]
    AWB1["Art. 3:46 → motivering_vereist"]
    AWB2["Art. 6:7 → bezwaartermijn_weken"]
  end

  %% Lid 1
  WIA_out1 --> LID1
  WIA_out2 --> LID1
  WAJ_out1 --> LID1

  %% Lid 2
  WAJ_out2 --> LID2
  P_wsw --> LID2
  PW_out1 --> LID2
  PW_out2 --> LID2
  PW_out3 --> LID2

  %% Lid 4
  WIA_out3 --> LID4

  %% bsn forward via source.parameters
  P_bsn -.->|"bsn"| WIA
  P_bsn -.->|"bsn"| WAJ
  P_bsn -.->|"bsn"| PW

  %% AWB hooks
  OUT1 -. "BESCHIKKING-hook" .-> AWB1
  OUT1 -. "BESCHIKKING-hook" .-> AWB2

  class WIA_out1,WIA_out2,WIA_out3,WAJ_out1,WAJ_out2,PW_out1,PW_out2,PW_out3 input
  class P_bsn,P_wsw param
  class LID1,LID2,LID4 gate
  class OUT1,OUT2 output
  class AWB1,AWB2 proces
```

**Legenda detail-versie:**

- 🟦 *input* — boolean uit bron-wet via `input.source.regulation`
  (8 stuks: 3 uit WIA, 2 uit Wajong, 3 uit Pwet)
- ⬜ *parameter* — directe input bij de aanvraag, niet via cross-law
  (`bsn` voor BSN, `is_wsw_werknemer` omdat Wsw buiten scope is)
- 🟥 *gate* — lid-niveau OR-poort die de doelgroep-toets doet
- 🟥 *bold output* — finale uitkomst van NRP (recht + duur)
- 🟨 *proces* — AWB-hook die automatisch firet

**Wat de jurist hieruit kan lezen:**

- Lid 1 vraagt drie alternatieven: WIA-uitkering OR <35% AO OR
  jonggehandicapt-schoolverlater. Eén volstaat.
- Lid 2 voegt vier extra doelgroepen toe (Wajong, Wsw, banenafspraak/LKS,
  beschut werk). Pwet `loonwaarde_lager_dan_minimumloon` is nu nog niet
  als input gebruikt — staat klaar voor LKS-uitwerking.
- Lid 4 is een aparte route (voortzetting WIA-recht na vaststelling).
- De duur is conditionneel — lid 2 telt als 0 jaar
  (gemodelleerd, zie untranslatable; in werkelijkheid zolang
  dienstverband voortduurt).
- Recht op NRP = OR van de drie lid-uitkomsten — als één route lukt,
  is het recht er.

### Detail-zoom: alle 7 regelingen na uitwerking — outputs en condities

```mermaid
flowchart LR
  classDef regeling fill:#fff5f5,stroke:#c0392b,color:#000;
  classDef condition fill:#f0f6ff,stroke:#2980b9,color:#000;
  classDef amount fill:#f5e6ff,stroke:#7d3c98,color:#000;
  classDef param fill:#f5f5f5,stroke:#7f8c8d,color:#000;
  classDef untranslated fill:#fff9e6,stroke:#b58900,color:#000;

  subgraph PP["PP — WW art. 76a"]
    PP_p["params: bsn, heeft_recht_op_ww_uitkering,<br/>in_staat_tot_werkzaamheden,<br/>aansprakelijkheidsverzekering_aanwezig,<br/>niet_eerder_proefplaatsing_zelfde_werkgever,<br/>reeel_uitzicht_op_dienstbetrekking_zes_maanden"]
    PP_lid3["voldoet_aan_lid_3_voorwaarden<br/>= AND(a, b, c, d)"]
    PP_out["mag_proefplaatsing_aangaan<br/>max_duur_proefplaatsing_maanden = 6<br/>ww_uitkering_blijft_bestaan"]
    PP_u["⚠ untranslatables:<br/>• onderbreking wegens ziekte<br/>• 'reëel uitzicht' UWV-discretie"]
    PP_p --> PP_lid3 --> PP_out
    PP_u -.- PP_out
  end

  subgraph LIV["LIV — Wtl art. 3.1 + 3.2"]
    LIV_p["params: bsn, jaarloon_eurocent, verloonde_uren,<br/>heeft_pensioengerechtigde_leeftijd_bereikt"]
    LIV_calc["gemiddeld_uurloon = jaarloon / uren<br/>voldoet_aan_uurloongrens (€14,33-14,91)<br/>voldoet_aan_minimum_verloonde_uren (≥1248)"]
    LIV_out["heeft_recht_op_liv<br/>hoogte_liv_per_jaar_eurocent =<br/>MIN(49 × uren, 96000)"]
    LIV_p --> LIV_calc --> LIV_out
  end

  subgraph LKV["LKV — Wtl art. 2.1 + categorieën"]
    LKV_p["params: bsn, verloonde_uren,<br/>4× boolean (oudere, arbeidsgehandicapt,<br/>herplaatsen, banenafspraak),<br/>heeft_loonaangifte_verzoek_ingediend"]
    LKV_cat["categorie_lkv via IF-volgorde:<br/>oudere → arbeidsgehandicapt →<br/>herplaatsen → banenafspraak"]
    LKV_out["heeft_recht_op_lkv<br/>bedrag_per_uur (305 of 101 cent)<br/>maximum_per_jaar (600000 of 200000)<br/>hoogte_lkv_per_jaar_eurocent"]
    LKV_p --> LKV_cat --> LKV_out
  end

  subgraph LKS["LKS — Pwet art. 10c + 10d"]
    LKS_p["params: bsn, behoort_tot_doelgroep_lks,<br/>kan_minimumloon_niet_verdienen,<br/>aanvraag_binnen_zes_maanden,<br/>onderwijsroute_of_doelgroep,<br/>is_wsw_dienstbetrekking,<br/>loonwaarde_eurocent_per_maand,<br/>minimumloon_plus_VB_eurocent"]
    LKS_calc["bruto = WML+VB - loonwaarde+VB<br/>maximum = 70% × WML+VB<br/>hoogte = MIN(bruto, max)"]
    LKS_out["heeft_recht_op_lks<br/>hoogte_lks_eurocent_per_maand"]
    LKS_u["⚠ untranslatables:<br/>• lid 5 (50% eerste 6 mnd)<br/>• lid 4 zin 2 (evenredigheid <36u)<br/>• lid 7 (jaarlijkse herziening)"]
    LKS_p --> LKS_calc --> LKS_out
    LKS_u -.- LKS_out
  end

  subgraph LDP["LDP — Wajong art. 2:20"]
    LDP_p["params: bsn, is_wsw_werknemer,<br/>arbeidsprestatie_duidelijk_minder,<br/>aanvraag_loondispensatie_ingediend,<br/>heeft_recht_op_arbeidsondersteuning_wajong"]
    LDP_out["heeft_recht_op_loondispensatie<br/>beding_lagere_beloning_is_nietig (lid 2)"]
    LDP_u["⚠ untranslatables:<br/>• 'duidelijk minder' UWV-discretie<br/>• 'naar evenredigheid' (% via UWV-beleidsregels)"]
    LDP_p --> LDP_out
    LDP_u -.- LDP_out
  end

  subgraph JCWPA["JC + WPA — Wet WIA art. 35"]
    JCW_p["params: bsn,<br/>heeft_structurele_functionele_beperking,<br/>heeft_arbeidsverhouding_of_voorbereiding,<br/>is_wsw_werknemer,<br/>heeft_recht_op_arbeidsondersteuning_wajong,<br/>pwet_college_draagt_zorg_uitsluiting,<br/>aanvraag_jobcoaching_ingediend,<br/>aanvraag_werkplekaanpassing_ingediend"]
    JCW_gates["artikel_35_van_toepassing (NOT lid 4 a/b)<br/>voldoet_aan_basisvoorwaarden_lid_1<br/>(beperking + arbeid + niet-Wsw)"]
    JCW_out["heeft_recht_op_jobcoaching (lid 2.d)<br/>heeft_recht_op_werkplekaanpassing (lid 2.c)"]
    JCW_u["⚠ untranslatables:<br/>• structurele functionele beperking<br/>• lid 4.b 2-jaars/LKS-toets<br/>• 'in overwegende mate op individu'<br/>• 'noodzakelijk + compensatie'"]
    JCW_p --> JCW_gates --> JCW_out
    JCW_u -.- JCW_out
  end

  subgraph NRP["NRP — Ziektewet art. 29b"]
    NRP_p["params: bsn, is_wsw_werknemer +<br/>8 doelgroep-bools van 3 bron-wetten"]
    NRP_lid["voldoet_aan_lid_1 (a, b, c+d)<br/>voldoet_aan_lid_2 (a-f)<br/>voldoet_aan_lid_4 (voortzetting WIA)"]
    NRP_out["heeft_recht_op_no_risk_polis<br/>duur_no_risk_polis_jaren"]
    NRP_u["⚠ untranslatables:<br/>• vijfjaarstermijn bij onderbreking<br/>• lid 2-duur als 'onbeperkt'<br/>• samenloop met LKV/LKS/LDP"]
    NRP_p --> NRP_lid --> NRP_out
    NRP_u -.- NRP_out
  end

  class PP_p,LIV_p,LKV_p,LKS_p,LDP_p,JCW_p,NRP_p param
  class PP_lid3,LIV_calc,LKV_cat,LKS_calc,JCW_gates,NRP_lid condition
  class PP_out,LIV_out,LKV_out,LKS_out,LDP_out,JCW_out,NRP_out regeling
  class PP_u,LKS_u,LDP_u,JCW_u,NRP_u untranslated
```

**Legenda detail-versie alle 7:**

- ⬜ *param* — directe input bij de aanvraag (parameter)
- 🟦 *condition* — tussenresultaat (lid-niveau gate, voorwaarden-AND, hoogte-formule)
- 🟥 *output* — finale uitkomst van de regeling
- 🟨 *untranslatable* — gemarkeerde semantische gaps voor de jurist
- ──── solide pijl — actieflow
- ╶╶╶╶ gestippeld — verbinding naar untranslatables-annotatie

### Visualisatie tijdens demo

Start de editor lokaal en navigeer naar de law dependency graph view:

```bash
just dev
# editor: http://localhost:3000
```

Vereist `GITHUB_TOKEN` (read:packages-scope) in keychain
(`security add-generic-password -a "$USER" -s github-packages-read -w "$GITHUB_TOKEN"`)
of `.env` voor het ophalen van de private `@minbzk/storybook`-package.

Kies in de editor een Ziektewet-context met peildatum 2025-01-01;
de graph toont vanuit `ziektewet` drie cross-law edges naar wet WIA,
Wajong en Participatiewet. Per skeleton zijn de hoofduitkomsten als
output-leaves zichtbaar onder de bijbehorende wet-knoop.

> Pas de graph-code zelf niet aan tenzij er een duidelijke bug is.
> Aanpassingen aan YAML hebben prioriteit; documenteer eventuele
> bugs als open punt in dit document.

## Untranslatables (Ziektewet 29b)

Drie geaccepteerde untranslatables (engine voert door, jurist moet
beoordelen):

1. **Vijfjaarstermijn bij onderbroken dienstverbanden** (lid 1.b 4°,
   lid 1.c, lid 1.d). Wettekst is niet eenduidig over teleffect bij
   opvolgende dienstverbanden binnen vijf jaar.
2. **Lid 2-duur als 'onbeperkt'**. Voor doelgroepen Wajong, Wsw,
   banenafspraak en beschut werk kent het artikel geen vaste duur. In
   de YAML als 0 gemodelleerd; semantisch verschilt dat van een
   eindige duur.
3. **Samenloop met LKV / LKS / LDP**. Artikel 29b regelt de
   cumulatie niet expliciet.

## Open vragen voor de jurist (dinsdag)

1. NRP-vijfjaarstermijn — geldt die per dienstverband of per persoon?
2. Wajong: oude (BWBR0008657) vs nieuwe Wajong (Wet vereenvoudiging
   Wajong 2021) — voor LDP (artikel 2:20) geldt het oude regime; voor
   NRP-doelgroepbepaling beide?
3. WPA: artikel 35 Wet WIA dekt ook vervoersvoorzieningen,
   intermediaire activiteiten (dovenondersteuning) en overige
   voorzieningen. Wat valt onder "WPA" in de regelhulp Financieel CV?
4. PP-termijn (max 6 maanden, lid 1) — afgerond als 26 weken; correct?
   Of beter als 6 maanden in `months`-eenheid modelleren?
5. Cumulatie van NRP, LKV, LKS en LDP bij dezelfde dienstbetrekking —
   welke gelden tegelijk, en welke sluiten elkaar uit?
6. LIV is afgeschaft per 2025-01-01. Hoort die nog in de regelhulp?
   Voor de demo nu peildatum 2024-01-01 voor Wtl.
7. Doelgroepvaststelling banenafspraak (Pwet 7 lid 1 a) — verzonken
   in UWV-doelgroepregister. Hoe gemodelleerd te krijgen?
8. Wsw is buiten scope gehouden (BWBR0008903 niet geharvest). Wel
   relevant voor NRP lid 2 b/d.

## Bestanden

| Pad                                                                               | Inhoud                                                |
|-----------------------------------------------------------------------------------|-------------------------------------------------------|
| `corpus/regulation/nl/wet/ziektewet/2025-01-01.yaml`                              | NRP (volledig) + ongemoeide artikelen                 |
| `corpus/regulation/nl/wet/wet_tegemoetkomingen_loondomein/2024-01-01.yaml`        | LIV + LKV (skeleton)                                  |
| `corpus/regulation/nl/wet/participatiewet/2025-01-01.yaml`                        | LKS (skeleton) + doelgroepstub voor NRP/LKV           |
| `corpus/regulation/nl/wet/wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten/2025-01-01.yaml` | LDP (skeleton) + doelgroepstub voor NRP |
| `corpus/regulation/nl/wet/wet_werk_en_inkomen_naar_arbeidsvermogen/2025-01-01.yaml` | JC + WPA (skeleton) + doelgroepstub voor NRP        |
| `corpus/regulation/nl/wet/werkloosheidswet/2024-01-01.yaml`                       | PP (skeleton)                                         |
| `features/no_risk_polis.feature`                                                  | Drie BDD-scenarios voor NRP                           |
| `docs/financieel-cv/persona-traces/`                                              | Twee traces (WIA-uitkering, banenafspraak)            |
| `PLAN.md`                                                                         | Sessieplan + tijdsbudget                              |

## Quality checks

- `just format` — groen
- `just lint` — groen
- `just validate` — groen (alle YAMLs schema v0.5.1)
- `just test` — groen
- `just bdd` — 56/56 scenarios (351/351 steps)

`just check` faalt op `admin-test` omdat dat Docker (testcontainers) vereist;
geen regressie van deze sessie.

## Niet-gedaan

- **Editor-screenshot** van de graph view: vereist `GITHUB_TOKEN` met
  `read:packages` scope voor de private `@minbzk/storybook`-package
  (zie `frontend/package.json`). De gebruiker draait `just dev` zelf
  voor de demo; deze README beschrijft wat de graph behoort te tonen.
- **Wet sociale werkvoorziening** (BWBR0008903) niet geharvest;
  Wsw-doelgroep (NRP lid 2 b/d) afgehandeld als parameter
  `is_wsw_werknemer` op artikel-niveau in zowel Ziektewet als Wajong.
- **Volledige uitwerking** van de zeven illustratieve regelingen.
  Skeleton-status is per ontwerp.
