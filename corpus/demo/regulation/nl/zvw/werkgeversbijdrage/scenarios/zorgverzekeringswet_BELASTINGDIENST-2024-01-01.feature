# Converted from belastingen/zorgverzekeringswet_BELASTINGDIENST-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Berekening Werkgeversbijdrage Zorgverzekeringswet 2024
  Als werkgever
  Wil ik weten hoeveel werkgeversbijdrage Zvw ik moet afdragen
  Zodat ik de juiste premies kan afdragen aan de Belastingdienst

  Background:
    Given the calculation date is "2024-06-01"

  Scenario: Werknemer met modaal inkomen - volledige berekening
    Given parameter "loonheffingennummer" is "123456789L01"
    And parameter "bruto_loon" is 40000
    When I evaluate outputs "bijdrage_inkomen, zvw_werkgeversbijdrage" of "zvw/werkgeversbijdrage"
    Then output "bijdrage_inkomen" equals 4000000
    And output "zvw_werkgeversbijdrage" equals 262800

  Scenario: Werknemer met laag inkomen (minimumloon)
    Given parameter "loonheffingennummer" is "123456789L01"
    And parameter "bruto_loon" is 24000
    When I evaluate outputs "bijdrage_inkomen, zvw_werkgeversbijdrage" of "zvw/werkgeversbijdrage"
    Then output "bijdrage_inkomen" equals 2400000
    And output "zvw_werkgeversbijdrage" equals 157680

  Scenario: Werknemer met hoog inkomen - aftopping op maximum bijdrage-inkomen
    Given parameter "loonheffingennummer" is "123456789L01"
    And parameter "bruto_loon" is 100000
    When I evaluate outputs "bijdrage_inkomen, zvw_werkgeversbijdrage" of "zvw/werkgeversbijdrage"
    Then output "bijdrage_inkomen" equals 7162800
    And output "zvw_werkgeversbijdrage" equals 470596

  Scenario: Werknemer met inkomen precies op maximum
    Given parameter "loonheffingennummer" is "123456789L01"
    And parameter "bruto_loon" is 71628
    When I evaluate outputs "bijdrage_inkomen, zvw_werkgeversbijdrage" of "zvw/werkgeversbijdrage"
    Then output "bijdrage_inkomen" equals 7162800
    And output "zvw_werkgeversbijdrage" equals 470596

  Scenario: Werknemer met inkomen net onder maximum
    Given parameter "loonheffingennummer" is "123456789L01"
    And parameter "bruto_loon" is 71627
    When I evaluate outputs "bijdrage_inkomen, zvw_werkgeversbijdrage" of "zvw/werkgeversbijdrage"
    Then output "bijdrage_inkomen" equals 7162700
    And output "zvw_werkgeversbijdrage" equals 470589

  Scenario: Werknemer met zeer hoog inkomen - maximale werkgeversbijdrage
    Given parameter "loonheffingennummer" is "123456789L01"
    And parameter "bruto_loon" is 200000
    When I evaluate outputs "bijdrage_inkomen, zvw_werkgeversbijdrage" of "zvw/werkgeversbijdrage"
    Then output "bijdrage_inkomen" equals 7162800
    And output "zvw_werkgeversbijdrage" equals 470596

  Scenario: Werknemer met nul inkomen
    Given parameter "loonheffingennummer" is "123456789L01"
    And parameter "bruto_loon" is 0
    When I evaluate outputs "bijdrage_inkomen, zvw_werkgeversbijdrage" of "zvw/werkgeversbijdrage"
    Then output "bijdrage_inkomen" equals 0
    And output "zvw_werkgeversbijdrage" equals 0

  Scenario: Werknemer parttime - proportioneel inkomen
    Given parameter "loonheffingennummer" is "123456789L01"
    And parameter "bruto_loon" is 20000
    When I evaluate outputs "bijdrage_inkomen, zvw_werkgeversbijdrage" of "zvw/werkgeversbijdrage"
    Then output "bijdrage_inkomen" equals 2000000
    And output "zvw_werkgeversbijdrage" equals 131400

  Scenario: Directeur-grootaandeelhouder (DGA) met gebruikelijk loon
    Given parameter "loonheffingennummer" is "123456789L01"
    And parameter "bruto_loon" is 56000
    When I evaluate outputs "bijdrage_inkomen, zvw_werkgeversbijdrage" of "zvw/werkgeversbijdrage"
    Then output "bijdrage_inkomen" equals 5600000
    And output "zvw_werkgeversbijdrage" equals 367920

  Scenario: Berekening met decimalen in bruto loon
    Given parameter "loonheffingennummer" is "123456789L01"
    And parameter "bruto_loon" is 45123.45
    When I evaluate outputs "bijdrage_inkomen, zvw_werkgeversbijdrage" of "zvw/werkgeversbijdrage"
    Then output "bijdrage_inkomen" equals 4512345
    And output "zvw_werkgeversbijdrage" equals 296461
