@tier:core
Feature: Date comparison and difference operations
  As an author of machine-readable law
  I want to compare dates and measure the span between them
  So that I can express deadlines and durations against a peildatum

  # Exercises RFC-021:
  #   - route A: type-safe comparison operators on dates (LESS_THAN_OR_EQUAL)
  #     and date-aware EQUALS in the mixed form (date string vs $referencedate)
  #   - route B: DATE_DIFF with an explicit unit (days / months / years)
  # and RFC-032:
  #   - DATE_PART reading a component (year / month / day) out of a date
  #   - START_OF truncating to the start of a year or a month
  # Driven against the test law corpus/regulation/nl/wet/test_date_operations.
  # $referencedate.iso is the peildatum (the calculation date).

  Scenario: A request filed before the peildatum is timely, with its duration measured
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | indieningsdatum | 2025-01-01 |
    When I evaluate "tijdig_ingediend" of "test_date_operations"
    Then output "tijdig_ingediend" is true
    When I evaluate "op_peildatum_ingediend" of "test_date_operations"
    Then output "op_peildatum_ingediend" is false
    When I evaluate "doorlooptijd_dagen" of "test_date_operations"
    Then output "doorlooptijd_dagen" equals 181
    When I evaluate "doorlooptijd_maanden" of "test_date_operations"
    Then output "doorlooptijd_maanden" equals 6
    When I evaluate "doorlooptijd_jaren" of "test_date_operations"
    Then output "doorlooptijd_jaren" equals 0

  Scenario: A request filed on the peildatum is timely with zero duration
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | indieningsdatum | 2025-07-01 |
    When I evaluate "tijdig_ingediend" of "test_date_operations"
    Then output "tijdig_ingediend" is true
    When I evaluate "op_peildatum_ingediend" of "test_date_operations"
    Then output "op_peildatum_ingediend" is true
    When I evaluate "doorlooptijd_dagen" of "test_date_operations"
    Then output "doorlooptijd_dagen" equals 0

  Scenario: A request filed after the peildatum is not timely, with a negative span
    Given the calculation date is "2025-01-01"
    Given the following parameters:
      | indieningsdatum | 2025-07-01 |
    When I evaluate "tijdig_ingediend" of "test_date_operations"
    Then output "tijdig_ingediend" is false
    When I evaluate "doorlooptijd_dagen" of "test_date_operations"
    Then output "doorlooptijd_dagen" equals -181

  Scenario: A multi-year span is measured in whole years
    Given the calculation date is "2025-06-01"
    Given the following parameters:
      | indieningsdatum | 2020-06-01 |
    When I evaluate "doorlooptijd_jaren" of "test_date_operations"
    Then output "doorlooptijd_jaren" equals 5

  Scenario: An end-of-month span counts as a whole month
    # Jan 31 has no Feb 31 counterpart; the clamp makes Jan 31 -> Feb 28 one
    # complete month, the same arithmetic AGE uses (BW art. 1:2).
    Given the calculation date is "2025-02-28"
    Given the following parameters:
      | indieningsdatum | 2025-01-31 |
    When I evaluate "doorlooptijd_maanden" of "test_date_operations"
    Then output "doorlooptijd_maanden" equals 1

  Scenario: The three components of a date are read back out of it
    # DATE_PART is the inverse of DATE, so `in` covers exactly year, month and
    # day. The day number is read through the article-49 branch below.
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | wijzigingsdatum | 2025-03-14 |
    When I evaluate "wijzigingsjaar" of "test_date_operations"
    Then output "wijzigingsjaar" equals 2025
    When I evaluate "wijzigingsmaand" of "test_date_operations"
    Then output "wijzigingsmaand" equals 3
    When I evaluate "begin_wijzigingsmaand" of "test_date_operations"
    Then output "begin_wijzigingsmaand" equals "2025-03-01"
    When I evaluate "begin_wijzigingsjaar" of "test_date_operations"
    Then output "begin_wijzigingsjaar" equals "2025-01-01"

  Scenario: Truncating a date that already sits on the start of its unit changes nothing
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | wijzigingsdatum | 2025-01-01 |
    When I evaluate "begin_wijzigingsmaand" of "test_date_operations"
    Then output "begin_wijzigingsmaand" equals "2025-01-01"
    When I evaluate "begin_wijzigingsjaar" of "test_date_operations"
    Then output "begin_wijzigingsjaar" equals "2025-01-01"

  Scenario: The first of February truncates to itself and moves on to March
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | wijzigingsdatum | 2025-02-01 |
    When I evaluate "wijzigingsmaand" of "test_date_operations"
    Then output "wijzigingsmaand" equals 2
    When I evaluate "begin_wijzigingsmaand" of "test_date_operations"
    Then output "begin_wijzigingsmaand" equals "2025-02-01"
    When I evaluate "eerste_dag_volgende_maand" of "test_date_operations"
    Then output "eerste_dag_volgende_maand" equals "2025-03-01"

  Scenario: A leap day truncates to the first of February and moves on to March
    # The peildatum stays inside the law's validity; the leap day is the
    # parameter under test.
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | wijzigingsdatum | 2024-02-29 |
    When I evaluate "begin_wijzigingsmaand" of "test_date_operations"
    Then output "begin_wijzigingsmaand" equals "2024-02-01"
    When I evaluate "eerste_dag_volgende_maand" of "test_date_operations"
    Then output "eerste_dag_volgende_maand" equals "2024-03-01"

  Scenario: The first day of the next month is the same for the first, the middle and the last day
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | wijzigingsdatum | 2025-03-01 |
    When I evaluate "eerste_dag_volgende_maand" of "test_date_operations"
    Then output "eerste_dag_volgende_maand" equals "2025-04-01"
    Given the following parameters:
      | wijzigingsdatum | 2025-03-15 |
    When I evaluate "eerste_dag_volgende_maand" of "test_date_operations"
    Then output "eerste_dag_volgende_maand" equals "2025-04-01"
    Given the following parameters:
      | wijzigingsdatum | 2025-03-31 |
    When I evaluate "eerste_dag_volgende_maand" of "test_date_operations"
    Then output "eerste_dag_volgende_maand" equals "2025-04-01"

  Scenario: The first day of the month after 31 January is 1 February
    # Truncate first, add afterwards: the truncated date is day 1, so the
    # day-clamping of DATE_ADD (31 Jan + 1 month = 28 Feb) never fires. The
    # reverse order reaches the same answer through that clamp.
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | wijzigingsdatum | 2025-01-31 |
    When I evaluate "eerste_dag_volgende_maand" of "test_date_operations"
    Then output "eerste_dag_volgende_maand" equals "2025-02-01"

  Scenario: The first day of the month after December is in the next year
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | wijzigingsdatum | 2025-12-17 |
    When I evaluate "eerste_dag_volgende_maand" of "test_date_operations"
    Then output "eerste_dag_volgende_maand" equals "2026-01-01"

  Scenario: A change on the first of the month counts in that month itself
    # Awir art. 49, the branch the day number decides. Day 1 is not "after the
    # first day of the month", so the change takes effect that same month.
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | wijzigingsdatum | 2025-03-01 |
    When I evaluate "ingangsdatum_wijziging" of "test_date_operations"
    Then output "ingangsdatum_wijziging" equals "2025-03-01"

  Scenario: A change on the second of the month counts from the next month
    Given the calculation date is "2025-07-01"
    Given the following parameters:
      | wijzigingsdatum | 2025-03-02 |
    When I evaluate "ingangsdatum_wijziging" of "test_date_operations"
    Then output "ingangsdatum_wijziging" equals "2025-04-01"
