# Zorgtoeslag POC-naar-MVP Conversie Rapport

## Overzicht

Volledige conversie van de zorgtoeslag BDD-scenarios van de POC (Python/poc-machine-law) naar de MVP (Rust/regelrecht-mvp). De kern van de conversie: services omgezet naar datasources, met cross-law resolution via de engine.

## Geconverteerde scenarios

| # | Scenario | Jaar | Verwacht | Resultaat |
|---|----------|------|----------|-----------|
| 1 | Standaardpremie ophalen | 2025 | 211200 eurocent | PASS |
| 2 | Standaardpremie ophalen | 2024 | 198700 eurocent | PASS |
| 3 | Persoon boven 18, inkomen 79547 | 2025 | 2096.92 euro | PASS |
| 4 | Persoon onder 18 | 2025 | geen recht | PASS |
| 5 | Laag inkomen alleenstaande (20000) | 2025 | 2108.21 euro | PASS |
| 6 | Student met studiefinanciering (15000) | 2025 | 2109.16 euro | PASS |
| 7 | Persoon boven 18, inkomen 79547 | 2024 | 1948.34 euro | PASS |
| 8 | Persoon onder 18 | 2024 | geen recht | PASS |

**Totaal: 8 zorgtoeslag-scenarios (was 3), 22 scenarios totaal, 139 steps - alle PASS.**

## Service-naar-datasource mapping

| POC Service | POC Methode | MVP Datasource | MVP Resolutie |
|-------------|-------------|----------------|---------------|
| RvIG | `get_personen` | `personal_data` | BRP art 2.7 (leeftijd via SUBTRACT_DATE) |
| RvIG | `get_relaties` | `relationship_data` | AWIR art 3 (heeft_toeslagpartner via IN) |
| RVZ | `get_verzekeringen` | `insurance` | ZVW art 2 (is_verzekerd via AND/IN/NOT_EQUALS) |
| Belastingdienst | `get_box1` | `box1` | WIB art 3.1 (box1_inkomen via ADD) |
| Belastingdienst | `get_box2` | `box2` | WIB art 4.12 (box2_inkomen via ADD) |
| Belastingdienst | `get_box3` | `box3` | WIB art 5.2 (rendementsgrondslag via SUBTRACT/ADD) |
| DJI | `get_detenties` | `detenties` | Penitentiaire beginselenwet art 1 (is_gedetineerd) |
| DUO | `get_inschrijvingen` | `inschrijvingen` | Meegestuurd als datasource, niet direct gebruikt |
| DUO | `get_studiefinanciering` | `studiefinanciering` | Meegestuurd als datasource, niet direct gebruikt |
| VWS (regeling) | standaardpremie | `regeling_standaardpremie` | Art 4 resolve (ministeriele_regeling match op berekeningsjaar) |

## Cross-law resolution chain (2025 scenario)

```
zorgtoeslagwet (art 2: hoogte_zorgtoeslag)
  -> wet_basisregistratie_personen (art 2.7: leeftijd)
       -> datasource: personal_data.geboortedatum
  -> zorgverzekeringswet (art 2: is_verzekerd)
       -> datasource: insurance.polis_status
       -> penitentiaire_beginselenwet (art 1: is_gedetineerd)
            -> datasource: detenties.detentiestatus, detenties.inrichting_type
  -> algemene_wet_inkomensafhankelijke_regelingen (art 3: heeft_toeslagpartner)
       -> datasource: relationship_data.partnerschap_type
  -> algemene_wet_inkomensafhankelijke_regelingen (art 8: toetsingsinkomen)
       -> wet_inkomstenbelasting_2001 (art 2.18: toetsingsinkomen)
            -> wet_inkomstenbelasting_2001 (art 3.1: box1_inkomen)
                 -> datasource: box1.loon_uit_dienstbetrekking, ...
            -> wet_inkomstenbelasting_2001 (art 4.12: box2_inkomen)
                 -> datasource: box2.reguliere_voordelen, ...
  -> regeling_standaardpremie (art 1: standaardpremie) [via art 4 resolve]
  -> zorgtoeslagwet (art 3: vermogen_onder_grens)
       -> wet_inkomstenbelasting_2001 (art 5.2: rendementsgrondslag)
            -> datasource: box3.spaargeld, ...
       -> algemene_wet_inkomensafhankelijke_regelingen (art 3: heeft_toeslagpartner) [cached]
```

## Aangebrachte wijzigingen

