Feature: Participatiewet — de rekenvoorbeelden van de MvT bij de kostendelersnorm

  # Bron: Kamerstukken II 2015/16, 34 273, nr. 3 (memorie van toelichting bij de
  #       Wet harmonisatie kindregelingen / kostendelersnorm), tabel met
  #       bijstandsnormen naar aantal kostendelende medebewoners.
  #       De verwachte bedragen komen uit die tabel en niet uit de encoding.
  #       Deze scenario's zijn gedolven met de instructie van sectie 4 en
  #       daarna letterlijk overgenomen; de bedragen zijn omgerekend naar
  #       eurocent, de eenheid van de engine.

  Background:
    Given the calculation date is "2024-06-01"

  # MvT-geval 1: alleenstaande zonder kostendelende medebewoners, art. 20/21.
  # De MvT noemt EUR 925,20; de engine rekent EUR 1.091,71, de norm die in de
  # encoding staat (peildatum 2022-03-15). Het scenario FAALT daarom, en dat is
  # de bevinding: het bedrag in het rekenvoorbeeld is een jaarbedrag dat door
  # indexatie is ingehaald, precies de veroudering die sectie 8 meet. Wie het
  # voorbeeld als orakel gebruikt zonder de parameters van het jaar erbij te
  # halen, toetst niets.
  @wip
  Scenario: MvT-geval, alleenstaande zonder kostendelende medebewoners
    Given parameter "bsn" is "999993653"
    Given parameter "leeftijd" is 30
    Given parameter "is_alleenstaande" is "true"
    Given parameter "heeft_kostendelende_medebewoners" is "false"
    Given parameter "heeft_pensioengerechtigde_leeftijd_bereikt" is "false"
    When I evaluate "normbedrag_artikel_21" of "participatiewet"
    Then the execution succeeds
    Then output "normbedrag_artikel_21" equals 92520

  # MvT-geval 2: gehuwden met twee kostendelende medebewoners, art. 22a.
  # De MvT geeft EUR 1.108,56. Art. 22a is niet gemodelleerd, dus dit scenario
  # kan niet slagen; het legt vast welke bepaling het record wel exemplificeert
  # en de encoding niet dekt.
  @wip
  Scenario: MvT-geval, twee kostendelende medebewoners (art. 22a, niet gemodelleerd)
    Given parameter "bsn" is "999993653"
    Given parameter "leeftijd" is 30
    Given parameter "is_alleenstaande" is "false"
    Given parameter "heeft_kostendelende_medebewoners" is "true"
    Given parameter "heeft_pensioengerechtigde_leeftijd_bereikt" is "false"
    When I evaluate "normbedrag_artikel_21" of "participatiewet"
    Then the execution succeeds
    Then output "normbedrag_artikel_21" equals 110856
