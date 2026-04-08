Feature: Erfgrensbeplanting via BW 5:42
  Als perceeleigenaar
  Wil ik weten op welke afstand ik bomen of heggen mag planten
  Zodat ik geen conflict krijg met mijn buurman

  Background:
    Given the calculation date is "2024-06-01"

  # === Amsterdam: gemeente met eigen verordening ===

  Scenario: Boom in Amsterdam - gemeente wijkt af van rijkswet
    # Amsterdam heeft eigen APV: 1 meter voor bomen ipv 2 meter
    Given a query with the following data:
      | gemeente_code   | GM0363 |
      | type_beplanting | boom   |
    When the erfgrensbeplanting is requested for burgerlijk_wetboek_boek_5 article 42
    Then the minimale_afstand_cm is "100"
    And the minimale_afstand_m is "1"

  Scenario: Heg in Amsterdam - gemeente volgt rijkswet
    # Amsterdam houdt 0,5 meter aan voor heggen (zelfde als rijkswet)
    Given a query with the following data:
      | gemeente_code   | GM0363        |
      | type_beplanting | heg_of_heester |
    When the erfgrensbeplanting is requested for burgerlijk_wetboek_boek_5 article 42
    Then the minimale_afstand_cm is "50"
    And the minimale_afstand_m is "0.5"

  # === Gemeente zonder eigen verordening: defaults uit rijkswet ===

  Scenario: Boom in gemeente zonder verordening - rijkswet defaults
    # GM9999 heeft geen verordening, dus gelden de defaults uit BW 5:42
    Given a query with the following data:
      | gemeente_code   | GM9999 |
      | type_beplanting | boom   |
    When the erfgrensbeplanting is requested for burgerlijk_wetboek_boek_5 article 42
    Then the minimale_afstand_cm is "200"
    And the minimale_afstand_m is "2"

  Scenario: Heg in gemeente zonder verordening - rijkswet defaults
    Given a query with the following data:
      | gemeente_code   | GM9999        |
      | type_beplanting | heg_of_heester |
    When the erfgrensbeplanting is requested for burgerlijk_wetboek_boek_5 article 42
    Then the minimale_afstand_cm is "50"
    And the minimale_afstand_m is "0.5"
