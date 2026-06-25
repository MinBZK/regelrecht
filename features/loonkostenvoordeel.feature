Feature: Loonkostenvoordeel (Wtl artikel 2.1 — vier categorieën)
  Als werkgever
  Wil ik weten op welk loonkostenvoordeel ik recht heb
  Zodat ik per uur tegemoetkoming krijg op de loonkosten van een
  werknemer uit een doelgroep

  # Bron: Wtl artikel 2.1 (verzoek), 2.5/2.9/2.13/2.17 (hoogte per
  # categorie), BWBR0037522. Peildatum 2024-01-01.
  #
  # 2024-tarieven:
  #   - oudere werknemer (a):           305 cent/uur, max 600000 cent
  #   - arbeidsgehandicapt (b):         305 cent/uur, max 600000 cent
  #   - banenafspraak (c):              101 cent/uur, max 200000 cent
  #   - herplaatsen arbeidsgehandicapt (d): 305 cent/uur, max 600000 cent
  #
  # Pensioengerechtigde leeftijd sluit alle vier categorieën uit
  # (art. 2.2 lid 2.a, 2.6 lid 3.a, 2.10 lid 2.a, 2.14 lid 2.a).

  Background:
    Given the calculation date is "2024-06-01"

  # Werknemer in categorie b (arbeidsgehandicapt), 2000 uur.
  # Bedrag = MIN(305 × 2000, 600000) = MIN(610000, 600000) = 600000.
  Scenario: Arbeidsgehandicapte werknemer met fulltime dienstverband krijgt LKV op maximum
    Given a citizen with the following data:
      | bsn                                          | 999990060 |
      | verloonde_uren                               | 2000      |
      | is_oudere_werknemer                          | false     |
      | is_arbeidsgehandicapte_werknemer             | true      |
      | is_herplaatsen_arbeidsgehandicapte           | false     |
      | is_doelgroep_banenafspraak                   | false     |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
      | heeft_loonaangifte_verzoek_ingediend         | true      |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_lkv,categorie_lkv,bedrag_per_uur_eurocent,maximum_per_jaar_eurocent,hoogte_lkv_per_jaar_eurocent"
    Then the execution succeeds
    And the output "heeft_recht_op_lkv" is "true"
    And the output "categorie_lkv" is "arbeidsgehandicapte_werknemer"
    And the output "bedrag_per_uur_eurocent" is "305"
    And the output "maximum_per_jaar_eurocent" is "600000"
    And the output "hoogte_lkv_per_jaar_eurocent" is "600000"

  # Werknemer in categorie c (banenafspraak), 1500 uur.
  # Bedrag = MIN(101 × 1500, 200000) = MIN(151500, 200000) = 151500.
  Scenario: Banenafspraak-werknemer krijgt LKV onder maximum
    Given a citizen with the following data:
      | bsn                                          | 999990061 |
      | verloonde_uren                               | 1500      |
      | is_oudere_werknemer                          | false     |
      | is_arbeidsgehandicapte_werknemer             | false     |
      | is_herplaatsen_arbeidsgehandicapte           | false     |
      | is_doelgroep_banenafspraak                   | true      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
      | heeft_loonaangifte_verzoek_ingediend         | true      |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_lkv,categorie_lkv,hoogte_lkv_per_jaar_eurocent"
    Then the execution succeeds
    And the output "heeft_recht_op_lkv" is "true"
    And the output "categorie_lkv" is "banenafspraak"
    And the output "hoogte_lkv_per_jaar_eurocent" is "151500"

  # Bij dubbele doelgroep: oudere wint volgens IF-volgorde.
  Scenario: Bij dubbele doelgroep wint oudere werknemer in IF-volgorde
    Given a citizen with the following data:
      | bsn                                          | 999990062 |
      | verloonde_uren                               | 1500      |
      | is_oudere_werknemer                          | true      |
      | is_arbeidsgehandicapte_werknemer             | false     |
      | is_herplaatsen_arbeidsgehandicapte           | false     |
      | is_doelgroep_banenafspraak                   | true      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
      | heeft_loonaangifte_verzoek_ingediend         | true      |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "categorie_lkv,bedrag_per_uur_eurocent"
    Then the execution succeeds
    And the output "categorie_lkv" is "oudere_werknemer"
    And the output "bedrag_per_uur_eurocent" is "305"

  Scenario: Werknemer zonder doelgroepstatus heeft geen recht op LKV
    Given a citizen with the following data:
      | bsn                                          | 999990063 |
      | verloonde_uren                               | 1800      |
      | is_oudere_werknemer                          | false     |
      | is_arbeidsgehandicapte_werknemer             | false     |
      | is_herplaatsen_arbeidsgehandicapte           | false     |
      | is_doelgroep_banenafspraak                   | false     |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
      | heeft_loonaangifte_verzoek_ingediend         | true      |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_lkv,categorie_lkv,hoogte_lkv_per_jaar_eurocent"
    Then the execution succeeds
    And the output "heeft_recht_op_lkv" is "false"
    And the output "categorie_lkv" is "geen"
    And the output "hoogte_lkv_per_jaar_eurocent" is "0"

  Scenario: Zonder loonaangifte-verzoek (art. 2.1) geen recht
    Given a citizen with the following data:
      | bsn                                          | 999990064 |
      | verloonde_uren                               | 1500      |
      | is_oudere_werknemer                          | true      |
      | is_arbeidsgehandicapte_werknemer             | false     |
      | is_herplaatsen_arbeidsgehandicapte           | false     |
      | is_doelgroep_banenafspraak                   | false     |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
      | heeft_loonaangifte_verzoek_ingediend         | false     |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_lkv,hoogte_lkv_per_jaar_eurocent"
    Then the execution succeeds
    And the output "heeft_recht_op_lkv" is "false"
    And the output "hoogte_lkv_per_jaar_eurocent" is "0"

  # ── Anti-cumulatie art. 4.1 lid 3 (peildatum 2025) ──────────────────
  # Per 2025-01-01 is het LKV oudere werknemer verlaagd naar € 1,35/uur
  # (max € 2.600), terwijl arbeidsgehandicapt € 3,05/uur (max € 6.000)
  # blijft. Een werknemer die in BEIDE categorieën valt krijgt de
  # hoogste berekende tegemoetkoming (art. 4.1 lid 3): de arbeids-
  # gehandicapt-LKV (€ 6.000), niet de lagere oudere-LKV (€ 2.600). Met
  # de oude eerst-passende-IF-volgorde zou ten onrechte oudere zijn
  # geselecteerd. Peildatum 2025-06-01 overschrijft de Background.
  Scenario: Dubbele doelgroep 2025 — hoogste tegemoetkoming wint (art. 4.1.3)
    Given the calculation date is "2025-06-01"
    And a citizen with the following data:
      | bsn                                          | 999990066 |
      | verloonde_uren                               | 2000      |
      | is_oudere_werknemer                          | true      |
      | is_arbeidsgehandicapte_werknemer             | true      |
      | is_herplaatsen_arbeidsgehandicapte           | false     |
      | is_doelgroep_banenafspraak                   | false     |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
      | heeft_loonaangifte_verzoek_ingediend         | true      |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_lkv,categorie_lkv,tegemoetkoming_oudere_eurocent,tegemoetkoming_arbeidsgehandicapte_eurocent,hoogte_lkv_per_jaar_eurocent"
    Then the execution succeeds
    And the output "heeft_recht_op_lkv" is "true"
    And the output "categorie_lkv" is "arbeidsgehandicapte_werknemer"
    And the output "tegemoetkoming_oudere_eurocent" is "260000"
    And the output "tegemoetkoming_arbeidsgehandicapte_eurocent" is "600000"
    And the output "hoogte_lkv_per_jaar_eurocent" is "600000"

  # Pensioengerechtigde werknemer is uitgesloten van alle vier
  # categorieën (art. 2.2 lid 2.a / 2.6 lid 3.a / 2.10 lid 2.a /
  # 2.14 lid 2.a).
  Scenario: Pensioengerechtigde oudere werknemer is uitgesloten van LKV
    Given a citizen with the following data:
      | bsn                                          | 999990065 |
      | verloonde_uren                               | 1500      |
      | is_oudere_werknemer                          | true      |
      | is_arbeidsgehandicapte_werknemer             | false     |
      | is_herplaatsen_arbeidsgehandicapte           | false     |
      | is_doelgroep_banenafspraak                   | false     |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | true      |
      | heeft_loonaangifte_verzoek_ingediend         | true      |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_lkv,hoogte_lkv_per_jaar_eurocent"
    Then the execution succeeds
    And the output "heeft_recht_op_lkv" is "false"
    And the output "hoogte_lkv_per_jaar_eurocent" is "0"
