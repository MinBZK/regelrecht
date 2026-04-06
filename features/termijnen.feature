Feature: Termijnen — Algemene termijnenwet en AWB beslistermijnen
  Testscenario's voor termijnberekening conform de Algemene
  termijnenwet en de Omgevingswet beslistermijnen (art 16.64).

  De Algemene termijnenwet (ATW) verlengt termijnen die op een
  zaterdag, zondag of feestdag eindigen. De Omgevingswet wijkt
  af van de AWB-standaard beslistermijn (8 of 12 weken).

  Feestdagdatums komen uit corpus/context/nl/calendar/ — niet
  hardcoded in de test of in de wet-vertaling.

  Background:
    Given the calculation date is "2025-01-01"

  # === ATW Art 3.1: Feestdag-check ===
  # Art 3.1 heeft een untranslatable (bewegelijke feestdagen).
  # De engine weigert executie zolang deze niet is opgelost.

  Scenario: Feestdag-check faalt vanwege untranslatable
    Given a citizen with the following data:
      | datum | 2025-01-01 |
    And the holiday calendar for year "2025"
    When the termijnenwet holiday check is executed
    Then the execution fails with "Untranslatable construct"

  # === ATW Art 1.1: Termijnverlenging ===
  # Art 1.1 heeft een eigen inline feestdag-check (alle 9
  # feestdagen als parameters). De untranslatable op art 3.1
  # blokkeert alleen de endpoint is_feestdag, niet art 1.1.

  Scenario: Termijn eindigt op zaterdag — verlenging naar maandag
    # 2025-03-08 is zaterdag -> maandag 2025-03-10
    Given a citizen with the following data:
      | termijn_einddatum | 2025-03-08 |
    And the holiday calendar for year "2025"
    When the termijnenwet deadline extension is executed
    Then the output "verlengde_einddatum" is "2025-03-10"

  Scenario: Termijn eindigt op maandag — geen verlenging
    # 2025-03-10 is maandag, geen feestdag
    Given a citizen with the following data:
      | termijn_einddatum | 2025-03-10 |
    And the holiday calendar for year "2025"
    When the termijnenwet deadline extension is executed
    Then the output "verlengde_einddatum" is "2025-03-10"

  Scenario: Kerst op donderdag — verlenging over feestdagen + weekend
    # 2025-12-25 do (1e Kerstdag) -> vr 26 (2e Kerstdag)
    # -> za 27 -> zo 28 -> ma 29 werkdag!
    Given a citizen with the following data:
      | termijn_einddatum | 2025-12-25 |
    And the holiday calendar for year "2025"
    When the termijnenwet deadline extension is executed
    Then the output "verlengde_einddatum" is "2025-12-29"

  Scenario: Goede Vrijdag — verlenging over Paasweekend
    # 2025-04-18 vr (Goede Vrijdag) -> za 19 -> zo 20
    # -> ma 21 (Tweede Paasdag) -> di 22 werkdag!
    Given a citizen with the following data:
      | termijn_einddatum | 2025-04-18 |
    And the holiday calendar for year "2025"
    When the termijnenwet deadline extension is executed
    Then the output "verlengde_einddatum" is "2025-04-22"

  Scenario: Hemelvaartsdag op donderdag — verlenging naar vrijdag
    # 2025-05-29 do (Hemelvaartsdag) -> vr 30 werkdag!
    Given a citizen with the following data:
      | termijn_einddatum | 2025-05-29 |
    And the holiday calendar for year "2025"
    When the termijnenwet deadline extension is executed
    Then the output "verlengde_einddatum" is "2025-05-30"

  # === Cross-year: 2026 Pasen ===

  Scenario: 2026 Goede Vrijdag — verlenging over Paasweekend
    # 2026 Pasen = 5 april
    # 2026-04-03 vr (Goede Vrijdag) -> za 4 -> zo 5 (Pasen)
    # -> ma 6 (Tweede Paasdag) -> di 7 werkdag!
    Given a citizen with the following data:
      | termijn_einddatum | 2026-04-03 |
    And the holiday calendar for year "2026"
    When the termijnenwet deadline extension is executed
    Then the output "verlengde_einddatum" is "2026-04-07"

  # === ATW Art 4: Uitgesloten termijnen ===

  Scenario: Termijn in weken binnen grens — ATW is van toepassing
    # 6 weken bezwaartermijn -> ATW geldt (niet meer dan 12 weken / 3 maanden)
    Given a citizen with the following data:
      | termijn_eenheid                        | weken |
      | termijn_waarde                         | 6     |
      | betreft_bekendmaking_wettelijk_voorschrift | false |
      | betreft_vrijheidsbeneming              | false |
    When the termijnenwet scope check is executed
    Then the output "termijnenwet_van_toepassing" is "true"

  Scenario: Termijn langer dan 90 dagen — ATW niet van toepassing
    Given a citizen with the following data:
      | termijn_eenheid                        | dagen |
      | termijn_waarde                         | 120   |
      | betreft_bekendmaking_wettelijk_voorschrift | false |
      | betreft_vrijheidsbeneming              | false |
    When the termijnenwet scope check is executed
    Then the output "termijnenwet_van_toepassing" is "false"

  Scenario: Termijn in jaren — ATW niet van toepassing
    Given a citizen with the following data:
      | termijn_eenheid                        | jaren |
      | termijn_waarde                         | 5     |
      | betreft_bekendmaking_wettelijk_voorschrift | false |
      | betreft_vrijheidsbeneming              | false |
    When the termijnenwet scope check is executed
    Then the output "termijnenwet_van_toepassing" is "false"

  # === Omgevingswet Art 16.64: Beslistermijn ===
  # TODO: Omgevingswet te groot voor BDD loader (>1MB).
  # Beslistermijn scenarios volgen zodra #495 is opgelost.
  # Verwacht: 8 weken standaard, 12 met instemming.
