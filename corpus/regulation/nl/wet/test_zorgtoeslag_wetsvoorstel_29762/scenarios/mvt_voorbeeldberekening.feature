Feature: MvT-voorbeeldberekening tegen het voorstel van wet (29 762, nr. 2)

  # Bron: Kamerstukken II 2003/04, 29 762, nr. 3 (memorie van toelichting),
  #       sectie "Voorbeeldberekening zorgtoeslag".
  # Parameters (MvT): drempelinkomen 17 500, standaardpremie per verzekerde
  #       1 000, toetsingsinkomen per huishouden 22 000. In eurocent:
  #       drempelinkomen 1750000, standaardpremie 100000, toetsingsinkomen 2200000.
  # De MvT belooft: huishouden 1 -> 120, huishouden 2 -> 682, huishouden 3 -> 0.
  # Deze scenarios draaien die belofte tegen de encoding van het voorstel van wet.

  Scenario: Verzekerde zonder partner (MvT-huishouden 1, belofte 120)
    # Bron: kst-29762-3, "Verzekerde zonder partner": normpremie 880, zorgtoeslag 120.
    Given the calculation date is "2004-09-27"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   | 100000  |
      | toetsingsinkomen  | 2200000 |
      | heeft_partner     | false   |
      | partner_verzekerd | false   |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_wetsvoorstel_29762"
    Then the execution succeeds
    Then output "normpremie" equals 88000
    Then output "hoogte_zorgtoeslag" equals 12000

  Scenario: Verzekerde met partner die verzekerde is (MvT-huishouden 2, belofte 682)
    # Bron: kst-29762-3, "Verzekerde met partner/verzekerde": normpremie 1 318,
    #       standaardpremie 2 000 (1000 per partner), zorgtoeslag 682.
    Given the calculation date is "2004-09-27"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   | 100000  |
      | toetsingsinkomen  | 2200000 |
      | heeft_partner     | true    |
      | partner_verzekerd | true    |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_wetsvoorstel_29762"
    Then the execution succeeds
    Then output "normpremie" equals 131800
    Then output "hoogte_zorgtoeslag" equals 68200

  Scenario: Verzekerde met partner die geen verzekerde is (MvT-huishouden 3, belofte 0)
    # Bron: kst-29762-3, "Verzekerde met partner/niet-verzekerde": normpremie 1 318,
    #       standaardpremie 1 000 (slechts eenmaal in aanmerking genomen),
    #       zorgtoeslag 0.
    Given the calculation date is "2004-09-27"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   | 100000  |
      | toetsingsinkomen  | 2200000 |
      | heeft_partner     | true    |
      | partner_verzekerd | false   |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_wetsvoorstel_29762"
    Then the execution succeeds
    Then output "normpremie" equals 131800
    Then output "hoogte_zorgtoeslag" equals 0
