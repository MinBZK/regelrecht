Feature: Jobcoaching en werkplekaanpassingen (Wet WIA artikel 35)
  Als werknemer met een structurele functionele beperking
  Wil ik weten of UWV mij voorzieningen kan toekennen
  Zodat ik in dienstbetrekking kan blijven of komen werken

  # Bron: Wet WIA artikel 35 lid 1, 2, 4 en 5 (BWBR0019057).
  # Peildatum 2025-01-01.
  #
  # Lid 2 onderdeel c: WPA — meeneembare werkplek-voorzieningen
  # Lid 2 onderdeel d: JC  — noodzakelijke persoonlijke ondersteuning
  # Lid 4: niet van toepassing op Wajong-gerechtigden of Pwet 7.1.a-cliënten

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Werknemer met structurele beperking en aanvragen voor JC en WPA krijgt beide
    Given a citizen with the following data:
      | bsn                                              | 999990070 |
      | heeft_structurele_functionele_beperking          | true      |
      | heeft_arbeidsverhouding_of_voorbereiding         | true      |
      | is_wsw_werknemer                                 | false     |
      | heeft_recht_op_arbeidsondersteuning_wajong       | false     |
      | pwet_college_draagt_zorg_uitsluiting             | false     |
      | aanvraag_jobcoaching_ingediend                   | true      |
      | aanvraag_werkplekaanpassing_ingediend            | true      |
    When the law "wet_werk_en_inkomen_naar_arbeidsvermogen" is executed for outputs "heeft_recht_op_jobcoaching,heeft_recht_op_werkplekaanpassing,artikel_35_van_toepassing,voldoet_aan_basisvoorwaarden_lid_1"
    Then the execution succeeds
    And the output "artikel_35_van_toepassing" is "true"
    And the output "voldoet_aan_basisvoorwaarden_lid_1" is "true"
    And the output "heeft_recht_op_jobcoaching" is "true"
    And the output "heeft_recht_op_werkplekaanpassing" is "true"

  # Lid 4.a: Wajong-gerechtigde valt buiten artikel 35.
  Scenario: Wajong-gerechtigde valt buiten artikel 35 (lid 4 a)
    Given a citizen with the following data:
      | bsn                                              | 999990071 |
      | heeft_structurele_functionele_beperking          | true      |
      | heeft_arbeidsverhouding_of_voorbereiding         | true      |
      | is_wsw_werknemer                                 | false     |
      | heeft_recht_op_arbeidsondersteuning_wajong       | true      |
      | pwet_college_draagt_zorg_uitsluiting             | false     |
      | aanvraag_jobcoaching_ingediend                   | true      |
      | aanvraag_werkplekaanpassing_ingediend            | true      |
    When the law "wet_werk_en_inkomen_naar_arbeidsvermogen" is executed for outputs "artikel_35_van_toepassing,heeft_recht_op_jobcoaching,heeft_recht_op_werkplekaanpassing"
    Then the execution succeeds
    And the output "artikel_35_van_toepassing" is "false"
    And the output "heeft_recht_op_jobcoaching" is "false"
    And the output "heeft_recht_op_werkplekaanpassing" is "false"

  # Lid 4.b: Pwet 7.1.a-cliënt valt buiten artikel 35.
  Scenario: Pwet-cliënt onder college-ondersteuning valt buiten artikel 35 (lid 4 b)
    Given a citizen with the following data:
      | bsn                                              | 999990072 |
      | heeft_structurele_functionele_beperking          | true      |
      | heeft_arbeidsverhouding_of_voorbereiding         | true      |
      | is_wsw_werknemer                                 | false     |
      | heeft_recht_op_arbeidsondersteuning_wajong       | false     |
      | pwet_college_draagt_zorg_uitsluiting             | true      |
      | aanvraag_jobcoaching_ingediend                   | true      |
      | aanvraag_werkplekaanpassing_ingediend            | true      |
    When the law "wet_werk_en_inkomen_naar_arbeidsvermogen" is executed for outputs "artikel_35_van_toepassing,heeft_recht_op_jobcoaching"
    Then the execution succeeds
    And the output "artikel_35_van_toepassing" is "false"
    And the output "heeft_recht_op_jobcoaching" is "false"

  Scenario: Wsw-werknemer is uitgesloten van artikel 35-voorzieningen
    Given a citizen with the following data:
      | bsn                                              | 999990073 |
      | heeft_structurele_functionele_beperking          | true      |
      | heeft_arbeidsverhouding_of_voorbereiding         | true      |
      | is_wsw_werknemer                                 | true      |
      | heeft_recht_op_arbeidsondersteuning_wajong       | false     |
      | pwet_college_draagt_zorg_uitsluiting             | false     |
      | aanvraag_jobcoaching_ingediend                   | true      |
      | aanvraag_werkplekaanpassing_ingediend            | true      |
    When the law "wet_werk_en_inkomen_naar_arbeidsvermogen" is executed for outputs "voldoet_aan_basisvoorwaarden_lid_1,heeft_recht_op_jobcoaching,heeft_recht_op_werkplekaanpassing"
    Then the execution succeeds
    And the output "voldoet_aan_basisvoorwaarden_lid_1" is "false"
    And the output "heeft_recht_op_jobcoaching" is "false"
    And the output "heeft_recht_op_werkplekaanpassing" is "false"

  # Persoon mag JC aanvragen zonder ook WPA aan te vragen.
  Scenario: Aanvraag alleen voor jobcoaching geeft alleen JC, geen WPA
    Given a citizen with the following data:
      | bsn                                              | 999990074 |
      | heeft_structurele_functionele_beperking          | true      |
      | heeft_arbeidsverhouding_of_voorbereiding         | true      |
      | is_wsw_werknemer                                 | false     |
      | heeft_recht_op_arbeidsondersteuning_wajong       | false     |
      | pwet_college_draagt_zorg_uitsluiting             | false     |
      | aanvraag_jobcoaching_ingediend                   | true      |
      | aanvraag_werkplekaanpassing_ingediend            | false     |
    When the law "wet_werk_en_inkomen_naar_arbeidsvermogen" is executed for outputs "heeft_recht_op_jobcoaching,heeft_recht_op_werkplekaanpassing"
    Then the execution succeeds
    And the output "heeft_recht_op_jobcoaching" is "true"
    And the output "heeft_recht_op_werkplekaanpassing" is "false"

  # Geen structurele beperking → basisvoorwaarden lid 1 niet vervuld.
  Scenario: Zonder structurele functionele beperking geen recht op artikel 35-voorzieningen
    Given a citizen with the following data:
      | bsn                                              | 999990075 |
      | heeft_structurele_functionele_beperking          | false     |
      | heeft_arbeidsverhouding_of_voorbereiding         | true      |
      | is_wsw_werknemer                                 | false     |
      | heeft_recht_op_arbeidsondersteuning_wajong       | false     |
      | pwet_college_draagt_zorg_uitsluiting             | false     |
      | aanvraag_jobcoaching_ingediend                   | true      |
      | aanvraag_werkplekaanpassing_ingediend            | true      |
    When the law "wet_werk_en_inkomen_naar_arbeidsvermogen" is executed for outputs "voldoet_aan_basisvoorwaarden_lid_1,heeft_recht_op_jobcoaching,heeft_recht_op_werkplekaanpassing"
    Then the execution succeeds
    And the output "voldoet_aan_basisvoorwaarden_lid_1" is "false"
    And the output "heeft_recht_op_jobcoaching" is "false"
    And the output "heeft_recht_op_werkplekaanpassing" is "false"
