Feature: De planning van de aanslagregeling (fictief)
  De inspecteur stelt de aanslag over een jaar vast op de dag die zijn
  planning geeft. Zonder dag in de planning is er (nog) geen aanslag.

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: De dag uit de planning voor dat jaar
    Given parameter "planning" is the collection:
      | jaar | datum_vaststelling |
      | 2024 | 2025-04-15         |
      | 2025 | 2026-04-15         |
    And the following parameters:
      | bsn           | 999100001 |
      | belastingjaar | 2025      |
    When I evaluate "dag_vaststelling_aanslag" of "fictief_beleid_aanslagregeling"
    Then output "dag_vaststelling_aanslag" equals "2026-04-15"

  Scenario: Geen dag in de planning
    Given parameter "planning" is the collection:
      | jaar | datum_vaststelling |
      | 2024 | 2025-04-15         |
    And the following parameters:
      | bsn           | 999100001 |
      | belastingjaar | 2025      |
    When I evaluate "dag_vaststelling_aanslag" of "fictief_beleid_aanslagregeling"
    Then output "dag_vaststelling_aanslag" is absent
