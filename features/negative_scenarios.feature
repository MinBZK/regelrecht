Feature: Boundary and edge-case scenarios
  These scenarios verify the engine handles edge cases correctly:
  missing inputs, legal boundary conditions, null/absent delegation,
  and discretion articles with fixed outcomes.

  # ==========================================================================
  # Missing required inputs
  # ==========================================================================

  Scenario: Bijstand without gemeente_code resolves verlaging as null
    # Art 8 open_terms delegation requires gemeente_code. Without it,
    # the engine resolves optional open_terms as null (per #403).
    # verlaging_percentage = null → 0 reduction → full bijstand.
    Given the calculation date is "2024-06-01"
    And a citizen with the following data:
      | leeftijd                               | 35    |
      | is_alleenstaande                       | true  |
      | heeft_kostendelende_medebewoners       | false |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false |
      | gedragscategorie                       | 0     |
    When the bijstandsaanvraag is executed for participatiewet article 43
    Then the citizen has the right to bijstand
    And the uitkering_bedrag is "109171" eurocent

  Scenario: WOO Art 5.3 without informatie_datum fails
    # Art 5.3 requires informatie_datum to compute age.
    Given the calculation date is "2025-03-01"
    And a query with the following data:
      | peildatum | 2025-03-01 |
    When the WOO motivation requirement is checked
    Then the execution fails with "informatie_datum"

  # ==========================================================================
  # Legal boundary conditions
  # ==========================================================================

  Scenario: WOO Art 5.1 lid 7 — emissies override all refusal grounds
    # Art 5.1 lid 7: absolute and relative grounds do not apply to
    # milieu-informatie about emissies. Even state security and Crown
    # unity are overridden. This is an absolute right to disclosure.
    Given the calculation date is "2025-03-01"
    And a query with the following data:
      | raakt_eenheid_kroon                    | true  |
      | raakt_veiligheid_staat                 | true  |
      | bevat_vertrouwelijke_bedrijfsgegevens  | true  |
      | bevat_bijzondere_persoonsgegevens      | false |
      | bevat_identificatienummers             | false |
      | betrokkene_heeft_toestemming           | false |
      | persoonsgegevens_kennelijk_openbaar_gemaakt | false |
      | verstrekking_geen_inbreuk_levenssfeer  | false |
      | is_milieu_informatie                   | true  |
      | betreft_emissies                       | true  |
      | raakt_internationale_betrekkingen      | true  |
      | raakt_economische_belangen             | true  |
      | raakt_opsporing_vervolging             | true  |
      | raakt_inspectie_toezicht               | true  |
      | raakt_persoonlijke_levenssfeer         | true  |
      | raakt_concurrentiegevoelige_gegevens   | true  |
      | raakt_milieubescherming                | true  |
      | raakt_beveiliging_personen             | true  |
      | raakt_goed_functioneren_staat          | true  |
      | belang_openbaarheid_weegt_zwaarder     | false |
      | onevenredige_benadeling_ander_belang   | true  |
      | bedrijfsgegevens_ernstig_geschaad      | true  |
      | milieu_belang_openbaarheid_weegt_op    | false |
    When the WOO disclosure decision is executed
    Then the execution succeeds
    And the output "heeft_absolute_weigeringsgrond" is "false"
    And the output "heeft_relatieve_weigeringsgrond" is "false"
    And the output "openbaarmaking_toegestaan" is "true"

  Scenario: WOO Art 5.3 — exactly 5 years old does NOT trigger enhanced motivation
    # "ouder dan vijf jaar" = strictly older than 5 years.
    # Information that is exactly 5 years old (to the day) should NOT
    # trigger verzwaarde motiveringsplicht.
    Given the calculation date is "2025-03-01"
    And a query with the following data:
      | informatie_datum | 2020-03-01 |
      | peildatum        | 2025-03-01 |
    When the WOO motivation requirement is checked
    Then the execution succeeds
    And the output "informatie_leeftijd_jaren" is "5"
    And the output "verzwaarde_motiveringsplicht" is "false"

  Scenario: WOO Art 5.3 — over 5 years old triggers enhanced motivation
    # Information from 2019-01-01, checked on 2025-03-01 = 6 completed years.
    # AGE operation counts completed years, so this is strictly > 5.
    Given the calculation date is "2025-03-01"
    And a query with the following data:
      | informatie_datum | 2019-01-01 |
      | peildatum        | 2025-03-01 |
    When the WOO motivation requirement is checked
    Then the execution succeeds
    And the output "verzwaarde_motiveringsplicht" is "true"

  Scenario: Bijstand at exact age 21 — boundary is accepted
    # Art 21 checks leeftijd >= 21. Exactly 21 passes.
    Given the calculation date is "2024-06-01"
    And a citizen with the following data:
      | gemeente_code                          | GM0384 |
      | leeftijd                               | 21     |
      | is_alleenstaande                       | true   |
      | heeft_kostendelende_medebewoners       | false  |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false |
      | gedragscategorie                       | 0      |
    When the bijstandsaanvraag is executed for participatiewet article 43
    Then the citizen has the right to bijstand
    And the uitkering_bedrag is "109171" eurocent

  Scenario: Bijstand at age 20 — boundary is rejected
    # Art 21 checks leeftijd >= 21. Age 20 fails this check,
    # propagating heeft_recht_op_bijstand = false through Art 43.
    Given the calculation date is "2024-06-01"
    And a citizen with the following data:
      | gemeente_code                          | GM0384 |
      | leeftijd                               | 20     |
      | is_alleenstaande                       | true   |
      | heeft_kostendelende_medebewoners       | false  |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false |
      | gedragscategorie                       | 0      |
    When the bijstandsaanvraag is executed for participatiewet article 43
    Then the citizen does not have the right to bijstand
    And the uitkering_bedrag is "0" eurocent

  # ==========================================================================
  # Correct null / no-verordening handling
  # ==========================================================================

  Scenario: Bijstand without verordening — verlaging resolves to 0
    # Art 18 lid 2: verlaging happens "overeenkomstig de verordening".
    # No verordening = no legal basis for reduction = full bijstand.
    # Even with gedragscategorie 3 (100% reduction in Diemen).
    Given the calculation date is "2024-06-01"
    And a citizen with the following data:
      | gemeente_code                          | GM9999 |
      | leeftijd                               | 35     |
      | is_alleenstaande                       | true   |
      | heeft_kostendelende_medebewoners       | false  |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false |
      | gedragscategorie                       | 3      |
    When the bijstandsaanvraag is executed for participatiewet article 43
    Then the citizen has the right to bijstand
    And the uitkering_bedrag is "109171" eurocent

  # ==========================================================================
  # Discretion boundaries
  # ==========================================================================

  Scenario: Vreemdelingenwet Art 14 — minister_is_bevoegd is always true
    # Art 14 merely states that the Minister is competent to grant/deny.
    # The article has no conditions — it is a declaratory statement of
    # authority. The engine must always return true, never false.
    Given the calculation date is "2026-01-01"
    When the vreemdelingenwet beschikking is executed
    Then the execution succeeds
    And the output "minister_is_bevoegd" is "true"