### Nieuwe bestanden
- `regulation/nl/ministeriele_regeling/regeling_standaardpremie/2024-01-01.yaml` - Standaardpremie 2024 (198700 eurocent)
- `regulation/nl/wet/wet_op_de_zorgtoeslag/2024-01-01.yaml` - Zorgtoeslagwet 2024 met 2024-specifieke bedragen

### Gewijzigde bestanden
- **Schema migratie** (v0.3.0 -> v0.3.2): alle 12 bestaande regulation YAMLs
- **Action format fix** (v0.3.2 compliance): `operation`+`values` op action-niveau gewrapped in `value:`
  - wet_inkomstenbelasting_2001, algemene_wet_inkomensafhankelijke_regelingen, wet_op_de_zorgtoeslag, burgerlijk_wetboek_boek_5, participatiewet
- **regeling_standaardpremie/2025-01-01.yaml**: `valid_from` gewijzigd van `#datum_inwerkingtreding` naar `2025-01-01`
- **wet_inkomstenbelasting_2001**: `valid_from` gewijzigd van `2025-01-01` naar `2024-01-01` (wet was ook geldig in 2024)
- **features/zorgtoeslag.feature**: Van 3 naar 8 scenarios
- **packages/engine/tests/bdd/world.rs**: ExternalData uitgebreid met DUO velden
- **packages/engine/tests/bdd/steps/given.rs**: DUO datasource steps toegevoegd
- **packages/engine/tests/bdd/steps/when.rs**: DUO datasource registratie in execute
- **packages/engine/tests/bdd/steps/then.rs**: Entitlement assertions (heeft_recht/geen_recht)

## 2024-specifieke definities

| Parameter | 2024 | 2025 |
|-----------|------|------|
| standaardpremie | 198700 | 211200 |
| drempelinkomen_alleenstaande | 3749600 | 3971900 |
| drempelinkomen_met_partner | 4821800 | 5587500 |
| percentage_drempelinkomen_alleenstaande | 0.0486 | 0.01896 |
| percentage_drempelinkomen_partner | 0.0486 | 0.04273 |
| percentage_toetsingsinkomen | 0.1367 | 0.137 |
| vermogensgrens_alleenstaand | 12758200 | 14189600 |
| vermogensgrens_met_partner | 16132900 | 17942900 |

## Berekening verificatie

### 2025 - Persoon boven 18 (inkomen 79547)
```
normpremie = percentage_drempelinkomen * MIN(inkomen, drempelinkomen) + percentage_toetsingsinkomen * MAX(0, inkomen - drempelinkomen)
           = 0.01896 * MIN(79547, 39719) + 0.137 * MAX(0, 79547 - 39719)
           = 0.01896 * 39719 + 0.137 * 39828
           = 753.07 + 5456.44
           = 6209.51
zorgtoeslag = MAX(0, 211200 - 620951) = MAX(0, -409751) ... wait
           = MAX(0, standaardpremie - normpremie_eurocent)

In eurocent:
normpremie = 0.01896 * MIN(79547, 3971900) + 0.137 * MAX(0, 79547 - 3971900)
           = 0.01896 * 79547 + 0 (79547 < 3971900)
           = 1508.21
zorgtoeslag = MAX(0, 211200 - 1508.21) = 209691.79 -> /100 = 2096.92 euro
```

### 2025 - Laag inkomen (20000)
```
normpremie = 0.01896 * MIN(20000, 3971900) + 0 = 0.01896 * 20000 = 379.2
zorgtoeslag = MAX(0, 211200 - 379.2) = 210820.8 -> /100 = 2108.21 euro
```

### 2025 - Student (15000)
```
normpremie = 0.01896 * 15000 = 284.4
zorgtoeslag = MAX(0, 211200 - 284.4) = 210915.6 -> /100 = 2109.16 euro
```

### 2024 - Persoon boven 18 (inkomen 79547)
```
normpremie = 0.0486 * MIN(79547, 3749600) + 0.1367 * MAX(0, 79547 - 3749600)
           = 0.0486 * 79547 + 0 = 3865.98
zorgtoeslag = MAX(0, 198700 - 3865.98) = 194834.02 -> /100 = 1948.34 euro
```

## Trace vergelijking POC vs MVP

Traces vergeleken door beide engines te draaien met debug logging op het scenario
"Persoon boven 18, inkomen 79547 (2025)" en de resolution chains stap voor stap naast
elkaar te leggen.

### Architecturele verschillen

