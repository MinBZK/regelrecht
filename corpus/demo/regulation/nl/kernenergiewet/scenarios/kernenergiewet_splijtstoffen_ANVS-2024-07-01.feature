# Converted from kernenergiewet/kernenergiewet_splijtstoffen_ANVS-2024-07-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Kernenergiewet Vergunningverlening Splijtstoffen
  Als nucleaire operator
  Wil ik weten of ik een vergunning nodig heb en kan krijgen
  Zodat ik wettelijk mag werken met splijtstoffen

  Background:
    Given the calculation date is "2024-07-01"

  Scenario: Transport van splijtstoffen met voldoende dosis en financiële zekerheid
    Given the following parameters:
      | activiteit_type             | transport  |
      | materiaal_type              | splijtstof |
      | is_nieuwe_installatie       | false      |
      | verwachte_dosis_jaar        | 0.5        |
      | verwachte_dosis_huid        | 10         |
      | dosis_buiten_locatie        | 0.05       |
      | heeft_beveiligingsplan      | true       |
      | heeft_noodplan              | true       |
      | financiele_zekerheid_bedrag | 100000000  |
      | aantal_deskundigen          | 2          |
    # POC: parameter "technologie_beschrijving" not provided by the scenario (None in the POC)
    And parameter "technologie_beschrijving" is "null"
    When I evaluate outputs "vergunning_vereist, vergunning_toegestaan, weigeringsgronden" of "kernenergiewet"
    Then output "vergunning_vereist" is true
    And output "vergunning_toegestaan" is true
    # POC: output is an empty list
    And output "weigeringsgronden" equals "[]"

  Scenario: Bezit van splijtstoffen met te hoge stralingsdosis
    Given the following parameters:
      | activiteit_type             | bezit      |
      | materiaal_type              | splijtstof |
      | is_nieuwe_installatie       | false      |
      | verwachte_dosis_jaar        | 2.5        |
      | verwachte_dosis_huid        | 10         |
      | dosis_buiten_locatie        | 0.05       |
      | heeft_beveiligingsplan      | true       |
      | heeft_noodplan              | true       |
      | financiele_zekerheid_bedrag | 100000000  |
      | aantal_deskundigen          | 2          |
    # POC: parameter "technologie_beschrijving" not provided by the scenario (None in the POC)
    And parameter "technologie_beschrijving" is "null"
    When I evaluate outputs "vergunning_vereist, vergunning_toegestaan" of "kernenergiewet"
    Then output "vergunning_vereist" is true
    And output "vergunning_toegestaan" is false
    # POC: output "weigeringsgronden" contains bescherming_mensen_dieren_planten_goederen (array membership not expressible in the canonical grammar)

  Scenario: Nieuwe kerninstallatie zonder financiële zekerheid
    Given the following parameters:
      | activiteit_type             | inrichting_oprichten |
      | materiaal_type              | splijtstof           |
      | is_nieuwe_installatie       | true                 |
      | verwachte_dosis_jaar        | 0.5                  |
      | verwachte_dosis_huid        | 10                   |
      | dosis_buiten_locatie        | 0.05                 |
      | heeft_beveiligingsplan      | true                 |
      | heeft_noodplan              | true                 |
      | financiele_zekerheid_bedrag | 50000000             |
      | aantal_deskundigen          | 2                    |
    # POC: parameter "technologie_beschrijving" not provided by the scenario (None in the POC)
    And parameter "technologie_beschrijving" is "null"
    When I evaluate outputs "vergunning_vereist, vergunning_toegestaan" of "kernenergiewet"
    Then output "vergunning_vereist" is true
    And output "vergunning_toegestaan" is false
    # POC: output "weigeringsgronden" contains zekerheid_betaling_schadevergoeding (array membership not expressible in the canonical grammar)

  Scenario: Exploitatie van kerninstallatie met alle voorwaarden voldaan
    Given the following parameters:
      | activiteit_type             | inrichting_exploiteren |
      | materiaal_type              | splijtstof             |
      | is_nieuwe_installatie       | false                  |
      | verwachte_dosis_jaar        | 0.3                    |
      | verwachte_dosis_huid        | 5                      |
      | dosis_buiten_locatie        | 0.02                   |
      | heeft_beveiligingsplan      | true                   |
      | heeft_noodplan              | true                   |
      | financiele_zekerheid_bedrag | 200000000              |
      | aantal_deskundigen          | 5                      |
    # POC: parameter "technologie_beschrijving" not provided by the scenario (None in the POC)
    And parameter "technologie_beschrijving" is "null"
    When I evaluate outputs "vergunning_vereist, vergunning_toegestaan, weigeringsgronden" of "kernenergiewet"
    Then output "vergunning_vereist" is true
    And output "vergunning_toegestaan" is true
    # POC: output is an empty list
    And output "weigeringsgronden" equals "[]"

  Scenario: Transport van ertsen met lage stralingsdosis
    Given the following parameters:
      | activiteit_type             | transport |
      | materiaal_type              | erts      |
      | is_nieuwe_installatie       | false     |
      | verwachte_dosis_jaar        | 0.1       |
      | verwachte_dosis_huid        | 1         |
      | dosis_buiten_locatie        | 0.01      |
      | heeft_beveiligingsplan      | true      |
      | heeft_noodplan              | true      |
      | financiele_zekerheid_bedrag | 100000000 |
      | aantal_deskundigen          | 1         |
    # POC: parameter "technologie_beschrijving" not provided by the scenario (None in the POC)
    And parameter "technologie_beschrijving" is "null"
    When I evaluate outputs "vergunning_vereist, vergunning_toegestaan, weigeringsgronden" of "kernenergiewet"
    Then output "vergunning_vereist" is true
    And output "vergunning_toegestaan" is true
    # POC: output is an empty list
    And output "weigeringsgronden" equals "[]"

  Scenario: Meerdere weigeringsgronden tegelijk (hoge dosis en geen financiële zekerheid)
    Given the following parameters:
      | activiteit_type             | invoer     |
      | materiaal_type              | splijtstof |
      | is_nieuwe_installatie       | false      |
      | verwachte_dosis_jaar        | 3          |
      | verwachte_dosis_huid        | 60         |
      | dosis_buiten_locatie        | 0.5        |
      | heeft_beveiligingsplan      | true       |
      | heeft_noodplan              | true       |
      | financiele_zekerheid_bedrag | 10000000   |
      | aantal_deskundigen          | 1          |
    # POC: parameter "technologie_beschrijving" not provided by the scenario (None in the POC)
    And parameter "technologie_beschrijving" is "null"
    When I evaluate outputs "vergunning_vereist, vergunning_toegestaan" of "kernenergiewet"
    Then output "vergunning_vereist" is true
    And output "vergunning_toegestaan" is false
    # POC: output "weigeringsgronden" contains bescherming_mensen_dieren_planten_goederen (array membership not expressible in the canonical grammar)
