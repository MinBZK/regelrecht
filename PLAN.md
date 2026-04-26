# Financieel CV - Plan voor 6-uur sessie

**Branch**: `feat/financieel_cv_RVO`
**Schema**: v0.5.2 (zie `schema/v0.5.2/schema.json`)
**Stijlreferentie**: `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`

## Scope

Acht regelingen uit de regelhulp Financieel CV
(<https://regelhulpenvoorbedrijven.nl/financieelcv/>):

1. **NRP** - No-riskpolis (grondig)
2. LIV, LKV, LKS, LDP, JC, WPA, PP (skeletons)

## BWB-IDs en peildata

### Regelingen-wetten

| Acroniem | Wet                                                          | BWB-ID       | Hoofdartikel  | Peildatum  | Hoofduitkomst                                  |
|----------|--------------------------------------------------------------|--------------|---------------|------------|------------------------------------------------|
| NRP      | Ziektewet                                                    | BWBR0001888  | 29b           | 2025-01-01 | `heeft_recht_op_no_risk_polis`, `duur_no_risk_polis_jaren` |
| LIV      | Wet financiering sociale verzekeringen (Wfsv)                | BWBR0017745  | 3.1 (hfdst 3) | 2025-01-01 | `heeft_recht_op_liv`, `hoogte_liv_per_jaar`    |
| LKV      | Wet tegemoetkomingen loondomein (Wtl)                        | BWBR0036953  | 2.1 (hfdst 2) | 2025-01-01 | `heeft_recht_op_lkv`, `categorie_lkv`, `hoogte_lkv_per_jaar` |
| LKS      | Participatiewet                                              | BWBR0015703  | 10c, 10d      | 2025-01-01 | `heeft_recht_op_lks`, `loonwaarde_percentage`  |
| LDP      | Wet werk en arbeidsondersteuning jonggehandicapten (Wajong)  | BWBR0008657  | 2:20          | 2025-01-01 | `heeft_recht_op_loondispensatie`               |
| JC       | Wet werk en inkomen naar arbeidsvermogen (Wet WIA)           | BWBR0019057  | 35            | 2025-01-01 | `heeft_recht_op_jobcoaching`                   |
| WPA      | Wet werk en inkomen naar arbeidsvermogen (Wet WIA)           | BWBR0019057  | 35-36         | 2025-01-01 | `heeft_recht_op_werkplekaanpassing`            |
| PP       | Werkloosheidswet (WW)                                        | BWBR0004045  | 76e           | 2025-01-01 | `mag_proefplaatsing_aangaan`, `duur_proefplaatsing_weken` |

### Bron-wetten (gedeeld, voor doelgroepafbakening)

| Wet                                                          | BWB-ID       | Reden opnemen                                               |
|--------------------------------------------------------------|--------------|-------------------------------------------------------------|
| Wet werk en inkomen naar arbeidsvermogen (Wet WIA)           | BWBR0019057  | Doelgroepbepaling (WIA-gerechtigde) voor NRP, JC, WPA       |
| Wet werk en arbeidsondersteuning jonggehandicapten (Wajong)  | BWBR0008657  | Doelgroepbepaling jonggehandicapten voor NRP, LDP           |
| Participatiewet                                              | BWBR0015703  | Doelgroep banenafspraak voor NRP, LKS                       |
| Wet financiering sociale verzekeringen (Wfsv)                | BWBR0017745  | Grondslag LIV (al opgenomen via LIV)                        |

NB. Wet WIA wordt al gebruikt voor zowel JC/WPA als bron voor NRP — twee
hoedanigheden in één YAML.

NB. Wajong (BWBR0008657) komt zowel voor als bron-wet (NRP) als regeling
(LDP). LDP-artikel 2:20 vereist een eigen `machine_readable`; doelgroep-stub
in art. 1a.

## Outputs per regeling - graph-knopen

Compacte aliaslijst (boolean tenzij anders aangegeven):

- LIV: `heeft_recht_op_liv`, `hoogte_liv_per_jaar` (amount, eurocent)
- LKV: `heeft_recht_op_lkv`, `categorie_lkv` (string), `hoogte_lkv_per_jaar` (amount)
- LKS: `heeft_recht_op_lks`, `loonwaarde_percentage` (number)
- LDP: `heeft_recht_op_loondispensatie`
- NRP: `heeft_recht_op_no_risk_polis`, `duur_no_risk_polis_jaren` (number)
- JC:  `heeft_recht_op_jobcoaching`
- WPA: `heeft_recht_op_werkplekaanpassing`
- PP:  `mag_proefplaatsing_aangaan`, `duur_proefplaatsing_weken` (number)

## Afhankelijkhedendiagram

```
NRP ─┬─→ wet_WIA          (is_wia_uitkeringsgerechtigd)
     ├─→ wajong           (is_wajong_gerechtigd)
     └─→ participatiewet  (is_banenafspraak_doelgroep)

LIV ─→ wfsv               (jaarloon_werknemer)
LKV ─→ wtl                (categorie_doelgroep)         # in Wtl zelf
LKS ─→ participatiewet    (loonwaarde_lager_dan_minimumloon)
LDP ─→ wajong             (is_wajong_gerechtigd)
JC  ─→ wet_WIA            (heeft_arbeidsbeperking)
WPA ─→ wet_WIA            (heeft_arbeidsbeperking)
PP  ─→ ww                 (heeft_recht_op_ww_uitkering)
```

NRP is bewust de meest verbonden — drie bron-wetten (WIA, Wajong, Pwet)
omdat artikel 29b lid 1 Ziektewet de doelgroep via meerdere kanalen
afbakent.

## Aanpak per regeling

### NRP (grondig)

- Ziektewet 29b: volledige `machine_readable` met `parameters` (bsn,
  indiensttredingsdatum) en `input.source` blokken naar wet_WIA, wajong,
  participatiewet voor doelgroepbepaling.
- Bron-wetten krijgen een minimale stub-`machine_readable` die de
  doelgroepvariabelen als parameter doorgeeft (zo werkt de BDD met een
  citizen-data-tabel, en de graph toont de cross-law edges).
- Untranslatables markeren waar de wettekst niet eenduidig is
  (bijv. vijfjaarstermijn bij onderbroken dienstverbanden).

### Skeletons (LIV, LKV, LKS, LDP, JC, WPA, PP)

Per skeleton:

- Volledig artikel uit harvest, ongewijzigd qua tekst.
- `machine_readable.execution.parameters` met minstens `bsn`.
- `machine_readable.execution.input` met minstens één `source` blok naar
  een bron-wet (zodat er een graph-edge ontstaat).
- `machine_readable.execution.output` met de hoofduitkomst.
- `machine_readable.execution.actions` met één placeholder-action: een
  `EQUALS` of vergelijkbaar zodat de YAML schema-conform is. **Geen
  uitgewerkte logica** — commentaarregel `# SKELETON: logica nog niet
  uitgewerkt`.

## Open punten voor jurist (nu al)

1. NRP-vijfjaarstermijn bij onderbroken dienstverbanden — Ziektewet
   art. 29b lid 1.
2. Verhouding LIV / LKV / LKS / NRP bij dezelfde dienstbetrekking.
3. Wajong: oude (BWBR0008657) vs Wet vereenvoudiging Wajong (2021) — voor
   LDP geldt het oude regime, voor NRP-doelgroepbepaling beide?
4. WPA: art. 35 Wet WIA dekt ook voorzieningen, niet alleen
   werkplekaanpassingen — afbakening?
5. PP: termijn van 2 maanden in WW art. 76e — uitzonderingen via
   UWV-beleidsregels?

## Tijdsbudget

| Stap | Tijd      |
|------|-----------|
| 4.1  | 30 min ✓  |
| 4.2  | 45 min    |
| 4.3  | 2 uur     |
| 4.4  | 45 min    |
| 4.5  | 1 uur 15  |
| 4.6  | 30 min    |
| 4.7  | 15 min    |
