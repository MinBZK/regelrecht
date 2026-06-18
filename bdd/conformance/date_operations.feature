@tier:core
Feature: Date comparison and difference operations
  As an author of machine-readable law
  I want to compare dates and measure the span between them
  So that I can express deadlines and durations against a peildatum

  # Exercises RFC-021:
  #   - route A: type-safe comparison operators on dates (LESS_THAN_OR_EQUAL)
  #     and date-aware EQUALS in the mixed form (date string vs $referencedate)
  #   - route B: DATE_DIFF with an explicit unit (days / months / years)
  # Driven against the test law corpus/regulation/nl/wet/test_date_operations.
  # $referencedate.iso is the peildatum (the calculation date).

  Scenario: A request filed before the peildatum is timely, with its duration measured
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | indieningsdatum | 2025-01-01 |
    When I evaluate outputs "tijdig_ingediend, op_peildatum_ingediend, doorlooptijd_dagen, doorlooptijd_maanden, doorlooptijd_jaren" of "test_date_operations"
    Then output "tijdig_ingediend" is true
    Then output "op_peildatum_ingediend" is false
    Then output "doorlooptijd_dagen" equals 181
    Then output "doorlooptijd_maanden" equals 6
    Then output "doorlooptijd_jaren" equals 0

  Scenario: A request filed on the peildatum is timely with zero duration
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | indieningsdatum | 2025-07-01 |
    When I evaluate outputs "tijdig_ingediend, op_peildatum_ingediend, doorlooptijd_dagen" of "test_date_operations"
    Then output "tijdig_ingediend" is true
    Then output "op_peildatum_ingediend" is true
    Then output "doorlooptijd_dagen" equals 0

  Scenario: A request filed after the peildatum is not timely, with a negative span
    Given the calculation date is "2025-01-01"
    Given the following parameters:
      | indieningsdatum | 2025-07-01 |
    When I evaluate outputs "tijdig_ingediend, doorlooptijd_dagen" of "test_date_operations"
    Then output "tijdig_ingediend" is false
    Then output "doorlooptijd_dagen" equals -181

  Scenario: A multi-year span is measured in whole years
    Given the calculation date is "2025-06-01"
    Given the following parameters:
      | indieningsdatum | 2020-06-01 |
    When I evaluate outputs "doorlooptijd_jaren" of "test_date_operations"
    Then output "doorlooptijd_jaren" equals 5

  Scenario: An end-of-month span counts as a whole month
    # Jan 31 has no Feb 31 counterpart; the clamp makes Jan 31 -> Feb 28 one
    # complete month, the same arithmetic AGE uses (BW art. 1:2).
    Given the calculation date is "2025-02-28"
    Given the following parameters:
      | indieningsdatum | 2025-01-31 |
    When I evaluate outputs "doorlooptijd_maanden" of "test_date_operations"
    Then output "doorlooptijd_maanden" equals 1
