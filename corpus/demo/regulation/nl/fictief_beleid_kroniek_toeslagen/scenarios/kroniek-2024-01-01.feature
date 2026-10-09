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

  # Art. 5: het laatste inkomensgegeven over het jaar voor de BSN uit de
  # aanvraag; een inkomensgegeven over een ander jaar of van een ander telt
  # niet.
  Scenario: Het inkomensgegeven over het berekeningsjaar van de aanvrager
    Given parameter "grams" is the collection:
      | id | root | sequence | event                     | bsn       | kalenderjaar | inkomensgegeven | datum_vaststelling_aanslag |
      | a1 | a1   | 1        | aanvraag_ontvangen        | 999100001 | null         | null            | null                       |
      | i1 | i1   | 2        | inkomensgegeven_ontvangen | 999100001 | 2024         | 2415000         | 2025-04-15                 |
      | i2 | i2   | 3        | inkomensgegeven_ontvangen | 999100001 | 2025         | 1600000         | 2026-04-15                 |
      | i3 | i3   | 4        | inkomensgegeven_ontvangen | 999999990 | 2025         | 9900000         | 2026-04-01                 |
    And the following parameters:
      | root            | a1   |
      | berekeningsjaar | 2025 |
    When I evaluate outputs "inkomensgegeven, datum_vaststelling_aanslag" of "fictief_beleid_kroniek_toeslagen"
    Then output "inkomensgegeven" equals 1600000
    And output "datum_vaststelling_aanslag" equals "2026-04-15"

  Scenario: Zonder inkomensgegeven over het jaar is er geen
    Given parameter "grams" is the collection:
      | id | root | sequence | event                     | bsn       | kalenderjaar | inkomensgegeven | datum_vaststelling_aanslag |
      | a1 | a1   | 1        | aanvraag_ontvangen        | 999100001 | null         | null            | null                       |
      | i1 | i1   | 2        | inkomensgegeven_ontvangen | 999100001 | 2024         | 2415000         | 2025-04-15                 |
    And the following parameters:
      | root            | a1   |
      | berekeningsjaar | 2025 |
    When I evaluate outputs "inkomensgegeven, datum_vaststelling_aanslag" of "fictief_beleid_kroniek_toeslagen"
    Then output "inkomensgegeven" is absent
    And output "datum_vaststelling_aanslag" is absent

  # Art. 6: de laatste toekenning over het jaar, met wat zij nog uit te
  # betalen en terug te vorderen laat, en haar dagtekening.
  Scenario: De toekenning over het berekeningsjaar
    Given parameter "grams" is the collection:
      | id | root | sequence | event                 | stage      | period | nog_uit_te_betalen | terug_te_vorderen | effective_date |
      | a1 | a1   | 1        | aanvraag_ontvangen    | null       | null   | null               | null              | 2025-01-06     |
      | t1 | a1   | 9        | zorgtoeslag_toegekend | TOEKENNING | 2025   | 11400              | 0                 | 2026-04-15     |
      | t2 | a1   | 12       | zorgtoeslag_toegekend | TOEKENNING | 2026   | 0                  | 19000             | 2027-04-15     |
    And the following parameters:
      | root            | a1   |
      | berekeningsjaar | 2026 |
    When I evaluate outputs "nog_uit_te_betalen, terug_te_vorderen, dagtekening_toekenning" of "fictief_beleid_kroniek_toeslagen"
    Then output "nog_uit_te_betalen" equals 0
    And output "terug_te_vorderen" equals 19000
    And output "dagtekening_toekenning" equals "2027-04-15"

  # Art. 7: opgedragen min geweigerd.
  Scenario: Van de nabetaling is opgedragen wat de bank niet weigerde
    Given parameter "grams" is the collection:
      | id | root | sequence | event                 | period | bedrag | niet_nabetaald_bedrag |
      | n1 | a1   | 10       | nabetaling_opgedragen | 2025   | 11400  | null                  |
      | m1 | a1   | 11       | nabetaling_mislukt    | 2025   | null   | 11400                 |
      | n2 | a1   | 12       | nabetaling_opgedragen | 2025   | 11400  | null                  |
      | n3 | a1   | 13       | nabetaling_opgedragen | 2026   | 5000   | null                  |
    And the following parameters:
      | root            | a1   |
      | berekeningsjaar | 2025 |
    When I evaluate "opgedragen_nabetaling" of "fictief_beleid_kroniek_toeslagen"
    Then output "opgedragen_nabetaling" equals 11400

  # Art. 8: de terugvordering over het jaar en wat ervan is ingevorderd.
  Scenario: De terugvordering en wat ervan is ingevorderd
    Given parameter "grams" is the collection:
      | id | root | sequence | event                      | stage          | period | terugvorderingsbedrag | effective_date | incassobedrag | niet_geind_bedrag |
      | r1 | a1   | 13       | terugvordering_vastgesteld | TERUGVORDERING | 2026   | 19000                 | 2027-04-15     | null          | null              |
      | o1 | a1   | 14       | incasso_opgedragen         | null           | 2026   | null                  | 2027-05-01     | 19000         | null              |
      | f1 | a1   | 15       | incasso_mislukt            | null           | 2026   | null                  | 2027-05-01     | null          | 19000             |
    And the following parameters:
      | root            | a1   |
      | berekeningsjaar | 2026 |
    When I evaluate outputs "terugvorderingsbedrag, dagtekening_terugvordering, ingevorderd_bedrag" of "fictief_beleid_kroniek_toeslagen"
    Then output "terugvorderingsbedrag" equals 19000
    And output "dagtekening_terugvordering" equals "2027-04-15"
    And output "ingevorderd_bedrag" equals 0
