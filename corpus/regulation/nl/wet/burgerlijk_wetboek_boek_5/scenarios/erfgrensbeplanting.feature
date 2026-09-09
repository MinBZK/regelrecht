Feature: Erfgrensbeplanting via BW 5:42
  Als perceeleigenaar
  Wil ik weten op welke afstand ik bomen of heggen mag planten
  Zodat ik geen conflict krijg met mijn buurman

  Background:
    Given the calculation date is "2024-06-01"
    Given law "apv_erfgrens_amsterdam" is loaded

  # === Amsterdam: gemeente met eigen verordening ===

  Scenario: Boom in Amsterdam centrum - gemeente wijkt af van rijkswet
    # Amsterdam APV lid 1: 1 meter voor bomen in centrum (postcodegebied 1011-1018)
    # open_term gemeentelijke_afstand_cm = 100, overschrijft BW default van 200
    Given the following parameters:
      | gemeente_code   | GM0363 |
      | type_beplanting | boom   |
      | postcode        | 1012   |
    When I evaluate "minimale_afstand_cm" of "burgerlijk_wetboek_boek_5"
    Then output "minimale_afstand_cm" equals 100
    When I evaluate "minimale_afstand_m" of "burgerlijk_wetboek_boek_5"
    Then output "minimale_afstand_m" equals 1

  Scenario: Boom buiten Amsterdam centrum - APV zwijgt, de default van de open term geldt
    # Amsterdam APV zegt niets over bomen buiten postcodegebied 1011-1018
    # De APV levert voor deze boom geen afstand (afwezig): geen afwijking
    # toegelaten, dus de default van de open term (lid 2: 200 cm) geldt.
    Given the following parameters:
      | gemeente_code   | GM0363 |
      | type_beplanting | boom   |
      | postcode        | 1081   |
    When I evaluate "minimale_afstand_cm" of "burgerlijk_wetboek_boek_5"
    Then output "minimale_afstand_cm" equals 200
    When I evaluate "minimale_afstand_m" of "burgerlijk_wetboek_boek_5"
    Then output "minimale_afstand_m" equals 2

  Scenario: Heg in Amsterdam - gemeente volgt rijkswet
    # Amsterdam APV lid 2: 0,5 meter voor heggen (zelfde als rijkswet)
    # open_term gemeentelijke_afstand_cm = 50
    Given the following parameters:
      | gemeente_code   | GM0363         |
      | type_beplanting | heg_of_heester |
    When I evaluate "minimale_afstand_cm" of "burgerlijk_wetboek_boek_5"
    Then output "minimale_afstand_cm" equals 50
    When I evaluate "minimale_afstand_m" of "burgerlijk_wetboek_boek_5"
    Then output "minimale_afstand_m" equals 0.5

  # === Gemeente zonder eigen verordening: defaults uit rijkswet ===

  Scenario: Boom in gemeente zonder verordening - de default van de open term geldt
    # GM9999 heeft geen verordening: geen implementatie van de open term, dus
    # de default van lid 2 (200 cm).
    Given the following parameters:
      | gemeente_code   | GM9999 |
      | type_beplanting | boom   |
    When I evaluate "minimale_afstand_cm" of "burgerlijk_wetboek_boek_5"
    Then output "minimale_afstand_cm" equals 200
    When I evaluate "minimale_afstand_m" of "burgerlijk_wetboek_boek_5"
    Then output "minimale_afstand_m" equals 2

  Scenario: Heg in gemeente zonder verordening - rijkswet defaults
    Given the following parameters:
      | gemeente_code   | GM9999         |
      | type_beplanting | heg_of_heester |
    When I evaluate "minimale_afstand_cm" of "burgerlijk_wetboek_boek_5"
    Then output "minimale_afstand_cm" equals 50
    When I evaluate "minimale_afstand_m" of "burgerlijk_wetboek_boek_5"
    Then output "minimale_afstand_m" equals 0.5
