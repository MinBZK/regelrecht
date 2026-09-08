# Converted from burgerlijk_wetboek/burgerlijk_wetboek_handelingsonbekwaamheid_RECHTSPRAAK-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
Feature: Burgerlijk Wetboek Handelingsonbekwaamheid (BW 1:378-391)
  Als RECHTSPRAAK
  Wil ik bepalen of een persoon handelingsonbekwaam is
  Zodat hun rechten correct worden beperkt

  Background:
    Given the calculation date is "2025-03-01"

  Scenario: Persoon onder curatele is handelingsonbekwaam
    Given parameter "bsn" is "500000001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus                                                                                                                                    |
      | 500000001 | [{"bsn_curator":"400100001","bsn_curandus":"500000001","naam_curandus":"Sophie van Dam","datum_ingang":"2022-01-15","datum_einde":"","status":"ACTIEF"}] |
      | 400100001 | []                                                                                                                                                       |
    When I evaluate outputs "is_onder_curatele, is_handelingsonbekwaam" of "burgerlijk_wetboek_handelingsonbekwaamheid"
    Then output "is_onder_curatele" is true
    And output "is_handelingsonbekwaam" is true
    # POC: output "base_permissions" contains LEZEN (array membership not expressible in the canonical grammar)
    # POC: output "base_permissions" does not contain CLAIMS_INDIENEN (negative membership not expressible in the canonical grammar)
    # POC: output "base_permissions" does not contain BESLUITEN_ONTVANGEN (negative membership not expressible in the canonical grammar)

  Scenario: Persoon zonder curatele is handelingsbekwaam
    Given parameter "bsn" is "999993653"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus                                                                                                                                    |
      | 999993653 | []                                                                                                                                                       |
      | 400100001 | []                                                                                                                                                       |
      | 500000001 | [{"bsn_curator":"400100001","bsn_curandus":"500000001","naam_curandus":"Sophie van Dam","datum_ingang":"2022-01-15","datum_einde":"","status":"ACTIEF"}] |
    When I evaluate outputs "is_onder_curatele, is_handelingsonbekwaam" of "burgerlijk_wetboek_handelingsonbekwaamheid"
    Then output "is_onder_curatele" is false
    And output "is_handelingsonbekwaam" is false
    # POC: output "base_permissions" contains LEZEN (array membership not expressible in the canonical grammar)
    # POC: output "base_permissions" contains CLAIMS_INDIENEN (array membership not expressible in the canonical grammar)
    # POC: output "base_permissions" contains BESLUITEN_ONTVANGEN (array membership not expressible in the canonical grammar)

  Scenario: Beeindigde curatele - persoon is weer handelingsbekwaam
    Given parameter "bsn" is "500000009"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus                                                                                                                                              |
      | 500000009 | [{"bsn_curator":"400100007","bsn_curandus":"500000009","naam_curandus":"Jan de Boer","datum_ingang":"2020-01-01","datum_einde":"2023-12-31","status":"BEEINDIGD"}] |
      | 400100007 | []                                                                                                                                                                 |
    When I evaluate outputs "is_onder_curatele, is_handelingsonbekwaam" of "burgerlijk_wetboek_handelingsonbekwaamheid"
    Then output "is_onder_curatele" is false
    And output "is_handelingsonbekwaam" is false
    # POC: output "base_permissions" contains CLAIMS_INDIENEN (array membership not expressible in the canonical grammar)

  Scenario: Curatele met einddatum in verleden - persoon is weer handelingsbekwaam
    Given parameter "bsn" is "500000001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus                                                                                                                                              |
      | 500000001 | [{"bsn_curator":"400100001","bsn_curandus":"500000001","naam_curandus":"Sophie van Dam","datum_ingang":"2020-01-01","datum_einde":"2024-12-31","status":"ACTIEF"}] |
      | 400100001 | []                                                                                                                                                                 |
    When I evaluate outputs "is_onder_curatele, is_handelingsonbekwaam" of "burgerlijk_wetboek_handelingsonbekwaamheid"
    Then output "is_onder_curatele" is false
    And output "is_handelingsonbekwaam" is false
    # POC: output "base_permissions" contains CLAIMS_INDIENEN (array membership not expressible in the canonical grammar)

  Scenario: Curatele met toekomstige einddatum - nog handelingsonbekwaam
    Given parameter "bsn" is "500000001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus                                                                                                                                              |
      | 500000001 | [{"bsn_curator":"400100001","bsn_curandus":"500000001","naam_curandus":"Sophie van Dam","datum_ingang":"2022-01-15","datum_einde":"2026-12-31","status":"ACTIEF"}] |
      | 400100001 | []                                                                                                                                                                 |
    When I evaluate outputs "is_onder_curatele, is_handelingsonbekwaam" of "burgerlijk_wetboek_handelingsonbekwaamheid"
    Then output "is_onder_curatele" is true
    And output "is_handelingsonbekwaam" is true
    # POC: output "base_permissions" contains LEZEN (array membership not expressible in the canonical grammar)
    # POC: output "base_permissions" does not contain CLAIMS_INDIENEN (negative membership not expressible in the canonical grammar)

  Scenario: Handelingsonbekwame heeft SELF delegatie met alleen leesrecht
    Given parameter "bsn" is "500000001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus                                                                                                                                    |
      | 500000001 | [{"bsn_curator":"400100001","bsn_curandus":"500000001","naam_curandus":"Sophie van Dam","datum_ingang":"2022-01-15","datum_einde":"","status":"ACTIEF"}] |
      | 400100001 | []                                                                                                                                                       |
    When I evaluate outputs "heeft_delegaties" of "burgerlijk_wetboek_handelingsonbekwaamheid"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 500000001 (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains SELF (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains EIGEN_ZAKEN (array membership not expressible in the canonical grammar)
    # POC: output "permissions" contains ['LEZEN'] (array membership not expressible in the canonical grammar)

  Scenario: Handelingsbekwame heeft SELF delegatie met volledige rechten
    Given parameter "bsn" is "999993653"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus                                                                                                                                    |
      | 999993653 | []                                                                                                                                                       |
      | 400100001 | []                                                                                                                                                       |
      | 500000001 | [{"bsn_curator":"400100001","bsn_curandus":"500000001","naam_curandus":"Sophie van Dam","datum_ingang":"2022-01-15","datum_einde":"","status":"ACTIEF"}] |
    When I evaluate outputs "heeft_delegaties" of "burgerlijk_wetboek_handelingsonbekwaamheid"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999993653 (array membership not expressible in the canonical grammar)
    # POC: output "permissions" contains ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'] (array membership not expressible in the canonical grammar)
