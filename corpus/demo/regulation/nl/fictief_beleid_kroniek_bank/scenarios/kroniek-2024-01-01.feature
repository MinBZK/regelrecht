Feature: Teruglezen van de kroniek van de fictieve bank (fictief)
  De mutaties op een rekening: wat de bank erop bijschreef, min wat zij
  ervan afschreef. Boekingen op een andere rekening tellen niet.

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Bijgeschreven min afgeschreven, op deze rekening
    Given parameter "boekingen" is the collection:
      | event                     | rekeningnummer     | bijgeschreven_bedrag | afgeschreven_bedrag |
      | overboeking_bijgeschreven | NL00TEST0123456789 | 15409                | null                |
      | overboeking_geweigerd     | NL00TEST0123456789 | 0                    | null                |
      | overboeking_bijgeschreven | NL00TEST0000000000 | 99999                | null                |
      | incasso_afgeschreven      | NL00TEST0123456789 | null                 | 5000                |
    And the following parameters:
      | rekeningnummer | NL00TEST0123456789 |
    When I evaluate "saldomutaties" of "fictief_beleid_kroniek_bank"
    Then output "saldomutaties" equals 10409
