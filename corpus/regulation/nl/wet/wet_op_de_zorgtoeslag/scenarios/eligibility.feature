Feature: Zorgtoeslag eligibility

  Background:
    Given the calculation date is "2025-01-01"
    Given law "wet_basisregistratie_personen" is loaded
    Given law "zorgverzekeringswet" is loaded
    Given law "regeling_standaardpremie" is loaded
    Given law "algemene_wet_inkomensafhankelijke_regelingen" is loaded
    Given law "wet_inkomstenbelasting_2001" is loaded

  Scenario: Meerderjarige met actieve polis heeft recht op zorgtoeslag
    Given the following "personal_data" data with key "bsn":
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 2005-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 79547                     | 0                         | 0                     | 0                               | 0            | 0                   |
    Given the following "box2" data with key "bsn":
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true

  Scenario: Minderjarige heeft geen recht op zorgtoeslag
    Given the following "personal_data" data with key "bsn":
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 2008-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 0                         | 0                         | 0                     | 0                               | 0            | 0                   |
    Given the following "box2" data with key "bsn":
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is false
