Feature: HHNK kwijtschelding waterschapsbelastingen

  # Alleenstaande met inkomen gelijk aan de bijstandsnorm heeft geen betalingscapaciteit;
  # bij afwezig vermogen wordt de hele aanslag kwijtgescholden.
  Scenario: Alleenstaande bijstandsgerechtigde krijgt volledige kwijtschelding
    Given the calculation date is "2026-06-01"
    And a citizen with the following data:
      | bsn                                     | 999993653                       |
      | waterschap_code                         | WS0155                          |
      | belastingsoort                          | watersysteemheffing_ingezetenen |
      | netto_besteedbaar_inkomen_maand         | 140150                          |
      | netto_besteedbaar_inkomen_partner_maand | 0                               |
      | huishoudtype                            | alleenstaande                   |
      | is_pensioengerechtigd                   | false                           |
      | inboedel_waarde                         | 0                               |
      | auto_waarde                             | 0                               |
      | auto_onmisbaar                          | false                           |
      | onroerend_goed_waarde                   | 0                               |
      | andere_bezittingen                      | 0                               |
      | hoger_bevoorrechte_schulden             | 0                               |
      | liquide_middelen                        | 0                               |
      | aanslagbedrag                           | 15000                           |
      | is_ondernemer                           | false                           |
      | belasting_zakelijk                      | false                           |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "true"
    And the output "hoogte_kwijtschelding" is "15000"

  # Tweepersoonshuishouden met inkomen €50 boven de echtgenoten-bijstandsnorm:
  # betalingscapaciteit = 12 x (205213 - 200213) = 60000 eurocent.
  # 80% daarvan = 48000 wordt aangewend; hoogte = 60000 - 0 - 48000 = 12000 eurocent.
  Scenario: Tweepersoonshuishouden net boven norm krijgt gedeeltelijke kwijtschelding
    Given the calculation date is "2026-06-01"
    And a citizen with the following data:
      | bsn                                     | 999993653                  |
      | waterschap_code                         | WS0155                     |
      | belastingsoort                          | zuiveringsheffing_woonruimte |
      | netto_besteedbaar_inkomen_maand         | 205213                     |
      | netto_besteedbaar_inkomen_partner_maand | 0                          |
      | huishoudtype                            | echtgenoten                |
      | is_pensioengerechtigd                   | false                      |
      | inboedel_waarde                         | 0                          |
      | auto_waarde                             | 0                          |
      | auto_onmisbaar                          | false                      |
      | onroerend_goed_waarde                   | 0                          |
      | andere_bezittingen                      | 0                          |
      | hoger_bevoorrechte_schulden             | 0                          |
      | liquide_middelen                        | 0                          |
      | aanslagbedrag                           | 60000                      |
      | is_ondernemer                           | false                      |
      | belasting_zakelijk                      | false                      |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "true"
    And the output "hoogte_kwijtschelding" is "12000"

  # Ondernemer met zakelijke belastingschuld valt buiten de regeling (art 5 HHNK).
  Scenario: Ondernemer met zakelijke belastingschuld wordt afgewezen
    Given the calculation date is "2026-06-01"
    And a citizen with the following data:
      | bsn                                     | 999993653                       |
      | waterschap_code                         | WS0155                          |
      | belastingsoort                          | watersysteemheffing_ingezetenen |
      | netto_besteedbaar_inkomen_maand         | 140150                          |
      | netto_besteedbaar_inkomen_partner_maand | 0                               |
      | huishoudtype                            | alleenstaande                   |
      | is_pensioengerechtigd                   | false                           |
      | inboedel_waarde                         | 0                               |
      | auto_waarde                             | 0                               |
      | auto_onmisbaar                          | false                           |
      | onroerend_goed_waarde                   | 0                               |
      | andere_bezittingen                      | 0                               |
      | hoger_bevoorrechte_schulden             | 0                               |
      | liquide_middelen                        | 0                               |
      | aanslagbedrag                           | 15000                           |
      | is_ondernemer                           | true                            |
      | belasting_zakelijk                      | true                            |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "false"
    And the output "hoogte_kwijtschelding" is "0"

  # Pensioengerechtigde alleenstaande: kostennorm gebruikt netto_ouderdomspensioen_alleenstaand
  # (155815 eurocent = netto AOW alleenstaand 2026 per RKBM art 3).
  # Bij inkomen gelijk aan netto-AOW is betalingscapaciteit 0, dus volledige kwijtschelding.
  Scenario: AOW-gerechtigde alleenstaande krijgt kwijtschelding met netto-ouderdomspensioen-norm
    Given the calculation date is "2026-06-01"
    And a citizen with the following data:
      | bsn                                     | 999993653                     |
      | waterschap_code                         | WS0155                        |
      | belastingsoort                          | verontreinigingsheffing_woonruimte |
      | netto_besteedbaar_inkomen_maand         | 155815                        |
      | netto_besteedbaar_inkomen_partner_maand | 0                             |
      | huishoudtype                            | alleenstaande                 |
      | is_pensioengerechtigd                   | true                          |
      | inboedel_waarde                         | 0                             |
      | auto_waarde                             | 0                             |
      | auto_onmisbaar                          | false                         |
      | onroerend_goed_waarde                   | 0                             |
      | andere_bezittingen                      | 0                             |
      | hoger_bevoorrechte_schulden             | 0                             |
      | liquide_middelen                        | 0                             |
      | aanslagbedrag                           | 20000                         |
      | is_ondernemer                           | false                         |
      | belasting_zakelijk                      | false                         |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "true"
    And the output "hoogte_kwijtschelding" is "20000"

  # AOW-gerechtigde echtgenoten: kostennorm gebruikt netto_ouderdomspensioen_echtgenoten
  # (213540 eurocent = tweemaal netto AOW gehuwd 2026 per RKBM art 3 lid 2 sub a).
  Scenario: AOW-gerechtigde echtgenoten krijgen kwijtschelding met AOW-gehuwd-norm
    Given the calculation date is "2026-06-01"
    And a citizen with the following data:
      | bsn                                     | 999993653                       |
      | waterschap_code                         | WS0155                          |
      | belastingsoort                          | watersysteemheffing_ingezetenen |
      | netto_besteedbaar_inkomen_maand         | 213540                          |
      | netto_besteedbaar_inkomen_partner_maand | 0                               |
      | huishoudtype                            | echtgenoten                     |
      | is_pensioengerechtigd                   | true                            |
      | inboedel_waarde                         | 0                               |
      | auto_waarde                             | 0                               |
      | auto_onmisbaar                          | false                           |
      | onroerend_goed_waarde                   | 0                               |
      | andere_bezittingen                      | 0                               |
      | hoger_bevoorrechte_schulden             | 0                               |
      | liquide_middelen                        | 0                               |
      | aanslagbedrag                           | 25000                           |
      | is_ondernemer                           | false                           |
      | belasting_zakelijk                      | false                           |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "true"
    And the output "hoogte_kwijtschelding" is "25000"

  # Auto boven drempelwaarde van EUR 2269 (226900 eurocent) die absoluut onmisbaar is voor werk
  # of vanwege invaliditeit wordt niet als vermogen beschouwd (URI art 12 lid 2 onderdeel c).
  # Alleenstaande met bijstand + essential auto -> volledige kwijtschelding.
  Scenario: Auto onmisbaar voor werk/invaliditeit telt niet als vermogen
    Given the calculation date is "2026-06-01"
    And a citizen with the following data:
      | bsn                                     | 999993653                       |
      | waterschap_code                         | WS0155                          |
      | belastingsoort                          | watersysteemheffing_ingezetenen |
      | netto_besteedbaar_inkomen_maand         | 140150                          |
      | netto_besteedbaar_inkomen_partner_maand | 0                               |
      | huishoudtype                            | alleenstaande                   |
      | is_pensioengerechtigd                   | false                           |
      | inboedel_waarde                         | 0                               |
      | auto_waarde                             | 500000                          |
      | auto_onmisbaar                          | true                            |
      | onroerend_goed_waarde                   | 0                               |
      | andere_bezittingen                      | 0                               |
      | hoger_bevoorrechte_schulden             | 0                               |
      | liquide_middelen                        | 0                               |
      | aanslagbedrag                           | 15000                           |
      | is_ondernemer                           | false                           |
      | belasting_zakelijk                      | false                           |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "true"
    And the output "hoogte_kwijtschelding" is "15000"

  # Liquide middelen boven de HHNK-vrijstelling (kostennorm 140150 + art 4 verhoging alleenstaand
  # 150000 = 290150 eurocent) tellen bovenmatig mee als vermogen. Bij liquide 400000 is
  # 400000 - 290150 = 109850 vermogen. Vermogen > aanslag 80000 -> hoogte = 0.
  Scenario: Vermogen boven de HHNK-vrijstelling blokkeert kwijtschelding
    Given the calculation date is "2026-06-01"
    And a citizen with the following data:
      | bsn                                     | 999993653                       |
      | waterschap_code                         | WS0155                          |
      | belastingsoort                          | watersysteemheffing_ingezetenen |
      | netto_besteedbaar_inkomen_maand         | 140150                          |
      | netto_besteedbaar_inkomen_partner_maand | 0                               |
      | huishoudtype                            | alleenstaande                   |
      | is_pensioengerechtigd                   | false                           |
      | inboedel_waarde                         | 0                               |
      | auto_waarde                             | 0                               |
      | auto_onmisbaar                          | false                           |
      | onroerend_goed_waarde                   | 0                               |
      | andere_bezittingen                      | 0                               |
      | hoger_bevoorrechte_schulden             | 0                               |
      | liquide_middelen                        | 400000                          |
      | aanslagbedrag                           | 80000                           |
      | is_ondernemer                           | false                           |
      | belasting_zakelijk                      | false                           |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "false"
    And the output "hoogte_kwijtschelding" is "0"

  # Niet-begunstigde belastingsoort (niet genoemd in HHNK art 1) valt buiten scope.
  Scenario: Belastingsoort buiten de HHNK-scope wordt afgewezen
    Given the calculation date is "2026-06-01"
    And a citizen with the following data:
      | bsn                                     | 999993653                             |
      | waterschap_code                         | WS0155                                |
      | belastingsoort                          | watersysteemheffing_gebouwd_bedrijven |
      | netto_besteedbaar_inkomen_maand         | 140150                                |
      | netto_besteedbaar_inkomen_partner_maand | 0                                     |
      | huishoudtype                            | alleenstaande                         |
      | is_pensioengerechtigd                   | false                                 |
      | inboedel_waarde                         | 0                                     |
      | auto_waarde                             | 0                                     |
      | auto_onmisbaar                          | false                                 |
      | onroerend_goed_waarde                   | 0                                     |
      | andere_bezittingen                      | 0                                     |
      | hoger_bevoorrechte_schulden             | 0                                     |
      | liquide_middelen                        | 0                                     |
      | aanslagbedrag                           | 15000                                 |
      | is_ondernemer                           | false                                 |
      | belasting_zakelijk                      | false                                 |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "false"
    And the output "hoogte_kwijtschelding" is "0"

  # Hoog inkomen: alleenstaande, inkomen 300000 eurocent, HHNK-kostennorm 100%
  # (140150). Betalingscapaciteit = 12 x (300000 - 140150) = 1918200 eurocent.
  # Aanwendbaar = 80% x 1918200 = 1534560 eurocent, ver boven de aanslag van 15000.
  # Dus geen kwijtschelding mogelijk.
  Scenario: Hoog inkomen wordt afgewezen
    Given the calculation date is "2026-06-01"
    And a citizen with the following data:
      | bsn                                     | 999993653                       |
      | waterschap_code                         | WS0155                          |
      | belastingsoort                          | watersysteemheffing_ingezetenen |
      | netto_besteedbaar_inkomen_maand         | 300000                          |
      | netto_besteedbaar_inkomen_partner_maand | 0                               |
      | huishoudtype                            | alleenstaande                   |
      | is_pensioengerechtigd                   | false                           |
      | inboedel_waarde                         | 0                               |
      | auto_waarde                             | 0                               |
      | auto_onmisbaar                          | false                           |
      | onroerend_goed_waarde                   | 0                               |
      | andere_bezittingen                      | 0                               |
      | hoger_bevoorrechte_schulden             | 0                               |
      | liquide_middelen                        | 0                               |
      | aanslagbedrag                           | 15000                           |
      | is_ondernemer                           | false                           |
      | belasting_zakelijk                      | false                           |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "false"
    And the output "hoogte_kwijtschelding" is "0"

  # Ondernemer met uitsluitend privé-aanslag (niet zakelijk) valt wel binnen de
  # regeling: HHNK art 5 sluit alleen zakelijke belastingschulden uit. Verder
  # identiek aan het eerste scenario, dus volledige kwijtschelding.
  Scenario: Ondernemer met privé-aanslag wordt wel toegelaten
    Given the calculation date is "2026-06-01"
    And a citizen with the following data:
      | bsn                                     | 999993653                       |
      | waterschap_code                         | WS0155                          |
      | belastingsoort                          | watersysteemheffing_ingezetenen |
      | netto_besteedbaar_inkomen_maand         | 140150                          |
      | netto_besteedbaar_inkomen_partner_maand | 0                               |
      | huishoudtype                            | alleenstaande                   |
      | is_pensioengerechtigd                   | false                           |
      | inboedel_waarde                         | 0                               |
      | auto_waarde                             | 0                               |
      | auto_onmisbaar                          | false                           |
      | onroerend_goed_waarde                   | 0                               |
      | andere_bezittingen                      | 0                               |
      | hoger_bevoorrechte_schulden             | 0                               |
      | liquide_middelen                        | 0                               |
      | aanslagbedrag                           | 15000                           |
      | is_ondernemer                           | true                            |
      | belasting_zakelijk                      | false                           |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "true"
    And the output "hoogte_kwijtschelding" is "15000"
