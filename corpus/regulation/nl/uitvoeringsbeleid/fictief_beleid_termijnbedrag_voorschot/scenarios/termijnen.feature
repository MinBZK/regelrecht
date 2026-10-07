Feature: Termijnen van het voorschot op een tegemoetkoming
  Awir art. 22 zegt per maand of er een termijn van het voorschot valt, uit de
  dagtekening van de beschikking die het voorschot verleent. Het fictieve
  uitvoeringsbeleid zegt hoe hoog die termijn is: gelijke delen, het restant
  in de laatste termijn, en een deel in één bedrag naar rato van de maanden
  (art. 22 lid 4 en 5). Het voorschotbedrag is steeds 1.000,01 euro, zodat
  het restant zichtbaar is.

  Background:
    Given the calculation date is "2025-01-01"

  # Lid 1: verleend vóór de aanvang van het berekeningsjaar: 12 termijnen,
  # de eerste in december daarvoor, de laatste in november.
  Scenario: Een voorschot van vóór het jaar gaat in twaalf termijnen vanaf december
    Given the following parameters:
      | voorschotbedrag      | 100001     |
      | dagtekening_voorschot | 2024-11-20 |
      | berekeningsjaar      | 2025       |
      | maand                | 2024-12-15 |
    When I evaluate "aantal_termijnen" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "aantal_termijnen" equals 12
    When I evaluate "termijnbedrag" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "termijnbedrag" equals 8333

  Scenario: De laatste van twaalf termijnen draagt het restant
    Given the following parameters:
      | voorschotbedrag      | 100001     |
      | dagtekening_voorschot | 2024-11-20 |
      | berekeningsjaar      | 2025       |
      | maand                | 2025-11-01 |
    When I evaluate "termijnbedrag" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "termijnbedrag" equals 8338

  Scenario: In december van het berekeningsjaar valt geen termijn meer
    Given the following parameters:
      | voorschotbedrag      | 100001     |
      | dagtekening_voorschot | 2024-11-20 |
      | berekeningsjaar      | 2025       |
      | maand                | 2025-12-01 |
    When I evaluate "termijn_in_maand" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "termijn_in_maand" is false
    When I evaluate "termijnbedrag" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "termijnbedrag" equals 0

  # Lid 2 en 4: verleend in maart: de verstreken maanden januari tot en met
  # maart in één bedrag in maart, de rest in de termijnen maart tot en met
  # november ("zoveel termijnen als er na de maand van dagtekening nog
  # kalendermaanden overblijven").
  Scenario: Een voorschot in maart betaalt drie maanden ineens met de eerste termijn
    Given the following parameters:
      | voorschotbedrag      | 100001     |
      | dagtekening_voorschot | 2025-03-10 |
      | berekeningsjaar      | 2025       |
      | maand                | 2025-03-31 |
    When I evaluate "aantal_termijnen" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "aantal_termijnen" equals 9
    When I evaluate "maanden_ineens" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "maanden_ineens" equals 3
    When I evaluate "termijnbedrag" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "termijnbedrag" equals 33333

  Scenario: Een voorschot in maart heeft in februari nog geen termijn
    Given the following parameters:
      | voorschotbedrag      | 100001     |
      | dagtekening_voorschot | 2025-03-10 |
      | berekeningsjaar      | 2025       |
      | maand                | 2025-02-01 |
    When I evaluate "termijnbedrag" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "termijnbedrag" equals 0

  Scenario: Een voorschot in maart heeft in de laatste termijn het restant
    Given the following parameters:
      | voorschotbedrag      | 100001     |
      | dagtekening_voorschot | 2025-03-10 |
      | berekeningsjaar      | 2025       |
      | maand                | 2025-11-01 |
    When I evaluate "termijnbedrag" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "termijnbedrag" equals 8337

  # Lid 2 zonder lid 4: in januari is de maand van dagtekening de maand
  # waarin de aanspraak ontstaat, dus er gaat niets ineens.
  Scenario: Een voorschot in januari gaat in elf termijnen, zonder deel ineens
    Given the following parameters:
      | voorschotbedrag      | 100001     |
      | dagtekening_voorschot | 2025-01-15 |
      | berekeningsjaar      | 2025       |
      | maand                | 2025-01-20 |
    When I evaluate "maanden_ineens" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "maanden_ineens" equals 0
    When I evaluate "aantal_termijnen" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "aantal_termijnen" equals 11
    When I evaluate "termijnbedrag" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "termijnbedrag" equals 9091

  # Lid 5: na 31 oktober één bedrag in de maand van dagtekening.
  Scenario: Een voorschot na oktober gaat in één bedrag
    Given the following parameters:
      | voorschotbedrag      | 100001     |
      | dagtekening_voorschot | 2025-11-05 |
      | berekeningsjaar      | 2025       |
      | maand                | 2025-11-05 |
    When I evaluate "termijnbedrag" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "termijnbedrag" equals 100001

  Scenario: Een voorschot na het berekeningsjaar gaat ook in één bedrag
    Given the following parameters:
      | voorschotbedrag      | 100001     |
      | dagtekening_voorschot | 2026-04-13 |
      | berekeningsjaar      | 2025       |
      | maand                | 2026-04-01 |
    When I evaluate "termijnbedrag" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "termijnbedrag" equals 100001

  # Awir art. 14 lid 4 en 5: afgerond op hele euro's, onder 24 euro niet
  # toegekend.
  Scenario: Een tegemoetkoming wordt afgerond op hele euro's
    Given the following parameters:
      | berekende_tegemoetkoming | 157731 |
    When I evaluate "afgeronde_tegemoetkoming" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "afgeronde_tegemoetkoming" equals 157700
    When I evaluate "tegemoetkoming_wordt_toegekend" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "tegemoetkoming_wordt_toegekend" is true

  Scenario: Een tegemoetkoming onder 24 euro wordt niet toegekend
    Given the following parameters:
      | berekende_tegemoetkoming | 2349 |
    When I evaluate "afgeronde_tegemoetkoming" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "afgeronde_tegemoetkoming" equals 2300
    When I evaluate "tegemoetkoming_wordt_toegekend" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "tegemoetkoming_wordt_toegekend" is false
