Feature: No-risk polis (Ziektewet artikel 29b)
  Als werkgever
  Wil ik weten of ik recht heb op de no-risk polis
  Zodat ik bij ziekte van de werknemer ziekengeld via UWV ontvang

  # Bron: Ziektewet artikel 29b (BWBR0001888) lid 1, 2 en 4.
  # Doelgroepafbakening loopt via cross-law sources naar Wet WIA
  # (BWBR0019057), Wajong (BWBR0008657) en Participatiewet (BWBR0015703).
  #
  # Deze scenario's gebruiken de generieke "a citizen with the following
  # data" stap; alle parameters voor zowel Ziektewet als de bron-wet-stubs
  # worden in één tabel aangeleverd.

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Werknemer met WIA-uitkering geeft recht op no-risk polis
    Given a citizen with the following data:
      | bsn                              | 999990010 |
      | is_wsw_werknemer                 | false     |
      | is_wia_uitkeringsgerechtigd      | true      |
      | is_wia_min_35_arbeidsongeschikt  | false     |
      | heeft_voortgezet_wia_recht       | false     |
      | heeft_arbeidsbeperking_wia       | true      |
      | is_wajong_gerechtigd             | false     |
      | is_jonggehandicapt_schoolverlater | false    |
      | is_banenafspraak_doelgroep       | false     |
      | is_pwet_loonkostensubsidie       | false     |
      | is_beschut_werk                  | false     |
      | loonwaarde_lager_dan_minimumloon | false     |
    When the law "ziektewet" is executed for outputs "heeft_recht_op_no_risk_polis,duur_no_risk_polis_jaren"
    Then the execution succeeds
    And the output "heeft_recht_op_no_risk_polis" is "true"
    And the output "duur_no_risk_polis_jaren" is "5"

  Scenario: Werknemer zonder doelgroepstatus heeft geen recht op no-risk polis
    Given a citizen with the following data:
      | bsn                              | 999990011 |
      | is_wsw_werknemer                 | false     |
      | is_wia_uitkeringsgerechtigd      | false     |
      | is_wia_min_35_arbeidsongeschikt  | false     |
      | heeft_voortgezet_wia_recht       | false     |
      | heeft_arbeidsbeperking_wia       | false     |
      | is_wajong_gerechtigd             | false     |
      | is_jonggehandicapt_schoolverlater | false    |
      | is_banenafspraak_doelgroep       | false     |
      | is_pwet_loonkostensubsidie       | false     |
      | is_beschut_werk                  | false     |
      | loonwaarde_lager_dan_minimumloon | false     |
    When the law "ziektewet" is executed for outputs "heeft_recht_op_no_risk_polis,duur_no_risk_polis_jaren"
    Then the execution succeeds
    And the output "heeft_recht_op_no_risk_polis" is "false"
    And the output "duur_no_risk_polis_jaren" is "0"

  # Randgeval: lid 2 (banenafspraak) — duur is 0 (gemodelleerd als
  # niet-tijdsgebonden, zie untranslatable in machine_readable van 29b.1).
  Scenario: Doelgroep banenafspraak (lid 2 e) heeft recht op no-risk polis zonder vaste duur
    Given a citizen with the following data:
      | bsn                              | 999990012 |
      | is_wsw_werknemer                 | false     |
      | is_wia_uitkeringsgerechtigd      | false     |
      | is_wia_min_35_arbeidsongeschikt  | false     |
      | heeft_voortgezet_wia_recht       | false     |
      | heeft_arbeidsbeperking_wia       | false     |
      | is_wajong_gerechtigd             | false     |
      | is_jonggehandicapt_schoolverlater | false    |
      | is_banenafspraak_doelgroep       | true      |
      | is_pwet_loonkostensubsidie       | false     |
      | is_beschut_werk                  | false     |
      | loonwaarde_lager_dan_minimumloon | true      |
    When the law "ziektewet" is executed for outputs "heeft_recht_op_no_risk_polis,duur_no_risk_polis_jaren"
    Then the execution succeeds
    And the output "heeft_recht_op_no_risk_polis" is "true"
    And the output "duur_no_risk_polis_jaren" is "0"
