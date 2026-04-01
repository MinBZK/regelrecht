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
    Given a query with the following data:
      | raakt_eenheid_kroon                    | false |
      | raakt_veiligheid_staat                 | false |
      | bevat_vertrouwelijke_bedrijfsgegevens  | false |
      | bevat_bijzondere_persoonsgegevens      | false |
      | bevat_identificatienummers             | false |
      | betrokkene_heeft_toestemming           | false |
      | persoonsgegevens_kennelijk_openbaar_gemaakt | false |
      | verstrekking_geen_inbreuk_levenssfeer  | false |
      | is_milieu_informatie                   | false |
      | betreft_emissies                       | false |
      | raakt_internationale_betrekkingen      | false |
      | raakt_economische_belangen             | false |
      | raakt_opsporing_vervolging             | false |
      | raakt_inspectie_toezicht               | false |
      | raakt_persoonlijke_levenssfeer         | false |
      | raakt_concurrentiegevoelige_gegevens   | false |
      | raakt_milieubescherming                | false |
      | raakt_beveiliging_personen             | false |
      | raakt_goed_functioneren_staat          | false |
      | belang_openbaarheid_weegt_zwaarder     | false |
      | onevenredige_benadeling_ander_belang   | true  |
      | bedrijfsgegevens_ernstig_geschaad      | false |
      | milieu_belang_openbaarheid_weegt_op    | false |
    When the WOO disclosure decision is executed
    Then the execution succeeds
    And the output "heeft_lid5_weigeringsgrond" is "true"
    And the output "openbaarmaking_toegestaan" is "false"

  Scenario: Lid 5 does not apply to environmental information
    # Art 5.1 lid 5 applies only to non-milieu-informatie.
    # Environmental info with disproportionate harm to other interests
    # is NOT blocked by lid 5 — must go through lid 6 instead.
    Given a query with the following data:
      | raakt_eenheid_kroon                    | false |
      | raakt_veiligheid_staat                 | false |
      | bevat_vertrouwelijke_bedrijfsgegevens  | false |
      | bevat_bijzondere_persoonsgegevens      | false |
      | bevat_identificatienummers             | false |
      | betrokkene_heeft_toestemming           | false |
      | persoonsgegevens_kennelijk_openbaar_gemaakt | false |
      | verstrekking_geen_inbreuk_levenssfeer  | false |
      | is_milieu_informatie                   | true  |
      | betreft_emissies                       | false |
      | raakt_internationale_betrekkingen      | false |
      | raakt_economische_belangen             | false |
      | raakt_opsporing_vervolging             | false |
      | raakt_inspectie_toezicht               | false |
      | raakt_persoonlijke_levenssfeer         | false |
      | raakt_concurrentiegevoelige_gegevens   | false |
      | raakt_milieubescherming                | false |
      | raakt_beveiliging_personen             | false |
      | raakt_goed_functioneren_staat          | false |
      | belang_openbaarheid_weegt_zwaarder     | false |
      | onevenredige_benadeling_ander_belang   | true  |
      | bedrijfsgegevens_ernstig_geschaad      | false |
      | milieu_belang_openbaarheid_weegt_op    | false |
    When the WOO disclosure decision is executed
    Then the execution succeeds
    And the output "heeft_lid5_weigeringsgrond" is "false"
    And the output "openbaarmaking_toegestaan" is "true"

  # === Lid 6: milieu-informatie + bedrijfsgegevens ===

  Scenario: Lid 6 blocks milieu-informatie with confidential business data seriously harmed
    # Art 5.1 lid 1c voor milieu-informatie: bedrijfsgegevens only blocked
    # when "ernstig geschaad" AND public interest does not outweigh.
    Given a query with the following data:
      | raakt_eenheid_kroon                    | false |
      | raakt_veiligheid_staat                 | false |
      | bevat_vertrouwelijke_bedrijfsgegevens  | true  |
      | bevat_bijzondere_persoonsgegevens      | false |
      | bevat_identificatienummers             | false |
      | betrokkene_heeft_toestemming           | false |
      | persoonsgegevens_kennelijk_openbaar_gemaakt | false |
      | verstrekking_geen_inbreuk_levenssfeer  | false |
      | is_milieu_informatie                   | true  |
      | betreft_emissies                       | false |
      | raakt_internationale_betrekkingen      | false |
      | raakt_economische_belangen             | false |
      | raakt_opsporing_vervolging             | false |
      | raakt_inspectie_toezicht               | false |
      | raakt_persoonlijke_levenssfeer         | false |
      | raakt_concurrentiegevoelige_gegevens   | false |
      | raakt_milieubescherming                | false |
      | raakt_beveiliging_personen             | false |
      | raakt_goed_functioneren_staat          | false |
      | belang_openbaarheid_weegt_zwaarder     | false |
      | onevenredige_benadeling_ander_belang   | false |
      | bedrijfsgegevens_ernstig_geschaad      | true  |
      | milieu_belang_openbaarheid_weegt_op    | false |
    When the WOO disclosure decision is executed
    Then the execution succeeds
    And the output "heeft_absolute_weigeringsgrond" is "true"
    And the output "openbaarmaking_toegestaan" is "false"

  Scenario: Lid 6 allows milieu-informatie when public interest outweighs business harm
    # Art 5.1 lid 6: even when bedrijfsgegevens ernstig geschaad,
    # disclosure is allowed if public interest outweighs.
    Given a query with the following data:
      | raakt_eenheid_kroon                    | false |
      | raakt_veiligheid_staat                 | false |
      | bevat_vertrouwelijke_bedrijfsgegevens  | true  |
      | bevat_bijzondere_persoonsgegevens      | false |
      | bevat_identificatienummers             | false |
      | betrokkene_heeft_toestemming           | false |
      | persoonsgegevens_kennelijk_openbaar_gemaakt | false |
      | verstrekking_geen_inbreuk_levenssfeer  | false |
      | is_milieu_informatie                   | true  |
      | betreft_emissies                       | false |
      | raakt_internationale_betrekkingen      | false |
      | raakt_economische_belangen             | false |
      | raakt_opsporing_vervolging             | false |
      | raakt_inspectie_toezicht               | false |
      | raakt_persoonlijke_levenssfeer         | false |
      | raakt_concurrentiegevoelige_gegevens   | false |
      | raakt_milieubescherming                | false |
      | raakt_beveiliging_personen             | false |
      | raakt_goed_functioneren_staat          | false |
      | belang_openbaarheid_weegt_zwaarder     | false |
      | onevenredige_benadeling_ander_belang   | false |
      | bedrijfsgegevens_ernstig_geschaad      | true  |
      | milieu_belang_openbaarheid_weegt_op    | true  |
    When the WOO disclosure decision is executed
    Then the execution succeeds
    And the output "heeft_absolute_weigeringsgrond" is "false"
    And the output "openbaarmaking_toegestaan" is "true"

  Scenario: Milieu-informatie with business data not seriously harmed is disclosed
    # Art 5.1 lid 6: if bedrijfsgegevens are NOT ernstig geschaad,
    # the absolute ground does not apply — disclosure allowed.
    Given a query with the following data:
      | raakt_eenheid_kroon                    | false |
      | raakt_veiligheid_staat                 | false |
      | bevat_vertrouwelijke_bedrijfsgegevens  | true  |
      | bevat_bijzondere_persoonsgegevens      | false |
      | bevat_identificatienummers             | false |
      | betrokkene_heeft_toestemming           | false |
      | persoonsgegevens_kennelijk_openbaar_gemaakt | false |
      | verstrekking_geen_inbreuk_levenssfeer  | false |
      | is_milieu_informatie                   | true  |
      | betreft_emissies                       | false |
      | raakt_internationale_betrekkingen      | false |
      | raakt_economische_belangen             | false |
      | raakt_opsporing_vervolging             | false |
      | raakt_inspectie_toezicht               | false |
      | raakt_persoonlijke_levenssfeer         | false |
      | raakt_concurrentiegevoelige_gegevens   | false |
      | raakt_milieubescherming                | false |
      | raakt_beveiliging_personen             | false |
      | raakt_goed_functioneren_staat          | false |
      | belang_openbaarheid_weegt_zwaarder     | false |
      | onevenredige_benadeling_ander_belang   | false |
      | bedrijfsgegevens_ernstig_geschaad      | false |
      | milieu_belang_openbaarheid_weegt_op    | false |
    When the WOO disclosure decision is executed
    Then the execution succeeds
    And the output "heeft_absolute_weigeringsgrond" is "false"
    And the output "openbaarmaking_toegestaan" is "true"
