Feature: Bezwaartermijn chain
  As a citizen receiving a government decision
  I want to know when the objection deadline expires
  So that I can file an objection in time

  # This feature tests RFC-007 (hooks, overrides) working together:
  # - Hooks: AWB articles fire automatically on BESCHIKKING
  # - Overrides: Vreemdelingenwet overrides AWB 6:7 (lex specialis)

  Background:
    Given the calculation date is "2026-01-01"

  Scenario: Vreemdelingenwet beschikking triggers AWB hooks with override
    When the vreemdelingenwet beschikking is executed
    Then the execution succeeds
    And the output "minister_is_bevoegd" is "true"
    # AWB 3:46 hook fires pre_actions on BESCHIKKING
    And the output "motivering_vereist" is "true"
    # AWB 6:7 hook fires post_actions, but Vw art 69 overrides to 4 weeks
    And the output "bezwaartermijn_weken" is "4"
