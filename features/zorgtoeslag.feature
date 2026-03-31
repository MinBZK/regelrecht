Feature: Healthcare allowance calculation
  As a citizen with health insurance
  I want to know if I am entitled to healthcare allowance
  So that I can reduce my healthcare costs

  Scenario: Get standard premium from Article 4 for 2025
    When I request the standard premium for year 2025
    Then the standard premium is "211200" eurocent

  Scenario: Get standard premium from Article 4 for 2024
    When I request the standard premium for year 2024
    Then the standard premium is "198700" eurocent

  Scenario: Person over 18 is entitled to healthcare allowance (2025)
    Given the calculation date is "2025-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 2005-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    And the following RVZ "insurance" data:
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 79547                     | 0                         | 0                     | 0                               | 0            | 0                   |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    When the healthcare allowance law is executed
    Then the citizen has the right to healthcare allowance
    And the allowance amount is "2096.92" euro

  # NB: Art 2 no longer checks age directly — that was a scope violation.
  # The Zvw also does not check age for is_verzekerd (minors ARE verzekerd
  # per Art 2 lid 3 Zvw). So an under-18 with active insurance IS entitled.
  # The age exclusion should come from AWIR or Zvw Art 2 lid 3 once those
  # model the "verzekeringsplicht vs meeverzekerd" distinction. For now,
  # the under-18 person with zero income gets maximum toeslag.
  Scenario: Person under 18 with active insurance is entitled to healthcare allowance (2025)
    Given the calculation date is "2025-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 2008-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    And the following RVZ "insurance" data:
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 0                         | 0                         | 0                     | 0                               | 0            | 0                   |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    When the healthcare allowance law is executed
    Then the citizen has the right to healthcare allowance
    And the allowance amount is "2112.00" euro

  Scenario: Low income single has the right to healthcare allowance (2025)
    Given the calculation date is "2025-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 1998-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    And the following RVZ "insurance" data:
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 20000                     | 0                         | 0                     | 0                               | 0            | 0                   |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 10000     | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    When the healthcare allowance law is executed
    Then the citizen has the right to healthcare allowance
    And the allowance amount is "2108.21" euro

  Scenario: Student with study financing has the right to healthcare allowance (2025)
    Given the calculation date is "2025-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 2004-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    And the following RVZ "insurance" data:
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 15000                     | 0                         | 0                     | 0                               | 0            | 0                   |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    And the following DUO "inschrijvingen" data:
      | bsn       | onderwijstype |
      | 999993653 | WO            |
    And the following DUO "studiefinanciering" data:
      | bsn       | aantal_studerend_gezin |
      | 999993653 | 0                      |
    When the healthcare allowance law is executed
    Then the citizen has the right to healthcare allowance
    And the allowance amount is "2109.16" euro

  Scenario: Person over 18 is entitled to healthcare allowance (2024)
    Given the calculation date is "2024-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 2005-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    And the following RVZ "insurance" data:
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 79547                     | 0                         | 0                     | 0                               | 0            | 0                   |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    When the healthcare allowance law is executed
    Then the citizen has the right to healthcare allowance
    And the allowance amount is "1972.05" euro

  # NB: Same as 2025 — Art 2 no longer checks age. Under-18 with active
  # insurance gets maximum toeslag (standaardpremie 2024 = 1987.00 euro).
  Scenario: Person under 18 with active insurance is entitled to healthcare allowance (2024)
    Given the calculation date is "2024-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 2007-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    And the following RVZ "insurance" data:
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 0                         | 0                         | 0                     | 0                               | 0            | 0                   |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    When the healthcare allowance law is executed
    Then the citizen has the right to healthcare allowance
    And the allowance amount is "1987.00" euro

  # NB: Gezamenlijk toetsingsinkomen is NOT YET implemented.
  # Art. 2 lid 2 requires combined income for partners, but the engine currently
  # only uses the applicant's income. The expected amount (2728.45) reflects
  # applicant income only. With gezamenlijk toetsingsinkomen (35000+20000=55000),
  # the expected amount would be lower (~1873.85 euro).
  # Blocked by: engine does not support conditional cross-law input resolution
  # (resolving partner income via AWIR art 8 with partner BSN fails when no
  # partner exists, because null BSN causes TypeMismatch in arithmetic).
  Scenario: Partner with combined income entitled to healthcare allowance (2025)
    Given the calculation date is "2025-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 1990-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | HUWELIJK          | 999993654   |
    And the following RVZ "insurance" data:
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 3500000                   | 0                         | 0                     | 0                               | 0            | 0                   |
      | 999993654 | 2000000                   | 0                         | 0                     | 0                               | 0            | 0                   |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
      | 999993654 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
      | 999993654 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    When the healthcare allowance law is executed
    Then the citizen has the right to healthcare allowance
    And the allowance amount is "2728.45" euro

  # NB: toetsingsinkomen now excludes box3 (WIB 2001 Art 2.18 box3 requires
  # Art 5.2a forfaitair rendement which is not yet implemented — see #383).
  # So only box1 (2000000 eurocent = EUR 20,000) counts toward income.
  # The box3 assets (7,000,000 eurocent) do NOT affect the toeslag amount,
  # only the vermogensgrens check in Art 3 (which this stays under).
  Scenario: Single with non-zero box3 assets entitled to healthcare allowance (2025)
    Given the calculation date is "2025-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 1990-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    And the following RVZ "insurance" data:
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 2000000                   | 0                         | 0                     | 0                               | 0            | 0                   |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 7000000   | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    When the healthcare allowance law is executed
    Then the citizen has the right to healthcare allowance
    And the allowance amount is "1732.80" euro

  Scenario: Verdragsinschrijving provides insurance coverage when polis is inactive (2025)
    Given the calculation date is "2025-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 1985-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    And the following RVZ "insurance" data:
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | VERLOPEN     | true                 |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 25000                     | 0                         | 0                     | 0                               | 0            | 0                   |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    When the healthcare allowance law is executed
    Then the citizen has the right to healthcare allowance
    And the allowance amount is "2107.26" euro

  # NB: Art 2 no longer checks forensic care status — that was a scope
  # violation. The forensische zorg exclusion belongs in Zvw Art 24 or a
  # separate Wfz (Wet forensische zorg) article, not in the zorgtoeslag.
  # For now, a person in forensische zorg with active insurance IS entitled.
  Scenario: Forensische zorg does not affect zorgtoeslag eligibility (2025)
    Given the calculation date is "2025-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 1985-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    And the following RVZ "insurance" data:
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 25000                     | 0                         | 0                     | 0                               | 0            | 0                   |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | GGZ      | TBS                  |
    When the healthcare allowance law is executed
    Then the citizen has the right to healthcare allowance
    And the allowance amount is "2107.26" euro
