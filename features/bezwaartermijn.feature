Feature: Bezwaartermijn chain
  As a citizen receiving a government decision
  I want to know when the objection deadline expires
  So that I can file an objection in time

  # This feature tests RFC-007 (hooks, overrides) working together:
  # - Hooks: AWB articles fire automatically on BESCHIKKING
  # - Overrides: Vreemdelingenwet overrides AWB 6:7 (lex specialis)

  Background:
    Given the calculation date is "2026-01-01"

  Scenario: Vreemdelingenwet beschikking triggers AWB hooks
    Given a vreemdelingenwet application with:
      | key                   | value |
      | heeft_geldige_mvv     | true  |
      | heeft_geldig_document | true  |
    When the vreemdelingenwet beschikking is executed
    Then the execution succeeds
    And the output "verblijfsvergunning_verleend" is "true"
    And the output "motivering_vereist" is "true"
    And the output "bezwaartermijn_weken" is "4"

  Scenario: Rejected application still triggers hooks
    Given a vreemdelingenwet application with:
      | key                   | value |
      | heeft_geldige_mvv     | false |
      | heeft_geldig_document | true  |
    When the vreemdelingenwet beschikking is executed
    Then the execution succeeds
    And the output "verblijfsvergunning_verleend" is "false"
    # Hooks still fire — motivering is required even for rejections
    And the output "motivering_vereist" is "true"
    And the output "bezwaartermijn_weken" is "4"
