Feature: Het voorschot alleen op een aanvraag vóór 1 april
  Art. 16 lid 1 Awir verleent een voorschot aan wie de aanvraag indient "vóór
  1 april van het jaar volgend op het berekeningsjaar". De aanvraag is
  ingediend op de dag van ontvangst (Awb 4:13 lid 1). Een latere aanvraag
  krijgt geen voorschot; het voorschotbedrag is dan 0 (eigen keuze, zie het
  model van art. 16).

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Een aanvraag op 31 maart van het jaar erna krijgt een voorschot
    Given the following parameters:
      | aangevraagd_berekeningsjaar | 2025       |
      | datum_ontvangst             | 2026-03-31 |
      | tegemoetkoming              | 157731     |
    When I evaluate "voorschot_wordt_verleend" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "voorschot_wordt_verleend" is true
    When I evaluate "voorschotbedrag" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "voorschotbedrag" equals 157700

  Scenario: Een aanvraag op 1 april van het jaar erna krijgt geen voorschot
    Given the following parameters:
      | aangevraagd_berekeningsjaar | 2025       |
      | datum_ontvangst             | 2026-04-01 |
      | tegemoetkoming              | 157731     |
    When I evaluate "voorschot_wordt_verleend" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "voorschot_wordt_verleend" is false
    When I evaluate "voorschotbedrag" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "voorschotbedrag" equals 0

  # Zonder de dag van ontvangst is niet te zeggen of de aanvraag op tijd was
  # (RFC-036): dan is ook het voorschot onbekend.
  Scenario: Zonder dag van ontvangst is het voorschot onbekend
    Given the following parameters:
      | aangevraagd_berekeningsjaar | 2025   |
      | tegemoetkoming              | 157731 |
    When I evaluate "voorschotbedrag" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "voorschotbedrag" is unknown
