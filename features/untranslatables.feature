Feature: Untranslatables — RFC-012
  The engine handles articles with untranslatable constructs according to the
  configured mode: error (default), propagate, warn, or ignore.

  Background:
    Given the calculation date is "2025-01-01"

  # === Error mode (default) ===

  Scenario: Error mode rejects unaccepted untranslatable
    Given the untranslatable mode is "error"
    When the untranslatable test law is executed for output "afgerond_bedrag"
    Then the execution fails with "Untranslatable construct"

  Scenario: Error mode allows accepted untranslatable
    Given the untranslatable mode is "error"
    When the untranslatable test law is executed for output "som_deeltoeslagen"
    Then the execution succeeds

  # === Propagate mode (not yet implemented) ===

  Scenario: Propagate mode is not yet implemented
    Given the untranslatable mode is "propagate"
    When the untranslatable test law is executed for output "afgerond_bedrag"
    Then the execution fails with "propagate mode is not yet implemented"

  # === Warn mode ===

  Scenario: Warn mode executes unaccepted untranslatable with partial logic
    Given the untranslatable mode is "warn"
    And a citizen with the following data:
      | bedrag | 1234 |
    When the untranslatable test law is executed for output "afgerond_bedrag"
    Then the execution succeeds
    And the output "afgerond_bedrag" is "1234"

  # === Ignore mode ===

  Scenario: Ignore mode rejects unaccepted untranslatable
    Given the untranslatable mode is "ignore"
    When the untranslatable test law is executed for output "afgerond_bedrag"
    Then the execution fails with "Untranslatable construct"

  Scenario: Ignore mode allows accepted untranslatable
    Given the untranslatable mode is "ignore"
    When the untranslatable test law is executed for output "som_deeltoeslagen"
    Then the execution succeeds

  # === Articles without untranslatables work normally ===

  Scenario: Clean article executes normally in error mode
    Given the untranslatable mode is "error"
    When the untranslatable test law is executed for output "basistoeslag"
    Then the execution succeeds
    And the output "basistoeslag" is "1000"

  Scenario: Clean article with cross-ref executes normally
    Given the untranslatable mode is "error"
    When the untranslatable test law is executed for output "toegekende_toeslag"
    Then the execution succeeds
    And the output "toegekende_toeslag" is "2000"