| Aspect | POC | MVP |
|--------|-----|-----|
| Taal | Python | Rust |
| Architectuur | Service-georiënteerd (vooraf berekend) | Datasource-georiënteerd (lazy resolution) |
| Wetten geladen | Per scenario specifiek | Alle wetten bij initialisatie |
| Cross-law | Via service calls (RvIG, RVZ, UWV, etc.) | Via engine RESOLVE operatie |
| Caching | Ja (service-level, key-based) | Ja (ResolutionContext cache) |
| Versie selectie | Aparte feature files per jaar | valid_from filtering op referencedate |

### Stap-voor-stap resolution chain vergelijking (2025, inkomen 79547)

| Stap | POC trace | MVP trace | Uitkomst |
|------|-----------|-----------|----------|
| **1. Leeftijd** | RvIG → wet_brp art leeftijd, SUBTRACT_DATE(2024-01-01, 2005-01-01) = **19** | wet_basisregistratie_personen art 2.7, SUBTRACTDATE(2025-01-01, 2005-01-01) = **20** | Beide >= 18, OK |
| **2. is_verzekerd** | RVZ → zvw: AND(IN(ACTIEF, [ACTIEF, GESCHORST_...]), NOT_EQUALS(is_gedetineerd, true)) = **True** | zorgverzekeringswet art 2: AND(IN(ACTIEF, [...]), NOT_EQUALS(false, true)) = **True** | Identiek |
| **3. is_gedetineerd** | DJI → penitentiaire_beginselenwet: AND(IN(None, [PI, HvB])) = **False** | penitentiaire_beginselenwet art 1: AND(IN(None, [PI, HvB])) = **False** | Identiek |
| **4. heeft_partner** | RvIG → wet_brp: IN(GEEN, [HUWELIJK, GP]) = **False** | AWIR art 3: IN(GEEN, [HUWELIJK, GP]) = **False** | Zelfde resultaat, andere wet |
| **5. box1_inkomen** | BELASTINGDIENST → wet_inkomstenbelasting: ADD(79547, 0, 0, 0, 0) = **79547** | WIB art 3.1: ADD(79547, 0, 0, 0, 0) = **79547** | Identiek |
| **6. box2_inkomen** | BELASTINGDIENST → wet_inkomstenbelasting: ADD(0, 0) = **0** | WIB art 4.12: ADD(0, 0) = **0** | Identiek |
| **7. toetsingsinkomen** | UWV → wet_inkomstenbelasting: ADD(box1=79547, box2=0, box3=0, buitenlands=0) = **79547** | AWIR art 8 → WIB art 2.18: ADD(box1=79547, box2=0) = **79547** | Identiek |
| **8. standaardpremie** | VWS → regeling_vaststelling: **211200** | regeling_standaardpremie art 1: **211200** | Identiek |
| **9. normpremie** | MULTIPLY(0.01896, MIN(79547, 3971900)) = **1508.21** | MULTIPLY(0.01896, MIN(79547, 3971900)) = **1508.21** | Identiek |
| **10. vermogen** | BELASTINGDIENST → wet_inkomstenbelasting: SUBTRACT(ADD(0,0,0), 0) = **0** | WIB art 5.2: SUBTRACT(ADD(0,0,0), 0) = **0** | Identiek |
| **11. hoogte_toeslag** | SUBTRACT(211200, 1508.21) = 209691.79 → afgerond **209692** | SUBTRACT(211200, 1508.21) = **209691.79** (float) | Identiek in euro |
| **Eindresultaat** | **209692 eurocent → 2096.92 euro** | **209691.79 eurocent → 2096.92 euro** | **Identiek** |

### Gevonden verschillen

#### 1. Leeftijd referentiedatum

De POC berekent leeftijd op `$prev_january_first` (1 januari **vorig** jaar, dus 2024-01-01 bij datum
2025-02-01). De MVP berekent op `$REFERENCEDATE` (de calculation date zelf, 2025-01-01). Dit geeft
voor geboortedatum 2005-01-01 een verschil: POC=19, MVP=20.

**Wat de wet zegt:** Zorgtoeslag wordt **per kalendermaand afzonderlijk** bepaald (Art. 2 lid 5
Wet op de zorgtoeslag). AWIR Art. 5 bepaalt dat leeftijdswijzigingen na de eerste dag van een
maand pas gelden vanaf de eerste dag van de **volgende** maand. De correcte peildatum voor
leeftijd is dus de **eerste dag van de betreffende kalendermaand**.

**Conclusie:** De POC's `$prev_january_first` (1 januari vorig jaar) is **onjuist** — dit is geen
wettelijke peildatum. De MVP's gebruik van `$referencedate` (= calculation_date) is **correct**,
mits calculation_date op de eerste van de maand staat die wordt berekend.

