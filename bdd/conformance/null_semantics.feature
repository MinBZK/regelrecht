@tier:core
Feature: Null semantics — RFC-036
  A register that holds no value for this person (no rent, no partner, no
  decision) resolves an input to null. The engine treats null as "unknown",
  not as an error: comparing, calculating or reading a field of nothing is
  nothing, and asking another law about nobody does not run that law. What
  the unknown means is for the law's own null checks to decide.

  Driven against test_null_semantics (register inputs that may be missing),
  test_null_semantics_bron (called cross-law; declares an optional form
  parameter) and test_null_semantics_strikt (forgets a required parameter).

  Background:
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    # The bron knows both persons; the engine resolves every input of an
    # article, so the cross-law calls need a row even when the scenario is
    # about something else.
    Given the following "register" data with key "bsn" for law "test_null_semantics_bron":
      | bsn       | geboortejaar |
      | 999993653 | 1975         |
      | 999993641 | 1980         |

  Scenario: Every value present computes as before
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn       | huur | partner_bsn | beschikking          |
      | 999993653 | 650  | 999993641   | {"status": "ACTIEF"} |
    When I evaluate outputs "huur_hoog, verhoogde_huur, status, partner_geboortejaar, heeft_huur" of "test_null_semantics"
    Then the execution succeeds
    Then output "huur_hoog" is true
    Then output "verhoogde_huur" equals 750
    Then output "status" equals "ACTIEF"
    Then output "partner_geboortejaar" equals 1980
    Then output "heeft_huur" is true

  Scenario: Comparing and calculating with null is null
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn       | huur | partner_bsn | beschikking |
      | 999993653 | null | null        | null        |
    When I evaluate outputs "huur_hoog, verhoogde_huur, heeft_huur" of "test_null_semantics"
    Then the execution succeeds
    Then output "huur_hoog" is null
    Then output "verhoogde_huur" is null
    # EQUALS against null is the one comparison that answers: the law's own
    # null check still works.
    Then output "heeft_huur" is false

  Scenario: A field of a null record is null
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn       | huur | partner_bsn | beschikking |
      | 999993653 | 650  | null        | null        |
    When I evaluate outputs "status" of "test_null_semantics"
    Then the execution succeeds
    Then output "status" is null

  Scenario: A null required parameter does not run the other law
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn       | huur | partner_bsn | beschikking |
      | 999993653 | 650  | null        | null        |
    When I evaluate outputs "partner_geboortejaar" of "test_null_semantics"
    Then the execution succeeds
    Then output "partner_geboortejaar" is null

  Scenario: An optional parameter the caller does not pass reads as null
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn       | huur | partner_bsn | beschikking |
      | 999993653 | 650  | null        | null        |
    # The caller passes only bsn; the bron's `aanvraag_bedrag` (required: false)
    # is filled with null, so its comparison is unknown rather than an error.
    When I evaluate outputs "aanvraag_past" of "test_null_semantics"
    Then the execution succeeds
    Then output "aanvraag_past" is null

  Scenario: A required parameter the caller does not pass is still an error
    When I evaluate outputs "geboortejaar" of "test_null_semantics_strikt"
    Then the execution fails
