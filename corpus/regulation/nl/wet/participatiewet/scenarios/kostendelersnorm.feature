Feature: Kostendelersnorm via artikel 22a Participatiewet
  Als uitvoerder van de bijstand
  Wil ik het aantal kostendelende medebewoners kunnen vaststellen
  Zodat de leeftijdsgrenzen uit de wet komen en niet uit de bronregistratie

  # Artikel 22a lid 1 rekent met A: "het aantal kostendelende medebewoners plus
  # de belanghebbende en zijn echtgenoot van 21 jaar of ouder indien hij gehuwd
  # is". De BRP levert de leeftijden; dit artikel bepaalt wat ermee gebeurt.
  #
  # Twee leeftijdsgrenzen, allebei van 21, op verschillende personen:
  #   - de aanhef stelt de belanghebbende zelf op 21 of ouder, als voorwaarde
  #     voor het hele artikel;
  #   - "van 21 jaar of ouder" in de opsomming hangt aan "zijn echtgenoot", het
  #     direct voorafgaande zinsdeel.
  # De medebewoners kennen geen leeftijdsgrens.
  #
  # De norm zelf is niet gemodelleerd: de formule waarin A en B worden verrekend
  # staat in de BWB-bron als afbeelding. Dat staat als untranslatable in het
  # artikel. Zie ook de boolean-invoer `heeft_kostendelende_medebewoners` op
  # artikel 21, die deze telling plat sloeg.

  Scenario: Een alleenstaande van 21 of ouder met drie medebewoners
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "leeftijd" is 35
    Given parameter "is_gehuwd" is "false"
    Given parameter "medebewoners" is the collection:
      | leeftijd |
      | 25       |
      | 19       |
      | 34       |
    # Alle drie tellen mee: de opsomming stelt geen leeftijdsgrens aan de
    # medebewoners.
    When I evaluate "aantal_kostendelende_medebewoners" of "participatiewet"
    Then output "aantal_kostendelende_medebewoners" equals 3
    When I evaluate "is_kostendelersnorm_van_toepassing" of "participatiewet"
    Then output "is_kostendelersnorm_van_toepassing" is true
    # A = drie medebewoners plus de belanghebbende zelf.
    When I evaluate "rekengetal_a" of "participatiewet"
    Then output "rekengetal_a" equals 4

  Scenario: Een belanghebbende jonger dan 21 valt buiten dit artikel
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "leeftijd" is 19
    Given parameter "is_gehuwd" is "false"
    Given parameter "medebewoners" is the collection:
      | leeftijd |
      | 25       |
      | 25       |
    # De medebewoners zijn er wel, maar de aanhef stelt de belanghebbende op 21
    # of ouder: "Indien de belanghebbende van 21 jaar of ouder een of meer
    # kostendelende medebewoners heeft". Zonder die voorwaarde zou dit artikel
    # nooit "niet van toepassing" kunnen zeggen.
    When I evaluate "aantal_kostendelende_medebewoners" of "participatiewet"
    Then output "aantal_kostendelende_medebewoners" equals 2
    When I evaluate "is_kostendelersnorm_van_toepassing" of "participatiewet"
    Then output "is_kostendelersnorm_van_toepassing" is false

  Scenario: Een echtgenoot van 21 of ouder telt mee in A
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "leeftijd" is 35
    Given parameter "is_gehuwd" is "true"
    Given parameter "leeftijd_echtgenoot" is 33
    Given parameter "medebewoners" is the collection:
      | leeftijd |
      | 25       |
    # Eén medebewoner, plus de belanghebbende, plus de echtgenoot.
    When I evaluate "rekengetal_a" of "participatiewet"
    Then output "rekengetal_a" equals 3

  Scenario: Een echtgenoot jonger dan 21 telt niet mee in A
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "leeftijd" is 35
    Given parameter "is_gehuwd" is "true"
    Given parameter "leeftijd_echtgenoot" is 19
    Given parameter "medebewoners" is the collection:
      | leeftijd |
      | 25       |
    # "zijn echtgenoot van 21 jaar of ouder": de grens hangt aan de echtgenoot.
    # Eén medebewoner plus de belanghebbende, en verder niets.
    When I evaluate "rekengetal_a" of "participatiewet"
    Then output "rekengetal_a" equals 2

  Scenario: Zonder medebewoners is de kostendelersnorm niet van toepassing
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "leeftijd" is 35
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

  Scenario: Medebewoners onder de 21 tellen gewoon mee
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "leeftijd" is 35
    Given parameter "is_gehuwd" is "false"
    Given parameter "medebewoners" is the collection:
      | leeftijd |
      | 18       |
      | 20       |
    When I evaluate "is_kostendelersnorm_van_toepassing" of "participatiewet"
    Then output "is_kostendelersnorm_van_toepassing" is true
    # Twee medebewoners plus de belanghebbende. Dit is het onderscheid dat een
    # enkele boolean niet kan uitdrukken.
    When I evaluate "rekengetal_a" of "participatiewet"
    Then output "rekengetal_a" equals 3