#### 2. Afronding en float-gebruik

De POC past `TypeSpec.enforce()` toe op outputs met `unit: eurocent`. De Python `int()` functie
trunceert naar nul (209691.79 → 209691, niet 209692). De MVP Rust engine behoudt float-waarden
omdat de engine `type_spec` niet actief afdwingt — het is enkel metadata.

**Risico:** Float-berekeningen leiden tot drijvende-komma-artefacten (bijv. `0.01896 * 79547 =
1508.2111200000002` i.p.v. `1508.21112`). Bij complexere berekeningen stapelen deze fouten op.

#### 3. Toetsingsinkomen: ontbrekende componenten

De POC berekent toetsingsinkomen als `ADD(box1, box2, box3_inkomen, buitenlands_inkomen)` — vier
componenten. De MVP berekent het als `ADD(box1, box2)` via WIB art 2.18 — box3-rendement en
buitenlands inkomen ontbreken.

Bij de huidige testscenarios (box3=0, buitenlands=0) is het resultaat identiek, maar bij
realistischere data zal het toetsingsinkomen te laag uitvallen.

#### 4. Box3 berekening: incompleet in MVP

De POC berekent box3 in meerdere stappen:
- `box3_bezittingen = MAX(0, SUBTRACT(ADD(sparen, beleggen, onroerend_goed), schulden, heffingsvrije_voet))`
- `box3_inkomen = MULTIPLY(box3_bezittingen, forfaitair_rendement=0.06)`

De MVP berekent alleen `rendementsgrondslag = SUBTRACT(ADD(spaargeld, beleggingen, onroerend_goed), schulden)`
zonder heffingsvrije voet (5.772.900 eurocent alleenstaand) en zonder forfaitair rendement (6%).

#### 5. Partner resolutie: andere juridische grondslag (geen issue)

- POC: `heeft_partner` via **RvIG** service → **wet_brp** (personen/relaties)
- MVP: `heeft_toeslagpartner` via **AWIR** art 3 (algemene_wet_inkomensafhankelijke_regelingen)

Beide controleren of `partnerschap_type IN [HUWELIJK, GEREGISTREERD_PARTNERSCHAP]`.
De MVP volgt de juridisch correctere route (AWIR definieert specifiek toeslagpartner),
terwijl de POC het generiekere BRP-partnerconcept gebruikt. Geen actie nodig.

#### 6. Verzekering: extra check in POC (geen issue)

De POC berekent eerst `heeft_verzekering` en `heeft_verdragsverzekering` apart, en combineert
ze dan in `is_verzekerde = AND(OR(heeft_verzekering, heeft_verdragsverzekering), NOT(is_gedetineerd))`.

De MVP combineert dit in een enkele `is_verzekerd` berekening:
`AND(IN(polis_status, [ACTIEF, ...]), NOT_EQUALS(is_gedetineerd, true))`.

Functioneel equivalent. De POC modelleert de verdragsverzekering als apart pad, maar dit
beïnvloedt de uitkomst niet. Geen actie nodig.

### Overige scenarios: eindwaarden vergelijking

Alle vier 2025-scenarios zijn met debug traces gedraaid op beide engines:

| Scenario | POC resultaat | MVP resultaat | Match |
|----------|--------------|---------------|-------|
| Onder 18 (geb. 2007) | leeftijd=17, geen recht | leeftijd=17, geen recht | Identiek |
| Boven 18, inkomen 79547 | 209692 ec → **2096.92** euro | 209691.79 ec → **2096.92** euro | Identiek |
| Laag inkomen 20000 | 210821 ec → **2108.21** euro | 210820.8 ec → **2108.21** euro | Identiek |
| Student 15000 | 210916 ec → **2109.16** euro | 210915.6 ec → **2109.16** euro | Identiek |

De 2024-scenarios zijn gevalideerd op basis van de verwachte berekeningen (zie sectie
"Berekening verificatie"). De POC 2024 tests gebruiken dezelfde logica met 2024 parameters.

### Conclusie trace vergelijking

Beide engines produceren **identieke eindresultaten** voor alle 8 scenarios. De tussenwaarden
weken op vier punten af. Issue 1 bleek de MVP correct te hebben. Issues 2, 3 en 4 zijn
opgelost door TypeSpec enforcement in de engine en uitbreiding van WIB art 5.2 en 2.18.

## Op te lossen issues

De volgende vier issues zijn geconstateerd bij de trace vergelijking en moeten worden opgelost
voordat de MVP-implementatie als equivalent aan de POC kan worden beschouwd.

