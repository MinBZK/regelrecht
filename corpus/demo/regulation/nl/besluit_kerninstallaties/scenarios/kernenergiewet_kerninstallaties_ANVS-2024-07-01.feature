# Converted from kernenergiewet/kernenergiewet_kerninstallaties_ANVS-2024-07-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Besluit kerninstallaties, splijtstoffen en ertsen
  Als nucleaire operator
  Wil ik weten of mijn kerninstallatie voldoet aan de administratieve eisen
  Zodat ik een vergunning kan krijgen

  Background:
    Given the calculation date is "2024-07-01"

  Scenario: Alle administratieve eisen voldaan
    Given the following parameters:
      | heeft_beveiligingsplan      | true      |
      | heeft_noodplan              | true      |
      | financiele_zekerheid_bedrag | 100000000 |
      | aantal_deskundigen          | 2         |
    # POC: parameter "heeft_beeindigingsplan" not provided by the scenario (None in the POC)
    And parameter "heeft_beeindigingsplan" is "null"
    When I evaluate outputs "financiele_zekerheid_gesteld, beveiligingsplan_voldoet, noodplan_voldoet, deskundigheid_voldoende, administratieve_eisen_voldaan" of "besluit_kerninstallaties"
    Then output "financiele_zekerheid_gesteld" is true
    And output "beveiligingsplan_voldoet" is true
    And output "noodplan_voldoet" is true
    And output "deskundigheid_voldoende" is true
    And output "administratieve_eisen_voldaan" is true

  Scenario: Financiële zekerheid te laag
    Given the following parameters:
      | heeft_beveiligingsplan      | true     |
      | heeft_noodplan              | true     |
      | financiele_zekerheid_bedrag | 50000000 |
      | aantal_deskundigen          | 2        |
    # POC: parameter "heeft_beeindigingsplan" not provided by the scenario (None in the POC)
    And parameter "heeft_beeindigingsplan" is "null"
    When I evaluate outputs "financiele_zekerheid_gesteld, administratieve_eisen_voldaan" of "besluit_kerninstallaties"
    Then output "financiele_zekerheid_gesteld" is false
    And output "administratieve_eisen_voldaan" is false

  Scenario: Geen beveiligingsplan
    Given the following parameters:
      | heeft_beveiligingsplan      | false     |
      | heeft_noodplan              | true      |
      | financiele_zekerheid_bedrag | 100000000 |
      | aantal_deskundigen          | 2         |
    # POC: parameter "heeft_beeindigingsplan" not provided by the scenario (None in the POC)
    And parameter "heeft_beeindigingsplan" is "null"
    When I evaluate outputs "beveiligingsplan_voldoet, administratieve_eisen_voldaan" of "besluit_kerninstallaties"
    Then output "beveiligingsplan_voldoet" is false
    And output "administratieve_eisen_voldaan" is false

  Scenario: Geen noodplan
    Given the following parameters:
      | heeft_beveiligingsplan      | true      |
      | heeft_noodplan              | false     |
      | financiele_zekerheid_bedrag | 100000000 |
      | aantal_deskundigen          | 2         |
    # POC: parameter "heeft_beeindigingsplan" not provided by the scenario (None in the POC)
    And parameter "heeft_beeindigingsplan" is "null"
    When I evaluate outputs "noodplan_voldoet, administratieve_eisen_voldaan" of "besluit_kerninstallaties"
    Then output "noodplan_voldoet" is false
    And output "administratieve_eisen_voldaan" is false

  Scenario: Onvoldoende deskundigen
    Given the following parameters:
      | heeft_beveiligingsplan      | true      |
      | heeft_noodplan              | true      |
      | financiele_zekerheid_bedrag | 100000000 |
      | aantal_deskundigen          | 0         |
    # POC: parameter "heeft_beeindigingsplan" not provided by the scenario (None in the POC)
    And parameter "heeft_beeindigingsplan" is "null"
    When I evaluate outputs "deskundigheid_voldoende, administratieve_eisen_voldaan" of "besluit_kerninstallaties"
    Then output "deskundigheid_voldoende" is false
    And output "administratieve_eisen_voldaan" is false

  Scenario: Meerdere eisen niet voldaan
    Given the following parameters:
      | heeft_beveiligingsplan      | false    |
      | heeft_noodplan              | false    |
      | financiele_zekerheid_bedrag | 10000000 |
      | aantal_deskundigen          | 0        |
    # POC: parameter "heeft_beeindigingsplan" not provided by the scenario (None in the POC)
    And parameter "heeft_beeindigingsplan" is "null"
    When I evaluate outputs "financiele_zekerheid_gesteld, beveiligingsplan_voldoet, noodplan_voldoet, deskundigheid_voldoende, administratieve_eisen_voldaan" of "besluit_kerninstallaties"
    Then output "financiele_zekerheid_gesteld" is false
    And output "beveiligingsplan_voldoet" is false
    And output "noodplan_voldoet" is false
    And output "deskundigheid_voldoende" is false
    And output "administratieve_eisen_voldaan" is false

  Scenario: Minimale financiële zekerheid precies voldoende
    Given the following parameters:
      | heeft_beveiligingsplan      | true      |
      | heeft_noodplan              | true      |
      | financiele_zekerheid_bedrag | 100000000 |
      | aantal_deskundigen          | 1         |
    # POC: parameter "heeft_beeindigingsplan" not provided by the scenario (None in the POC)
    And parameter "heeft_beeindigingsplan" is "null"
    When I evaluate outputs "financiele_zekerheid_gesteld, deskundigheid_voldoende, administratieve_eisen_voldaan" of "besluit_kerninstallaties"
    Then output "financiele_zekerheid_gesteld" is true
    And output "deskundigheid_voldoende" is true
    And output "administratieve_eisen_voldaan" is true

  Scenario: Ruim voldoende financiële zekerheid en deskundigen
    Given the following parameters:
      | heeft_beveiligingsplan      | true      |
      | heeft_noodplan              | true      |
      | financiele_zekerheid_bedrag | 500000000 |
      | aantal_deskundigen          | 10        |
    # POC: parameter "heeft_beeindigingsplan" not provided by the scenario (None in the POC)
    And parameter "heeft_beeindigingsplan" is "null"
    When I evaluate outputs "financiele_zekerheid_gesteld, deskundigheid_voldoende, administratieve_eisen_voldaan" of "besluit_kerninstallaties"
    Then output "financiele_zekerheid_gesteld" is true
    And output "deskundigheid_voldoende" is true
    And output "administratieve_eisen_voldaan" is true

  Scenario: Zeer hoge financiële zekerheid maar geen plannen
    Given the following parameters:
      | heeft_beveiligingsplan      | false      |
      | heeft_noodplan              | false      |
      | financiele_zekerheid_bedrag | 1000000000 |
      | aantal_deskundigen          | 5          |
    # POC: parameter "heeft_beeindigingsplan" not provided by the scenario (None in the POC)
    And parameter "heeft_beeindigingsplan" is "null"
    When I evaluate outputs "financiele_zekerheid_gesteld, deskundigheid_voldoende, beveiligingsplan_voldoet, noodplan_voldoet, administratieve_eisen_voldaan" of "besluit_kerninstallaties"
    Then output "financiele_zekerheid_gesteld" is true
    And output "deskundigheid_voldoende" is true
    And output "beveiligingsplan_voldoet" is false
    And output "noodplan_voldoet" is false
    And output "administratieve_eisen_voldaan" is false
