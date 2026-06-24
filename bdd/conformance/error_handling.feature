@tier:core
Feature: Engine error handling
  Engine-agnostic conformance for error reporting: a request for an output that
  does not exist must fail with a clear, output-naming message rather than
  silently returning nothing. Law-specific missing-input and null-delegation
  cases live next to their laws (bucket A).

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: A non-existent output fails with the output name
    When I evaluate "nonexistent_output" of "test_untranslatables"
    Then the execution fails with "nonexistent_output"
