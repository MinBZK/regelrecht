Feature: Multi-output evaluation
  As a caller of the engine API
  I want to request multiple specific outputs from a single evaluation
  So that I avoid redundant evaluations while only receiving the data I asked for

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Request two outputs from different articles in the same law
    Given the untranslatable mode is "error"
    When the law "test_untranslatables" is executed for outputs "basistoeslag, toegekende_toeslag"
    Then the execution succeeds
    And the result contains exactly the outputs "basistoeslag, toegekende_toeslag"
    And the output "basistoeslag" is "1000"
    And the output "toegekende_toeslag" is "2000"

  Scenario: Single output via multi-output API returns only that output
    Given the untranslatable mode is "error"
    When the law "test_untranslatables" is executed for outputs "basistoeslag"
    Then the execution succeeds
    And the result contains exactly the outputs "basistoeslag"
    And the output "basistoeslag" is "1000"

  Scenario: Non-existent output returns error
    When the law "test_untranslatables" is executed for outputs "nonexistent_output"
    Then the execution fails with "Output 'nonexistent_output' not found"