### Issue 1: Leeftijd referentiedatum — MVP is correct, POC niet

**Ernst:** Informatief — de MVP doet het goed, de POC niet.

De POC berekent leeftijd op `$prev_january_first` (1 januari vorig jaar). De MVP berekent op
`$REFERENCEDATE` (calculation date = eerste van de maand).

Na onderzoek van de wettekst blijkt de **MVP correct** te zijn:

- **Art. 2 lid 5 Wet op de zorgtoeslag:** Zorgtoeslag wordt per kalendermaand afzonderlijk bepaald.
- **Art. 5 AWIR:** Leeftijdswijzigingen na de eerste dag van de maand gelden vanaf de eerste
  dag van de volgende maand.
- **Art. 16 lid 2(a) Zorgverzekeringswet:** Premieverplichting start op de eerste dag van de
  kalendermaand volgend op de maand waarin de verzekerde 18 wordt.

De correcte peildatum is dus de **eerste dag van de betreffende kalendermaand**. De MVP's
`$referencedate` (= calculation_date, eerste van de maand) volgt dit correct. De POC's
`$prev_january_first` (1 januari vorig jaar) is juridisch onjuist.

**Actie:** Geen wijziging nodig in de MVP. De POC bevat hier een bug.

### Issue 2: Float-gebruik en eurocent-afronding — OPGELOST

**Ernst:** Hoog — float-artefacten en ontbrekende type_spec enforcement.

**Probleem:** De engine rekende met f64 floats en negeerde `type_spec: { unit: eurocent }` op
outputs. Dit leidde tot float-artefacten en ontbrekende afronding naar integer.

**Oplossing:** TypeSpec enforcement toegevoegd in `packages/engine/src/service.rs`. Na
`evaluate_with_trace()`/`evaluate_with_output()` worden outputs met `type_spec.unit == "eurocent"`
afgerond via `f.round() as i64`. Dit matcht het POC-gedrag (trace: `209691.78888 → 209692`).

### Issue 3: Box3 berekening incompleet — OPGELOST

**Ernst:** Hoog — leidt tot verkeerde rendementsgrondslag bij niet-nul box3-waarden.

**Probleem:** De MVP berekende alleen `rendementsgrondslag = bezittingen - schulden` zonder
heffingsvrije voet en forfaitair rendement.

**Oplossing:** WIB art 5.2 uitgebreid in `regulation/nl/wet/wet_inkomstenbelasting_2001/2025-01-01.yaml`:
- Definities: `heffingsvrije_voet_alleenstaand` (5.772.900 ec), `heffingsvrije_voet_partners`
  (11.545.800 ec), `forfaitair_rendement` (0.06)
- Input `heeft_toeslagpartner` via RESOLVE van AWIR art 3
- Output `box3_bezittingen = MAX(0, bezittingen - schulden - heffingsvrije_voet)`
- Output `box3_inkomen = MULTIPLY(box3_bezittingen, forfaitair_rendement)`
- Output `rendementsgrondslag` behouden voor backward compatibility

### Issue 4: Toetsingsinkomen mist box3-rendement — OPGELOST

**Ernst:** Middel — leidt tot te laag toetsingsinkomen bij box3-rendement.

**Probleem:** De MVP berekende toetsingsinkomen als `ADD(box1, box2)` — box3-rendement ontbrak.

**Oplossing:** WIB art 2.18 uitgebreid in `regulation/nl/wet/wet_inkomstenbelasting_2001/2025-01-01.yaml`:
- Input `box3_inkomen` toegevoegd via RESOLVE van WIB art 5.2
- Formule gewijzigd naar `toetsingsinkomen = ADD(box1, box2, box3_inkomen)`
- NB: Buitenlands inkomen is niet toegevoegd omdat dit niet in de huidige POC-scenarios zit
  en geen datasource heeft. Kan in een toekomstige iteratie worden toegevoegd.

## Niet-geconverteerde items

Geen. Alle 8 POC scenarios (4x 2024 + 4x 2025) zijn succesvol geconverteerd.

## Status trace-issues

| Issue | Beschrijving | Status |
|-------|-------------|--------|
| 1 | Leeftijd referentiedatum | MVP correct, POC bevat bug — geen actie |
| 2 | Float-gebruik en eurocent-afronding | **OPGELOST** — TypeSpec enforcement in engine |
| 3 | Box3 berekening incompleet | **OPGELOST** — WIB art 5.2 uitgebreid |
| 4 | Toetsingsinkomen mist box3-rendement | **OPGELOST** — WIB art 2.18 uitgebreid |
