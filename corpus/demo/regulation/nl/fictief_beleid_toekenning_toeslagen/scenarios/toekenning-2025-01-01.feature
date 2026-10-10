Feature: Toekenning, nabetaling en terugvordering van een tegemoetkoming (fictief)
  Het fictieve beleid van de Dienst Toeslagen: zij legt een ontvangen
  inkomensgegeven vast (art. 1), neemt het bij de toekenning als
  toetsingsinkomen (art. 2), kent toe op de dag van de aanslag (art. 3),
  betaalt een nabetaling op de dag van de toekenning (art. 4 en 5), stelt de
  terugvordering vast op de dag van de toekenning (art. 6) en int haar op de
  eerste van de maand erna (art. 7 en 8).

  Background:
    Given the calculation date is "2025-06-01"

  Scenario: Een ontvangen inkomensgegeven wordt vastgelegd
    Given the following parameters:
      | kenmerk_aanslag            | aanslag-1  |
      | bsn                        | 999100001  |
      | kalenderjaar               | 2025       |
      | inkomensgegeven            | 1600000    |
      | datum_vaststelling_aanslag | 2026-04-15 |
    When I evaluate "inkomensgegeven_ontvangen" of "fictief_beleid_toekenning_toeslagen"
    Then output "inkomensgegeven_ontvangen" is true

  Scenario: Bij de toekenning is het toetsingsinkomen het verstrekte inkomensgegeven
    Given the following parameters:
      | inkomensgegeven | 1600000 |
    When I evaluate "toetsingsinkomen" of "fictief_beleid_toekenning_toeslagen"
    Then output "toetsingsinkomen" equals 1600000

  Scenario: De toekenning valt op de dag van de aanslag
    Given the following parameters:
      | datum_vaststelling_aanslag | 2026-04-15 |
    When I evaluate "dag_toekenning" of "fictief_beleid_toekenning_toeslagen"
    Then output "dag_toekenning" equals "2026-04-15"

  Scenario: Een nabetaling wordt op de dag van de toekenning opgedragen
    Given the following parameters:
      | nog_uit_te_betalen    | 11400              |
      | opgedragen_nabetaling | 0                  |
      | rekeningnummer        | NL00TEST0123456789 |
      | berekeningsjaar       | 2025               |
      | dag                   | 2026-04-15         |
    When I evaluate outputs "nabetaling_wordt_opgedragen, bedrag, rekeningnummer_begunstigde, uitvoerdatum" of "fictief_beleid_toekenning_toeslagen"
    Then output "nabetaling_wordt_opgedragen" is true
    And output "bedrag" equals 11400
    And output "rekeningnummer_begunstigde" equals "NL00TEST0123456789"
    And output "uitvoerdatum" equals "2026-04-15"

  Scenario: Een opgedragen nabetaling wordt niet nog eens opgedragen
    Given the following parameters:
      | nog_uit_te_betalen    | 11400              |
      | opgedragen_nabetaling | 11400              |
      | rekeningnummer        | NL00TEST0123456789 |
      | berekeningsjaar       | 2025               |
      | dag                   | 2026-05-01         |
    When I evaluate "nabetaling_wordt_opgedragen" of "fictief_beleid_toekenning_toeslagen"
    Then output "nabetaling_wordt_opgedragen" is false

  Scenario: Een nabetaling die de bank weigerde, blijft openstaan
    Given the following parameters:
      | bijgeschreven        | false               |
      | bijgeschreven_bedrag | 0                   |
      | bedrag_opdracht      | 11400               |
      | reden_weigering      | rekening geblokkeerd |
    When I evaluate outputs "nabetaling_uitgevoerd, nabetaling_niet_uitgevoerd, niet_nabetaald_bedrag" of "fictief_beleid_toekenning_toeslagen"
    Then output "nabetaling_uitgevoerd" is false
    And output "nabetaling_niet_uitgevoerd" is true
    And output "niet_nabetaald_bedrag" equals 11400

  Scenario: Een terugvordering op de dag van de toekenning
    Given the following parameters:
      | terug_te_vorderen      | 19000      |
      | dagtekening_toekenning | 2027-04-15 |
    When I evaluate "dag_terugvordering" of "fictief_beleid_toekenning_toeslagen"
    Then output "dag_terugvordering" equals "2027-04-15"

  Scenario: Niets terug te vorderen, geen dag
    Given the following parameters:
      | terug_te_vorderen      | 0          |
      | dagtekening_toekenning | 2026-04-15 |
    When I evaluate "dag_terugvordering" of "fictief_beleid_toekenning_toeslagen"
    Then output "dag_terugvordering" is absent

  Scenario: Niet op de dag van de beschikking zelf geïnd
    Given the following parameters:
      | terugvorderingsbedrag      | 19000              |
      | dagtekening_terugvordering | 2027-04-15         |
      | ingevorderd_bedrag         | 0                  |
      | rekeningnummer             | NL00TEST0123456789 |
      | berekeningsjaar            | 2026               |
      | dag                        | 2027-04-15         |
    When I evaluate "incasso_wordt_opgedragen" of "fictief_beleid_toekenning_toeslagen"
    Then output "incasso_wordt_opgedragen" is false

  Scenario: Geïnd op de eerste van de maand erna
    Given the following parameters:
      | terugvorderingsbedrag      | 19000              |
      | dagtekening_terugvordering | 2027-04-15         |
      | ingevorderd_bedrag         | 0                  |
      | rekeningnummer             | NL00TEST0123456789 |
      | berekeningsjaar            | 2026               |
      | dag                        | 2027-05-01         |
    When I evaluate outputs "incasso_wordt_opgedragen, incassobedrag, rekeningnummer_debiteur, incassodatum" of "fictief_beleid_toekenning_toeslagen"
    Then output "incasso_wordt_opgedragen" is true
    And output "incassobedrag" equals 19000
    And output "rekeningnummer_debiteur" equals "NL00TEST0123456789"
    And output "incassodatum" equals "2027-05-01"

  Scenario: Wat geïnd is, wordt niet nog eens geïnd
    Given the following parameters:
      | terugvorderingsbedrag      | 19000              |
      | dagtekening_terugvordering | 2027-04-15         |
      | ingevorderd_bedrag         | 19000              |
      | rekeningnummer             | NL00TEST0123456789 |
      | berekeningsjaar            | 2026               |
      | dag                        | 2027-06-01         |
    When I evaluate "incasso_wordt_opgedragen" of "fictief_beleid_toekenning_toeslagen"
    Then output "incasso_wordt_opgedragen" is false

  Scenario: Een incasso die de bank weigerde, blijft openstaan
    Given the following parameters:
      | afgeschreven            | false              |
      | afgeschreven_bedrag     | 0                  |
      | bedrag_incasso          | 19000              |
      | reden_weigering_incasso | saldo ontoereikend |
    When I evaluate outputs "incasso_uitgevoerd, incasso_niet_uitgevoerd, niet_geind_bedrag, reden_incasso" of "fictief_beleid_toekenning_toeslagen"
    Then output "incasso_uitgevoerd" is false
    And output "incasso_niet_uitgevoerd" is true
    And output "niet_geind_bedrag" equals 19000
    And output "reden_incasso" equals "saldo ontoereikend"
