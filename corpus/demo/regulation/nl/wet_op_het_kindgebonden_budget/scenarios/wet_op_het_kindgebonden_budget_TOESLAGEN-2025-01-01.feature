# Converted from toeslagen/wet_op_het_kindgebonden_budget_TOESLAGEN-2025-01-01.feature by corpus/demo/tools/convert_features.mjs
Feature: Berekening Kindgebonden Budget
  Als ouder
  Wil ik weten of ik recht heb op kindgebonden budget
  Zodat ik financiële ondersteuning krijg voor mijn kinderen

  Background:
    Given the calculation date is "2025-02-01"

  Scenario: Alleenstaande ouder met 1 kind krijgt basisbedrag + ALO-kop
    Given parameter "bsn" is "200000001"
    And the following "SVB" data with key "bsn" for law "algemene_kinderbijslagwet":
      | bsn       | kinderen_data                                                                 |
      | 200000001 | {"aantal_kinderen":1,"kinderen_leeftijden":[5],"ontvangt_kinderbijslag":true} |
    And the following "BELASTINGDIENST" data with key "bsn" for law "belastingdienst_vermogen":
      | bsn       | vermogensgegevens    |
      | 200000001 | {"vermogen":5000000} |
    And the following "UWV" data with key "bsn" for law "uwv_toetsingsinkomen":
      | bsn       | inkomensgegevens             |
      | 200000001 | {"toetsingsinkomen":2500000} |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000001 | 1988-04-12    | GEEN              | null        | []                | null           | []             | null          | null          | null  | []           | null                  |
    When I evaluate outputs "voldoet_aan_voorwaarden, alo_kop_bedrag, kindgebonden_budget_jaar" of "wet_op_het_kindgebonden_budget"
    Then output "voldoet_aan_voorwaarden" is true
    And output "alo_kop_bedrag" equals 348000
    And output "kindgebonden_budget_jaar" equals 599100

  Scenario: Paar met 2 kinderen krijgt aangepast bedrag zonder ALO-kop
    Given parameter "bsn" is "200000002"
    And the following "SVB" data with key "bsn" for law "algemene_kinderbijslagwet":
      | bsn       | kinderen_data                                                                    |
      | 200000002 | {"aantal_kinderen":2,"kinderen_leeftijden":[7,10],"ontvangt_kinderbijslag":true} |
      | 200000003 | null                                                                             |
    And the following "BELASTINGDIENST" data with key "bsn" for law "belastingdienst_vermogen":
      | bsn       | vermogensgegevens    |
      | 200000002 | {"vermogen":8000000} |
      | 200000003 | {"vermogen":7000000} |
    And the following "UWV" data with key "bsn" for law "uwv_toetsingsinkomen":
      | bsn       | inkomensgegevens             |
      | 200000002 | {"toetsingsinkomen":3500000} |
      | 200000003 | {"toetsingsinkomen":3000000} |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000002 | 1985-09-22    | HUWELIJK          | 200000003   | []                | null           | []             | null          | null          | null  | []           | null                  |
      | 200000003 | null          | null              | null        | []                | null           | []             | null          | null          | null  | []           | null                  |
    When I evaluate outputs "voldoet_aan_voorwaarden, alo_kop_bedrag, kindgebonden_budget_jaar" of "wet_op_het_kindgebonden_budget"
    Then output "voldoet_aan_voorwaarden" is true
    And output "alo_kop_bedrag" equals 0
    # POC asserted approximately €3.925,00 per year (2% tolerance); value adopted from the engine
    And output "kindgebonden_budget_jaar" equals 392541

  Scenario: Alleenstaande met inkomen boven grens krijgt geen kindgebonden budget
    Given parameter "bsn" is "200000004"
    And the following "SVB" data with key "bsn" for law "algemene_kinderbijslagwet":
      | bsn       | kinderen_data                                                                 |
      | 200000004 | {"aantal_kinderen":1,"kinderen_leeftijden":[8],"ontvangt_kinderbijslag":true} |
    And the following "BELASTINGDIENST" data with key "bsn" for law "belastingdienst_vermogen":
      | bsn       | vermogensgegevens    |
      | 200000004 | {"vermogen":5000000} |
    And the following "UWV" data with key "bsn" for law "uwv_toetsingsinkomen":
      | bsn       | inkomensgegevens              |
      | 200000004 | {"toetsingsinkomen":12000000} |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000004 | 1982-11-30    | GEEN              | null        | []                | null           | []             | null          | null          | null  | []           | null                  |
    When I evaluate outputs "voldoet_aan_voorwaarden, kindgebonden_budget_jaar" of "wet_op_het_kindgebonden_budget"
    Then output "voldoet_aan_voorwaarden" is true
    And output "kindgebonden_budget_jaar" equals 0

  Scenario: Alleenstaande met kind krijgt basisbedrag
    Given parameter "bsn" is "200000005"
    And the following "SVB" data with key "bsn" for law "algemene_kinderbijslagwet":
      | bsn       | kinderen_data                                                                  |
      | 200000005 | {"aantal_kinderen":1,"kinderen_leeftijden":[10],"ontvangt_kinderbijslag":true} |
    And the following "BELASTINGDIENST" data with key "bsn" for law "belastingdienst_vermogen":
      | bsn       | vermogensgegevens    |
      | 200000005 | {"vermogen":3000000} |
    And the following "UWV" data with key "bsn" for law "uwv_toetsingsinkomen":
      | bsn       | inkomensgegevens             |
      | 200000005 | {"toetsingsinkomen":2200000} |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000005 | 1990-01-15    | GEEN              | null        | []                | null           | []             | null          | null          | null  | []           | null                  |
    When I evaluate outputs "voldoet_aan_voorwaarden, kindgebonden_budget_jaar" of "wet_op_het_kindgebonden_budget"
    Then output "voldoet_aan_voorwaarden" is true
    And output "kindgebonden_budget_jaar" equals 599100

  Scenario: Alleenstaande met kind en hoger inkomen
    Given parameter "bsn" is "200000006"
    And the following "SVB" data with key "bsn" for law "algemene_kinderbijslagwet":
      | bsn       | kinderen_data                                                                 |
      | 200000006 | {"aantal_kinderen":1,"kinderen_leeftijden":[8],"ontvangt_kinderbijslag":true} |
    And the following "BELASTINGDIENST" data with key "bsn" for law "belastingdienst_vermogen":
      | bsn       | vermogensgegevens    |
      | 200000006 | {"vermogen":4000000} |
    And the following "UWV" data with key "bsn" for law "uwv_toetsingsinkomen":
      | bsn       | inkomensgegevens             |
      | 200000006 | {"toetsingsinkomen":2400000} |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000006 | 1987-06-20    | GEEN              | null        | []                | null           | []             | null          | null          | null  | []           | null                  |
    When I evaluate outputs "voldoet_aan_voorwaarden, kindgebonden_budget_jaar" of "wet_op_het_kindgebonden_budget"
    Then output "voldoet_aan_voorwaarden" is true
    And output "kindgebonden_budget_jaar" equals 599100

  Scenario: Geen kinderbijslag betekent geen kindgebonden budget
    Given parameter "bsn" is "200000007"
    And the following "SVB" data with key "bsn" for law "algemene_kinderbijslagwet":
      | bsn       | kinderen_data                                                                 |
      | 200000007 | {"aantal_kinderen":0,"kinderen_leeftijden":[],"ontvangt_kinderbijslag":false} |
    And the following "BELASTINGDIENST" data with key "bsn" for law "belastingdienst_vermogen":
      | bsn       | vermogensgegevens    |
      | 200000007 | {"vermogen":2000000} |
    And the following "UWV" data with key "bsn" for law "uwv_toetsingsinkomen":
      | bsn       | inkomensgegevens             |
      | 200000007 | {"toetsingsinkomen":2000000} |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000007 | 1995-03-08    | GEEN              | null        | []                | null           | []             | null          | null          | null  | []           | null                  |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "wet_op_het_kindgebonden_budget"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Vermogen boven grens betekent geen kindgebonden budget
    Given parameter "bsn" is "200000008"
    And the following "SVB" data with key "bsn" for law "algemene_kinderbijslagwet":
      | bsn       | kinderen_data                                                                 |
      | 200000008 | {"aantal_kinderen":1,"kinderen_leeftijden":[6],"ontvangt_kinderbijslag":true} |
    And the following "BELASTINGDIENST" data with key "bsn" for law "belastingdienst_vermogen":
      | bsn       | vermogensgegevens     |
      | 200000008 | {"vermogen":15000000} |
    And the following "UWV" data with key "bsn" for law "uwv_toetsingsinkomen":
      | bsn       | inkomensgegevens             |
      | 200000008 | {"toetsingsinkomen":2500000} |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000008 | 1983-12-05    | GEEN              | null        | []                | null           | []             | null          | null          | null  | []           | null                  |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "wet_op_het_kindgebonden_budget"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Alleenstaande met laag inkomen krijgt maximaal kindgebonden budget
    Given parameter "bsn" is "200000009"
    And the following "SVB" data with key "bsn" for law "algemene_kinderbijslagwet":
      | bsn       | kinderen_data                                                                 |
      | 200000009 | {"aantal_kinderen":1,"kinderen_leeftijden":[4],"ontvangt_kinderbijslag":true} |
    And the following "BELASTINGDIENST" data with key "bsn" for law "belastingdienst_vermogen":
      | bsn       | vermogensgegevens    |
      | 200000009 | {"vermogen":1000000} |
    And the following "UWV" data with key "bsn" for law "uwv_toetsingsinkomen":
      | bsn       | inkomensgegevens             |
      | 200000009 | {"toetsingsinkomen":1500000} |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | verblijfsadres | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 200000009 | 1991-08-18    | GEEN              | null        | []                | null           | []             | null          | null          | null  | []           | null                  |
    When I evaluate outputs "voldoet_aan_voorwaarden, alo_kop_bedrag, kindgebonden_budget_jaar" of "wet_op_het_kindgebonden_budget"
    Then output "voldoet_aan_voorwaarden" is true
    And output "alo_kop_bedrag" equals 348000
    And output "kindgebonden_budget_jaar" equals 599100
