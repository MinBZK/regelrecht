# POC to MVP Corrections: Zorgtoeslag

During the conversion of the zorgtoeslag from POC (poc-machine-law) to MVP (regelrecht-mvp), the execution traces of both systems were compared side-by-side for all 8 shared scenarios (4x 2025, 4x 2024). This document exists to record the differences found and to explain *why* the POC and MVP produce different results in certain cases. Each difference is either a correction of an error in the POC, or a deliberate architectural improvement in the MVP.

## 1. Age Calculation: Wrong Reference Date

**POC behaviour**: Age was calculated using `$prev_january_first` — January 1st of the *previous* year relative to the reference date. For a 2025 calculation, the age was determined as of 2024-01-01.

**Example** (birthdate 2005-01-01, calculation year 2025):
- POC: `SUBTRACTDATE(2024-01-01, 2005-01-01)` = **19**
- MVP: `SUBTRACTDATE(2025-01-01, 2005-01-01)` = **20**

**MVP correction**: Age is calculated using `$REFERENCEDATE`, which equals the calculation date itself (e.g. 2025-01-01 for a 2025 zorgtoeslag calculation). This matches the legal requirement.

**Impact on results**: None for the current test scenarios (all persons are either clearly above or below 18), but would cause incorrect eligibility decisions for persons who turn 18 during the calculation year.

## 2. Fabricated 2024 Percentages

**POC behaviour**: The 2024 zorgtoeslag YAML used identical fabricated percentages for both income thresholds:
- `percentage_drempelinkomen_alleenstaande`: **0.0486** (4.86%)
- `percentage_drempelinkomen_partner`: **0.0486** (4.86%)

These do not correspond to any published legal values.

**MVP correction**: Replaced with the statutory values from the Wet op de zorgtoeslag article 2 paragraph 3:
- `percentage_drempelinkomen_alleenstaande`: **0.01879** (1.879%)
- `percentage_drempelinkomen_partner`: **0.04256** (4.256%)

**Impact on results**: All 2024 zorgtoeslag amounts differ. Example for income 79,547 eurocent:

| | POC | MVP |
|--|-----|-----|
| Norm premium | 0.0486 × 79,547 = 3,865.98 | 0.01879 × 79,547 = 1,494.29 |
| Zorgtoeslag | 198,700 − 3,865.98 = **194,834** | 198,700 − 1,494.29 = **197,205** |
| In euros | EUR 1,948.34 | EUR 1,972.05 |

## 3. Incorrect Heffingsvrije Voet (Box 3 Tax-Free Allowance)

**POC behaviour**: Used a single value of **5,772,900** eurocent for the heffingsvrije voet alleenstaand, applied to both 2024 and 2025 calculations. This value does not match either year's legal amount.

**MVP correction**: Year-specific values matching the Wet Inkomstenbelasting 2001:
- 2025: **5,768,400** eurocent (alleenstaand) / **11,536,800** (partners)
- 2024: **5,700,000** eurocent (alleenstaand) / **11,400,000** (partners)

**Impact on results**: None for the current test scenarios (all savings amounts are well below the threshold), but would produce incorrect box3 income calculations for persons with assets near the threshold.

## 4. Missing Partner Premium Doubling

**POC behaviour**: The standard premium (`standaardpremie`) was used as-is for all calculations, regardless of partner status.

**MVP correction**: Per article 2 paragraph 1 of the Zorgtoeslagwet: *"Voor een verzekerde met een toeslagpartner geldt het dubbele van de standaardpremie."* The MVP applies an IF/MULTIPLY to double the standard premium when `heeft_toeslagpartner = true`.

**Impact on results**: All partner scenarios would have been incorrect in the POC. The MVP adds explicit partner test scenarios to verify this.

## 5. Eurocent Rounding (Trace Issue 2)

**POC behaviour**: Float outputs with `type_spec.unit == "eurocent"` were implicitly rounded to integers in the trace output (e.g. `209691.78888 → 209692`), but the rounding mechanism was not explicit in the engine.

**MVP correction**: Added explicit TypeSpec enforcement in the engine (`service.rs`). After evaluation, outputs with `type_spec.unit == "eurocent"` are rounded via `f64_to_i64_safe(f.round())`, which also guards against silent overflow on NaN/Infinity values.

