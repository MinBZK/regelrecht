Feature: Een incasso bij de fictieve bank (fictief, artikel 2)
  De bank schrijft een incasso af, tenzij de rekening onbekend of geblokkeerd
  is of het saldo lager is dan het bedrag. Het saldo is het beginsaldo plus
  wat zij bijschreef, min wat zij afschreef (het teruglezen van haar kroniek).

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Genoeg saldo: afgeschreven
    Given the following parameters:
      | incassokenmerk       | i1                 |
      | rekeningnummer       | NL00TEST0123456789 |
      | bedrag               | 19000              |
      | uitvoerdatum         | 2027-05-01         |
      | saldomutaties        | 500000             |
      | rekening_geblokkeerd | false              |
      | beginsaldo           | 42350              |
    When I evaluate outputs "saldo, afgeschreven, afgeschreven_bedrag" of "fictieve_bankvoorwaarden"
    Then output "saldo" equals 542350
    And output "afgeschreven" is true
    And output "afgeschreven_bedrag" equals 19000

  Scenario: Te weinig saldo: geweigerd
    Given the following parameters:
      | incassokenmerk       | i1                 |
      | rekeningnummer       | NL00TEST0123456789 |
      | bedrag               | 19000              |
      | uitvoerdatum         | 2027-05-01         |
      | saldomutaties        | 0                  |
      | rekening_geblokkeerd | false              |
      | beginsaldo           | 100                |
    When I evaluate outputs "afgeschreven, incasso_geweigerd, afgeschreven_bedrag, reden_incasso" of "fictieve_bankvoorwaarden"
    Then output "afgeschreven" is false
    And output "incasso_geweigerd" is true
    And output "afgeschreven_bedrag" equals 0
    And output "reden_incasso" equals "saldo ontoereikend"

  Scenario: Een geblokkeerde rekening: geweigerd
    Given the following parameters:
      | incassokenmerk       | i1                 |
      | rekeningnummer       | NL00TEST0123456789 |
      | bedrag               | 19000              |
      | uitvoerdatum         | 2027-05-01         |
      | saldomutaties        | 500000             |
      | rekening_geblokkeerd | true               |
      | beginsaldo           | 42350              |
    When I evaluate outputs "afgeschreven, reden_incasso" of "fictieve_bankvoorwaarden"
    Then output "afgeschreven" is false
    And output "reden_incasso" equals "rekening geblokkeerd"
