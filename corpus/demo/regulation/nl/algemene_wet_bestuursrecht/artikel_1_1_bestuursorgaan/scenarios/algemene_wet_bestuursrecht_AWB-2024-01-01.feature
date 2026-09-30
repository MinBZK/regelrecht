# Converted from bestuursrecht/algemene_wet_bestuursrecht_AWB-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: AWB Article 1:1 - Bestuursorgaan Definition
  Als burger of organisatie
  Wil ik weten of een bepaalde instantie een bestuursorgaan is
  Zodat ik weet of de AWB van toepassing is

  Background:
    Given the calculation date is "2024-01-01"

  Scenario: Gemeente is bestuursorgaan (A-orgaan)
    Given parameter "organisatie_id" is "ORG001"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG001         | true                        | GEMEENTE                              | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is true

  Scenario: Provincie is bestuursorgaan (A-orgaan)
    Given parameter "organisatie_id" is "ORG002"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG002         | true                        | PROVINCIE                             | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is true

  Scenario: Waterschap is bestuursorgaan (A-orgaan)
    Given parameter "organisatie_id" is "ORG003"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG003         | true                        | WATERSCHAP                            | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is true

  Scenario: Rijksorgaan is bestuursorgaan (A-orgaan)
    Given parameter "organisatie_id" is "ORG004"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG004         | true                        | RIJK                                  | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is true

  Scenario: Openbaar lichaam is bestuursorgaan (A-orgaan)
    Given parameter "organisatie_id" is "ORG005"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG005         | true                        | OPENBAAR_LICHAAM                      | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is true

  Scenario: APK-keuringsinstantie is bestuursorgaan (B-orgaan)
    Given parameter "organisatie_id" is "ORG010"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG010         | null                        | null                                  | true                                 | true                             | 0                                | null                           | false                               | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is true

  Scenario: Notaris met publieke bevoegdheid is bestuursorgaan (B-orgaan)
    Given parameter "organisatie_id" is "ORG011"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG011         | null                        | null                                  | true                                 | true                             | 0                                | null                           | false                               | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is true

  Scenario: Stichting met wettelijke toezichtstaak is bestuursorgaan (B-orgaan)
    Given parameter "organisatie_id" is "ORG012"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG012         | null                        | null                                  | true                                 | true                             | 0                                | null                           | false                               | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is true

  Scenario: Uitkeringsinstantie met 70% overheidsfinanciering is bestuursorgaan
    Given parameter "organisatie_id" is "ORG020"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG020         | null                        | null                                  | true                                 | true                             | 70                               | true                           | true                                | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is true

  Scenario: Uitkeringsinstantie met precies 66.67% financiering is bestuursorgaan
    Given parameter "organisatie_id" is "ORG021"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG021         | null                        | null                                  | true                                 | true                             | 66.67                            | true                           | true                                | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is true

  Scenario: Uitkeringsinstantie met 60% overheidsfinanciering is geen bestuursorgaan
    Given parameter "organisatie_id" is "ORG022"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG022         | null                        | null                                  | true                                 | true                             | 60                               | true                           | true                                | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false

  Scenario: Uitkeringsinstantie met 70% financiering maar eigen criteria is geen bestuursorgaan
    Given parameter "organisatie_id" is "ORG023"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG023         | null                        | null                                  | true                                 | true                             | 70                               | false                          | true                                | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false

  Scenario: Tweede Kamer is geen bestuursorgaan (volksvertegenwoordiging)
    Given parameter "organisatie_id" is "ORG030"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG030         | null                        | null                                  | null                                 | null                             | 0                                | null                           | null                                | true                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false

  Scenario: Rechtbank is geen bestuursorgaan (rechterlijk orgaan)
    Given parameter "organisatie_id" is "ORG031"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG031         | null                        | null                                  | null                                 | null                             | 0                                | null                           | null                                | null                              | true                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false

  Scenario: Nationale Ombudsman is geen bestuursorgaan (expliciet uitgezonderd)
    Given parameter "organisatie_id" is "ORG032"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG032         | null                        | null                                  | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | true         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false

  Scenario: Algemene Rekenkamer is geen bestuursorgaan (expliciet uitgezonderd)
    Given parameter "organisatie_id" is "ORG033"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG033         | null                        | null                                  | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | null         | true | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false

  Scenario: Privaat bedrijf zonder publieke bevoegdheid is geen bestuursorgaan
    Given parameter "organisatie_id" is "ORG040"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG040         | null                        | null                                  | false                                | null                             | 0                                | null                           | null                                | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false

  Scenario: Organisatie met publieke bevoegdheid niet bij wet is geen bestuursorgaan
    Given parameter "organisatie_id" is "ORG041"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG041         | null                        | null                                  | true                                 | false                            | 0                                | null                           | null                                | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false

  Scenario: ZBO met publieke bevoegdheid is bestuursorgaan (B-orgaan)
    Given parameter "organisatie_id" is "ORG042"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG042         | null                        | null                                  | true                                 | true                             | 0                                | null                           | false                               | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is true

  Scenario: Havenbedrijf Rotterdam is geen bestuursorgaan (privatized port authority)
    Given parameter "organisatie_id" is "ORG050"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG050         | null                        | null                                  | false                                | false                            | 0                                | null                           | null                                | null                              | null                  | null         | null | null | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false

  Scenario: Wetgevende macht is geen bestuursorgaan (lid 2 onder a)
    Given parameter "organisatie_id" is "ORG060"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG060         | true                        | RIJK                                  | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | null         | null | true | null | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false
    And output "exclusion_reason" equals "WETGEVENDE_MACHT"

  Scenario: Raad van State is geen bestuursorgaan (lid 2 onder d)
    Given parameter "organisatie_id" is "ORG061"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG061         | true                        | RIJK                                  | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | null         | null | null | true | null | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false
    And output "exclusion_reason" equals "RAAD_VAN_STATE"

  Scenario: Procureur-generaal bij de Hoge Raad is geen bestuursorgaan (lid 2 onder g)
    Given parameter "organisatie_id" is "ORG062"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG062         | true                        | RIJK                                  | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | null         | null | null | null | true | null | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false
    And output "exclusion_reason" equals "FUNCTIONARIS_UITGEZONDERD_ORGAAN"

  Scenario: CTIVD is geen bestuursorgaan (lid 2 onder h)
    Given parameter "organisatie_id" is "ORG063"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG063         | true                        | RIJK                                  | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | null         | null | null | null | null | true | null | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false
    And output "exclusion_reason" equals "CTIVD"

  Scenario: TIB is geen bestuursorgaan (lid 2 onder i)
    Given parameter "organisatie_id" is "ORG064"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG064         | true                        | RIJK                                  | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | null         | null | null | null | null | null | true | null | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false
    And output "exclusion_reason" equals "TIB"

  Scenario: Uitgezonderd orgaan is toch bestuursorgaan voor ambtenarenrechtelijk besluit over eigen personeel (lid 3)
    Given parameter "organisatie_id" is "ORG065"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG065         | true                        | RIJK                                  | null                                 | null                             | 0                                | null                           | null                                | null                              | true                  | null         | null | null | null | null | null | null | true | null |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is true

  Scenario: Voor het leven benoemde ambtenaar bij de Raad van State blijft uitgezonderd (lid 3 terug-uitzondering)
    Given parameter "organisatie_id" is "ORG067"
    And the following "AWB" data with key "organisatie_id" for law "algemene_wet_bestuursrecht":
      | organisatie_id | is_orgaan_van_rechtspersoon | publiekrechtelijke_rechtspersoon_type | heeft_publiekrechtelijke_bevoegdheid | bevoegdheid_bij_of_krachtens_wet | overheidsfinanciering_percentage | criteria_door_overheid_bepaald | is_uitsluitend_financiele_uitkering | is_volksvertegenwoordigend_orgaan | is_rechterlijk_orgaan | is_ombudsman | is_algemene_rekenkamer | is_wetgevende_macht | is_raad_van_state | is_functionaris_uitgezonderd_orgaan | is_ctivd | is_tib | is_ambtenarenrechtelijk_besluit_eigen_personeel | is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer |
      | ORG067         | true                        | RIJK                                  | null                                 | null                             | 0                                | null                           | null                                | null                              | null                  | null         | null | null | true | null | null | null | true | true |
    When I evaluate outputs "is_bestuursorgaan" of "algemene_wet_bestuursrecht"
    Then output "is_bestuursorgaan" is false
