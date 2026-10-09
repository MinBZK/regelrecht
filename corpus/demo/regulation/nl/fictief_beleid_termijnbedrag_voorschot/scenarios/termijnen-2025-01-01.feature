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

  # Eigen keuze van het beleid: een voorschot van nul (Awir 16 lid 1, een
  # aanvraag na 1 april van het jaar erna) geeft geen termijnen.
  Scenario: Een voorschot van nul geeft geen termijn
    Given the following parameters:
      | voorschotbedrag      | 0          |
      | dagtekening_voorschot | 2025-03-10 |
      | berekeningsjaar      | 2025       |
      | maand                | 2025-04-01 |
    When I evaluate "termijn_in_maand" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then output "termijn_in_maand" is true
    When I evaluate "termijn_wordt_betaald" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "termijn_wordt_betaald" is false

  Scenario: Een voorschot boven nul geeft in een termijnmaand een termijn
    Given the following parameters:
      | voorschotbedrag      | 100001     |
      | dagtekening_voorschot | 2025-03-10 |
      | berekeningsjaar      | 2025       |
      | maand                | 2025-04-01 |
    When I evaluate "termijn_wordt_betaald" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "termijn_wordt_betaald" is true

  # Art. 1, de betaalopdracht: in een maand met een termijn, of zolang er een
  # mislukte termijn openstaat. Het bedrag is de termijn plus wat openstaat;
  # de bank voert haar uit op de dag van de opdracht, op de rekening uit de
  # aanvraag.
  Scenario: In een termijnmaand zonder achterstand is de opdracht de termijn
    Given the following parameters:
      | voorschotbedrag       | 100001             |
      | dagtekening_voorschot | 2024-11-20         |
      | berekeningsjaar       | 2025               |
      | maand                 | 2024-12-01         |
      | rekeningnummer        | NL00TEST0123456789 |
      | achterstallig_bedrag  | 0                  |
    When I evaluate outputs "opdracht_wordt_gegeven, meegenomen_achterstand, bedrag, rekeningnummer_begunstigde, uitvoerdatum" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "opdracht_wordt_gegeven" is true
    And output "meegenomen_achterstand" equals 0
    And output "bedrag" equals 8333
    And output "rekeningnummer_begunstigde" equals "NL00TEST0123456789"
    And output "uitvoerdatum" equals "2024-12-01"

  Scenario: Een mislukte termijn gaat mee met de opdracht van de volgende termijn
    Given the following parameters:
      | voorschotbedrag       | 100001             |
      | dagtekening_voorschot | 2024-11-20         |
      | berekeningsjaar       | 2025               |
      | maand                 | 2025-01-01         |
      | rekeningnummer        | NL00TEST0123456789 |
      | achterstallig_bedrag  | 8333               |
    When I evaluate outputs "opdracht_wordt_gegeven, meegenomen_achterstand, termijnbedrag, bedrag" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "opdracht_wordt_gegeven" is true
    And output "termijnbedrag" equals 8333
    And output "meegenomen_achterstand" equals 8333
    And output "bedrag" equals 16666

  Scenario: Zonder termijn maar met een achterstand is er toch een opdracht
    Given the following parameters:
      | voorschotbedrag       | 100001             |
      | dagtekening_voorschot | 2024-11-20         |
      | berekeningsjaar       | 2025               |
      | maand                 | 2025-12-01         |
      | rekeningnummer        | NL00TEST0123456789 |
      | achterstallig_bedrag  | 8338               |
    When I evaluate outputs "opdracht_wordt_gegeven, termijnbedrag, bedrag" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "opdracht_wordt_gegeven" is true
    And output "termijnbedrag" equals 0
    And output "bedrag" equals 8338

  Scenario: Zonder termijn en zonder achterstand is er geen opdracht
    Given the following parameters:
      | voorschotbedrag       | 100001             |
      | dagtekening_voorschot | 2024-11-20         |
      | berekeningsjaar       | 2025               |
      | maand                 | 2025-12-01         |
      | rekeningnummer        | NL00TEST0123456789 |
      | achterstallig_bedrag  | 0                  |
    When I evaluate "opdracht_wordt_gegeven" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "opdracht_wordt_gegeven" is false

  # Art. 2, het antwoord van de bank: pas wat zij bijschreef is betaald;
  # weigert zij, dan is de betaling mislukt voor het bedrag van de opdracht.
  Scenario: De bank schreef de opdracht bij
    Given the following parameters:
      | bijgeschreven        | true  |
      | bijgeschreven_bedrag | 16666 |
      | bedrag_opdracht      | 16666 |
      | reden_weigering      | null  |
    When I evaluate outputs "uitgevoerd, niet_uitgevoerd, betaald_bedrag, mislukt_bedrag, reden" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "uitgevoerd" is true
    And output "niet_uitgevoerd" is false
    And output "betaald_bedrag" equals 16666
    And output "mislukt_bedrag" equals 0
    And output "reden" is absent

  Scenario: De bank weigerde de opdracht
    Given the following parameters:
      | bijgeschreven        | false                |
      | bijgeschreven_bedrag | 0                    |
      | bedrag_opdracht      | 16666                |
      | reden_weigering      | rekening geblokkeerd |
    When I evaluate outputs "uitgevoerd, niet_uitgevoerd, betaald_bedrag, mislukt_bedrag, reden" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "uitgevoerd" is false
    And output "niet_uitgevoerd" is true
    And output "betaald_bedrag" equals 0
    And output "mislukt_bedrag" equals 16666
    And output "reden" equals "rekening geblokkeerd"

  # Art. 4: het voorschot voor een volgend berekeningsjaar (art. 15 lid 5 Awir)
  # verleent Toeslagen op 1 november van het jaar ervoor (eigen keuze).
  Scenario: Het voorschot voor het volgende jaar wordt op 1 november verleend
    Given the following parameters:
      | berekeningsjaar             | 2026 |
      | aangevraagd_berekeningsjaar | 2025 |
    When I evaluate "dag_verlening_voorschot" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "dag_verlening_voorschot" equals "2025-11-01"

  Scenario: Voor het aangevraagde jaar geeft het beleid geen dag
    Given the following parameters:
      | berekeningsjaar             | 2025 |
      | aangevraagd_berekeningsjaar | 2025 |
    When I evaluate "dag_verlening_voorschot" of "fictief_beleid_termijnbedrag_voorschot"
    Then output "dag_verlening_voorschot" is absent
