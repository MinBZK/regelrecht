Feature: Duurzaamheidskorting vliegbelasting (LU)

  Background:
    Given the calculation date is "2026-07-21"

  Scenario: Klimaatneutrale binnenlandse vlucht → korting
    Given the following parameters:
      | klimaatneutraal       | true |
      | binnenlands           | true |
      | afstand_km            | 200  |
      | vliegbelasting_bedrag | 45   |
    When I evaluate outputs "recht_op_korting, korting_bedrag" of "vliegbelasting_korting_klimaatneutraal_lu"
    Then the execution succeeds
    Then output "recht_op_korting" is true
    Then output "korting_bedrag" equals 22.5

  Scenario: Niet-klimaatneutrale vlucht → geen korting
    Given the following parameters:
      | klimaatneutraal       | false |
      | binnenlands           | false |
      | afstand_km            | 300   |
      | vliegbelasting_bedrag | 60    |
    When I evaluate outputs "recht_op_korting, korting_bedrag" of "vliegbelasting_korting_klimaatneutraal_lu"
    Then the execution succeeds
    Then output "recht_op_korting" is false
    Then output "korting_bedrag" equals 0

  Scenario: Klimaatneutrale korte-afstand vlucht → korting
    Given the following parameters:
      | klimaatneutraal       | true  |
      | binnenlands           | false |
      | afstand_km            | 480   |
      | vliegbelasting_bedrag | 50    |
    When I evaluate outputs "recht_op_korting, korting_bedrag" of "vliegbelasting_korting_klimaatneutraal_lu"
    Then the execution succeeds
    Then output "recht_op_korting" is true
    Then output "korting_bedrag" equals 25

  Scenario: Klimaatneutrale lange-afstand vlucht → geen korting
    Given the following parameters:
      | klimaatneutraal       | true  |
      | binnenlands           | false |
      | afstand_km            | 1200  |
      | vliegbelasting_bedrag | 80    |
    When I evaluate outputs "recht_op_korting, korting_bedrag" of "vliegbelasting_korting_klimaatneutraal_lu"
    Then the execution succeeds
    Then output "recht_op_korting" is false
    Then output "korting_bedrag" equals 0
