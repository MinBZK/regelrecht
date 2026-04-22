Feature: Unit-tests voor nieuwe machine_readable blocks in HHNK-kwijtschelding keten

  # Deze scenarios testen de nieuwe machine_readable blocks die in deze branch zijn
  # toegevoegd aan Regeling medeoverheden art 2/3/4, URI art 14/15/17 en Leidraad 2008
  # art 26.2.2/26.2.3. Per artikel één happy-path + minstens één edge-case.

  # === Regeling medeoverheden art 2: kostennorm-percentage grenzen ===

  Background:
    Given the calculation date is "2026-06-01"

  Scenario: HHNK kiest 100% kostennorm-percentage - binnen grenzen 90-100%
    Given a citizen with the following data:
      | kostennorm_percentage_verordening | 1.0 |
    When the law "regeling_kwijtschelding_belastingen_medeoverheden" is executed for outputs "kostennorm_percentage_rechtsgeldig"
    Then the execution succeeds
    And the output "kostennorm_percentage_rechtsgeldig" is "true"

  Scenario: Verordening kiest 85% kostennorm - onder ondergrens, ongeldig
    Given a citizen with the following data:
      | kostennorm_percentage_verordening | 0.85 |
    When the law "regeling_kwijtschelding_belastingen_medeoverheden" is executed for outputs "kostennorm_percentage_rechtsgeldig"
    Then the execution succeeds
    And the output "kostennorm_percentage_rechtsgeldig" is "false"

  # === Regeling medeoverheden art 4: verhogings-cap per huishoudtype ===

  Scenario: HHNK verhoging echtgenoten EUR 2000 raakt de cap
    Given a citizen with the following data:
      | verhoging_verordening | 200000      |
      | huishoudtype          | echtgenoten |
    When the law "regeling_kwijtschelding_belastingen_medeoverheden" is executed for outputs "verhoging_max,verhoging_rechtsgeldig"
    Then the execution succeeds
    And the output "verhoging_max" is "200000"
    And the output "verhoging_rechtsgeldig" is "true"

  Scenario: Alleenstaande-ouder cap is 90% van echtgenoten-cap
    Given a citizen with the following data:
      | verhoging_verordening | 180000              |
      | huishoudtype          | alleenstaande_ouder |
    When the law "regeling_kwijtschelding_belastingen_medeoverheden" is executed for outputs "verhoging_max,verhoging_rechtsgeldig"
    Then the execution succeeds
    And the output "verhoging_max" is "180000"
    And the output "verhoging_rechtsgeldig" is "true"

  Scenario: Alleenstaande cap is 75% van echtgenoten-cap
    Given a citizen with the following data:
      | verhoging_verordening | 150000        |
      | huishoudtype          | alleenstaande |
    When the law "regeling_kwijtschelding_belastingen_medeoverheden" is executed for outputs "verhoging_max,verhoging_rechtsgeldig"
    Then the execution succeeds
    And the output "verhoging_max" is "150000"
    And the output "verhoging_rechtsgeldig" is "true"

  Scenario: Verordening overschrijdt echtgenoten-cap - ongeldig
    Given a citizen with the following data:
      | verhoging_verordening | 250000      |
      | huishoudtype          | echtgenoten |
    When the law "regeling_kwijtschelding_belastingen_medeoverheden" is executed for outputs "verhoging_rechtsgeldig"
    Then the execution succeeds
    And the output "verhoging_rechtsgeldig" is "false"

  # === URI art 17: €136 drempel aflossingen andere schulden ===

  Scenario: Aflossingen EUR 100 per maand - onder drempel, geen blokkade
    Given a citizen with the following data:
      | aflossingen_andere_schulden_maand | 10000 |
      | zeer_bijzondere_omstandigheden    | false |
      | betalingscapaciteit_toereikend    | false |
    When the law "uitvoeringsregeling_invorderingswet_1990" is executed for outputs "drempel_overschreden,kwijtschelding_blokkade_art_17"
    Then the execution succeeds
    And the output "drempel_overschreden" is "false"
    And the output "kwijtschelding_blokkade_art_17" is "false"

  Scenario: Aflossingen EUR 200 per maand - boven drempel, geen zeer bijzondere omstandigheden, blokkade
    Given a citizen with the following data:
      | aflossingen_andere_schulden_maand | 20000 |
      | zeer_bijzondere_omstandigheden    | false |
      | betalingscapaciteit_toereikend    | false |
    When the law "uitvoeringsregeling_invorderingswet_1990" is executed for outputs "drempel_overschreden,kwijtschelding_blokkade_art_17"
    Then the execution succeeds
    And the output "drempel_overschreden" is "true"
    And the output "kwijtschelding_blokkade_art_17" is "true"

  Scenario: Aflossingen EUR 200 per maand - boven drempel met zeer bijzondere omstandigheden, geen blokkade
    Given a citizen with the following data:
      | aflossingen_andere_schulden_maand | 20000 |
      | zeer_bijzondere_omstandigheden    | true  |
      | betalingscapaciteit_toereikend    | false |
    When the law "uitvoeringsregeling_invorderingswet_1990" is executed for outputs "kwijtschelding_blokkade_art_17"
    Then the execution succeeds
    And the output "kwijtschelding_blokkade_art_17" is "false"

  # === Leidraad 2008 art 26.2.2: inboedel-drempel EUR 2269 ===

  Scenario: Inboedel EUR 2000 - onder drempel, niet als vermogen
    Given a citizen with the following data:
      | inboedel_waarde | 200000 |
    When the law "leidraad_invordering_2008" is executed for outputs "inboedel_als_vermogen"
    Then the execution succeeds
    And the output "inboedel_als_vermogen" is "0"

  Scenario: Inboedel EUR 3000 - boven drempel, volle waarde als vermogen
    Given a citizen with the following data:
      | inboedel_waarde | 300000 |
    When the law "leidraad_invordering_2008" is executed for outputs "inboedel_als_vermogen"
    Then the execution succeeds
    And the output "inboedel_als_vermogen" is "300000"

  # === Leidraad 2008 art 26.2.3: auto-drempel EUR 3350 + onmisbaarheid ===

  Scenario: Auto EUR 3000 - onder drempel, niet als vermogen
    Given a citizen with the following data:
      | auto_waarde    | 300000 |
      | auto_onmisbaar | false  |
    When the law "leidraad_invordering_2008" is executed for outputs "auto_als_vermogen"
    Then the execution succeeds
    And the output "auto_als_vermogen" is "0"

  Scenario: Auto EUR 4000 - boven drempel, wel als vermogen
    Given a citizen with the following data:
      | auto_waarde    | 400000 |
      | auto_onmisbaar | false  |
    When the law "leidraad_invordering_2008" is executed for outputs "auto_als_vermogen"
    Then the execution succeeds
    And the output "auto_als_vermogen" is "400000"

  Scenario: Auto EUR 10000 onmisbaar - altijd vermogensvrij
    Given a citizen with the following data:
      | auto_waarde    | 1000000 |
      | auto_onmisbaar | true    |
    When the law "leidraad_invordering_2008" is executed for outputs "auto_als_vermogen"
    Then the execution succeeds
    And the output "auto_als_vermogen" is "0"

  # === HHNK-leidraad art 26: verzoek-gate ===

  # === Zvw art 41: inkomensafhankelijke bijdrage ===

  Scenario: Zvw art 41 - pensioengerechtigde laag tarief 5.26% onder maximum
    Given a citizen with the following data:
      | bruto_inkomen_maand   | 200000 |
      | is_pensioengerechtigd | true   |
    When the law "zorgverzekeringswet" is executed for outputs "bijdrage_percentage,inkomensafhankelijke_bijdrage_maand"
    Then the execution succeeds
    And the output "bijdrage_percentage" is "0.0526"
    And the output "inkomensafhankelijke_bijdrage_maand" is "10520"

  Scenario: Zvw art 41 - werknemer hoog tarief 6.51%
    Given a citizen with the following data:
      | bruto_inkomen_maand   | 300000 |
      | is_pensioengerechtigd | false  |
    When the law "zorgverzekeringswet" is executed for outputs "bijdrage_percentage,inkomensafhankelijke_bijdrage_maand"
    Then the execution succeeds
    And the output "bijdrage_percentage" is "0.0651"
    And the output "inkomensafhankelijke_bijdrage_maand" is "19530"

  Scenario: Zvw art 41 - bruto boven maximum bijdrage-inkomen wordt gecapt
    Given a citizen with the following data:
      | bruto_inkomen_maand   | 1000000 |
      | is_pensioengerechtigd | false   |
    When the law "zorgverzekeringswet" is executed for outputs "gecap_bijdrage_inkomen_maand,inkomensafhankelijke_bijdrage_maand"
    Then the execution succeeds
    And the output "gecap_bijdrage_inkomen_maand" is "632200"
    And the output "inkomensafhankelijke_bijdrage_maand" is "41156"

  # === BRP afgeleid-kwijtschelding: is_pensioengerechtigd + huishoudtype ===

  Scenario: BRP afgeleid - 50-jarige met partner is echtgenoten-huishouden
    Given a citizen with the following data:
      | bsn                                  | 999993653 |
      | heeft_ten_laste_komende_kinderen     | false     |
      | leeftijd                             | 50        |
      | heeft_partner                        | true      |
    When the law "wet_basisregistratie_personen" is executed for outputs "is_pensioengerechtigd,huishoudtype"
    Then the execution succeeds
    And the output "is_pensioengerechtigd" is "false"
    And the output "huishoudtype" is "echtgenoten"

  Scenario: BRP afgeleid - 70-jarige alleenstaande is pensioengerechtigd alleenstaande
    Given a citizen with the following data:
      | bsn                                  | 999993653 |
      | heeft_ten_laste_komende_kinderen     | false     |
      | leeftijd                             | 70        |
      | heeft_partner                        | false     |
    When the law "wet_basisregistratie_personen" is executed for outputs "is_pensioengerechtigd,huishoudtype"
    Then the execution succeeds
    And the output "is_pensioengerechtigd" is "true"
    And the output "huishoudtype" is "alleenstaande"

  Scenario: BRP afgeleid - 35-jarige zonder partner met kinderen is alleenstaande ouder
    Given a citizen with the following data:
      | bsn                                  | 999993653 |
      | heeft_ten_laste_komende_kinderen     | true      |
      | leeftijd                             | 35        |
      | heeft_partner                        | false     |
    When the law "wet_basisregistratie_personen" is executed for outputs "huishoudtype"
    Then the execution succeeds
    And the output "huishoudtype" is "alleenstaande_ouder"

  # === HHNK-leidraad art 48.2: erfgenaam-drempel EUR 23 ===

  Scenario: Erfgenaam EUR 20 - onder drempel, invordering blijft achterwege
    Given a citizen with the following data:
      | te_vorderen_bedrag_erfgenaam                | 2000  |
      | betreft_hoofdsom_aanslag                    | false |
      | niet_meer_tegen_gezamenlijke_erfgenamen     | true  |
    When the law "leidraad_invordering_waterschapsbelastingen_hhnk" is executed for outputs "onder_drempel,invordering_blijft_achterwege"
    Then the execution succeeds
    And the output "onder_drempel" is "true"
    And the output "invordering_blijft_achterwege" is "true"

  Scenario: Erfgenaam EUR 20 hoofdsom - drempel niet van toepassing
    Given a citizen with the following data:
      | te_vorderen_bedrag_erfgenaam                | 2000 |
      | betreft_hoofdsom_aanslag                    | true |
      | niet_meer_tegen_gezamenlijke_erfgenamen     | true |
    When the law "leidraad_invordering_waterschapsbelastingen_hhnk" is executed for outputs "invordering_blijft_achterwege"
    Then the execution succeeds
    And the output "invordering_blijft_achterwege" is "false"

  # === HHNK-leidraad art 80 Incassoreglement: EUR 10 minimumtermijn ===

  Scenario: Aanslag EUR 200 - standaard 10 termijnen van EUR 20
    Given a citizen with the following data:
      | aanslagbedrag | 20000 |
    When the law "leidraad_invordering_waterschapsbelastingen_hhnk" is executed for outputs "aanslag_onder_drempel,effectief_termijnbedrag"
    Then the execution succeeds
    And the output "aanslag_onder_drempel" is "false"
    And the output "effectief_termijnbedrag" is "2000"

  Scenario: Aanslag EUR 50 - minder dan 10 termijnen, minimum EUR 10
    Given a citizen with the following data:
      | aanslagbedrag | 5000 |
    When the law "leidraad_invordering_waterschapsbelastingen_hhnk" is executed for outputs "aanslag_onder_drempel,effectief_termijnbedrag"
    Then the execution succeeds
    And the output "aanslag_onder_drempel" is "true"
    And the output "effectief_termijnbedrag" is "1000"

  Scenario: HHNK-leidraad art 26 - verzoek ingediend, verordening wijst toe, beleidsregel kent toe
    Given a citizen with the following data:
      | bsn                                          | 999993653                       |
      | waterschap_code                              | WS0155                          |
      | belastingsoort                               | watersysteemheffing_ingezetenen |
      | netto_besteedbaar_inkomen_maand              | 140150                          |
      | netto_besteedbaar_inkomen_partner_maand      | 0                               |
      | huishoudtype                                 | alleenstaande                   |
      | is_pensioengerechtigd                        | false                           |
      | inboedel_waarde                              | 0                               |
      | auto_waarde                                  | 0                               |
      | auto_onmisbaar                               | false                           |
      | onroerend_goed_waarde                        | 0                               |
      | andere_bezittingen                           | 0                               |
      | hoger_bevoorrechte_schulden                  | 0                               |
      | liquide_middelen                             | 0                               |
      | aanslagbedrag                                | 15000                           |
      | kinderopvang_nettokosten_maand               | 0                               |
      | is_ondernemer                                | false                           |
      | belasting_zakelijk                           | false                           |
      | gegevens_onvolledig_of_onjuist               | false                           |
      | bezwaar_of_beroep_aanhangig                  | false                           |
      | zekerheid_gesteld                            | false                           |
      | meerdere_belastingschuldigen                 | false                           |
      | derde_aansprakelijk_gesteld                  | false                           |
      | verwijtbaarheid_belastingschuld              | false                           |
      | in_faillissement_of_surseance_zonder_akkoord | false                           |
      | verzoek_ingediend                            | true                            |
    When the law "leidraad_invordering_waterschapsbelastingen_hhnk" is executed for outputs "kwijtschelding_komt_in_aanmerking,kwijtschelding_bedrag"
    Then the execution succeeds
    And the output "kwijtschelding_komt_in_aanmerking" is "true"
    And the output "kwijtschelding_bedrag" is "15000"

  # === HHNK-leidraad art 25.5.3: kort uitstel particulier (EUR 500 drempel) ===

  Scenario: Kort uitstel particulier - aan alle voorwaarden voldaan
    Given a citizen with the following data:
      | totale_openstaande_schuld                         | 40000 |
      | ander_uitstel_van_toepassing                      | false |
      | dwangbevel_betekend_of_postdwangbevel_verstreken  | false |
      | goed_betalingsgedrag_afgelopen_3_jaar             | true  |
      | automatische_incasso_geactiveerd                  | true  |
    When the law "leidraad_invordering_waterschapsbelastingen_hhnk" is executed for outputs "schuld_onder_drempel,komt_in_aanmerking_kort_uitstel_particulier"
    Then the execution succeeds
    And the output "schuld_onder_drempel" is "true"
    And the output "komt_in_aanmerking_kort_uitstel_particulier" is "true"

  Scenario: Kort uitstel particulier - schuld boven EUR 500 drempel, afwijzing
    Given a citizen with the following data:
      | totale_openstaande_schuld                         | 60000 |
      | ander_uitstel_van_toepassing                      | false |
      | dwangbevel_betekend_of_postdwangbevel_verstreken  | false |
      | goed_betalingsgedrag_afgelopen_3_jaar             | true  |
      | automatische_incasso_geactiveerd                  | true  |
    When the law "leidraad_invordering_waterschapsbelastingen_hhnk" is executed for outputs "schuld_onder_drempel,komt_in_aanmerking_kort_uitstel_particulier"
    Then the execution succeeds
    And the output "schuld_onder_drempel" is "false"
    And the output "komt_in_aanmerking_kort_uitstel_particulier" is "false"

  Scenario: Kort uitstel particulier - geen automatische incasso, afwijzing
    Given a citizen with the following data:
      | totale_openstaande_schuld                         | 40000 |
      | ander_uitstel_van_toepassing                      | false |
      | dwangbevel_betekend_of_postdwangbevel_verstreken  | false |
      | goed_betalingsgedrag_afgelopen_3_jaar             | true  |
      | automatische_incasso_geactiveerd                  | false |
    When the law "leidraad_invordering_waterschapsbelastingen_hhnk" is executed for outputs "komt_in_aanmerking_kort_uitstel_particulier"
    Then the execution succeeds
    And the output "komt_in_aanmerking_kort_uitstel_particulier" is "false"

  # === Leidraad 2008 art 26.2.3 overrides URI art 12: auto-drempel EUR 3350 vs EUR 2269 ===
  # Een auto van waarde EUR 3300 telt WEL als vermogen per URI (boven EUR 2269), maar NIET
  # per Leidraad 2008 (onder EUR 3350). De beleidsregel is lex pro cive.

  Scenario: Auto EUR 3300 - onder Leidraad-drempel ondanks URI-drempel, beleidsregel overrule
    Given a citizen with the following data:
      | auto_waarde    | 330000 |
      | auto_onmisbaar | false  |
    When the law "leidraad_invordering_2008" is executed for outputs "auto_als_vermogen"
    Then the execution succeeds
    And the output "auto_als_vermogen" is "0"

  Scenario: HHNK-leidraad art 26 - geen verzoek, beleidsregel blokkeert ondanks gunstige financiën
    Given a citizen with the following data:
      | bsn                                          | 999993653                       |
      | waterschap_code                              | WS0155                          |
      | belastingsoort                               | watersysteemheffing_ingezetenen |
      | netto_besteedbaar_inkomen_maand              | 140150                          |
      | netto_besteedbaar_inkomen_partner_maand      | 0                               |
      | huishoudtype                                 | alleenstaande                   |
      | is_pensioengerechtigd                        | false                           |
      | inboedel_waarde                              | 0                               |
      | auto_waarde                                  | 0                               |
      | auto_onmisbaar                               | false                           |
      | onroerend_goed_waarde                        | 0                               |
      | andere_bezittingen                           | 0                               |
      | hoger_bevoorrechte_schulden                  | 0                               |
      | liquide_middelen                             | 0                               |
      | aanslagbedrag                                | 15000                           |
      | kinderopvang_nettokosten_maand               | 0                               |
      | is_ondernemer                                | false                           |
      | belasting_zakelijk                           | false                           |
      | gegevens_onvolledig_of_onjuist               | false                           |
      | bezwaar_of_beroep_aanhangig                  | false                           |
      | zekerheid_gesteld                            | false                           |
      | meerdere_belastingschuldigen                 | false                           |
      | derde_aansprakelijk_gesteld                  | false                           |
      | verwijtbaarheid_belastingschuld              | false                           |
      | in_faillissement_of_surseance_zonder_akkoord | false                           |
      | verzoek_ingediend                            | false                           |
    When the law "leidraad_invordering_waterschapsbelastingen_hhnk" is executed for outputs "kwijtschelding_komt_in_aanmerking,kwijtschelding_bedrag"
    Then the execution succeeds
    And the output "kwijtschelding_komt_in_aanmerking" is "false"
    And the output "kwijtschelding_bedrag" is "0"
