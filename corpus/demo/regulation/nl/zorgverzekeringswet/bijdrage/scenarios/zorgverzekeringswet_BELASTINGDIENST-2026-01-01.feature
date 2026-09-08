# Converted from belastingen/zorgverzekeringswet_BELASTINGDIENST-2026-01-01.feature by corpus/demo/tools/convert_features.mjs
Feature: Zorgverzekeringswet - Inkomensafhankelijke bijdrage
  Als belastingplichtige
  Wil ik weten wat mijn inkomensafhankelijke bijdrage Zvw is
  Zodat ik mijn zorgkosten kan plannen

  Background:
    Given the calculation date is "2026-07-01"

  Scenario: Werknemer met hoog tarief (werkgeversheffing)
    Given parameter "bsn" is "200000001"
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000001 | 1985-05-20    | null              | null        | []                | Amsterdam      | []             | null          | null          | null  | []           | null                  |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | 200000001 | 5000000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn       | verwachting_65 |
      | 200000001 | 20.5           |
    When I evaluate outputs "voldoet_aan_voorwaarden, bijdrage_type, bijdrage_inkomen, verschuldigde_bijdrage" of "zorgverzekeringswet/bijdrage"
    Then output "voldoet_aan_voorwaarden" is true
    And output "bijdrage_type" equals "HOOG"
    And output "bijdrage_inkomen" equals 5000000
    And output "verschuldigde_bijdrage" equals 325000

  Scenario: AOW-gerechtigde met laag tarief
    Given parameter "bsn" is "200000002"
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000002 | 1958-03-15    | null              | null        | []                | Rotterdam      | []             | null          | null          | null  | []           | null                  |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | 200000002 | 0                         | 3000000                   | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn       | verwachting_65 |
      | 200000002 | 20.5           |
    When I evaluate outputs "voldoet_aan_voorwaarden, bijdrage_type, is_aow_gerechtigd, verschuldigde_bijdrage" of "zorgverzekeringswet/bijdrage"
    Then output "voldoet_aan_voorwaarden" is true
    And output "bijdrage_type" equals "LAAG"
    And output "is_aow_gerechtigd" is true
    And output "verschuldigde_bijdrage" equals 157500

  Scenario: Zelfstandige zonder werkgever
    Given parameter "bsn" is "200000003"
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000003 | 1975-08-10    | null              | null        | []                | Utrecht        | []             | null          | null          | null  | []           | null                  |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | 200000003 | 0                         | 0                         | 7000000               | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn       | verwachting_65 |
      | 200000003 | 20.5           |
    When I evaluate outputs "voldoet_aan_voorwaarden, bijdrage_type, verschuldigde_bijdrage" of "zorgverzekeringswet/bijdrage"
    Then output "voldoet_aan_voorwaarden" is true
    And output "bijdrage_type" equals "HOOG"
    And output "verschuldigde_bijdrage" equals 455000

  Scenario: Werknemer met inkomen boven maximum
    Given parameter "bsn" is "200000004"
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000004 | 1980-01-01    | null              | null        | []                | Den Haag       | []             | null          | null          | null  | []           | null                  |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | 200000004 | 10000000                  | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn       | verwachting_65 |
      | 200000004 | 20.5           |
    When I evaluate outputs "voldoet_aan_voorwaarden, bijdrage_type, bijdrage_inkomen_begrensd, verschuldigde_bijdrage" of "zorgverzekeringswet/bijdrage"
    Then output "voldoet_aan_voorwaarden" is true
    And output "bijdrage_type" equals "HOOG"
    And output "bijdrage_inkomen_begrensd" equals 7541200
    And output "verschuldigde_bijdrage" equals 490178

  Scenario: Persoon zonder inkomen voldoet niet aan voorwaarden
    Given parameter "bsn" is "200000005"
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000005 | 1990-06-15    | null              | null        | []                | Eindhoven      | []             | null          | null          | null  | []           | null                  |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | 200000005 | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn       | verwachting_65 |
      | 200000005 | 20.5           |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "zorgverzekeringswet/bijdrage"
    Then output "voldoet_aan_voorwaarden" is false
