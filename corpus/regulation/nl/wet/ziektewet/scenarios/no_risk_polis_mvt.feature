Feature: No-risk polis — scenarios uit Memorie van Toelichting
  Testscenario's afgeleid uit de MvT bij Wet harmonisatie instrumenten
  arbeidsdeelname (kst-34194-3) bij Ziektewet artikel 29b.

  # Bron: Kamerstuk 34194 nr. 3 (MvT)
  # URL: https://zoek.officielebekendmakingen.nl/kst-34194-3.html

  Background:
    Given the calculation date is "2025-01-01"

  # === Rekenvoorbeelden uit MvT ===

  # MvT § 2.1: "Werknemer verdient 100% WML met loonwaarde 50%.
  # Werkgever betaalt loon aan werknemer 100% WML; gemeente verstrekt
  # 50% loonkostensubsidie. Bij ziekte meldt werkgever zich bij UWV.
  # UWV betaalt no-riskpolis: 100% van dagloon in eerste jaar,
  # 70% in tweede jaar. Gemeente stopt subsidie zodra UWV signaleert."
  #
  # In onze YAML modelleren we de RECHT-vraag (lid 1/2/4); de
  # samenloop met LKS is gemarkeerd als untranslatable. Dit scenario
  # toetst alleen het recht — niet de samenloop-effecten.
  Scenario: Werknemer banenafspraak met LKS heeft recht op no-risk polis (MvT § 2.1)
    # Bron: kst-34194-3, § 2.1 — banenafspraak-doelgroep met loonwaarde
    Given the following parameters:
      | bsn                              | 999990080 |
      | is_wsw_werknemer                 | false     |
      | is_wia_uitkeringsgerechtigd      | false     |
      | is_wia_min_35_arbeidsongeschikt  | false     |
      | heeft_voortgezet_wia_recht       | false     |
      | heeft_arbeidsbeperking_wia       | false     |
      | is_wajong_gerechtigd             | false     |
      | is_jonggehandicapt_schoolverlater | false    |
      | is_banenafspraak_doelgroep       | true      |
      | is_pwet_loonkostensubsidie       | true      |
      | is_beschut_werk                  | false     |
      | loonwaarde_lager_dan_minimumloon | true      |
    When I evaluate "heeft_recht_op_no_risk_polis" of "ziektewet"
    Then the execution succeeds
    And output "heeft_recht_op_no_risk_polis" is true
    And output "voldoet_aan_lid_2" is true

  # MvT bij art 29b lid 4: "Op basis van het vierde lid is de
  # no-riskpolis tevens van toepassing als een werkgever de
  # dienstbetrekking met een werknemer voortzet nadat de
  # WIA-aanspraak is vastgesteld."
  Scenario: Lid 4 — voortzetting dienstbetrekking na WIA-vaststelling geeft 5 jaar recht
    # Bron: kst-34194-3, MvT bij lid 4 (zie ook ECLI rechtbank,
    # svland-publicatie over toepassing lid 4)
    Given the following parameters:
      | bsn                              | 999990081 |
      | is_wsw_werknemer                 | false     |
      | is_wia_uitkeringsgerechtigd      | true      |
      | is_wia_min_35_arbeidsongeschikt  | false     |
      | heeft_voortgezet_wia_recht       | true      |
      | heeft_arbeidsbeperking_wia       | true      |
      | is_wajong_gerechtigd             | false     |
      | is_jonggehandicapt_schoolverlater | false    |
      | is_banenafspraak_doelgroep       | false     |
      | is_pwet_loonkostensubsidie       | false     |
      | is_beschut_werk                  | false     |
      | loonwaarde_lager_dan_minimumloon | false     |
    When I evaluate "heeft_recht_op_no_risk_polis" of "ziektewet"
    Then the execution succeeds
    And output "heeft_recht_op_no_risk_polis" is true
    And output "voldoet_aan_lid_4" is true
    And output "duur_no_risk_polis_jaren" equals 5

  # === Open vragen uit MvT — NIET als scenario, alleen documentatie ===
  #
  # 1. Hoogte ziekengeld eerste 52 weken: art 29b lid 5 zegt 70% van
  #    dagloon, maar lid 6 maakt afwijking voor eerste tijdvak van
  #    52 weken (terug naar dagloon, met cap op loon werkgever).
  #    Onze YAML modelleert alleen het RECHT, niet de hoogte.
  #
  # 2. Beschut werk (Pwet 10b) — historisch UITGESLOTEN tijdens MvT
  #    34194 (2015), nu INGESLOTEN per 29b lid 2.f. Verandering doet
  #    twijfelen aan welke MvT-passage nog geldend is.
  #
  # 3. Tijdelijke 5-jaars-uitbreiding 2016-2021 voor langdurig
  #    werklozen (lid 1.b 2°) is verlopen; huidige tekst kent geen
  #    werkloosheids-route meer in lid 1.b.
