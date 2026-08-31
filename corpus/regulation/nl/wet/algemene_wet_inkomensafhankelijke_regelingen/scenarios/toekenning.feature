Feature: Awir artikel 14 — toekennen van de tegemoetkoming

  # Bron: encoder-derived. Art. 14 lid 3 (rekenkundig afronden op hele euro's)
  #       en lid 4 (niet toekennen onder EUR 24) staan in de wettekst; de
  #       verwachte waarden hieronder volgen uit die twee regels en zijn niet
  #       aan een parlementair rekenvoorbeeld ontleend.

  Background:
    Given the calculation date is "2025-01-01"
    Given law "algemene_wet_inkomensafhankelijke_regelingen" is loaded

  Scenario: Bedrag boven de drempel wordt rekenkundig afgerond op hele euro's
    # Art. 14 lid 3: 210.849 eurocent is EUR 2.108,49, rekenkundig EUR 2.108.
    Given the following parameters:
      | berekende_tegemoetkoming | 210849 |
    When I evaluate "toegekende_tegemoetkoming" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution succeeds
    Then output "toegekende_tegemoetkoming" equals 210800

  Scenario: Halve euro rondt naar boven
    # Rekenkundig afronden (half-up), de Hoge Raad-conventie: EUR 2.108,50 wordt EUR 2.109.
    Given the following parameters:
      | berekende_tegemoetkoming | 210850 |
    When I evaluate "toegekende_tegemoetkoming" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution succeeds
    Then output "toegekende_tegemoetkoming" equals 210900

  Scenario: Bedrag onder de drempel wordt niet toegekend
    # Art. 14 lid 4: minder dan EUR 24 wordt niet toegekend.
    Given the following parameters:
      | berekende_tegemoetkoming | 2399 |
    When I evaluate "toegekende_tegemoetkoming" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution succeeds
    Then output "toegekende_tegemoetkoming" equals 0

  Scenario: Bedrag precies op de drempel wordt wel toegekend
    # De grens zelf: EUR 24,00 is niet minder dan EUR 24, dus toekennen.
    Given the following parameters:
      | berekende_tegemoetkoming | 2400 |
    When I evaluate "toegekende_tegemoetkoming" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution succeeds
    Then output "toegekende_tegemoetkoming" equals 2400
