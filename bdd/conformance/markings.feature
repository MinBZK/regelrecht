@tier:untranslatable
Feature: Markings — RFC-012
  An article that carries a marking says the format cannot express one of its
  constructs. The engine handles such an article according to the configured
  untranslatable mode: error (default), propagate, warn, or ignore.

  Background:
    Given the calculation date is "2025-01-01"

  # === Error mode (default) ===

  Scenario: Error mode rejects an unaccepted marking
    Given the untranslatable mode is "error"
    When I evaluate "afgerond_bedrag" of "test_untranslatables"
    Then the execution fails with "Untranslatable construct"

  Scenario: Error mode allows an accepted marking
    Given the untranslatable mode is "error"
    When I evaluate "som_deeltoeslagen" of "test_untranslatables"
    Then the execution succeeds

  # === Propagate mode ===

  Scenario: Propagate mode taints outputs from marked articles
    Given the untranslatable mode is "propagate"
    Given the following parameters:
      | bedrag | 1234 |
    When I evaluate "afgerond_bedrag" of "test_untranslatables"
    Then the execution succeeds
    Then output "afgerond_bedrag" is tainted as untranslatable

  Scenario: Propagate mode allows unmarked articles to execute normally
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

  Scenario: Warn mode executes an unaccepted marking with partial logic
    Given the untranslatable mode is "warn"
    Given the following parameters:
      | bedrag | 1234 |
    When I evaluate "afgerond_bedrag" of "test_untranslatables"
    Then the execution succeeds
    Then output "afgerond_bedrag" equals 1234

  # === Ignore mode ===

  Scenario: Ignore mode rejects an unaccepted marking
    Given the untranslatable mode is "ignore"
    When I evaluate "afgerond_bedrag" of "test_untranslatables"
    Then the execution fails with "Untranslatable construct"

  Scenario: Ignore mode allows an accepted marking
    Given the untranslatable mode is "ignore"
    When I evaluate "som_deeltoeslagen" of "test_untranslatables"
    Then the execution succeeds

  # === Articles without markings work normally ===

  Scenario: Unmarked article executes normally in error mode
    Given the untranslatable mode is "error"
    When I evaluate "basistoeslag" of "test_untranslatables"
    Then the execution succeeds
    Then output "basistoeslag" equals 1000

  Scenario: Unmarked article with cross-ref executes normally
    Given the untranslatable mode is "error"
    When I evaluate "toegekende_toeslag" of "test_untranslatables"
    Then the execution succeeds
    Then output "toegekende_toeslag" equals 2000
