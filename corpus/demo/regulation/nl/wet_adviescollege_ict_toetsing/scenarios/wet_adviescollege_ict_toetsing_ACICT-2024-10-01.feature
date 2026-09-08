# Converted from bestuursrecht/wet_adviescollege_ict_toetsing_ACICT-2024-10-01.feature by corpus/demo/tools/convert_features.mjs
Feature: Bepalen adviesplicht ICT-projecten
  Als verantwoordelijk ministerie of organisatie
  Wil ik weten of mijn ICT-project onder de adviesplicht valt
  Zodat ik tijdig advies kan aanvragen bij het Adviescollege ICT-toetsing

  Background:
    Given the calculation date is "2024-10-01"

  Scenario: ICT-project ministerie van €10 miljoen valt onder adviesplicht
    Given parameter "project_id" is "PROJ-001"
    And the following "ACICT" data with key "project_id" for law "wet_adviescollege_ict_toetsing":
      | project_id | totale_kosten | organisatie_type | project_type |
      | PROJ-001   | 1000000000    | MINISTERIE       | REGULIER     |
    When I evaluate outputs "adviesplicht, project_kosten" of "wet_adviescollege_ict_toetsing"
    Then output "adviesplicht" is true
    And output "project_kosten" equals 1000000000

  Scenario: ICT-project ZBO van €6 miljoen valt onder adviesplicht
    Given parameter "project_id" is "PROJ-002"
    And the following "ACICT" data with key "project_id" for law "wet_adviescollege_ict_toetsing":
      | project_id | totale_kosten | organisatie_type | project_type |
      | PROJ-002   | 600000000     | ZBO              | REGULIER     |
    When I evaluate outputs "adviesplicht, project_kosten" of "wet_adviescollege_ict_toetsing"
    Then output "adviesplicht" is true
    And output "project_kosten" equals 600000000

  Scenario: ICT-project politie van €5 miljoen valt precies op drempel
    Given parameter "project_id" is "PROJ-003"
    And the following "ACICT" data with key "project_id" for law "wet_adviescollege_ict_toetsing":
      | project_id | totale_kosten | organisatie_type | project_type |
      | PROJ-003   | 500000000     | POLITIE          | REGULIER     |
    When I evaluate outputs "adviesplicht, project_kosten" of "wet_adviescollege_ict_toetsing"
    Then output "adviesplicht" is true
    And output "project_kosten" equals 500000000

  Scenario: ICT-project ministerie van €4 miljoen valt niet onder adviesplicht
    Given parameter "project_id" is "PROJ-004"
    And the following "ACICT" data with key "project_id" for law "wet_adviescollege_ict_toetsing":
      | project_id | totale_kosten | organisatie_type | project_type |
      | PROJ-004   | 400000000     | MINISTERIE       | REGULIER     |
    When I evaluate outputs "adviesplicht, project_kosten" of "wet_adviescollege_ict_toetsing"
    Then output "adviesplicht" is false
    And output "project_kosten" equals 400000000

  Scenario: Wapensysteem defensie van €20 miljoen valt niet onder adviesplicht
    Given parameter "project_id" is "PROJ-005"
    And the following "ACICT" data with key "project_id" for law "wet_adviescollege_ict_toetsing":
      | project_id | totale_kosten | organisatie_type | project_type |
      | PROJ-005   | 2000000000    | MINISTERIE       | WAPENSYSTEEM |
    When I evaluate outputs "adviesplicht, project_kosten" of "wet_adviescollege_ict_toetsing"
    Then output "adviesplicht" is false
    And output "project_kosten" equals 2000000000

  Scenario: ICT-project rechterlijke macht van €8 miljoen valt onder adviesplicht
    Given parameter "project_id" is "PROJ-006"
    And the following "ACICT" data with key "project_id" for law "wet_adviescollege_ict_toetsing":
      | project_id | totale_kosten | organisatie_type   | project_type |
      | PROJ-006   | 800000000     | RECHTERLIJKE_MACHT | REGULIER     |
    When I evaluate outputs "adviesplicht, project_kosten" of "wet_adviescollege_ict_toetsing"
    Then output "adviesplicht" is true
    And output "project_kosten" equals 800000000

  Scenario: ICT-project gemeente van €10 miljoen valt niet onder adviesplicht
    Given parameter "project_id" is "PROJ-007"
    And the following "ACICT" data with key "project_id" for law "wet_adviescollege_ict_toetsing":
      | project_id | totale_kosten | organisatie_type | project_type |
      | PROJ-007   | 1000000000    | GEMEENTE         | REGULIER     |
    When I evaluate outputs "adviesplicht, project_kosten" of "wet_adviescollege_ict_toetsing"
    Then output "adviesplicht" is false
    And output "project_kosten" equals 1000000000

  Scenario: Complex ICT-project ZBO met hoge kosten valt onder adviesplicht
    Given parameter "project_id" is "PROJ-008"
    And the following "ACICT" data with key "project_id" for law "wet_adviescollege_ict_toetsing":
      | project_id | totale_kosten | organisatie_type | project_type |
      | PROJ-008   | 5000000000    | ZBO              | REGULIER     |
    When I evaluate outputs "adviesplicht, project_kosten" of "wet_adviescollege_ict_toetsing"
    Then output "adviesplicht" is true
    And output "project_kosten" equals 5000000000
