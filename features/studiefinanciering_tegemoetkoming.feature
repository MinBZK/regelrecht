Feature: Tegemoetkoming leenstelselstudenten
  Als oud-student die onder het leenstelsel heeft gestudeerd
  Wil ik weten of ik recht heb op een tegemoetkoming en hoe deze wordt toegekend
  Zodat ik weet wat ik kan verwachten

  # Wet studiefinanciering 2000, Art. 12.30 (eligibility + bedrag)
  # Besluit studiefinanciering 2000, Hoofdstuk 8a (Art. 21b-21c)
  #
  # Bron: Nota van Toelichting, Stb. 2023, 187
  # URL: https://zoek.officielebekendmakingen.nl/stb-2023-187.html

  Background:
    Given the calculation date is "2025-01-01"

  # =================================================================
  # Art. 12.30 WSF2000 — Eligibility and amount
  # =================================================================

  Scenario: Student met 48 maanden SF heeft recht op tegemoetkoming
    # Art. 12.30 lid 2+3: €29,92 × 48 = 143616 eurocent
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel | true |
      | maanden_studiefinanciering         | 48   |
      | heeft_diploma_binnen_termijn       | true |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "is_rechthebbende" is "true"
    And the output "tegemoetkoming_bedrag" is "143616"

  Scenario: Student met 12 maanden SF — minimale aanspraak
    # Art. 12.30 lid 2b: ten minste twaalf maanden
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel | true |
      | maanden_studiefinanciering         | 12   |
      | heeft_diploma_binnen_termijn       | true |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "is_rechthebbende" is "true"
    And the output "tegemoetkoming_bedrag" is "35904"

  Scenario: Student met 11 maanden SF — onder minimum
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel | true |
      | maanden_studiefinanciering         | 11   |
      | heeft_diploma_binnen_termijn       | true |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "is_rechthebbende" is "false"
    And the output "tegemoetkoming_bedrag" is "0"

  Scenario: Student zonder diploma — geen recht
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel | true  |
      | maanden_studiefinanciering         | 48    |
      | heeft_diploma_binnen_termijn       | false |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "is_rechthebbende" is "false"
    And the output "tegemoetkoming_bedrag" is "0"

  Scenario: Student niet tijdens leenstelsel — geen recht
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel | false |
      | maanden_studiefinanciering         | 48    |
      | heeft_diploma_binnen_termijn       | true  |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "is_rechthebbende" is "false"
    And the output "tegemoetkoming_bedrag" is "0"

  # =================================================================
  # Art. 21b Besluit SF2000 — Ambtshalve of op aanvraag
  #
  # "Benodigde gegevens" = de drie inputs van Art. 12.30:
  #   1. HO-inschrijving tijdens leenstelsel (ROD of SF-admin)
  #   2. Maanden studiefinanciering (SF-admin)
  #   3. Diploma binnen termijn (ROD of SF-admin)
  #
  # MvT aanname: alleen SF-studenten bekend bij DUO
  # Werkelijkheid: ROD bevat ook gegevens van niet-SF studenten
  # =================================================================

  Scenario: Student met SF — alle gegevens uit SF-admin — ambtshalve
    # Bron: NvT Stb. 2023, 187, par. 2.2
    # Alle drie gegevens beschikbaar via SF-administratie
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel | true  |
      | maanden_studiefinanciering         | 48    |
      | heeft_diploma_binnen_termijn       | true  |
      | gegeven_inschrijving_ho_bekend     | true  |
      | gegeven_studiefinanciering_bekend  | true  |
      | gegeven_diploma_bekend             | true  |
      | heeft_aanvraag_ingediend           | false |
    When the tegemoetkoming toekenning is executed for besluit_studiefinanciering_2000 article 21b
    Then the output "minister_beschikt_over_benodigde_gegevens" is "true"
    And the output "wijze_van_toekenning" is "AMBTSHALVE"

  Scenario: Student zonder SF — MvT aanname: gegevens onbekend — op aanvraag
    # Bron: NvT Stb. 2023, 187, par. 2.2
    # MvT veronderstelde dat DUO GEEN gegevens had voor niet-SF studenten
    # Dus: gegeven_studiefinanciering_bekend = false (geen SF aangevraagd)
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel | true  |
      | maanden_studiefinanciering         | 0     |
      | heeft_diploma_binnen_termijn       | true  |
      | gegeven_inschrijving_ho_bekend     | false |
      | gegeven_studiefinanciering_bekend  | false |
      | gegeven_diploma_bekend             | false |
      | heeft_aanvraag_ingediend           | true  |
    When the tegemoetkoming toekenning is executed for besluit_studiefinanciering_2000 article 21b
    Then the output "minister_beschikt_over_benodigde_gegevens" is "false"
    And the output "wijze_van_toekenning" is "OP_AANVRAAG"

  Scenario: Student zonder SF maar bekend via register onderwijsdeelnemers — ambtshalve
    # *** KERN VAN DE CASUS "WENDBARE WETGEVING" ***
    #
    # De MvT ging ervan uit dat deze student een aanvraag zou moeten
    # indienen. Maar DUO bleek via het register onderwijsdeelnemers (ROD)
    # te beschikken over inschrijvings- en diplomagegevens.
    #
    # De wettekst zegt "beschikt over benodigde gegevens" — niet
    # "heeft studiefinanciering ontvangen". Dus: als DUO de gegevens
    # heeft (ongeacht de bron), is ambtshalve toekenning toegestaan.
    #
    # NB: gegeven_studiefinanciering_bekend = true want DUO weet dat
    # deze student 0 maanden SF had (dat is ook een gegeven).
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel | true  |
      | maanden_studiefinanciering         | 14    |
      | heeft_diploma_binnen_termijn       | true  |
      | gegeven_inschrijving_ho_bekend     | true  |
      | gegeven_studiefinanciering_bekend  | true  |
      | gegeven_diploma_bekend             | true  |
      | heeft_aanvraag_ingediend           | false |
    When the tegemoetkoming toekenning is executed for besluit_studiefinanciering_2000 article 21b
    Then the output "minister_beschikt_over_benodigde_gegevens" is "true"
    And the output "wijze_van_toekenning" is "AMBTSHALVE"

  Scenario: Buitenlandstudent — diploma niet bij DUO — op aanvraag
    # Bron: NvT Stb. 2023, 187, artikelsgewijze toelichting
    # Inschrijving en SF bekend, maar diploma behaald in buitenland
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel | true  |
      | maanden_studiefinanciering         | 36    |
      | heeft_diploma_binnen_termijn       | true  |
      | gegeven_inschrijving_ho_bekend     | true  |
      | gegeven_studiefinanciering_bekend  | true  |
      | gegeven_diploma_bekend             | false |
      | heeft_aanvraag_ingediend           | true  |
    When the tegemoetkoming toekenning is executed for besluit_studiefinanciering_2000 article 21b
    Then the output "minister_beschikt_over_benodigde_gegevens" is "false"
    And the output "wijze_van_toekenning" is "OP_AANVRAAG"

  Scenario: Geen gegevens en geen aanvraag — geen toekenning
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel | true  |
      | maanden_studiefinanciering         | 48    |
      | heeft_diploma_binnen_termijn       | true  |
      | gegeven_inschrijving_ho_bekend     | false |
      | gegeven_studiefinanciering_bekend  | false |
      | gegeven_diploma_bekend             | false |
      | heeft_aanvraag_ingediend           | false |
    When the tegemoetkoming toekenning is executed for besluit_studiefinanciering_2000 article 21b
    Then the output "minister_beschikt_over_benodigde_gegevens" is "false"
    And the output "wijze_van_toekenning" is "GEEN_TOEKENNING"

  # =================================================================
  # Art. 21c Besluit SF2000 — Wijze van verstrekking
  # =================================================================

  Scenario: Tegemoetkoming bij openstaande studieschuld — kwijtschelding
    # Art. 21c lid 1 sub a: schuld > tegemoetkoming → volledige kwijtschelding
    Given a citizen with the following data:
      | heeft_openstaande_studieschuld | true   |
      | tegemoetkoming_bedrag          | 143616 |
      | openstaande_studieschuld       | 500000 |
    When the tegemoetkoming verstrekking is executed for besluit_studiefinanciering_2000 article 21c
    Then the output "wijze_van_verstrekking" is "KWIJTSCHELDING"
    And the output "kwijtschelding_bedrag" is "143616"
    And the output "uitbetaling_bedrag" is "0"

  Scenario: Tegemoetkoming zonder studieschuld — uitbetaling
    # Art. 21c lid 1 sub b onder 1°
    Given a citizen with the following data:
      | heeft_openstaande_studieschuld | false  |
      | tegemoetkoming_bedrag          | 143616 |
      | openstaande_studieschuld       | 0      |
    When the tegemoetkoming verstrekking is executed for besluit_studiefinanciering_2000 article 21c
    Then the output "wijze_van_verstrekking" is "UITBETALING"
    And the output "kwijtschelding_bedrag" is "0"
    And the output "uitbetaling_bedrag" is "143616"

  Scenario: Tegemoetkoming groter dan studieschuld — kwijtschelding en uitbetaling
    # Art. 21c lid 1 sub b onder 2°
    Given a citizen with the following data:
      | heeft_openstaande_studieschuld | true   |
      | tegemoetkoming_bedrag          | 143616 |
      | openstaande_studieschuld       | 50000  |
    When the tegemoetkoming verstrekking is executed for besluit_studiefinanciering_2000 article 21c
    Then the output "wijze_van_verstrekking" is "KWIJTSCHELDING_EN_UITBETALING"
    And the output "kwijtschelding_bedrag" is "50000"
    And the output "uitbetaling_bedrag" is "93616"
