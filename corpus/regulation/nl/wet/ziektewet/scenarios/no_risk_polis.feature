Feature: No-risk polis (Ziektewet artikel 29b)
  Als werkgever
  Wil ik weten of ik recht heb op de no-risk polis
  Zodat ik bij ziekte van de werknemer ziekengeld via UWV ontvang

  # Bron: Ziektewet artikel 29b (BWBR0001888) lid 1, 2 en 4.
  # Doelgroepafbakening loopt via cross-law sources naar Wet WIA
  # (BWBR0019057), Wajong (BWBR0008657) en Participatiewet (BWBR0015703).
  #
  # De doelgroep-inputs worden hier als parameters aangeleverd; dat overschrijft
  # de cross-law resolutie zodat elk lid van 29b los toetsbaar is.

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Werknemer met WIA-uitkering geeft recht op no-risk polis
    Given the following parameters:
      | bsn                               | 999990010 |
      | is_wsw_werknemer                  | false     |
      | is_wia_uitkeringsgerechtigd       | true      |
      | is_wia_min_35_arbeidsongeschikt   | false     |
      | heeft_voortgezet_wia_recht        | false     |
      | heeft_arbeidsbeperking_wia        | true      |
      | is_wajong_gerechtigd              | false     |
      | is_jonggehandicapt_schoolverlater | false     |
      | is_banenafspraak_doelgroep        | false     |
      | is_pwet_loonkostensubsidie        | false     |
      | is_beschut_werk                   | false     |
      | loonwaarde_lager_dan_minimumloon  | false     |
    When I evaluate "heeft_recht_op_no_risk_polis" of "ziektewet"
    Then the execution succeeds
    And output "heeft_recht_op_no_risk_polis" is true
    And output "duur_no_risk_polis_jaren" equals 5

  Scenario: Werknemer zonder doelgroepstatus heeft geen recht op no-risk polis
    Given the following parameters:
      | bsn                               | 999990011 |
      | is_wsw_werknemer                  | false     |
      | is_wia_uitkeringsgerechtigd       | false     |
      | is_wia_min_35_arbeidsongeschikt   | false     |
      | heeft_voortgezet_wia_recht        | false     |
      | heeft_arbeidsbeperking_wia        | false     |
      | is_wajong_gerechtigd              | false     |
      | is_jonggehandicapt_schoolverlater | false     |
      | is_banenafspraak_doelgroep        | false     |
      | is_pwet_loonkostensubsidie        | false     |
      | is_beschut_werk                   | false     |
      | loonwaarde_lager_dan_minimumloon  | false     |
    When I evaluate "heeft_recht_op_no_risk_polis" of "ziektewet"
    Then the execution succeeds
    And output "heeft_recht_op_no_risk_polis" is false
    And output "duur_no_risk_polis_jaren" equals 0

  # Randgeval: lid 2 (banenafspraak) — duur is -1 (sentinel voor
  # 'onbeperkt', zie untranslatable in machine_readable van 29b.1).
  # Zo wint lid 2 ook bij overlap met lid 1 of lid 4 als langste termijn.
  Scenario: Doelgroep banenafspraak (lid 2 e) heeft recht op no-risk polis zonder vaste duur
    Given the following parameters:
      | bsn                               | 999990012 |
      | is_wsw_werknemer                  | false     |
      | is_wia_uitkeringsgerechtigd       | false     |
      | is_wia_min_35_arbeidsongeschikt   | false     |
      | heeft_voortgezet_wia_recht        | false     |
      | heeft_arbeidsbeperking_wia        | false     |
      | is_wajong_gerechtigd              | false     |
      | is_jonggehandicapt_schoolverlater | false     |
      | is_banenafspraak_doelgroep        | true      |
      | is_pwet_loonkostensubsidie        | false     |
      | is_beschut_werk                   | false     |
      | loonwaarde_lager_dan_minimumloon  | true      |
    When I evaluate "heeft_recht_op_no_risk_polis" of "ziektewet"
    Then the execution succeeds
    And output "heeft_recht_op_no_risk_polis" is true
    And output "duur_no_risk_polis_jaren" equals -1
