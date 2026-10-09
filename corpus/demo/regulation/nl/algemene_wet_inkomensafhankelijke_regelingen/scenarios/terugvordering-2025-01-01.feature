Feature: De beschikking tot terugvordering (Awir art. 26 en 28)
  Laat de verrekening bij de toekenning een bedrag terug te vorderen (art. 24
  lid 3, na art. 26a), dan is de belanghebbende het in zijn geheel
  verschuldigd en vordert de Dienst Toeslagen het volledig terug (art. 26 lid 1
  en 2). Hij betaalt binnen zes weken na de dagtekening (art. 28 lid 1).

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Wat de toekenning terug te vorderen laat, wordt volledig teruggevorderd
    Given the following parameters:
      | terug_te_vorderen | 19000 |
      | berekeningsjaar   | 2026  |
    When I evaluate "terugvordering_wordt_vastgesteld" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "terugvordering_wordt_vastgesteld" is true
    When I evaluate "terugvorderingsbedrag" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "terugvorderingsbedrag" equals 19000

  # Art. 26a: tot de grens laat de toekenning nihil terug te vorderen.
  Scenario: Zonder terug te vorderen bedrag wordt niets teruggevorderd
    Given the following parameters:
      | terug_te_vorderen | 0    |
      | berekeningsjaar   | 2025 |
    When I evaluate "terugvordering_wordt_vastgesteld" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "terugvordering_wordt_vastgesteld" is false

  Scenario: De belanghebbende betaalt binnen zes weken na de dagtekening
    Given the following parameters:
      | dagtekening_terugvordering | 2027-04-15 |
    When I evaluate "uiterste_betaaldatum_terugvordering" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "uiterste_betaaldatum_terugvordering" equals "2027-05-27"
