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
      | bsn       | polis_status |
      | 999993653 | ACTIEF       |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | 999993653 | 79547                     | 0                         | 0                     | 0                               | 0            |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type |
      | 999993653 | null           | null            |
    When the healthcare allowance law is executed
    Then the citizen has the right to healthcare allowance
    And the allowance amount is "2096.92" euro

  Scenario: Person under 18 does not have the right to healthcare allowance (2025)
    Given the calculation date is "2025-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 2008-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    And the following RVZ "insurance" data:
      | bsn       | polis_status |
      | 999993653 | ACTIEF       |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | 999993653 | 0                         | 0                         | 0                     | 0                               | 0            |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type |
      | 999993653 | null           | null            |
    When the healthcare allowance law is executed
    Then the citizen does not have the right to healthcare allowance

  Scenario: Low income single has the right to healthcare allowance (2025)
    Given the calculation date is "2025-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 1998-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    And the following RVZ "insurance" data:
      | bsn       | polis_status |
      | 999993653 | ACTIEF       |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | 999993653 | 20000                     | 0                         | 0                     | 0                               | 0            |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 10000     | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type |
      | 999993653 | null           | null            |
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
      | bsn       | polis_status |
      | 999993653 | ACTIEF       |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | 999993653 | 15000                     | 0                         | 0                     | 0                               | 0            |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type |
      | 999993653 | null           | null            |
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
      | bsn       | polis_status |
      | 999993653 | ACTIEF       |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | 999993653 | 79547                     | 0                         | 0                     | 0                               | 0            |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type |
      | 999993653 | null           | null            |
    When the healthcare allowance law is executed
    Then the citizen has the right to healthcare allowance
    And the allowance amount is "1948.34" euro

  Scenario: Person under 18 does not have the right to healthcare allowance (2024)
    Given the calculation date is "2024-01-01"
    And the following RVIG "personal_data" data:
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 2007-01-01    | Amsterdam      | NEDERLAND     |
    And the following RVIG "relationship_data" data:
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    And the following RVZ "insurance" data:
      | bsn       | polis_status |
      | 999993653 | ACTIEF       |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | 999993653 | 0                         | 0                         | 0                     | 0                               | 0            |
    And the following BELASTINGDIENST "box2" data:
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    And the following BELASTINGDIENST "box3" data:
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    And the following DJI "detenties" data:
      | bsn       | detentiestatus | inrichting_type |
      | 999993653 | null           | null            |
    When the healthcare allowance law is executed
    Then the citizen does not have the right to healthcare allowance
