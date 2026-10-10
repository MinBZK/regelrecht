Feature: De aanslag inkomstenbelasting (AWR art. 11) en het inkomensgegeven (art. 21)
  De inspecteur stelt de aanslag over een kalenderjaar vast. In de demo rekent
  de aanslag het verzamelinkomen niet zelf uit: het is per jaar een gegeven
  van de inspecteur (fictief). Er is een aanslag, dus het inkomensgegeven is
  dat verzamelinkomen (art. 21 onder e, 1°).

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: De aanslag over een jaar stelt het verzamelinkomen van dat jaar vast
    Given parameter "aanslaggegevens" is the collection:
      | jaar | verzamelinkomen |
      | 2024 | 2415000         |
      | 2025 | 1600000         |
    And the following parameters:
      | bsn           | 999100001  |
      | belastingjaar | 2025       |
      | besluit_datum | 2026-04-15 |
    When I evaluate outputs "verzamelinkomen, inkomensgegeven, datum_vaststelling_aanslag" of "algemene_wet_inzake_rijksbelastingen"
    Then output "verzamelinkomen" equals 1600000
    And output "inkomensgegeven" equals 1600000
    And output "datum_vaststelling_aanslag" equals "2026-04-15"
