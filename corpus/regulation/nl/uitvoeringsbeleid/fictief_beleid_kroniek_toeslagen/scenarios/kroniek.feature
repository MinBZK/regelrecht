Feature: Teruglezen van de kroniek van Toeslagen
  Het fictieve beleid van Toeslagen leest haar eigen kroniek terug als de
  gegevens die art. 22 Awir en het beleid over de uitbetaling vragen: het
  voorschot dat geldt, de rekening uit de aanvraag en wat er achterstallig
  is. De kroniek is hier een tabel met een rij per vastgelegd feit: zijn
  kenmerk, de aanvraag waar het bij hoort (`root`), zijn plaats in de tijd
  (`sequence`) en zijn velden. Er staan steeds ook feiten van een andere
  aanvraag in: die tellen niet mee.

  Background:
    Given the calculation date is "2025-06-01"

  # Lid 1: het laatste voorschot op de aanvraag is dat met de hoogste plaats
  # in de tijd; een voorschot op een andere aanvraag telt niet.
  Scenario: Het voorschot is het laatste besluit in de fase VOORSCHOT op deze aanvraag
    Given parameter "grams" is the collection:
      | id | root | sequence | event              | stage     | voorschotbedrag | effective_date | period |
      | a1 | a1   | 1        | aanvraag_ontvangen | null      | null            | 2024-11-04     | null   |
      | v1 | a1   | 2        | voorschot_verleend | VOORSCHOT | 100000          | 2024-11-20     | 2025   |
      | v2 | a1   | 5        | voorschot_verleend | VOORSCHOT | 120000          | 2025-02-10     | 2025   |
      | w1 | a2   | 7        | voorschot_verleend | VOORSCHOT | 999900          | 2025-03-01     | 2025   |
    And the following parameters:
      | root | a1 |
    When I evaluate outputs "laatste_voorschot, voorschotbedrag, dagtekening_voorschot, berekeningsjaar" of "fictief_beleid_kroniek_toeslagen"
    Then output "laatste_voorschot" equals 5
    And output "voorschotbedrag" equals 120000
    And output "dagtekening_voorschot" equals "2025-02-10"
    And output "berekeningsjaar" equals 2025

  Scenario: Zonder voorschot op de aanvraag is er geen voorschot
    Given parameter "grams" is the collection:
      | id | root | sequence | event              | stage     | voorschotbedrag | effective_date | period |
      | a1 | a1   | 1        | aanvraag_ontvangen | null      | null            | 2024-11-04     | null   |
      | w1 | a2   | 7        | voorschot_verleend | VOORSCHOT | 999900          | 2025-03-01     | 2025   |
    And the following parameters:
      | root | a1 |
    When I evaluate outputs "laatste_voorschot, voorschotbedrag, dagtekening_voorschot, berekeningsjaar" of "fictief_beleid_kroniek_toeslagen"
    Then output "laatste_voorschot" is absent
    And output "voorschotbedrag" is absent
    And output "dagtekening_voorschot" is absent
    And output "berekeningsjaar" is absent

  # Art. 2: de rekening uit de aanvraag.
  Scenario: De rekening is die uit de aanvraag
    Given parameter "grams" is the collection:
      | id | root | sequence | event              | rekeningnummer     |
      | a1 | a1   | 1        | aanvraag_ontvangen | NL00TEST0123456789 |
    And the following parameters:
      | root | a1 |
    When I evaluate "rekeningnummer" of "fictief_beleid_kroniek_toeslagen"
    Then output "rekeningnummer" equals "NL00TEST0123456789"

  Scenario: Een aanvraag zonder rekening heeft geen rekening
    Given parameter "grams" is the collection:
      | id | root | sequence | event              | rekeningnummer |
      | a1 | a1   | 1        | aanvraag_ontvangen | null           |
    And the following parameters:
      | root | a1 |
    When I evaluate "rekeningnummer" of "fictief_beleid_kroniek_toeslagen"
    Then output "rekeningnummer" is absent

  Scenario: Zonder aanvraag met dit kenmerk is er geen rekening
    Given parameter "grams" is the collection:
      | id | root | sequence | event              | rekeningnummer     |
      | a2 | a2   | 1        | aanvraag_ontvangen | NL00TEST0123456789 |
    And the following parameters:
      | root | a1 |
    When I evaluate "rekeningnummer" of "fictief_beleid_kroniek_toeslagen"
    Then output "rekeningnummer" is absent

  # Art. 3: wat de bank niet bijschreef, min wat sindsdien opnieuw is
  # opgedragen. December mislukt (8333) en gaat mee met januari; januari
  # (8333 + 8333) mislukt ook, en staat dan helemaal open.
  Scenario: Achterstallig is wat mislukte en nog niet opnieuw is opgedragen
    Given parameter "grams" is the collection:
      | id | root | sequence | event                  | mislukt_bedrag | meegenomen_achterstand |
      | o1 | a1   | 3        | betaalopdracht_gegeven | null           | 0                      |
      | m1 | a1   | 4        | betaling_mislukt       | 8333           | null                   |
      | o2 | a1   | 5        | betaalopdracht_gegeven | null           | 8333                   |
      | m2 | a1   | 6        | betaling_mislukt       | 16666          | null                   |
      | m9 | a2   | 9        | betaling_mislukt       | 5000           | null                   |
    And the following parameters:
      | root | a1 |
    When I evaluate "achterstallig_bedrag" of "fictief_beleid_kroniek_toeslagen"
    Then output "achterstallig_bedrag" equals 16666

  Scenario: Wat opnieuw is opgedragen en bijgeschreven, staat niet meer open
    Given parameter "grams" is the collection:
      | id | root | sequence | event                    | mislukt_bedrag | meegenomen_achterstand |
      | o1 | a1   | 3        | betaalopdracht_gegeven   | null           | 0                      |
      | m1 | a1   | 4        | betaling_mislukt         | 8333           | null                   |
      | o2 | a1   | 5        | betaalopdracht_gegeven   | null           | 8333                   |
      | b2 | a1   | 6        | voorschottermijn_betaald | null           | null                   |
    And the following parameters:
      | root | a1 |
    When I evaluate "achterstallig_bedrag" of "fictief_beleid_kroniek_toeslagen"
    Then output "achterstallig_bedrag" equals 0
