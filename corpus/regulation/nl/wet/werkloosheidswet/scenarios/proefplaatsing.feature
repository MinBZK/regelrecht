Feature: Proefplaatsing (Werkloosheidswet artikel 76a)
  Als WW-uitkeringsgerechtigde werknemer
  Wil ik weten of ik een proefplaatsing mag aangaan
  Zodat ik zes maanden onbeloonde arbeid kan verrichten met behoud
  van WW-uitkering

  # Bron: Werkloosheidswet artikel 76a (BWBR0004045) lid 1 t/m 5.
  # Geldend op peildatum 2024-01-01.

  Background:
    Given the calculation date is "2024-06-01"

  Scenario: WW-gerechtigde met aansprakelijkheidsverzekering en uitzicht op vaste baan mag proefplaatsing aangaan
    Given the following parameters:
      | bsn                                            | 999990020 |
      | heeft_recht_op_ww_uitkering                    | true      |
      | in_staat_tot_werkzaamheden                     | true      |
      | aansprakelijkheidsverzekering_aanwezig         | true      |
      | niet_eerder_proefplaatsing_zelfde_werkgever    | true      |
      | reeel_uitzicht_op_dienstbetrekking_zes_maanden | true      |
    When I evaluate "mag_proefplaatsing_aangaan" of "werkloosheidswet"
    Then the execution succeeds
    And output "mag_proefplaatsing_aangaan" is true
    And output "voldoet_aan_lid_3_voorwaarden" is true
    And output "max_duur_proefplaatsing_maanden" equals 6
    And output "ww_uitkering_blijft_bestaan" is true

  Scenario: Werknemer zonder WW-uitkering mag geen proefplaatsing aangaan
    Given the following parameters:
      | bsn                                            | 999990021 |
      | heeft_recht_op_ww_uitkering                    | false     |
      | in_staat_tot_werkzaamheden                     | true      |
      | aansprakelijkheidsverzekering_aanwezig         | true      |
      | niet_eerder_proefplaatsing_zelfde_werkgever    | true      |
      | reeel_uitzicht_op_dienstbetrekking_zes_maanden | true      |
    When I evaluate "mag_proefplaatsing_aangaan" of "werkloosheidswet"
    Then the execution succeeds
    And output "mag_proefplaatsing_aangaan" is false
    And output "ww_uitkering_blijft_bestaan" is false

  # Randgeval: WW-uitkering aanwezig, maar werkgever heeft geen
  # aansprakelijkheidsverzekering — proefplaatsing mag niet (lid 3.b).
  Scenario: WW-gerechtigde zonder aansprakelijkheidsverzekering mag geen proefplaatsing aangaan
    Given the following parameters:
      | bsn                                            | 999990022 |
      | heeft_recht_op_ww_uitkering                    | true      |
      | in_staat_tot_werkzaamheden                     | true      |
      | aansprakelijkheidsverzekering_aanwezig         | false     |
      | niet_eerder_proefplaatsing_zelfde_werkgever    | true      |
      | reeel_uitzicht_op_dienstbetrekking_zes_maanden | true      |
    When I evaluate "mag_proefplaatsing_aangaan" of "werkloosheidswet"
    Then the execution succeeds
    And output "mag_proefplaatsing_aangaan" is false
    And output "voldoet_aan_lid_3_voorwaarden" is false

  # Randgeval: alle voorwaarden vervuld, behalve dat werknemer eerder
  # al onbeloond bij deze werkgever heeft gewerkt — niet toegestaan
  # (lid 3.c, voorkomt herhaaldelijk gratis werk).
  Scenario: Eerdere proefplaatsing bij zelfde werkgever blokkeert nieuwe proefplaatsing
    Given the following parameters:
      | bsn                                            | 999990023 |
      | heeft_recht_op_ww_uitkering                    | true      |
      | in_staat_tot_werkzaamheden                     | true      |
      | aansprakelijkheidsverzekering_aanwezig         | true      |
      | niet_eerder_proefplaatsing_zelfde_werkgever    | false     |
      | reeel_uitzicht_op_dienstbetrekking_zes_maanden | true      |
    When I evaluate "mag_proefplaatsing_aangaan" of "werkloosheidswet"
    Then the execution succeeds
    And output "mag_proefplaatsing_aangaan" is false
