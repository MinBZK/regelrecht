Feature: Lage-inkomensvoordeel (Wtl artikel 3.1 + 3.2)
  Als werkgever
  Wil ik weten of ik recht heb op het lage-inkomensvoordeel
  Zodat ik per verloond uur tegemoetkoming op de loonbelasting krijg

  # Bron: Wtl artikel 3.1 (recht) + 3.2.1 (hoogte). Peildatum 2024-01-01.
  # NB: LIV is per 2025-01-01 afgeschaft door Wet 36458.
  #
  # 2024-grenzen:
  #   - uurloon ondergrens € 14,33 (1433 eurocent)
  #   - uurloon bovengrens € 14,91 (1491 eurocent)
  #   - minimum 1248 verloonde uren
  #   - bedrag € 0,49 per verloond uur (49 eurocent)
  #   - maximum € 960 per werknemer per jaar (96000 eurocent)

  Background:
    Given the calculation date is "2024-06-01"

  # Werknemer met €15/uur (1500 eurocent), 1500 verloonde uren.
  # Uurloon (1500) > bovengrens (1491) → geen recht.
  Scenario: Uurloon boven LIV-bovengrens geeft geen recht
    Given a citizen with the following data:
      | bsn                                          | 999990030 |
      | jaarloon_eurocent                            | 2250000   |
      | verloonde_uren                               | 1500      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_liv,gemiddeld_uurloon_eurocent,voldoet_aan_uurloongrens,hoogte_liv_per_jaar_eurocent"
    Then the execution succeeds
    And the output "gemiddeld_uurloon_eurocent" is "1500"
    And the output "voldoet_aan_uurloongrens" is "false"
    And the output "heeft_recht_op_liv" is "false"
    And the output "hoogte_liv_per_jaar_eurocent" is "0"

  # Werknemer met €14,50/uur (1450 eurocent), 1500 verloonde uren.
  # Uurloon binnen grenzen, > 1248 uren, niet pensioengerechtigd → recht.
  # Hoogte = MIN(49 × 1500, 96000) = MIN(73500, 96000) = 73500 eurocent (€735).
  Scenario: Uurloon binnen LIV-grenzen en voldoende uren geeft recht
    Given a citizen with the following data:
      | bsn                                          | 999990031 |
      | jaarloon_eurocent                            | 2175000   |
      | verloonde_uren                               | 1500      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_liv,hoogte_liv_per_jaar_eurocent"
    Then the execution succeeds
    And the output "heeft_recht_op_liv" is "true"
    And the output "hoogte_liv_per_jaar_eurocent" is "73500"

  # Werknemer met €14,40/uur (1440 eurocent), 2200 verloonde uren (fulltime).
  # 49 × 2200 = 107800, gecapt op 96000 (€960 maximum).
  Scenario: Maximum LIV bij fulltime werknemer wordt gecapt op 96000 eurocent
    Given a citizen with the following data:
      | bsn                                          | 999990032 |
      | jaarloon_eurocent                            | 3168000   |
      | verloonde_uren                               | 2200      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_liv,hoogte_liv_per_jaar_eurocent"
    Then the execution succeeds
    And the output "heeft_recht_op_liv" is "true"
    And the output "hoogte_liv_per_jaar_eurocent" is "96000"

  # Werknemer met te weinig uren (1000) — geen recht ondanks correct uurloon.
  Scenario: Onvoldoende verloonde uren geeft geen recht
    Given a citizen with the following data:
      | bsn                                          | 999990033 |
      | jaarloon_eurocent                            | 1450000   |
      | verloonde_uren                               | 1000      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_liv,voldoet_aan_minimum_verloonde_uren"
    Then the execution succeeds
    And the output "voldoet_aan_minimum_verloonde_uren" is "false"
    And the output "heeft_recht_op_liv" is "false"

  # Pensioengerechtigde werknemer — uitsluiting lid 2.
  Scenario: Pensioengerechtigde werknemer is uitgesloten van LIV
    Given a citizen with the following data:
      | bsn                                          | 999990034 |
      | jaarloon_eurocent                            | 2175000   |
      | verloonde_uren                               | 1500      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | true      |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_liv,hoogte_liv_per_jaar_eurocent"
    Then the execution succeeds
    And the output "heeft_recht_op_liv" is "false"
    And the output "hoogte_liv_per_jaar_eurocent" is "0"
