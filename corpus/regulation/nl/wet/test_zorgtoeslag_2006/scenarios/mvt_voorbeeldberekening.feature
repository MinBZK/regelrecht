Feature: MvT-voorbeeldberekening tegen de vastgestelde wet (Stb. 2005, 369)

  # Bron: Kamerstukken II 2003/04, 29 762, nr. 3 (memorie van toelichting),
  #       sectie "Voorbeeldberekening zorgtoeslag".
  # Dezelfde drie huishoudens en dezelfde MvT-parameters (drempelinkomen 17 500,
  #       standaardpremie 1 000, toetsingsinkomen 22 000), maar nu gedraaid tegen
  #       de VASTGESTELDE wet (Stb. 2005, 369), waarin Artikel 2 lid 4 de
  #       vijftig-procent-regel bevat die in het voorstel (nr. 2) nog ontbrak.
  # Huishoudens 1 en 2 slagen identiek. Huishouden 3, dat de MvT op 0 belooft,
  #       FAALT tegen de vastgestelde wet: het faalbericht toont wat de engine
  #       in plaats daarvan berekent (34100 eurocent = EUR 341).

  Scenario: Verzekerde zonder partner (MvT-huishouden 1, belofte 120)
    # Bron: kst-29762-3, "Verzekerde zonder partner": normpremie 880, zorgtoeslag 120.
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   | 100000  |
      | toetsingsinkomen  | 2200000 |
      | heeft_partner     | false   |
      | partner_verzekerd | false   |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "normpremie" equals 88000
    Then output "hoogte_zorgtoeslag" equals 12000

  Scenario: Verzekerde met partner die verzekerde is (MvT-huishouden 2, belofte 682)
    # Bron: kst-29762-3, "Verzekerde met partner/verzekerde": normpremie 1 318,
    #       standaardpremie 2 000, zorgtoeslag 682.
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   | 100000  |
      | toetsingsinkomen  | 2200000 |
      | heeft_partner     | true    |
      | partner_verzekerd | true    |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "normpremie" equals 131800
    Then output "hoogte_zorgtoeslag" equals 68200

  # Deze assertie is OPZETTELIJK de MvT-belofte (0). Tegen de vastgestelde wet
  # halveert lid 4 de aanspraak i.p.v. haar op 0 te zetten; dit scenario FAALT
  # daarom, en het faalbericht is precies de bevinding: de vastgestelde wet wijkt
  # af van wat de MvT bij het voorstel beloofde.
  Scenario: Verzekerde met partner die geen verzekerde is (MvT-belofte 0, FAALT tegen Stb. 2005, 369)
    # Bron: kst-29762-3, "Verzekerde met partner/niet-verzekerde": zorgtoeslag 0.
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   | 100000  |
      | toetsingsinkomen  | 2200000 |
      | heeft_partner     | true    |
      | partner_verzekerd | false   |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "normpremie" equals 131800
    Then output "hoogte_zorgtoeslag" equals 0

  # Observed-then-pinned: de daadwerkelijk door de engine berekende uitkomst voor
  # huishouden 3 onder de vastgestelde wet. Lid 1 telt tweemaal de standaardpremie
  # (2000 - 1318 = 682); lid 4 halveert dat tot 341 (34100 eurocent). Dit getal is
  # eerst waargenomen in de output en daarna hier vastgepind.
  Scenario: Verzekerde met partner die geen verzekerde is (vastgestelde uitkomst 341)
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   | 100000  |
      | toetsingsinkomen  | 2200000 |
      | heeft_partner     | true    |
      | partner_verzekerd | false   |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "normpremie" equals 131800
    Then output "hoogte_zorgtoeslag" equals 34100
