Feature: Unit-of-measurement enforcement (RFC-019)
  Values carry a unit of measurement so they cannot be silently
  misinterpreted during a calculation. The engine rejects an operation
  that combines incompatible declared units at runtime.

  Scenario: Adding eurocent to days fails at runtime
    # test_unit_mismatch adds a eurocent amount to a number of days. The
    # operation is declared at action level, so the static validator (which
    # walks value-expressions) passes the file, and this exercises the
    # runtime UnitMismatch guard in evaluate_action.
    Given the calculation date is "2025-01-01"
    When the law "test_unit_mismatch" is executed for outputs "resultaat"
    Then the execution fails with "unit mismatch"
