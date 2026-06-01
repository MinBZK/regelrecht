Feature: Date comparison and difference operations
  As an author of machine-readable law
  I want to compare dates and measure the span between them
  So that I can express deadlines and durations against a peildatum

  # Exercises RFC-016:
  #   - route A: type-safe comparison operators on dates (LESS_THAN_OR_EQUAL)
  #   - route B: DATE_DIFF with an explicit unit (days / years)
  # Driven against the test law corpus/regulation/nl/wet/test_date_operations.
  # $referencedate.iso is the peildatum (the calculation date).

  Scenario: A request filed before the peildatum is timely, with its duration measured
    Given the calculation date is "2025-07-01"
    And a query with the following data:
      | indieningsdatum | 2025-01-01 |
    When the law "test_date_operations" is executed for outputs "tijdig_ingediend,doorlooptijd_dagen,doorlooptijd_jaren"
    Then the output "tijdig_ingediend" is "true"
    And the output "doorlooptijd_dagen" is "181"
    And the output "doorlooptijd_jaren" is "0"

  Scenario: A request filed on the peildatum is timely with zero duration
    Given the calculation date is "2025-07-01"
    And a query with the following data:
      | indieningsdatum | 2025-07-01 |
    When the law "test_date_operations" is executed for outputs "tijdig_ingediend,doorlooptijd_dagen"
    Then the output "tijdig_ingediend" is "true"
    And the output "doorlooptijd_dagen" is "0"

  Scenario: A request filed after the peildatum is not timely, with a negative span
    Given the calculation date is "2025-01-01"
    And a query with the following data:
      | indieningsdatum | 2025-07-01 |
    When the law "test_date_operations" is executed for outputs "tijdig_ingediend,doorlooptijd_dagen"
    Then the output "tijdig_ingediend" is "false"
    And the output "doorlooptijd_dagen" is "-181"

  Scenario: A multi-year span is measured in whole years
    Given the calculation date is "2025-06-01"
    And a query with the following data:
      | indieningsdatum | 2020-06-01 |
    When the law "test_date_operations" is executed for outputs "doorlooptijd_jaren"
    Then the output "doorlooptijd_jaren" is "5"
