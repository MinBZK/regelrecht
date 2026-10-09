Feature: Toekenning en verrekening van een tegemoetkoming
  Bij de toekenning haken art. 19, 24 en 26a Awir op de beschikking in de fase
  TOEKENNING. Art. 19 zegt tot wanneer de toekenning mag duren, art. 24
  verrekent de uitbetaalde voorschotten met de toegekende tegemoetkoming
  (afgerond volgens art. 14 lid 4), en art. 26a vordert een klein bedrag niet
  terug. Nabetaling en terugvordering zijn twee uitkomsten, geen saldo.

  Background:
    Given the calculation date is "2025-01-01"

  # Art. 24 lid 1 en 2: de tegemoetkoming is hoger dan wat er aan
  # voorschotten is uitbetaald; het verschil wordt binnen vier weken betaald.
  Scenario: Een hogere tegemoetkoming geeft een nabetaling
    Given the following parameters:
      | tegemoetkoming           | 157731     |
      | uitbetaalde_voorschotten | 100000     |
      | dagtekening_toekenning   | 2026-06-01 |
    When I evaluate "nog_uit_te_betalen" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "nog_uit_te_betalen" equals 57700
    When I evaluate "toegekende_tegemoetkoming" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "toegekende_tegemoetkoming" equals 157700
    When I evaluate "terug_te_vorderen_na_verrekening" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "terug_te_vorderen_na_verrekening" equals 0
    When I evaluate "uiterste_uitbetaaldatum" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "uiterste_uitbetaaldatum" equals "2026-06-29"

  # Art. 24 lid 3 en 26a: de verrekening laat een terug te vorderen bedrag.
  Scenario: Een lagere tegemoetkoming geeft een terugvordering
    Given the following parameters:
      | tegemoetkoming           | 50000  |
      | uitbetaalde_voorschotten | 100000 |
    When I evaluate "terug_te_vorderen" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "terug_te_vorderen" equals 50000

  # Art. 26a lid 1: tot en met € 118 (tekst van 2025) wordt niet
  # teruggevorderd.
  Scenario: Een terug te vorderen bedrag van € 118 wordt niet teruggevorderd
    Given the following parameters:
      | tegemoetkoming           | 88200  |
      | uitbetaalde_voorschotten | 100000 |
    When I evaluate "terug_te_vorderen" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "terug_te_vorderen" equals 0

  Scenario: Een terug te vorderen bedrag van € 119 wordt teruggevorderd
    Given the following parameters:
      | tegemoetkoming           | 88100  |
      | uitbetaalde_voorschotten | 100000 |
    When I evaluate "terug_te_vorderen" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "terug_te_vorderen" equals 11900

  Scenario: Zonder te weten of er een aanslag is, is de uiterste datum onbekend
    Given the following parameters:
      | berekeningsjaar             | 2025 |
    When I evaluate "uiterste_toekenningsdatum" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "uiterste_toekenningsdatum" is unknown

  # Art. 19 lid 1: zes maanden na de vaststelling van de aanslag.
  Scenario: De toekenning volgt binnen zes maanden na de aanslag
    Given the following parameters:
      | berekeningsjaar             | 2025       |
      | datum_vaststelling_aanslag  | 2026-03-15 |
    When I evaluate "uiterste_toekenningsdatum" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "uiterste_toekenningsdatum" equals "2026-09-15"

  # Art. 19 lid 2: zonder aanslag uiterlijk 31 december van het jaar erna.
  # Dat er geen aanslag is, is een gegeven (null); een datum die niemand
  # doorgeeft, is onbekend.
  Scenario: Zonder aanslag volgt de toekenning uiterlijk 31 december van het jaar erna
    Given the following parameters:
      | berekeningsjaar             | 2025 |
    And parameter "datum_vaststelling_aanslag" is "null"
    When I evaluate "uiterste_toekenningsdatum" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "uiterste_toekenningsdatum" equals "2026-12-31"

  # Art. 19 lid 3 (sinds 2026): een late aanvraag, op of na 1 september van
  # het jaar na het berekeningsjaar.
  Scenario: Een late aanvraag wordt uiterlijk 30 april van het jaar erna toegekend
    Given the calculation date is "2026-01-01"
    And the following parameters:
      | berekeningsjaar             | 2026       |
      | datum_ontvangst             | 2027-10-01 |
    And parameter "datum_vaststelling_aanslag" is "null"
    When I evaluate "uiterste_toekenningsdatum" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "uiterste_toekenningsdatum" equals "2028-04-30"
