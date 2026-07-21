Feature: WOO Art 5.1 disclosure decision
  Als bestuursorgaan
  Wil ik weten of informatie openbaar gemaakt mag worden
  Zodat ik de Wet open overheid correct toepas

  # Art 5.1 WOO — weigeringsgronden:
  #   lid 1: absolute gronden (eenheid kroon, veiligheid staat, bedrijfsgegevens, etc.)
  #   lid 2: relatieve gronden (belangenafweging vereist)
  #   lid 5: onevenredige benadeling (niet-milieu-informatie)
  #   lid 6: milieu-informatie + bedrijfsgegevens → "ernstig geschaad" toets
  #   lid 7: emissies → absolute/relatieve gronden gelden niet

  Background:
    Given the calculation date is "2025-03-01"

  # === Lid 5: onevenredige benadeling ===

  Scenario: Lid 5 blocks disclosure for non-environmental info with disproportionate harm
    # Art 5.1 lid 5: openbaarmaking blijft achterwege voor zover het belang
    # daarvan niet opweegt tegen onevenredige benadeling van een ander belang.
    # Only applies to non-milieu-informatie.
    Given the following parameters:
      | raakt_eenheid_kroon                         | false |
      | raakt_veiligheid_staat                      | false |
      | bevat_vertrouwelijke_bedrijfsgegevens       | false |
      | bevat_bijzondere_persoonsgegevens           | false |
      | bevat_identificatienummers                  | false |
      | betrokkene_heeft_toestemming                | false |
      | persoonsgegevens_kennelijk_openbaar_gemaakt | false |
      | verstrekking_geen_inbreuk_levenssfeer       | false |
      | is_milieu_informatie                        | false |
      | betreft_emissies                            | false |
      | raakt_internationale_betrekkingen           | false |
      | raakt_economische_belangen                  | false |
      | raakt_opsporing_vervolging                  | false |
      | raakt_inspectie_toezicht                    | false |
      | raakt_persoonlijke_levenssfeer              | false |
      | raakt_concurrentiegevoelige_gegevens        | false |
      | raakt_milieubescherming                     | false |
      | raakt_beveiliging_personen                  | false |
      | raakt_goed_functioneren_staat               | false |
      | belang_openbaarheid_weegt_zwaarder          | false |
      | onevenredige_benadeling_ander_belang        | true  |
      | bedrijfsgegevens_ernstig_geschaad           | false |
      | milieu_belang_openbaarheid_weegt_op         | false |
    When I evaluate "heeft_lid5_weigeringsgrond" of "wet_open_overheid"
    Then the execution succeeds
    Then output "heeft_lid5_weigeringsgrond" is true
    When I evaluate "openbaarmaking_toegestaan" of "wet_open_overheid"
    Then output "openbaarmaking_toegestaan" is false

  Scenario: Lid 5 does not apply to environmental information
    # Art 5.1 lid 5 applies only to non-milieu-informatie.
    # Environmental info with disproportionate harm to other interests
    # is NOT blocked by lid 5 — must go through lid 6 instead.
    Given the following parameters:
      | raakt_eenheid_kroon                         | false |
      | raakt_veiligheid_staat                      | false |
      | bevat_vertrouwelijke_bedrijfsgegevens       | false |
      | bevat_bijzondere_persoonsgegevens           | false |
      | bevat_identificatienummers                  | false |
      | betrokkene_heeft_toestemming                | false |
      | persoonsgegevens_kennelijk_openbaar_gemaakt | false |
      | verstrekking_geen_inbreuk_levenssfeer       | false |
      | is_milieu_informatie                        | true  |
      | betreft_emissies                            | false |
      | raakt_internationale_betrekkingen           | false |
      | raakt_economische_belangen                  | false |
      | raakt_opsporing_vervolging                  | false |
      | raakt_inspectie_toezicht                    | false |
      | raakt_persoonlijke_levenssfeer              | false |
      | raakt_concurrentiegevoelige_gegevens        | false |
      | raakt_milieubescherming                     | false |
      | raakt_beveiliging_personen                  | false |
      | raakt_goed_functioneren_staat               | false |
      | belang_openbaarheid_weegt_zwaarder          | false |
      | onevenredige_benadeling_ander_belang        | true  |
      | bedrijfsgegevens_ernstig_geschaad           | false |
      | milieu_belang_openbaarheid_weegt_op         | false |
    When I evaluate "heeft_lid5_weigeringsgrond" of "wet_open_overheid"
    Then the execution succeeds
    Then output "heeft_lid5_weigeringsgrond" is false
    When I evaluate "openbaarmaking_toegestaan" of "wet_open_overheid"
    Then output "openbaarmaking_toegestaan" is true

  # === Lid 6: milieu-informatie + bedrijfsgegevens ===

  Scenario: Lid 6 blocks milieu-informatie with confidential business data seriously harmed
    # Art 5.1 lid 1c voor milieu-informatie: bedrijfsgegevens only blocked
    # when "ernstig geschaad" AND public interest does not outweigh.
    Given the following parameters:
      | raakt_eenheid_kroon                         | false |
      | raakt_veiligheid_staat                      | false |
      | bevat_vertrouwelijke_bedrijfsgegevens       | true  |
      | bevat_bijzondere_persoonsgegevens           | false |
      | bevat_identificatienummers                  | false |
      | betrokkene_heeft_toestemming                | false |
      | persoonsgegevens_kennelijk_openbaar_gemaakt | false |
      | verstrekking_geen_inbreuk_levenssfeer       | false |
      | is_milieu_informatie                        | true  |
      | betreft_emissies                            | false |
      | raakt_internationale_betrekkingen           | false |
      | raakt_economische_belangen                  | false |
      | raakt_opsporing_vervolging                  | false |
      | raakt_inspectie_toezicht                    | false |
      | raakt_persoonlijke_levenssfeer              | false |
      | raakt_concurrentiegevoelige_gegevens        | false |
      | raakt_milieubescherming                     | false |
      | raakt_beveiliging_personen                  | false |
      | raakt_goed_functioneren_staat               | false |
      | belang_openbaarheid_weegt_zwaarder          | false |
      | onevenredige_benadeling_ander_belang        | false |
      | bedrijfsgegevens_ernstig_geschaad           | true  |
      | milieu_belang_openbaarheid_weegt_op         | false |
    When I evaluate "heeft_absolute_weigeringsgrond" of "wet_open_overheid"
    Then the execution succeeds
    Then output "heeft_absolute_weigeringsgrond" is true
    When I evaluate "openbaarmaking_toegestaan" of "wet_open_overheid"
    Then output "openbaarmaking_toegestaan" is false

  Scenario: Lid 6 allows milieu-informatie when public interest outweighs business harm
    # Art 5.1 lid 6: even when bedrijfsgegevens ernstig geschaad,
    # disclosure is allowed if public interest outweighs.
    Given the following parameters:
      | raakt_eenheid_kroon                         | false |
      | raakt_veiligheid_staat                      | false |
      | bevat_vertrouwelijke_bedrijfsgegevens       | true  |
      | bevat_bijzondere_persoonsgegevens           | false |
      | bevat_identificatienummers                  | false |
      | betrokkene_heeft_toestemming                | false |
      | persoonsgegevens_kennelijk_openbaar_gemaakt | false |
      | verstrekking_geen_inbreuk_levenssfeer       | false |
      | is_milieu_informatie                        | true  |
      | betreft_emissies                            | false |
      | raakt_internationale_betrekkingen           | false |
      | raakt_economische_belangen                  | false |
      | raakt_opsporing_vervolging                  | false |
      | raakt_inspectie_toezicht                    | false |
      | raakt_persoonlijke_levenssfeer              | false |
      | raakt_concurrentiegevoelige_gegevens        | false |
      | raakt_milieubescherming                     | false |
      | raakt_beveiliging_personen                  | false |
      | raakt_goed_functioneren_staat               | false |
      | belang_openbaarheid_weegt_zwaarder          | false |
      | onevenredige_benadeling_ander_belang        | false |
      | bedrijfsgegevens_ernstig_geschaad           | true  |
      | milieu_belang_openbaarheid_weegt_op         | true  |
    When I evaluate "heeft_absolute_weigeringsgrond" of "wet_open_overheid"
    Then the execution succeeds
    Then output "heeft_absolute_weigeringsgrond" is false
    When I evaluate "openbaarmaking_toegestaan" of "wet_open_overheid"
    Then output "openbaarmaking_toegestaan" is true

  Scenario: Milieu-informatie with business data not seriously harmed is disclosed
    # Art 5.1 lid 6: if bedrijfsgegevens are NOT ernstig geschaad,
    # the absolute ground does not apply — disclosure allowed.
    Given the following parameters:
      | raakt_eenheid_kroon                         | false |
      | raakt_veiligheid_staat                      | false |
      | bevat_vertrouwelijke_bedrijfsgegevens       | true  |
      | bevat_bijzondere_persoonsgegevens           | false |
      | bevat_identificatienummers                  | false |
      | betrokkene_heeft_toestemming                | false |
      | persoonsgegevens_kennelijk_openbaar_gemaakt | false |
      | verstrekking_geen_inbreuk_levenssfeer       | false |
      | is_milieu_informatie                        | true  |
      | betreft_emissies                            | false |
      | raakt_internationale_betrekkingen           | false |
      | raakt_economische_belangen                  | false |
      | raakt_opsporing_vervolging                  | false |
      | raakt_inspectie_toezicht                    | false |
      | raakt_persoonlijke_levenssfeer              | false |
      | raakt_concurrentiegevoelige_gegevens        | false |
      | raakt_milieubescherming                     | false |
      | raakt_beveiliging_personen                  | false |
      | raakt_goed_functioneren_staat               | false |
      | belang_openbaarheid_weegt_zwaarder          | false |
      | onevenredige_benadeling_ander_belang        | false |
      | bedrijfsgegevens_ernstig_geschaad           | false |
      | milieu_belang_openbaarheid_weegt_op         | false |
    When I evaluate "heeft_absolute_weigeringsgrond" of "wet_open_overheid"
    Then the execution succeeds
    Then output "heeft_absolute_weigeringsgrond" is false
    When I evaluate "openbaarmaking_toegestaan" of "wet_open_overheid"
    Then output "openbaarmaking_toegestaan" is true

  # === Lid 7: emissies overriden alle weigeringsgronden (uit negative_scenarios) ===

  Scenario: Lid 7 — emissies override all refusal grounds
    # Art 5.1 lid 7: absolute and relative grounds do not apply to
    # milieu-informatie about emissies. Even state security and Crown
    # unity are overridden. This is an absolute right to disclosure.
    Given the following parameters:
      | raakt_eenheid_kroon                         | true  |
      | raakt_veiligheid_staat                      | true  |
      | bevat_vertrouwelijke_bedrijfsgegevens       | true  |
      | bevat_bijzondere_persoonsgegevens           | false |
      | bevat_identificatienummers                  | false |
      | betrokkene_heeft_toestemming                | false |
      | persoonsgegevens_kennelijk_openbaar_gemaakt | false |
      | verstrekking_geen_inbreuk_levenssfeer       | false |
      | is_milieu_informatie                        | true  |
      | betreft_emissies                            | true  |
      | raakt_internationale_betrekkingen           | true  |
      | raakt_economische_belangen                  | true  |
      | raakt_opsporing_vervolging                  | true  |
      | raakt_inspectie_toezicht                    | true  |
      | raakt_persoonlijke_levenssfeer              | true  |
      | raakt_concurrentiegevoelige_gegevens        | true  |
      | raakt_milieubescherming                     | true  |
      | raakt_beveiliging_personen                  | true  |
      | raakt_goed_functioneren_staat               | true  |
      | belang_openbaarheid_weegt_zwaarder          | false |
      | onevenredige_benadeling_ander_belang        | true  |
      | bedrijfsgegevens_ernstig_geschaad           | true  |
      | milieu_belang_openbaarheid_weegt_op         | false |
    When I evaluate "heeft_absolute_weigeringsgrond" of "wet_open_overheid"
    Then the execution succeeds
    Then output "heeft_absolute_weigeringsgrond" is false
    When I evaluate "heeft_relatieve_weigeringsgrond" of "wet_open_overheid"
    Then output "heeft_relatieve_weigeringsgrond" is false
    When I evaluate "openbaarmaking_toegestaan" of "wet_open_overheid"
    Then output "openbaarmaking_toegestaan" is true

  # === Art 5.3: verzwaarde motiveringsplicht (uit negative_scenarios) ===

  Scenario: Art 5.3 without informatie_datum fails
    # Art 5.3 requires informatie_datum to compute age.
    Given the following parameters:
      | peildatum | 2025-03-01 |
    When I evaluate "verzwaarde_motiveringsplicht" of "wet_open_overheid"
    Then the execution fails with "informatie_datum"

  Scenario: Art 5.3 — exactly 5 years old does NOT trigger enhanced motivation
    # "ouder dan vijf jaar" = strictly older than 5 years.
    # Information that is exactly 5 years old (to the day) should NOT
    # trigger verzwaarde motiveringsplicht.
    Given the following parameters:
      | informatie_datum | 2020-03-01 |
      | peildatum        | 2025-03-01 |
    When I evaluate "informatie_leeftijd_jaren" of "wet_open_overheid"
    Then the execution succeeds
    Then output "informatie_leeftijd_jaren" equals 5
    When I evaluate "verzwaarde_motiveringsplicht" of "wet_open_overheid"
    Then output "verzwaarde_motiveringsplicht" is false

  Scenario: Art 5.3 — over 5 years old triggers enhanced motivation
    # Information from 2019-01-01, checked on 2025-03-01 = 6 completed years.
    # AGE operation counts completed years, so this is strictly > 5.
    Given the following parameters:
      | informatie_datum | 2019-01-01 |
      | peildatum        | 2025-03-01 |
    When I evaluate "verzwaarde_motiveringsplicht" of "wet_open_overheid"
    Then the execution succeeds
    Then output "verzwaarde_motiveringsplicht" is true
