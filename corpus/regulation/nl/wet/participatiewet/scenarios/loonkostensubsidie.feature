Feature: Loonkostensubsidie (Participatiewet artikel 10c + 10d)
  Als werkgever van een persoon uit de doelgroep banenafspraak
  Wil ik weten of ik recht heb op loonkostensubsidie en hoeveel
  Zodat ik de loonkosten van werknemer met loonwaarde onder minimum­
  loon krijg gecompenseerd

  # Bron: Participatiewet artikel 10d lid 1, 2, 3 en 4 (BWBR0015703).
  # Peildatum 2025-01-01.

  Background:
    Given the calculation date is "2025-01-01"

  # Persona: doelgroep banenafspraak met loonwaarde 50% van minimumloon.
  # WML+VB 2025: 1995 + 8% ≈ 2155 EUR/mnd = 215500 eurocent (rond gerekend).
  # Loonwaarde 50% = 107750 eurocent. Bruto = 215500 - 107750 = 107750.
  # Maximum 70% = 150850 eurocent.
  # Hoogte = MIN(107750, 150850) = 107750.
  Scenario: Doelgroep met loonwaarde 50% krijgt LKS van het verschil
    Given the following parameters:
      | bsn                                                | 999990050 |
      | behoort_tot_doelgroep_lks                          | true      |
      | kan_minimumloon_niet_verdienen                     | true      |
      | aanvraag_lks_ingediend_binnen_zes_maanden          | true      |
      | voorafgaand_relevante_onderwijsroute_of_doelgroep  | true      |
      | is_wsw_dienstbetrekking                            | false     |
      | loonwaarde_eurocent_per_maand                      | 107750    |
      | minimumloon_plus_vakantiebijslag_eurocent_per_maand | 215500   |
    When I evaluate "heeft_recht_op_lks" of "participatiewet"
    Then the execution succeeds
    And output "heeft_recht_op_lks" is true
    And output "bruto_subsidie_eurocent_per_maand" equals 107750
    And output "maximum_subsidie_eurocent_per_maand" equals 150850
    And output "hoogte_lks_eurocent_per_maand" equals 107750

  # Persona: doelgroep met zeer lage loonwaarde (10%) — bruto > 70%-cap.
  # Bruto = 215500 - 21550 = 193950. Cap = 150850. Hoogte = 150850.
  Scenario: Hoogte LKS wordt gecapt op 70 procent van WML+vakantiebijslag
    Given the following parameters:
      | bsn                                                | 999990051 |
      | behoort_tot_doelgroep_lks                          | true      |
      | kan_minimumloon_niet_verdienen                     | true      |
      | aanvraag_lks_ingediend_binnen_zes_maanden          | true      |
      | voorafgaand_relevante_onderwijsroute_of_doelgroep  | true      |
      | is_wsw_dienstbetrekking                            | false     |
      | loonwaarde_eurocent_per_maand                      | 21550     |
      | minimumloon_plus_vakantiebijslag_eurocent_per_maand | 215500   |
    When I evaluate "heeft_recht_op_lks" of "participatiewet"
    Then the execution succeeds
    And output "heeft_recht_op_lks" is true
    And output "hoogte_lks_eurocent_per_maand" equals 150850

  Scenario: Persoon zonder doelgroepvaststelling heeft geen recht op LKS
    Given the following parameters:
      | bsn                                                | 999990052 |
      | behoort_tot_doelgroep_lks                          | false     |
      | kan_minimumloon_niet_verdienen                     | true      |
      | aanvraag_lks_ingediend_binnen_zes_maanden          | true      |
      | voorafgaand_relevante_onderwijsroute_of_doelgroep  | true      |
      | is_wsw_dienstbetrekking                            | false     |
      | loonwaarde_eurocent_per_maand                      | 100000    |
      | minimumloon_plus_vakantiebijslag_eurocent_per_maand | 215500   |
    When I evaluate "heeft_recht_op_lks" of "participatiewet"
    Then the execution succeeds
    And output "heeft_recht_op_lks" is false
    And output "hoogte_lks_eurocent_per_maand" equals 0

  Scenario: Wsw-dienstbetrekking is uitgesloten van LKS
    Given the following parameters:
      | bsn                                                | 999990053 |
      | behoort_tot_doelgroep_lks                          | true      |
      | kan_minimumloon_niet_verdienen                     | true      |
      | aanvraag_lks_ingediend_binnen_zes_maanden          | true      |
      | voorafgaand_relevante_onderwijsroute_of_doelgroep  | true      |
      | is_wsw_dienstbetrekking                            | true      |
      | loonwaarde_eurocent_per_maand                      | 100000    |
      | minimumloon_plus_vakantiebijslag_eurocent_per_maand | 215500   |
    When I evaluate "heeft_recht_op_lks" of "participatiewet"
    Then the execution succeeds
    And output "heeft_recht_op_lks" is false

  Scenario: Aanvraag te laat ingediend (na 6 maanden) geeft geen recht
    Given the following parameters:
      | bsn                                                | 999990054 |
      | behoort_tot_doelgroep_lks                          | true      |
      | kan_minimumloon_niet_verdienen                     | true      |
      | aanvraag_lks_ingediend_binnen_zes_maanden          | false     |
      | voorafgaand_relevante_onderwijsroute_of_doelgroep  | true      |
      | is_wsw_dienstbetrekking                            | false     |
      | loonwaarde_eurocent_per_maand                      | 100000    |
      | minimumloon_plus_vakantiebijslag_eurocent_per_maand | 215500   |
    When I evaluate "heeft_recht_op_lks" of "participatiewet"
    Then the execution succeeds
    And output "heeft_recht_op_lks" is false