**Impact on results**: Functionally equivalent for normal values, but the MVP is safer (explicit rounding, overflow protection) and produces consistent integer values in both the result and the trace.

## 6. Incomplete Box 3 Calculation (Trace Issue 3)

**POC behaviour**: The WIB article 5.2 lacked the complete box 3 income calculation. The heffingsvrije voet, forfaitair rendement, and the intermediate `box3_bezittingen` computation were not fully modelled.

**MVP correction**: WIB article 5.2 now computes:
```
box3_bezittingen = MAX(0, (spaargeld + beleggingen + onroerend_goed) - schulden - heffingsvrije_voet)
box3_inkomen = box3_bezittingen × forfaitair_rendement (6%)
```

The heffingsvrije voet is selected based on partner status (alleenstaand vs partners).

**Impact on results**: For the existing test scenarios with low or zero savings, box3_inkomen remains 0. The new MVP-only scenario (EUR 70,000 savings) exercises this path and produces a zorgtoeslag of EUR 1,718.79 — confirming the box3 calculation affects the outcome.

## 7. Toetsingsinkomen Missing Box 3 Component (Trace Issue 4)

**POC behaviour**: The WIB article 2.18 computed toetsingsinkomen as `ADD(box1, box2)`, omitting box 3 income entirely.

**MVP correction**: Toetsingsinkomen is now `ADD(box1, box2, box3_inkomen)`, where `box3_inkomen` is resolved from WIB article 5.2.

**Impact on results**: For persons with significant savings/investments above the heffingsvrije voet, the POC would underestimate their toetsingsinkomen, leading to an overstated zorgtoeslag.

## 8. Implicit None Handling in Data Resolution

**POC behaviour**: When input data was not provided (e.g. no box2 dividends for a student), the engine produced warnings:
```
Could not resolve value for BOX2_DIVIDEND
Could not resolve value for BOX2_AANDELEN
No values found (or they where None), returning 0 for ADD([None, None])
```

The engine silently coerced `None` values to 0 in ADD operations.

**MVP correction**: The data model uses explicit field names (`reguliere_voordelen`, `vervreemdingsvoordelen` for box2) with explicit 0 defaults provided by the data source. No implicit None-to-0 coercion occurs, and the trace shows clean resolution paths without warnings.

## 9. Structural Differences in Requirements (2024)

**POC behaviour**: The 2024 zorgtoeslag law had a different requirements structure than 2025:

```
2024 requirements (POC):
├── HEEFT_VERZEKERING (via RVZ, simple polis check)
├── LEEFTIJD >= 18
├── IS_GEDETINEERD = false (via DJI.penitentiaire_beginselenwet)
└── IS_FORENSISCH = false (via DJI.wet_forensische_zorg)
```

The 2025 version combined insurance and detention into a single `IS_VERZEKERDE` check.

**MVP correction**: Both 2024 and 2025 use a unified approach through `zorgverzekeringswet#is_verzekerd`, which computes `AND(IN(polis_status, actieve_statussen), NOT(is_gedetineerd))`. This is structurally consistent and easier to maintain.

**Note**: The forensische zorg check (`wet_forensische_zorg`) present in the POC 2024 is not yet modelled in the MVP. See [open items](#open-items).

## 10. WIB 2025 Metadata Errors

**POC behaviour**: The WIB 2025 YAML contained metadata errors:
- `valid_from` was set to `2024-01-01` instead of `2025-01-01`
- `competent_authority` was set to `TODO`

**MVP correction**: Corrected to `valid_from: 2025-01-01` and `competent_authority: Belastingdienst`.

## Open Items

The following POC features are not yet modelled in the MVP:

1. **Missing 2024 scenarios**: The POC has 4 scenarios per year; the MVP only has 2 for 2024 (over-18 and under-18). The low-income and student 2024 scenarios should be added.
2. **Buitenlands inkomen**: The POC includes `buitenlands_inkomen` as a component of toetsingsinkomen. The MVP does not model this.
3. **Forensische zorg check**: The POC 2024 checks `IS_FORENSISCH` via `DJI.wet_forensische_zorg`. The MVP does not include this requirement.
4. **Verdragsinschrijving**: The POC `is_verzekerde` check includes a path for `VERDRAGSINSCHRIJVING` (foreign treaty registration). The MVP does not model this.
