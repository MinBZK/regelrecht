# Converted from kernenergiewet/kernenergiewet_stralingsbescherming_ANVS-2024-07-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Besluit basisveiligheidsnormen stralingsbescherming
  Als nucleaire operator
  Wil ik weten of mijn verwachte stralingsdosis binnen de wettelijke limieten blijft
  Zodat ik een vergunning kan krijgen

  Background:
    Given the calculation date is "2024-07-01"

  Scenario: Stralingsdosis onder alle limieten
    Given the following parameters:
      | verwachte_dosis_jaar | 0.5  |
      | verwachte_dosis_huid | 20   |
      | dosis_buiten_locatie | 0.05 |
    When I evaluate outputs "dosislimiet_overschreden, vergunning_toegestaan_straling, overschreden_limieten" of "besluit_basisveiligheidsnormen_stralingsbescherming"
    Then output "dosislimiet_overschreden" is false
    And output "vergunning_toegestaan_straling" is true
    # POC: output is an empty list
    And output "overschreden_limieten" equals "[]"

  Scenario: Effectieve dosis net op de limiet (1 mSv/jaar)
    Given the following parameters:
      | verwachte_dosis_jaar | 1    |
      | verwachte_dosis_huid | 30   |
      | dosis_buiten_locatie | 0.08 |
    When I evaluate outputs "dosislimiet_overschreden, vergunning_toegestaan_straling, overschreden_limieten" of "besluit_basisveiligheidsnormen_stralingsbescherming"
    Then output "dosislimiet_overschreden" is false
    And output "vergunning_toegestaan_straling" is true
    # POC: output is an empty list
    And output "overschreden_limieten" equals "[]"

  Scenario: Effectieve dosis overschrijdt algemene limiet (>1 mSv/jaar)
    Given the following parameters:
      | verwachte_dosis_jaar | 1.5  |
      | verwachte_dosis_huid | 30   |
      | dosis_buiten_locatie | 0.08 |
    When I evaluate outputs "dosislimiet_overschreden, vergunning_toegestaan_straling" of "besluit_basisveiligheidsnormen_stralingsbescherming"
    Then output "dosislimiet_overschreden" is true
    And output "vergunning_toegestaan_straling" is false
    # POC: output "overschreden_limieten" contains effectieve_dosis_algemeen (array membership not expressible in the canonical grammar)

  Scenario: Dosis buiten locatie overschrijdt limiet (>0.1 mSv/jaar)
    Given the following parameters:
      | verwachte_dosis_jaar | 0.8  |
      | verwachte_dosis_huid | 40   |
      | dosis_buiten_locatie | 0.15 |
    When I evaluate outputs "dosislimiet_overschreden, vergunning_toegestaan_straling" of "besluit_basisveiligheidsnormen_stralingsbescherming"
    Then output "dosislimiet_overschreden" is true
    And output "vergunning_toegestaan_straling" is false
    # POC: output "overschreden_limieten" contains effectieve_dosis_buiten_locatie (array membership not expressible in the canonical grammar)

  Scenario: Huiddosis overschrijdt limiet (>50 mSv/jaar)
    Given the following parameters:
      | verwachte_dosis_jaar | 0.7  |
      | verwachte_dosis_huid | 55   |
      | dosis_buiten_locatie | 0.05 |
    When I evaluate outputs "dosislimiet_overschreden, vergunning_toegestaan_straling" of "besluit_basisveiligheidsnormen_stralingsbescherming"
    Then output "dosislimiet_overschreden" is true
    And output "vergunning_toegestaan_straling" is false
    # POC: output "overschreden_limieten" contains equivalente_dosis_huid (array membership not expressible in the canonical grammar)

  Scenario: Meerdere dosislimieten overschreden
    Given the following parameters:
      | verwachte_dosis_jaar | 2.5 |
      | verwachte_dosis_huid | 60  |
      | dosis_buiten_locatie | 0.3 |
    When I evaluate outputs "dosislimiet_overschreden, vergunning_toegestaan_straling" of "besluit_basisveiligheidsnormen_stralingsbescherming"
    Then output "dosislimiet_overschreden" is true
    And output "vergunning_toegestaan_straling" is false
    # POC: output "overschreden_limieten" contains effectieve_dosis_algemeen (array membership not expressible in the canonical grammar)

  Scenario: Zeer lage stralingsdosis (optimalisatie)
    Given the following parameters:
      | verwachte_dosis_jaar | 0.001  |
      | verwachte_dosis_huid | 0.1    |
      | dosis_buiten_locatie | 0.0001 |
    When I evaluate outputs "dosislimiet_overschreden, vergunning_toegestaan_straling, overschreden_limieten" of "besluit_basisveiligheidsnormen_stralingsbescherming"
    Then output "dosislimiet_overschreden" is false
    And output "vergunning_toegestaan_straling" is true
    # POC: output is an empty list
    And output "overschreden_limieten" equals "[]"

  Scenario: Alleen algemene dosis opgegeven (andere waarden niet ingevuld)
    Given the following parameters:
      | verwachte_dosis_jaar | 0.8 |
    # POC: parameters "verwachte_dosis_huid", "dosis_buiten_locatie" not provided by the scenario (None in the POC)
    And parameter "verwachte_dosis_huid" is "null"
    And parameter "dosis_buiten_locatie" is "null"
    When I evaluate outputs "dosislimiet_overschreden, vergunning_toegestaan_straling, overschreden_limieten" of "besluit_basisveiligheidsnormen_stralingsbescherming"
    Then output "dosislimiet_overschreden" is false
    And output "vergunning_toegestaan_straling" is true
    # POC: output is an empty list
    And output "overschreden_limieten" equals "[]"

  Scenario: Hoge algemene dosis met alleen algemene waarde opgegeven
    Given the following parameters:
      | verwachte_dosis_jaar | 1.2 |
    # POC: parameters "verwachte_dosis_huid", "dosis_buiten_locatie" not provided by the scenario (None in the POC)
    And parameter "verwachte_dosis_huid" is "null"
    And parameter "dosis_buiten_locatie" is "null"
    When I evaluate outputs "dosislimiet_overschreden, vergunning_toegestaan_straling" of "besluit_basisveiligheidsnormen_stralingsbescherming"
    Then output "dosislimiet_overschreden" is true
    And output "vergunning_toegestaan_straling" is false
    # POC: output "overschreden_limieten" contains effectieve_dosis_algemeen (array membership not expressible in the canonical grammar)
