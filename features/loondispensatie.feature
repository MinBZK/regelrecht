Feature: Loondispensatie (Wajong artikel 2:20)
  Als werkgever van een Wajong-gerechtigde
  Wil ik weten of ik loondispensatie kan aanvragen
  Zodat ik de beloning evenredig kan verlagen volgens de
  loonwaardevaststelling van UWV

  # Bron: Wajong artikel 2:20 lid 1 + 2 (BWBR0008657).
  # Peildatum 2025-01-01.

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Wajong-gerechtigde met UWV-vaststelling en aanvraag krijgt loondispensatie
    Given a citizen with the following data:
      | bsn                                                | 999990040 |
      | is_wsw_werknemer                                   | false     |
      | arbeidsprestatie_duidelijk_minder_dan_minimumloon  | true      |
      | aanvraag_loondispensatie_ingediend                 | true      |
      | heeft_recht_op_arbeidsondersteuning_wajong         | true      |
    When the law "wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten" is executed for outputs "heeft_recht_op_loondispensatie,beding_lagere_beloning_is_nietig"
    Then the execution succeeds
    And the output "heeft_recht_op_loondispensatie" is "true"
    And the output "beding_lagere_beloning_is_nietig" is "true"

  Scenario: Niet-Wajong-gerechtigde krijgt geen loondispensatie
    Given a citizen with the following data:
      | bsn                                                | 999990041 |
      | is_wsw_werknemer                                   | false     |
      | arbeidsprestatie_duidelijk_minder_dan_minimumloon  | true      |
      | aanvraag_loondispensatie_ingediend                 | true      |
      | heeft_recht_op_arbeidsondersteuning_wajong         | false     |
    When the law "wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten" is executed for outputs "heeft_recht_op_loondispensatie"
    Then the execution succeeds
    And the output "heeft_recht_op_loondispensatie" is "false"

  Scenario: Wsw-werknemer is uitgesloten van loondispensatie
    Given a citizen with the following data:
      | bsn                                                | 999990042 |
      | is_wsw_werknemer                                   | true      |
      | arbeidsprestatie_duidelijk_minder_dan_minimumloon  | true      |
      | aanvraag_loondispensatie_ingediend                 | true      |
      | heeft_recht_op_arbeidsondersteuning_wajong         | true      |
    When the law "wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten" is executed for outputs "heeft_recht_op_loondispensatie"
    Then the execution succeeds
    And the output "heeft_recht_op_loondispensatie" is "false"

  Scenario: Zonder UWV-vaststelling van prestatieverlaging geen recht
    Given a citizen with the following data:
      | bsn                                                | 999990043 |
      | is_wsw_werknemer                                   | false     |
      | arbeidsprestatie_duidelijk_minder_dan_minimumloon  | false     |
      | aanvraag_loondispensatie_ingediend                 | true      |
      | heeft_recht_op_arbeidsondersteuning_wajong         | true      |
    When the law "wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten" is executed for outputs "heeft_recht_op_loondispensatie"
    Then the execution succeeds
    And the output "heeft_recht_op_loondispensatie" is "false"

  Scenario: Zonder aanvraag geen recht (lid 1 vereist verzoek werkgever of werknemer)
    Given a citizen with the following data:
      | bsn                                                | 999990044 |
      | is_wsw_werknemer                                   | false     |
      | arbeidsprestatie_duidelijk_minder_dan_minimumloon  | true      |
      | aanvraag_loondispensatie_ingediend                 | false     |
      | heeft_recht_op_arbeidsondersteuning_wajong         | true      |
    When the law "wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten" is executed for outputs "heeft_recht_op_loondispensatie"
    Then the execution succeeds
    And the output "heeft_recht_op_loondispensatie" is "false"
