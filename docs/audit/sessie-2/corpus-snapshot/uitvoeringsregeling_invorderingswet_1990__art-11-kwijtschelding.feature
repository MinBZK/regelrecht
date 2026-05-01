Feature: Artikel 11 URI 1990 — kwijtschelding orchestrator (federal-only)

  Drie scenarios die de keten art 11 -> 12 -> 16 + art 11 -> 13 -> 16 demonstreren.
  Open-terms gebruiken hun defaults (kostennorm_percentage=0.9, verhoging=0). HHNK-
  specifieke verhoging-vrijstelling en scope-restricties zitten niet in deze cut.

  Bedragen in eurocent. Alle scenarios: huishoudtype=alleenstaand omdat
  0.9 * bijstandsnorm_alleenstaand (140150) = 126135 zonder decimalen, wat exacte
  verwachte outputs oplevert.

  Background:
    Given the calculation date is "2026-01-01"
    Given law "uitvoeringsregeling_invorderingswet_1990" is loaded

  Scenario: Volledige kwijtschelding voor iemand zonder inkomen
    # NBI=0 -> betalingscapaciteit clamped op 0 (geen positief verschil met kostennorm).
    # Vermogen=0. Hoogte = aanslag = 10000 eurocent (EUR 100).
    Given parameter "bsn" is "999993653"
    Given parameter "huishoudtype" is "alleenstaand"
    Given the following parameters:
      | name                                    | value |
      | netto_besteedbaar_inkomen_maand         | 0     |
      | netto_besteedbaar_inkomen_partner_maand | 0     |
      | is_pensioengerechtigd                   | false |
      | extra_uitgaven_maand                    | 0     |
      | inboedel_waarde                         | 0     |
      | auto_waarde                             | 0     |
      | auto_onmisbaar                          | false |
      | onroerend_goed_waarde                   | 0     |
      | andere_bezittingen                      | 0     |
      | hoger_bevoorrechte_schulden             | 0     |
      | liquide_middelen                        | 0     |
      | bruto_woonlasten_maand                  | 0     |
      | huursubsidie_ontvangen_maand            | 0     |
      | premies_ziektekosten_maand              | 0     |
      | kindgebonden_budget_tekort_maand        | 0     |
      | is_geboren_voor_1935                    | false |
      | is_partner_geboren_voor_1935            | false |
      | is_kostendeler                          | false |
      | kostendelersnorm_bedrag                 | 0     |
      | woont_buiten_nederland                  | false |
      | woonland_factor                         | 1     |
      | betalingen_belastingschulden_maand      | 0     |
      | betaalde_alimentatie_maand              | 0     |
      | aflossingen_belastingschulden_maand     | 0     |
      | kostgangerskosten_maand                 | 0     |
      | overige_noodzakelijke_uitgaven_maand    | 0     |
      | aanslagbedrag                           | 10000 |
    When I evaluate "hoogte_kwijtschelding" of "uitvoeringsregeling_invorderingswet_1990"
    Then the execution succeeds
    # vermogen_bedrag en betalingscapaciteit zijn inputs van art 11 (gesourced van
    # art 12/13), niet outputs — niet asserteerbaar via output-step. Tussen-resultaat
    # zichtbaar in execution-trace; eindresultaat is hoogte + kan.
    Then output "aanwendbare_betalingscapaciteit" equals 0
    Then output "hoogte_kwijtschelding" equals 10000
    Then output "kan_kwijtschelding_worden_verleend" is true

  Scenario: Gedeeltelijke kwijtschelding bij beperkte betalingscapaciteit
    # NBI=130000 (EUR 1300/maand), kostennorm=126135. BC = 12 * (130000 - 126135) = 46380.
    # aanwendbare BC = 0.8 * 46380 = 37104. Aanslag = 50000 (EUR 500).
    # Hoogte = MAX(0, 50000 - 0 - 37104) = 12896.
    Given parameter "bsn" is "999993653"
    Given parameter "huishoudtype" is "alleenstaand"
    Given the following parameters:
      | name                                    | value  |
      | netto_besteedbaar_inkomen_maand         | 130000 |
      | netto_besteedbaar_inkomen_partner_maand | 0      |
      | is_pensioengerechtigd                   | false  |
      | extra_uitgaven_maand                    | 0      |
      | inboedel_waarde                         | 0      |
      | auto_waarde                             | 0      |
      | auto_onmisbaar                          | false  |
      | onroerend_goed_waarde                   | 0      |
      | andere_bezittingen                      | 0      |
      | hoger_bevoorrechte_schulden             | 0      |
      | liquide_middelen                        | 0      |
      | bruto_woonlasten_maand                  | 0      |
      | huursubsidie_ontvangen_maand            | 0      |
      | premies_ziektekosten_maand              | 0      |
      | kindgebonden_budget_tekort_maand        | 0      |
      | is_geboren_voor_1935                    | false  |
      | is_partner_geboren_voor_1935            | false  |
      | is_kostendeler                          | false  |
      | kostendelersnorm_bedrag                 | 0      |
      | woont_buiten_nederland                  | false  |
      | woonland_factor                         | 1      |
      | betalingen_belastingschulden_maand      | 0      |
      | betaalde_alimentatie_maand              | 0      |
      | aflossingen_belastingschulden_maand     | 0      |
      | kostgangerskosten_maand                 | 0      |
      | overige_noodzakelijke_uitgaven_maand    | 0      |
      | aanslagbedrag                           | 50000  |
    When I evaluate "hoogte_kwijtschelding" of "uitvoeringsregeling_invorderingswet_1990"
    Then the execution succeeds
    # vermogen_bedrag (=0) en betalingscapaciteit (=46380) zijn inputs van art 11,
    # zichtbaar in execution-trace; niet in result.outputs.
    Then output "aanwendbare_betalingscapaciteit" equals 37104
    Then output "hoogte_kwijtschelding" equals 12896
    Then output "kan_kwijtschelding_worden_verleend" is true

  Scenario: Geen kwijtschelding wanneer betalingscapaciteit aanslag overstijgt
    # NBI=200000 (EUR 2000/maand). BC = 12 * (200000 - 126135) = 886380.
    # aanwendbare BC = 0.8 * 886380 = 709104. Aanslag = 10000 (EUR 100).
    # Hoogte = MAX(0, 10000 - 0 - 709104) = 0. Afwijzing.
    Given parameter "bsn" is "999993653"
    Given parameter "huishoudtype" is "alleenstaand"
    Given the following parameters:
      | name                                    | value  |
      | netto_besteedbaar_inkomen_maand         | 200000 |
      | netto_besteedbaar_inkomen_partner_maand | 0      |
      | is_pensioengerechtigd                   | false  |
      | extra_uitgaven_maand                    | 0      |
      | inboedel_waarde                         | 0      |
      | auto_waarde                             | 0      |
      | auto_onmisbaar                          | false  |
      | onroerend_goed_waarde                   | 0      |
      | andere_bezittingen                      | 0      |
      | hoger_bevoorrechte_schulden             | 0      |
      | liquide_middelen                        | 0      |
      | bruto_woonlasten_maand                  | 0      |
      | huursubsidie_ontvangen_maand            | 0      |
      | premies_ziektekosten_maand              | 0      |
      | kindgebonden_budget_tekort_maand        | 0      |
      | is_geboren_voor_1935                    | false  |
      | is_partner_geboren_voor_1935            | false  |
      | is_kostendeler                          | false  |
      | kostendelersnorm_bedrag                 | 0      |
      | woont_buiten_nederland                  | false  |
      | woonland_factor                         | 1      |
      | betalingen_belastingschulden_maand      | 0      |
      | betaalde_alimentatie_maand              | 0      |
      | aflossingen_belastingschulden_maand     | 0      |
      | kostgangerskosten_maand                 | 0      |
      | overige_noodzakelijke_uitgaven_maand    | 0      |
      | aanslagbedrag                           | 10000  |
    When I evaluate "hoogte_kwijtschelding" of "uitvoeringsregeling_invorderingswet_1990"
    Then the execution succeeds
    # vermogen_bedrag (=0) en betalingscapaciteit (=886380) zijn inputs van art 11,
    # zichtbaar in execution-trace; niet in result.outputs.
    Then output "aanwendbare_betalingscapaciteit" equals 709104
    Then output "hoogte_kwijtschelding" equals 0
    Then output "kan_kwijtschelding_worden_verleend" is false
