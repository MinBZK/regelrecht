Feature: Kostendelersnorm via artikel 22a Participatiewet
  Als uitvoerder van de bijstand
  Wil ik het aantal kostendelende medebewoners kunnen vaststellen
  Zodat de leeftijdsgrens uit de wet komt en niet uit de bronregistratie

  # Artikel 22a lid 1 rekent met A: "het aantal kostendelende medebewoners plus
  # de belanghebbende en zijn echtgenoot van 21 jaar of ouder indien hij gehuwd
  # is". De BRP levert de medebewoners met hun leeftijden; dit artikel bepaalt
  # wie er meetelt.
  #
  # De norm zelf is niet gemodelleerd. De wettekst in het corpus breekt af voor
  # de formule en noemt geen artikelnummer voor B; dat staat als niet-aanvaarde
  # untranslatable in het artikel. Zie ook de bestaande boolean-invoer
  # `heeft_kostendelende_medebewoners` op artikel 21, die deze telling plat sloeg.

  Scenario: Een alleenstaande met drie medebewoners, van wie twee 21 of ouder
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "is_gehuwd" is "false"
    Given parameter "medebewoners" is the collection:
      | leeftijd |
      | 25       |
      | 19       |
      | 34       |
    # Alle drie zijn kostendelende medebewoners: lid 1 stelt voor "een of meer"
    # geen leeftijdsgrens.
    When I evaluate "aantal_kostendelende_medebewoners" of "participatiewet"
    Then output "aantal_kostendelende_medebewoners" equals 3
    When I evaluate "is_kostendelersnorm_van_toepassing" of "participatiewet"
    Then output "is_kostendelersnorm_van_toepassing" is true
    # A telt alleen de 21-plussers, plus de belanghebbende zelf: 2 + 1.
    When I evaluate "rekengetal_a" of "participatiewet"
    Then output "rekengetal_a" equals 3

  Scenario: Gehuwd telt de echtgenoot mee in A
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "is_gehuwd" is "true"
    Given parameter "medebewoners" is the collection:
      | leeftijd |
      | 25       |
    # 1 medebewoner van 21+, plus de belanghebbende, plus de echtgenoot.
    When I evaluate "rekengetal_a" of "participatiewet"
    Then output "rekengetal_a" equals 3

  Scenario: Zonder medebewoners is de kostendelersnorm niet van toepassing
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "is_gehuwd" is "false"
    Given parameter "medebewoners" is the collection:
      | leeftijd |
    When I evaluate "aantal_kostendelende_medebewoners" of "participatiewet"
    Then output "aantal_kostendelende_medebewoners" equals 0
    When I evaluate "is_kostendelersnorm_van_toepassing" of "participatiewet"
    Then output "is_kostendelersnorm_van_toepassing" is false
    # A blijft de belanghebbende zelf, ook zonder medebewoners.
    When I evaluate "rekengetal_a" of "participatiewet"
    Then output "rekengetal_a" equals 1

  Scenario: Uitsluitend medebewoners onder de 21 tellen wel mee, maar niet in A
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "is_gehuwd" is "false"
    Given parameter "medebewoners" is the collection:
      | leeftijd |
      | 18       |
      | 20       |
    # De kostendelersnorm is van toepassing, want er zijn medebewoners.
    When I evaluate "is_kostendelersnorm_van_toepassing" of "participatiewet"
    Then output "is_kostendelersnorm_van_toepassing" is true
    # Maar geen van beiden telt mee in A. Dit is het onderscheid dat een enkele
    # boolean niet kan uitdrukken.
    When I evaluate "rekengetal_a" of "participatiewet"
    Then output "rekengetal_a" equals 1
