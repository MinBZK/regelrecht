# Converted from belastingen/wet_inkomstenbelasting_BELASTINGDIENST-2001-01-01.feature by corpus/demo/tools/convert_features.mjs
Feature: Berekening Inkomstenbelasting
  Als burger
  Wil ik weten hoeveel inkomstenbelasting ik verschuldigd ben
  Zodat ik mijn financiën kan plannen

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "bsn" is "999993653"

  Scenario: Berekening inkomstenbelasting voor alleenstaande met inkomen uit verschillende boxen
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999993653 | 1985-05-15    | GEEN              | null        | []                | Amsterdam      | []             | NEDERLAND     | NEDERLANDS    | null  | []           | null                  |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | 999993653 | 4800000                   | 0                         | 1200000               | 500000                          | -120000      | 300000              | 200000                 | 10000000  | 5000000     | 15000000       | 4000000  | 350000                  | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn       | verwachting_65 |
      | 999993653 | 0              |
    When I evaluate outputs "box1_inkomen, box2_inkomen, box3_inkomen, belastbaar_inkomen, totale_belastingschuld" of "wet_inkomstenbelasting"
    # POC: requirements met (law has no voldoet_aan_voorwaarden output)
    Then the execution succeeds
    And output "box1_inkomen" equals 6380000
    And output "box2_inkomen" equals 500000
    And output "box3_inkomen" equals 1213626
    And output "belastbaar_inkomen" equals 7743626
    And output "totale_belastingschuld" equals 2309416

  Scenario: Berekening inkomstenbelasting voor gepensioneerde met AOW
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999993653 | 1955-05-15    | GEEN              | null        | []                | Amsterdam      | []             | NEDERLAND     | NEDERLANDS    | null  | []           | null                  |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | 999993653 | 0                         | 2300000                   | 0                     | 0                               | -50000       | 0                   | 0                      | 8000000   | 2000000     | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn       | verwachting_65 |
      | 999993653 | 0              |
    When I evaluate outputs "box1_inkomen, box2_inkomen, box3_inkomen, totale_belastingschuld" of "wet_inkomstenbelasting"
    # POC: requirements met (law has no voldoet_aan_voorwaarden output)
    Then the execution succeeds
    And output "box1_inkomen" equals 2250000
    And output "box2_inkomen" equals 0
    And output "box3_inkomen" equals 253626
    And output "totale_belastingschuld" equals 126308

  Scenario: Berekening inkomstenbelasting voor werkende ouder met heffingskortingen
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999993653 | 1998-05-15    | GEEN              | null        | []                | Amsterdam      | []             | NEDERLAND     | NEDERLANDS    | null  | []           | null                  |
      | 111111111 | null          | null              | null        | []                | null           | []             | null          | null          | null  | []           | null                  |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | 999993653 | 3000000                   | 0                         | 0                     | 0                               | -90000       | 0                   | 0                      | 2000000   | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
      | 111111111 | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn       | verwachting_65 |
      | 999993653 | 0              |
      | 111111111 | 0              |
    When I evaluate outputs "box1_inkomen, box2_inkomen, box3_inkomen, totale_heffingskortingen, totale_belastingschuld" of "wet_inkomstenbelasting"
    # POC: requirements met (law has no voldoet_aan_voorwaarden output)
    Then the execution succeeds
    And output "box1_inkomen" equals 2910000
    And output "box2_inkomen" equals 0
    And output "box3_inkomen" equals 0
    And output "totale_heffingskortingen" equals 727646
    And output "totale_belastingschuld" equals 347017
