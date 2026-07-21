@tier:untranslatable
Feature: Untranslatables — RFC-012
  The engine handles articles with untranslatable constructs according to the
  configured mode: error (default), propagate, warn, or ignore.

  Background:
    Given the calculation date is "2025-01-01"

  # === Error mode (default) ===

  Scenario: Error mode rejects unaccepted untranslatable
    Given the untranslatable mode is "error"
    When I evaluate "afgerond_bedrag" of "test_untranslatables"
    Then the execution fails with "Untranslatable construct"

  Scenario: Error mode allows accepted untranslatable
    Given the untranslatable mode is "error"
    When I evaluate "som_deeltoeslagen" of "test_untranslatables"
    Then the execution succeeds

  # === Propagate mode ===

  Scenario: Propagate mode taints outputs from articles with untranslatables
    Given the untranslatable mode is "propagate"
    Given the following parameters:
      | bedrag | 1234 |
    When I evaluate "afgerond_bedrag" of "test_untranslatables"
    Then the execution succeeds
    Then output "afgerond_bedrag" is tainted as untranslatable

  Scenario: Propagate mode allows clean articles to execute normally
    Given the untranslatable mode is "propagate"
    When I evaluate "basistoeslag" of "test_untranslatables"
    Then the execution succeeds
    Then output "basistoeslag" equals 1000

  Scenario: Propagate mode taints downstream outputs via cross-ref
    Given the untranslatable mode is "propagate"
    When I evaluate "som_deeltoeslagen" of "test_untranslatables"
    Then the execution succeeds
    Then output "som_deeltoeslagen" is tainted as untranslatable

  # === Warn mode ===

  Scenario: Warn mode executes unaccepted untranslatable with partial logic
    Given the untranslatable mode is "warn"
    Given the following parameters:
      | bedrag | 1234 |
    When I evaluate "afgerond_bedrag" of "test_untranslatables"
    Then the execution succeeds
    Then output "afgerond_bedrag" equals 1234

  # === Ignore mode ===

  Scenario: Ignore mode rejects unaccepted untranslatable
    Given the untranslatable mode is "ignore"
    When I evaluate "afgerond_bedrag" of "test_untranslatables"
    Then the execution fails with "Untranslatable construct"

  Scenario: Ignore mode allows accepted untranslatable
    Given the untranslatable mode is "ignore"
    When I evaluate "som_deeltoeslagen" of "test_untranslatables"
    Then the execution succeeds

  # === Articles without untranslatables work normally ===

  Scenario: Clean article executes normally in error mode
    Given the untranslatable mode is "error"
    When I evaluate "basistoeslag" of "test_untranslatables"
    Then the execution succeeds
    Then output "basistoeslag" equals 1000

  Scenario: Clean article with cross-ref executes normally
    Given the untranslatable mode is "error"
    When I evaluate "toegekende_toeslag" of "test_untranslatables"
    Then the execution succeeds
    Then output "toegekende_toeslag" equals 2000
