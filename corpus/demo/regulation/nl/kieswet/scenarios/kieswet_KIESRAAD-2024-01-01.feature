# Converted from overig/kieswet_KIESRAAD-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Bepalen kiesrecht Tweede Kamer
  Als burger
  Wil ik weten of ik stemrecht heb voor de Tweede Kamerverkiezingen
  Zodat ik weet of ik mag stemmen

  Background:
    Given the calculation date is "2025-03-15"
    And parameter "bsn" is "999993653"

  Scenario: Persoon met Nederlandse nationaliteit van 18+ mag stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn       | verkiezingsdatum |
      | 999993653 | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn       | status | inrichting_type |
      | 999993653 | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999993653 | 2006-01-01    | null              | null        | []                | Amsterdam      | []             | NLD           | NEDERLANDS    | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn       | stemrecht_uitsluitingen |
      | 999993653 | []                      |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "kieswet"
    Then output "voldoet_aan_voorwaarden" is true

  Scenario: Persoon zonder Nederlandse nationaliteit mag niet stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn       | verkiezingsdatum |
      | 999993653 | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn       | status | inrichting_type |
      | 999993653 | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999993653 | 1990-01-01    | null              | null        | []                | Amsterdam      | []             | NLD           | DUITS         | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn       | stemrecht_uitsluitingen |
      | 999993653 | []                      |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "kieswet"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Persoon onder 18 mag niet stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn       | verkiezingsdatum |
      | 999993653 | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn       | status | inrichting_type |
      | 999993653 | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999993653 | 2008-01-01    | null              | null        | []                | Amsterdam      | []             | NLD           | NEDERLANDS    | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn       | stemrecht_uitsluitingen |
      | 999993653 | []                      |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "kieswet"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Persoon met uitsluiting kiesrecht mag niet stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn       | verkiezingsdatum |
      | 999993653 | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn       | status | inrichting_type |
      | 999993653 | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999993653 | 1990-01-01    | null              | null        | []                | Amsterdam      | []             | NLD           | NEDERLANDS    | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn       | stemrecht_uitsluitingen                                                                                     |
      | 999993653 | [{"startdatum":"2023-01-01","einddatum":"2024-01-01"},{"startdatum":"2024-06-01","einddatum":"2025-12-01"}] |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "kieswet"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Gedetineerde persoon mag wel stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn       | verkiezingsdatum |
      | 999993653 | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn       | status     | inrichting_type |
      | 999993653 | INGESLOTEN | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999993653 | 1990-01-01    | null              | null        | []                | Amsterdam      | []             | NLD           | NEDERLANDS    | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn       | stemrecht_uitsluitingen                                |
      | 999993653 | [{"startdatum":"2023-01-01","einddatum":"2024-01-01"}] |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "kieswet"
    Then output "voldoet_aan_voorwaarden" is true
