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

  # === Lid 1: geoorloofdheid binnen de afstand van lid 2 ===

  Scenario: Beplanting binnen de afstand is geoorloofd met toestemming van de nabuur
    # Lid 1: het verbod geldt niet als de eigenaar van het naburige erf
    # toestemming heeft gegeven.
    Given the following parameters:
      | gemeente_code                          | GM9999 |
      | type_beplanting                        | boom   |
      | toestemming_nabuur_gegeven              | true   |
      | naburig_erf_is_openbare_weg_of_water    | false  |
    When I evaluate "beplanting_geoorloofd" of "burgerlijk_wetboek_boek_5"
    Then output "beplanting_geoorloofd" is true

  Scenario: Beplanting binnen de afstand is geoorloofd naast een openbare weg
    # Lid 1: het verbod geldt niet als het naburige erf een openbare weg of
    # openbaar water is, ook zonder toestemming van een nabuur.
    Given the following parameters:
      | gemeente_code                          | GM9999 |
      | type_beplanting                        | boom   |
      | toestemming_nabuur_gegeven              | false  |
      | naburig_erf_is_openbare_weg_of_water    | true   |
    When I evaluate "beplanting_geoorloofd" of "burgerlijk_wetboek_boek_5"
    Then output "beplanting_geoorloofd" is true

  Scenario: Beplanting binnen de afstand is ongeoorloofd zonder toestemming of openbaar erf
    # Lid 1: het verbod (binnen de afstand van lid 2) geldt zonder toestemming
    # en zonder dat het naburige erf een openbare weg of openbaar water is.
    Given the following parameters:
      | gemeente_code                          | GM9999 |
      | type_beplanting                        | boom   |
      | toestemming_nabuur_gegeven              | false  |
      | naburig_erf_is_openbare_weg_of_water    | false  |
    When I evaluate "beplanting_geoorloofd" of "burgerlijk_wetboek_boek_5"
    Then output "beplanting_geoorloofd" is false

  Scenario: Geoorloofdheid is onbekend zonder gegevens over toestemming of erf
    # Lid 1: zonder enig gegeven over toestemming of het naburige erf is niet
    # vast te stellen of een van beide uitzonderingsgronden zich voordoet;
    # de uitkomst is onbekend, geen stilzwijgend "ongeoorloofd".
    Given the following parameters:
      | gemeente_code   | GM9999 |
      | type_beplanting | boom   |
    When I evaluate "beplanting_geoorloofd" of "burgerlijk_wetboek_boek_5"
    Then output "beplanting_geoorloofd" is unknown

  # === Lid 3: uitsluiting van het verzetsrecht ===

  Scenario: De nabuur kan zich niet verzetten tegen beplanting niet hoger dan de scheidsmuur
    # Lid 3: geen verzetsrecht als de beplanting niet hoger reikt dan de
    # scheidsmuur tussen de erven.
    Given the following parameters:
      | gemeente_code                       | GM9999         |
      | type_beplanting                     | heg_of_heester |
      | beplanting_hoger_dan_scheidsmuur     | false          |
    When I evaluate "nabuur_kan_zich_verzetten" of "burgerlijk_wetboek_boek_5"
    Then output "nabuur_kan_zich_verzetten" is false

  Scenario: De nabuur kan zich wel verzetten tegen beplanting hoger dan de scheidsmuur
    # Lid 3: buiten de uitsluiting (beplanting hoger dan de scheidsmuur)
    # bestaat het verzetsrecht.
    Given the following parameters:
      | gemeente_code                       | GM9999         |
      | type_beplanting                     | heg_of_heester |
      | beplanting_hoger_dan_scheidsmuur     | true           |
    When I evaluate "nabuur_kan_zich_verzetten" of "burgerlijk_wetboek_boek_5"
    Then output "nabuur_kan_zich_verzetten" is true

  Scenario: Het verzetsrecht is onbekend zonder gegeven over de hoogte
    # Lid 3: zonder een hoogtevergelijking met de scheidsmuur is niet vast te
    # stellen of de uitsluiting van lid 3 van toepassing is.
    Given the following parameters:
      | gemeente_code   | GM9999         |
      | type_beplanting | heg_of_heester |
    When I evaluate "nabuur_kan_zich_verzetten" of "burgerlijk_wetboek_boek_5"
    Then output "nabuur_kan_zich_verzetten" is unknown

  # === Lid 4: vergoedbare schade vanaf de aanmaning ===

  Scenario: Schade ontstaan na de aanmaning is vergoedbaar
    # Lid 4: alleen vergoeding voor schade die is ontstaan na het tijdstip
    # waartegen tot opheffing is aangemaand.
    Given the following parameters:
      | gemeente_code               | GM9999     |
      | type_beplanting             | boom       |
      | datum_aanmaning_opheffing   | 2024-01-01 |
      | datum_ontstaan_schade       | 2024-03-01 |
    When I evaluate "vergoedbare_schade" of "burgerlijk_wetboek_boek_5"
    Then output "vergoedbare_schade" is true

  Scenario: Schade ontstaan voor de aanmaning is niet vergoedbaar
    # Lid 4: schade die al bestond voordat is aangemaand tot opheffing valt
    # buiten de vergoedingsplicht.
    Given the following parameters:
      | gemeente_code               | GM9999     |
      | type_beplanting             | boom       |
      | datum_aanmaning_opheffing   | 2024-03-01 |
      | datum_ontstaan_schade       | 2024-01-01 |
    When I evaluate "vergoedbare_schade" of "burgerlijk_wetboek_boek_5"
    Then output "vergoedbare_schade" is false

  Scenario: Vergoedbare schade is onbekend zonder gegeven over de aanmaning
    # Lid 4: zonder een gegeven over de aanmaning tot opheffing is niet vast
    # te stellen of de schade voor of na dat tijdstip is ontstaan; de
    # parameter is niet gevraagd, dus onbekend (RFC-036), geen stilzwijgend
    # oordeel dat er geen aanmaning is geweest.
    Given the following parameters:
      | gemeente_code           | GM9999     |
      | type_beplanting         | boom       |
      | datum_ontstaan_schade   | 2024-03-01 |
    When I evaluate "vergoedbare_schade" of "burgerlijk_wetboek_boek_5"
    Then output "vergoedbare_schade" is unknown

  Scenario: Vergoedbare schade is onbekend zonder schadedatum bij een bestaande aanmaning
    # Lid 4: met een aanmaning maar zonder schadedatum is niet vast te stellen
    # of de schade voor of na de aanmaning is ontstaan.
    Given the following parameters:
      | gemeente_code               | GM9999     |
      | type_beplanting             | boom       |
      | datum_aanmaning_opheffing   | 2024-01-01 |
    When I evaluate "vergoedbare_schade" of "burgerlijk_wetboek_boek_5"
    Then output "vergoedbare_schade" is unknown
