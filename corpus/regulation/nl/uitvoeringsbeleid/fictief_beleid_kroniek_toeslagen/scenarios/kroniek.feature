Feature: Teruglezen van de kroniek van Toeslagen
  Het fictieve beleid van Toeslagen leest haar eigen kroniek terug als de
  gegevens die art. 16, 22 en 24 Awir en het beleid over de uitbetaling vragen:
  het voorschot dat geldt, de rekening uit de aanvraag, wat er achterstallig
  is, wat er is uitbetaald en de schatting van het inkomen. Het voorschot en de achterstand per
  berekeningsjaar (`period`): een aanvraag geldt ook voor de jaren erna. De kroniek is hier een tabel met een rij per vastgelegd feit: zijn
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
      | root            | a1   |
      | berekeningsjaar | 2025 |
    When I evaluate outputs "laatste_voorschot, voorschotbedrag, dagtekening_voorschot" of "fictief_beleid_kroniek_toeslagen"
    Then output "laatste_voorschot" equals 5
    And output "voorschotbedrag" equals 120000
    And output "dagtekening_voorschot" equals "2025-02-10"

  Scenario: Zonder voorschot op de aanvraag is er geen voorschot
    Given parameter "grams" is the collection:
      | id | root | sequence | event              | stage     | voorschotbedrag | effective_date | period |
      | a1 | a1   | 1        | aanvraag_ontvangen | null      | null            | 2024-11-04     | null   |
      | w1 | a2   | 7        | voorschot_verleend | VOORSCHOT | 999900          | 2025-03-01     | 2025   |
    And the following parameters:
      | root            | a1   |
      | berekeningsjaar | 2025 |
    When I evaluate outputs "laatste_voorschot, voorschotbedrag, dagtekening_voorschot" of "fictief_beleid_kroniek_toeslagen"
    Then output "laatste_voorschot" is absent
    And output "voorschotbedrag" is absent
    And output "dagtekening_voorschot" is absent

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
      | id | root | sequence | event                  | mislukt_bedrag | meegenomen_achterstand | period |
      | o1 | a1   | 3        | betaalopdracht_gegeven | null           | 0                      | 2025   |
      | m1 | a1   | 4        | betaling_mislukt       | 8333           | null                   | 2025   |
      | o2 | a1   | 5        | betaalopdracht_gegeven | null           | 8333                   | 2025   |
      | m2 | a1   | 6        | betaling_mislukt       | 16666          | null                   | 2025   |
      | m9 | a2   | 9        | betaling_mislukt       | 5000           | null                   | 2025   |
    And the following parameters:
      | root            | a1   |
      | berekeningsjaar | 2025 |
    When I evaluate "achterstallig_bedrag" of "fictief_beleid_kroniek_toeslagen"
    Then output "achterstallig_bedrag" equals 16666

  Scenario: Wat opnieuw is opgedragen en bijgeschreven, staat niet meer open
    Given parameter "grams" is the collection:
      | id | root | sequence | event                    | mislukt_bedrag | meegenomen_achterstand | period |
      | o1 | a1   | 3        | betaalopdracht_gegeven   | null           | 0                      | 2025   |
      | m1 | a1   | 4        | betaling_mislukt         | 8333           | null                   | 2025   |
      | o2 | a1   | 5        | betaalopdracht_gegeven   | null           | 8333                   | 2025   |
      | b2 | a1   | 6        | voorschottermijn_betaald | null           | null                   | 2025   |
    And the following parameters:
      | root            | a1   |
      | berekeningsjaar | 2025 |
    When I evaluate "achterstallig_bedrag" of "fictief_beleid_kroniek_toeslagen"
    Then output "achterstallig_bedrag" equals 0

  # Lid 1, per berekeningsjaar: een aanvraag geldt ook voor de jaren erna
  # (art. 15 lid 5 Awir). Het voorschot voor 2026 vervangt dat voor 2025
  # niet; elk jaar heeft zijn laatste voorschot.
  Scenario: Het voorschot voor het volgende jaar vervangt dat van dit jaar niet
    Given parameter "grams" is the collection:
      | id | root | sequence | event              | stage     | voorschotbedrag | effective_date | period |
      | a1 | a1   | 1        | aanvraag_ontvangen | null      | null            | 2025-01-02     | null   |
      | v1 | a1   | 2        | voorschot_verleend | VOORSCHOT | 100000          | 2025-01-02     | 2025   |
      | v2 | a1   | 9        | voorschot_verleend | VOORSCHOT | 110000          | 2025-11-01     | 2026   |
    And the following parameters:
      | root            | a1   |
      | berekeningsjaar | 2025 |
    When I evaluate outputs "laatste_voorschot, voorschotbedrag, dagtekening_voorschot" of "fictief_beleid_kroniek_toeslagen"
    Then output "laatste_voorschot" equals 2
    And output "voorschotbedrag" equals 100000
    And output "dagtekening_voorschot" equals "2025-01-02"

  # Art. 3, per berekeningsjaar: wat op het voorschot van 2025 mislukte, telt
  # niet bij de termijnen van 2026.
  Scenario: Achterstallig is per berekeningsjaar
    Given parameter "grams" is the collection:
      | id | root | sequence | event                  | mislukt_bedrag | meegenomen_achterstand | period |
      | o1 | a1   | 3        | betaalopdracht_gegeven | null           | 0                      | 2025   |
      | m1 | a1   | 4        | betaling_mislukt       | 8333           | null                   | 2025   |
      | o2 | a1   | 5        | betaalopdracht_gegeven | null           | 0                      | 2026   |
      | m2 | a1   | 6        | betaling_mislukt       | 9000           | null                   | 2026   |
    And the following parameters:
      | root            | a1   |
      | berekeningsjaar | 2025 |
    When I evaluate "achterstallig_bedrag" of "fictief_beleid_kroniek_toeslagen"
    Then output "achterstallig_bedrag" equals 8333

  # Art. 3a: uitbetaald is wat de bank op de termijnen van het voorschot voor
  # het berekeningsjaar bijschreef; een mislukte betaling, een ander jaar en
  # een andere aanvraag tellen niet.
  Scenario: Uitbetaald is wat de bank bijschreef op het voorschot van dit jaar
    Given parameter "grams" is the collection:
      | id | root | sequence | event                   | betaald_bedrag | period |
      | b1 | a1   | 3        | voorschottermijn_betaald | 14100          | 2025   |
      | m1 | a1   | 4        | betaling_mislukt         | null           | 2025   |
      | b2 | a1   | 5        | voorschottermijn_betaald | 14100          | 2025   |
      | b3 | a1   | 6        | voorschottermijn_betaald | 12500          | 2026   |
      | w1 | a2   | 7        | voorschottermijn_betaald | 99900          | 2025   |
    And the following parameters:
      | root            | a1   |
      | berekeningsjaar | 2025 |
    When I evaluate "uitbetaalde_voorschotten" of "fictief_beleid_kroniek_toeslagen"
    Then output "uitbetaalde_voorschotten" equals 28200

  Scenario: Zonder bijgeschreven termijn is er niets uitbetaald
    Given parameter "grams" is the collection:
      | id | root | sequence | event              | betaald_bedrag | period |
      | a1 | a1   | 1        | aanvraag_ontvangen | null           | null   |
    And the following parameters:
      | root            | a1   |
      | berekeningsjaar | 2025 |
    When I evaluate "uitbetaalde_voorschotten" of "fictief_beleid_kroniek_toeslagen"
    Then output "uitbetaalde_voorschotten" equals 0

  # Art. 4: de schatting van het inkomen is die uit de aanvraag (aanname: ook
  # voor een volgend berekeningsjaar).
  Scenario: De schatting is die uit de aanvraag
    Given parameter "grams" is the collection:
      | id | root | sequence | event              | vermoedelijk_toetsingsinkomen |
      | a1 | a1   | 1        | aanvraag_ontvangen | 2200000                       |
      | a2 | a2   | 2        | aanvraag_ontvangen | 3000000                       |
    And the following parameters:
      | root | a1 |
    When I evaluate "vermoedelijk_toetsingsinkomen" of "fictief_beleid_kroniek_toeslagen"
    Then output "vermoedelijk_toetsingsinkomen" equals 2200000

  Scenario: Een aanvraag zonder schatting heeft geen schatting
    Given parameter "grams" is the collection:
      | id | root | sequence | event              | vermoedelijk_toetsingsinkomen |
      | a1 | a1   | 1        | aanvraag_ontvangen | null                          |
    And the following parameters:
      | root | a1 |
    When I evaluate "vermoedelijk_toetsingsinkomen" of "fictief_beleid_kroniek_toeslagen"
    Then output "vermoedelijk_toetsingsinkomen" is absent
