Feature: Bijstandsaanvraag via Participatiewet
  Als burger zonder voldoende middelen
  Wil ik bijstand kunnen aanvragen bij mijn gemeente
  Zodat ik in mijn levensonderhoud kan voorzien

  # Keten: Participatiewet (Rijkswet) + Afstemmingsverordening (Gemeente)
  #
  # Art. 11: Rechthebbenden - Nederlanders zonder middelen
  # Art. 21: Normbedragen - €1.091,71 (alleenstaand) / €1.559,58 (gehuwd)
  # Art. 8:  Delegatie - gemeente stelt verordening vast
  # Art. 18: Verlaging - bij niet nakomen verplichtingen
  #
  # Afstemmingsverordening Diemen (GM0384):
  #   Categorie 1: 5%   - niet tijdig registreren UWV
  #   Categorie 2: 30%  - niet meewerken plan van aanpak
  #   Categorie 3: 100% - niet naar vermogen werk zoeken
  #
  # Formule: uitkering = normbedrag - (normbedrag × verlaging%)

  Background:
    Given the calculation date is "2024-06-01"

  # === Toekenningsscenario's voor burger uit Diemen (GM0384) ===

  Scenario: Alleenstaande burger krijgt volledige bijstand
    Given a citizen with the following data:
      | gemeente_code                          | GM0384       |
      | leeftijd                               | 35           |
      | is_nederlander                         | true         |
      | woont_in_nederland                     | true         |
      | is_alleenstaande                       | true         |
      | heeft_kostendelende_medebewoners       | false        |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false    |
      | is_geregistreerd_als_werkzoekende      | true         |
      | heeft_voldoende_middelen               | false        |
      | gedragscategorie                       | 0            |
    When the bijstandsaanvraag is executed for participatiewet article 43
    Then the citizen has the right to bijstand
    And the uitkering_bedrag is "109171" eurocent

  Scenario: Gehuwde burger krijgt volledige bijstand
    Given a citizen with the following data:
      | gemeente_code                          | GM0384       |
      | leeftijd                               | 42           |
      | is_nederlander                         | true         |
      | woont_in_nederland                     | true         |
      | is_alleenstaande                       | false        |
      | heeft_kostendelende_medebewoners       | false        |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false    |
      | is_geregistreerd_als_werkzoekende      | true         |
      | heeft_voldoende_middelen               | false        |
      | gedragscategorie                       | 0            |
    When the bijstandsaanvraag is executed for participatiewet article 43
    Then the citizen has the right to bijstand
    And the uitkering_bedrag is "155958" eurocent

  Scenario: Burger met gedragscategorie 1 krijgt 5% verlaging (Diemen)
    # Categorie 1: niet tijdig geregistreerd bij UWV
    Given a citizen with the following data:
      | gemeente_code                          | GM0384       |
      | leeftijd                               | 28           |
      | is_nederlander                         | true         |
      | woont_in_nederland                     | true         |
      | is_alleenstaande                       | true         |
      | heeft_kostendelende_medebewoners       | false        |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false    |
      | is_geregistreerd_als_werkzoekende      | true         |
      | heeft_voldoende_middelen               | false        |
      | gedragscategorie                       | 1            |
    When the bijstandsaanvraag is executed for participatiewet article 43
    Then the citizen has the right to bijstand
    And the uitkering_bedrag is "103712" eurocent

  Scenario: Burger met gedragscategorie 2 krijgt 30% verlaging (Diemen)
    # Categorie 2: niet meewerken aan plan van aanpak
    Given a citizen with the following data:
      | gemeente_code                          | GM0384       |
      | leeftijd                               | 45           |
      | is_nederlander                         | true         |
      | woont_in_nederland                     | true         |
      | is_alleenstaande                       | true         |
      | heeft_kostendelende_medebewoners       | false        |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false    |
      | is_geregistreerd_als_werkzoekende      | true         |
      | heeft_voldoende_middelen               | false        |
      | gedragscategorie                       | 2            |
    When the bijstandsaanvraag is executed for participatiewet article 43
    Then the citizen has the right to bijstand
    And the uitkering_bedrag is "76420" eurocent

  Scenario: Burger met gedragscategorie 3 krijgt 100% verlaging (Diemen)
    # Categorie 3: niet naar vermogen arbeid verkrijgen
    Given a citizen with the following data:
      | gemeente_code                          | GM0384       |
      | leeftijd                               | 30           |
      | is_nederlander                         | true         |
      | woont_in_nederland                     | true         |
      | is_alleenstaande                       | true         |
      | heeft_kostendelende_medebewoners       | false        |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false    |
      | is_geregistreerd_als_werkzoekende      | true         |
      | heeft_voldoende_middelen               | false        |
      | gedragscategorie                       | 3            |
    When the bijstandsaanvraag is executed for participatiewet article 43
    Then the citizen has the right to bijstand
    And the uitkering_bedrag is "0" eurocent

  # === Afwijzingsscenario's ===

  Scenario: Burger jonger dan 21 krijgt geen bijstand
    # Art. 21 checks leeftijd >= 21. Under-21 fails that check,
    # which propagates to Art. 43 via heeft_recht_op_bijstand = false.
    Given a citizen with the following data:
      | gemeente_code                          | GM0384       |
      | leeftijd                               | 19           |
      | is_nederlander                         | true         |
      | woont_in_nederland                     | true         |
      | is_alleenstaande                       | true         |
      | heeft_kostendelende_medebewoners       | false        |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false    |
      | is_geregistreerd_als_werkzoekende      | true         |
      | heeft_voldoende_middelen               | false        |
      | gedragscategorie                       | 0            |
    When the bijstandsaanvraag is executed for participatiewet article 43
    Then the citizen does not have the right to bijstand
    And the uitkering_bedrag is "0" eurocent

  # The following rejection scenarios tested conditions that were removed from
  # Art. 43 as scope violations (nationality, middelen, werkzoekende).
  # These checks belong in Art. 11 (Rechthebbenden), which is not yet
  # implemented as machine_readable. Blocked by #384.
  #
  # Scenario: Burger met voldoende middelen krijgt geen bijstand
  # Scenario: Niet-Nederlander zonder gelijkstelling krijgt geen bijstand
  # Scenario: Burger niet geregistreerd als werkzoekende krijgt geen bijstand

  # === Gemeente zonder afstemmingsverordening: volledige bijstand ===

  Scenario: Burger uit gemeente zonder verordening krijgt volledige bijstand
    # Gemeente GM9999 heeft geen afstemmingsverordening
    # Art. 18 lid 2: "verlaagt ... overeenkomstig de verordening"
    # Geen verordening = geen verlaging = volledige bijstand
    Given a citizen with the following data:
      | gemeente_code                          | GM9999       |
      | leeftijd                               | 35           |
      | is_nederlander                         | true         |
      | woont_in_nederland                     | true         |
      | is_alleenstaande                       | true         |
      | heeft_kostendelende_medebewoners       | false        |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false    |
      | is_geregistreerd_als_werkzoekende      | true         |
      | heeft_voldoende_middelen               | false        |
      | gedragscategorie                       | 1            |
    When the bijstandsaanvraag is executed for participatiewet article 43
    Then the citizen has the right to bijstand
    And the uitkering_bedrag is "109171" eurocent
