Feature: Huurtoeslag 2026 — verzegelde rekenvoorbeelden uit dossier 36608

  # Mechanische transcriptie van scenarios/ORACLE.md (verzegeld in commit
  # cd0d233d, VOOR het schrijven van de machine_readable-encoding). De
  # verwachte bedragen komen uit Kamerstukken II 2024/25, 36 608, nr. 6
  # (nota naar aanleiding van het verslag) en zijn hier niet aangepast.
  #
  # De documenten rekenen met ijkpunten van 2024 (het kabinet: de
  # parameters voor 2025/2026 "kunnen niet gedeeld worden omdat deze op
  # dit moment nog niet zijn vastgesteld"). De AOW-jaarbedragen hieronder
  # zijn daarom zo gekozen dat het minimum-inkomensijkpunt van art. 17
  # gelijk is aan het door het document gebruikte 2024-ijkpunt
  # (Regeling huurtoeslagparameters 2024, Stcrt. 2023, 31878):
  # eenpersoons 20.700; meerpersoons 26.975; het voorbeeld met een oudere
  # gebruikt het 2024-ouderenijkpunt 22.025, een categorie die de wet van
  # 2026 niet meer kent en die hier via het eenpersoonspad wordt gevoed.
  # De maximale huurprijsgrens (UHW, art. 5) speelt in de voorbeelden geen
  # rol en staat ruim boven de huren.

  Background:
    Given the calculation date is "2026-01-01"
    Given law "wet_verlaging_eigen_bijdrage_huurtoeslag" is loaded
    Given law "besluit_op_de_huurtoeslag" is loaded

  Scenario: Voorbeeld 1 — alleenstaande, huur 700, inkomen 25.000 (belofte 355,18)
    Given the following parameters:
      | huurprijs                             | 70000    |
      | maximale_huurprijsgrens_uhw           | 200000   |
      | rekeninkomen                          | 2500000  |
      | woont_met_partner_of_medebewoners     | false    |
      | huishouden_drie_of_meer_personen      | false    |
      | bewoner_21_of_ouder_of_kind_in_woning | true     |
      | jonger_dan_21_met_handicap            | false    |
      | aow_bruto_jaarbedrag_alleenstaande    | 1836000  |
      | aow_bruto_jaarbedrag_gehuwde          | 1223150  |
    When I evaluate "hoogte_huurtoeslag" of "wet_op_de_huurtoeslag"
    Then the execution succeeds
    Then output "hoogte_huurtoeslag" equals 35518

  Scenario: Voorbeeld 2 — gezin (3+), huur 800, inkomen 25.000 (belofte 463)
    Given the following parameters:
      | huurprijs                             | 80000    |
      | maximale_huurprijsgrens_uhw           | 200000   |
      | rekeninkomen                          | 2500000  |
      | woont_met_partner_of_medebewoners     | true     |
      | huishouden_drie_of_meer_personen      | true     |
      | bewoner_21_of_ouder_of_kind_in_woning | true     |
      | jonger_dan_21_met_handicap            | false    |
      | aow_bruto_jaarbedrag_alleenstaande    | 1836000  |
      | aow_bruto_jaarbedrag_gehuwde          | 1223150  |
    When I evaluate "hoogte_huurtoeslag" of "wet_op_de_huurtoeslag"
    Then the execution succeeds
    Then output "hoogte_huurtoeslag" equals 46300

  Scenario: Voorbeeld 3 — alleenstaande oudere, huur 1.000, inkomen 30.000 (belofte 315)
    Given the following parameters:
      | huurprijs                             | 100000   |
      | maximale_huurprijsgrens_uhw           | 200000   |
      | rekeninkomen                          | 3000000  |
      | woont_met_partner_of_medebewoners     | false    |
      | huishouden_drie_of_meer_personen      | false    |
      | bewoner_21_of_ouder_of_kind_in_woning | true     |
      | jonger_dan_21_met_handicap            | false    |
      | aow_bruto_jaarbedrag_alleenstaande    | 1968500  |
      | aow_bruto_jaarbedrag_gehuwde          | 1223150  |
    When I evaluate "hoogte_huurtoeslag" of "wet_op_de_huurtoeslag"
    Then the execution succeeds
    Then output "hoogte_huurtoeslag" equals 31500
