Feature: Tegemoetkoming leenstelselstudenten
  Als oud-student die onder het leenstelsel heeft gestudeerd
  Wil ik weten of ik recht heb op een tegemoetkoming en hoe deze wordt toegekend
  Zodat ik weet wat ik kan verwachten

  # Wet studiefinanciering 2000, Art. 12.30 (eligibility + bedrag)
  # Besluit studiefinanciering 2000, Hoofdstuk 8a (Art. 21b-21c)

  Background:
    Given the calculation date is "2025-01-01"

  # =================================================================
  # Art. 12.30 WSF2000 — Eligibility and amount
  # =================================================================

  Scenario: Student met 48 maanden SF en hbo-bachelor — heeft recht
    # Art. 12.30 lid 2+3: alle voorwaarden voldaan
    # Bedrag: €29,92 × 48 = 143616, maar cap op diplomatermijn (48) → 143616
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | true             |
      | maanden_studiefinanciering           | 48               |
      | heeft_studiefinanciering_aangevraagd | true             |
      | type_opleiding                       | hbo_bachelor     |
      | datum_eerste_inschrijving            | 2016-09-01       |
      | datum_diploma                        | 2020-07-01       |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "is_rechthebbende" is "true"
    And the output "tegemoetkoming_bedrag" is "143616"

  Scenario: Student met 12 maanden SF — minimale aanspraak
    # Art. 12.30 lid 2b: minimum 12 maanden
    # Bedrag: €29,92 × 12 = 35904
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | true             |
      | maanden_studiefinanciering           | 12               |
      | heeft_studiefinanciering_aangevraagd | true             |
      | type_opleiding                       | hbo_bachelor     |
      | datum_eerste_inschrijving            | 2016-09-01       |
      | datum_diploma                        | 2020-07-01       |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "is_rechthebbende" is "true"
    And the output "tegemoetkoming_bedrag" is "35904"

  Scenario: Student met 11 maanden SF — onder minimum, geen recht
    # Art. 12.30 lid 2b: ten minste twaalf maanden vereist
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | true             |
      | maanden_studiefinanciering           | 11               |
      | heeft_studiefinanciering_aangevraagd | true             |
      | type_opleiding                       | hbo_bachelor     |
      | datum_eerste_inschrijving            | 2016-09-01       |
      | datum_diploma                        | 2020-07-01       |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "is_rechthebbende" is "false"
    And the output "tegemoetkoming_bedrag" is "0"

  Scenario: Student buiten diplomatermijn afgestudeerd — geen recht
    # Art. 12.30 lid 2c: binnen diplomatermijn (4 jaar) vereist
    # Inschrijving 2016-09-01, diploma 2021-10-01 → > 4 jaar
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | true             |
      | maanden_studiefinanciering           | 48               |
      | heeft_studiefinanciering_aangevraagd | true             |
      | type_opleiding                       | hbo_bachelor     |
      | datum_eerste_inschrijving            | 2016-09-01       |
      | datum_diploma                        | 2021-10-01       |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "is_rechthebbende" is "false"
    And the output "tegemoetkoming_bedrag" is "0"

  Scenario: Student niet tijdens leenstelsel — geen recht
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | false            |
      | maanden_studiefinanciering           | 48               |
      | heeft_studiefinanciering_aangevraagd | true             |
      | type_opleiding                       | hbo_bachelor     |
      | datum_eerste_inschrijving            | 2016-09-01       |
      | datum_diploma                        | 2020-07-01       |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "is_rechthebbende" is "false"
    And the output "tegemoetkoming_bedrag" is "0"

  Scenario: Student met niet-kwalificerend diploma — geen recht (Finding 4)
    # Art. 5.7: alleen specifieke opleidingstypen kwalificeren
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | true             |
      | maanden_studiefinanciering           | 48               |
      | heeft_studiefinanciering_aangevraagd | true             |
      | type_opleiding                       | propedeuse       |
      | datum_eerste_inschrijving            | 2016-09-01       |
      | datum_diploma                        | 2020-07-01       |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "is_rechthebbende" is "false"
    And the output "tegemoetkoming_bedrag" is "0"

  Scenario: Student zonder SF maar binnen 10 jaar — heeft recht (Finding 2)
    # Art. 12.30 lid 2c: "indien hij geen studiefinanciering heeft aangevraagd,
    # binnen tien jaar nadat hij zich voor het eerst heeft ingeschreven"
    # Inschrijving 2016-09-01, diploma 2020-06-01 → binnen 10 jaar
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | true              |
      | maanden_studiefinanciering           | 14               |
      | heeft_studiefinanciering_aangevraagd | false             |
      | type_opleiding                       | wo_bachelor_en_master |
      | datum_eerste_inschrijving            | 2016-09-01       |
      | datum_diploma                        | 2020-07-01       |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "binnen_termijn" is "true"
    And the output "is_rechthebbende" is "true"

  Scenario: Student zonder SF en buiten 10 jaar — geen recht (Finding 2)
    # Inschrijving 2015-09-01, diploma 2026-10-01 → meer dan 10 jaar
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | true              |
      | maanden_studiefinanciering           | 14               |
      | heeft_studiefinanciering_aangevraagd | false             |
      | type_opleiding                       | hbo_bachelor      |
      | datum_eerste_inschrijving            | 2015-09-01       |
      | datum_diploma                        | 2026-10-01       |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "binnen_termijn" is "false"
    And the output "is_rechthebbende" is "false"

  Scenario: Student met 60 maanden SF — bedrag gecapt op diplomatermijn (Finding 3)
    # Art. 12.30 lid 3: "tot een maximum van de periode, genoemd in artikel 5.2"
    # 60 maanden SF maar diplomatermijn = 48, dus cap: €29,92 × 48 = 143616
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | true             |
      | maanden_studiefinanciering           | 60               |
      | heeft_studiefinanciering_aangevraagd | true             |
      | type_opleiding                       | hbo_bachelor     |
      | datum_eerste_inschrijving            | 2016-09-01       |
      | datum_diploma                        | 2020-07-01       |
    When the tegemoetkoming eligibility is executed for wet_studiefinanciering_2000 article 12.30
    Then the output "is_rechthebbende" is "true"
    And the output "tegemoetkoming_bedrag" is "143616"

  # =================================================================
  # Art. 21b Besluit SF2000 — Ambtshalve of op aanvraag
  # =================================================================

  Scenario: Student met SF — alle gegevens uit SF-admin — ambtshalve
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | true             |
      | maanden_studiefinanciering           | 48               |
      | heeft_studiefinanciering_aangevraagd | true             |
      | type_opleiding                       | hbo_bachelor     |
      | datum_eerste_inschrijving            | 2016-09-01       |
      | datum_diploma                        | 2020-07-01       |
      | gegeven_inschrijving_ho_bekend       | true             |
      | gegeven_studiefinanciering_bekend    | true             |
      | gegeven_diploma_bekend               | true             |
      | heeft_aanvraag_ingediend             | false            |
    When the tegemoetkoming toekenning is executed for besluit_studiefinanciering_2000 article 21b
    Then the output "minister_beschikt_over_benodigde_gegevens" is "true"
    And the output "wijze_van_toekenning" is "AMBTSHALVE"

  Scenario: Student zonder SF maar bekend via ROD — ambtshalve (wendbare wetgeving)
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | true              |
      | maanden_studiefinanciering           | 14               |
      | heeft_studiefinanciering_aangevraagd | false             |
      | type_opleiding                       | wo_bachelor_en_master |
      | datum_eerste_inschrijving            | 2016-09-01       |
      | datum_diploma                        | 2020-07-01       |
      | gegeven_inschrijving_ho_bekend       | true              |
      | gegeven_studiefinanciering_bekend    | true              |
      | gegeven_diploma_bekend               | true              |
      | heeft_aanvraag_ingediend             | false             |
    When the tegemoetkoming toekenning is executed for besluit_studiefinanciering_2000 article 21b
    Then the output "minister_beschikt_over_benodigde_gegevens" is "true"
    And the output "wijze_van_toekenning" is "AMBTSHALVE"

  Scenario: Buitenlandstudent — diploma niet bij DUO — op aanvraag
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | true             |
      | maanden_studiefinanciering           | 36               |
      | heeft_studiefinanciering_aangevraagd | true             |
      | type_opleiding                       | hbo_bachelor     |
      | datum_eerste_inschrijving            | 2016-09-01       |
      | datum_diploma                        | 2020-07-01       |
      | gegeven_inschrijving_ho_bekend       | true             |
      | gegeven_studiefinanciering_bekend    | true             |
      | gegeven_diploma_bekend               | false            |
      | heeft_aanvraag_ingediend             | true             |
    When the tegemoetkoming toekenning is executed for besluit_studiefinanciering_2000 article 21b
    Then the output "minister_beschikt_over_benodigde_gegevens" is "false"
    And the output "wijze_van_toekenning" is "OP_AANVRAAG"

  Scenario: Geen gegevens en geen aanvraag — geen toekenning
    Given a citizen with the following data:
      | was_ho_student_tijdens_leenstelsel   | true             |
      | maanden_studiefinanciering           | 48               |
      | heeft_studiefinanciering_aangevraagd | true             |
      | type_opleiding                       | hbo_bachelor     |
      | datum_eerste_inschrijving            | 2016-09-01       |
      | datum_diploma                        | 2020-07-01       |
      | gegeven_inschrijving_ho_bekend       | false            |
      | gegeven_studiefinanciering_bekend    | false            |
      | gegeven_diploma_bekend               | false            |
      | heeft_aanvraag_ingediend             | false            |
    When the tegemoetkoming toekenning is executed for besluit_studiefinanciering_2000 article 21b
    Then the output "minister_beschikt_over_benodigde_gegevens" is "false"
    And the output "wijze_van_toekenning" is "GEEN_TOEKENNING"

  # =================================================================
  # Art. 21c Besluit SF2000 — Wijze van verstrekking
  # =================================================================

  Scenario: Tegemoetkoming bij openstaande studieschuld — kwijtschelding
    Given a citizen with the following data:
      | heeft_openstaande_studieschuld | true   |
      | tegemoetkoming_bedrag          | 143616 |
      | openstaande_studieschuld       | 500000 |
    When the tegemoetkoming verstrekking is executed for besluit_studiefinanciering_2000 article 21c
    Then the output "wijze_van_verstrekking" is "KWIJTSCHELDING"
    And the output "kwijtschelding_bedrag" is "143616"
    And the output "uitbetaling_bedrag" is "0"

  Scenario: Tegemoetkoming zonder studieschuld — uitbetaling
    Given a citizen with the following data:
      | heeft_openstaande_studieschuld | false  |
      | tegemoetkoming_bedrag          | 143616 |
      | openstaande_studieschuld       | 0      |
    When the tegemoetkoming verstrekking is executed for besluit_studiefinanciering_2000 article 21c
    Then the output "wijze_van_verstrekking" is "UITBETALING"
    And the output "kwijtschelding_bedrag" is "0"
    And the output "uitbetaling_bedrag" is "143616"

  Scenario: Tegemoetkoming groter dan studieschuld — kwijtschelding en uitbetaling
    Given a citizen with the following data:
      | heeft_openstaande_studieschuld | true   |
      | tegemoetkoming_bedrag          | 143616 |
      | openstaande_studieschuld       | 50000  |
    When the tegemoetkoming verstrekking is executed for besluit_studiefinanciering_2000 article 21c
    Then the output "wijze_van_verstrekking" is "KWIJTSCHELDING_EN_UITBETALING"
    And the output "kwijtschelding_bedrag" is "50000"
    And the output "uitbetaling_bedrag" is "93616"
