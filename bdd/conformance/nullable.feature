@tier:core
Feature: Absence is declared — RFC-036, nullable
  Whether "geen" is a value a field can take is a property of the field's
  type, not of the data that happens to arrive: a parameter, input or output
  declares it with `nullable: true`, and the default is that it never is. The
  engine holds every party to that declaration at the boundary where the value
  arrives. A register that delivers null for a field the law declares never
  absent has contradicted the law, and that is a data error reported there and
  then, not an `operand is null` three operations later. An output that is
  not nullable and still comes out null is the law saying nothing where it
  promised an answer. Unknown (a fact nobody has) is not governed by the flag
  and propagates as before.

  Driven against test_nullable (article 1 reads a nullable and a non-nullable
  rent, article 2 passes a nullable value to a nullable and to a strict
  output, articles 3 and 3a fetch the partner's birth year, article 4 has a
  nullable parameter, article 5 passes a nullable amount on as an optional
  parameter) and test_nullable_bron (the law that is called).

  Background:
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given the following "register" data with key "bsn" for law "test_nullable_bron":
      | bsn       | geboortejaar |
      | 999993653 | 1975         |
      | 999993641 | 1980         |

  Scenario: A null cell for a nullable input is a value the law tests
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn       | huur | huurgrens |
      | 999993653 | null | 700       |
    When I evaluate outputs "heeft_huur, huurgrens_plus" of "test_nullable"
    Then the execution succeeds
    Then output "heeft_huur" is false
    Then output "huurgrens_plus" equals 800

  Scenario: A null cell for an input the law declares never absent is a data error at the boundary
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn       | huur | huurgrens |
      | 999993653 | 650  | null      |
    # The register says there is no rent limit; the law said there always is.
    # The contradiction names the source, the field and the law, before any
    # action has run.
    When I evaluate "huurgrens_plus" of "test_nullable"
    Then the execution fails
    Then the execution fails with "declares as never absent"
    Then the execution fails with "huurgrens"
    Then the execution fails with "source register"

  Scenario: An empty cell is unknown whatever the declaration says
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn       | huur | huurgrens |
      | 999993653 |      |           |
    When I evaluate outputs "heeft_huur, huurgrens_plus" of "test_nullable"
    Then the execution succeeds
    Then output "heeft_huur" is unknown for lack of "huur"
    Then output "huurgrens_plus" is unknown for lack of "huurgrens"

  Scenario: A nullable output may be absent
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn       | toeslag |
      | 999993653 | 700     |
    # No case of the IF matches 700 and there is no default: the class is
    # absent, and the output declared that it may be.
    When I evaluate outputs "toeslagklasse, toeslag_strikt" of "test_nullable"
    Then the execution succeeds
    Then output "toeslagklasse" is absent
    Then output "toeslag_strikt" equals 700

  Scenario: An output the law declares never absent may not evaluate to null
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn       | toeslag |
      | 999993653 | null    |
    # The nullable register value is passed straight through to a strict
    # output. The pass-through is allowed; the output is what breaks the
    # law's promise.
    When I evaluate "toeslag_strikt" of "test_nullable"
    Then the execution fails
    Then the execution fails with "evaluated to null"
    Then the execution fails with "not declared nullable"
    Then the execution fails with "toeslag_strikt"

  Scenario: A skipped call for nobody yields an absence the input has to allow
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn       | partner_bsn |
      | 999993653 | null        |
    # There is no partner, so the bron is not called and the input is null.
    # Article 3 declared that; article 3a did not.
    When I evaluate "partner_geboortejaar" of "test_nullable"
    Then the execution succeeds
    Then output "partner_geboortejaar" is absent
    When I evaluate "partner_geboortejaar_strikt" of "test_nullable"
    Then the execution fails
    Then the execution fails with "declares as never absent"
    Then the execution fails with "partner_bsn"

  Scenario: A partner who is there is looked up as before
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn       | partner_bsn |
      | 999993653 | 999993641   |
    When I evaluate outputs "partner_geboortejaar" of "test_nullable"
    Then the execution succeeds
    Then output "partner_geboortejaar" equals 1980
    When I evaluate outputs "partner_geboortejaar_strikt" of "test_nullable"
    Then the execution succeeds
    Then output "partner_geboortejaar_strikt" equals 1980

  Scenario: A nullable parameter may be passed as null
    Given parameter "toeslagpartner" is "null"
    When I evaluate "alleenstaand" of "test_nullable"
    Then the execution succeeds
    Then output "alleenstaand" is true

  Scenario: A parameter that is not nullable may not be passed as null
    Given parameter "aanvraag_bedrag" is "null"
    # The bron declared its amount optional (it may be left out) but not
    # nullable: an absence is not a value it takes, required or not.
    When I evaluate "past_aanvraag" of "test_nullable_bron"
    Then the execution fails
    Then the execution fails with "not declared nullable"
    Then the execution fails with "aanvraag_bedrag"

  Scenario: A null optional parameter is refused at the call when the target does not allow it
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn       | bedrag |
      | 999993653 | null   |
    # Article 5 hands the register's absent amount to the bron as its optional
    # `aanvraag_bedrag`. The call is not skipped (the parameter is optional),
    # but the bron never declared that "geen bedrag" is a value it takes.
    When I evaluate "aanvraag_past" of "test_nullable"
    Then the execution fails
    Then the execution fails with "declares as never absent"
    Then the execution fails with "aanvraag_bedrag"
    Then the execution fails with "parameter from test_nullable"

  Scenario: An optional parameter that is left out stays unknown, nullable or not
    When I evaluate "past_aanvraag" of "test_nullable_bron"
    Then the execution succeeds
    Then output "past_aanvraag" is unknown for lack of "aanvraag_bedrag"
