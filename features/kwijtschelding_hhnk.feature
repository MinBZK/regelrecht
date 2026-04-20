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
      | vermogen                                | 0                               |
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
      | vermogen                                | 0                          |
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
      | vermogen                                | 0                               |
      | aanslagbedrag                           | 15000                           |
      | is_ondernemer                           | true                            |
      | belasting_zakelijk                      | true                            |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "false"
    And the output "hoogte_kwijtschelding" is "0"

  # Pensioengerechtigde alleenstaande: kostennorm gebruikt bijstandsnorm_aow_alleenstaand
  # (156469 eurocent). Bij inkomen gelijk aan die norm is betalingscapaciteit 0,
  # dus de volledige aanslag wordt kwijtgescholden.
  Scenario: AOW-gerechtigde alleenstaande krijgt kwijtschelding met pensioen-norm
    Given the calculation date is "2026-06-01"
    And a citizen with the following data:
      | bsn                                     | 999993653                     |
      | waterschap_code                         | WS0155                        |
      | belastingsoort                          | verontreinigingsheffing_woonruimte |
      | netto_besteedbaar_inkomen_maand         | 156469                        |
      | netto_besteedbaar_inkomen_partner_maand | 0                             |
      | huishoudtype                            | alleenstaande                 |
      | is_pensioengerechtigd                   | true                          |
      | vermogen                                | 0                             |
      | aanslagbedrag                           | 20000                         |
      | is_ondernemer                           | false                         |
      | belasting_zakelijk                      | false                         |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "true"
    And the output "hoogte_kwijtschelding" is "20000"

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
      | vermogen                                | 0                                     |
      | aanslagbedrag                           | 15000                                 |
      | is_ondernemer                           | false                                 |
      | belasting_zakelijk                      | false                                 |
    When the law "kwijtscheldingsregeling_waterschapsbelastingen_hhnk" is executed for outputs "kan_kwijtschelding_worden_verleend,hoogte_kwijtschelding"
    Then the execution succeeds
    And the output "kan_kwijtschelding_worden_verleend" is "false"
    And the output "hoogte_kwijtschelding" is "0"
