Feature: Zorguitgaven-tabellen uit de memorie van antwoord aan de Eerste Kamer

  # Bron: Kamerstukken I 2004/05, 29 762 e.a., D (memorie van antwoord,
  #       30 maart 2005), tabellen 2, 3 en 4: zorguitgaven van jongeren
  #       (18-23) met minimumjeugdloon en van 21-plussers op bijstandsniveau.
  #       De rij "Zorgtoeslag" is de enige die deze encoding berekent.
  #
  # Transcriptie-aannames, expliciet omdat de tabellen ze niet noemen:
  #   - standaardpremie 1 030: het document noemt elders zelf "de nominale
  #     premie EUR 966 [...] in plaats van EUR 1 030"; 1 030 = de 1 105 uit de
  #     tabelrij "nominale premie" minus de 75 no-claimteruggaaf.
  #   - drempelinkomen 17 500: het gestileerde drempelinkomen van ditzelfde
  #     dossier (MvT, kst-29762-3); de MvA noemt geen eigen waarde.
  #   - toetsingsinkomen: voor elk hieronder getranscribeerd huishouden ligt
  #     het (gezamenlijke) minimumjeugdloon of de bijstandsnorm onder het
  #     drempelinkomen, en daaronder is de uitkomst inkomens-ongevoelig
  #     (normpremie = percentage x drempelinkomen). Ingevuld is het wettelijke
  #     minimumjeugdloon per 1 januari 2005 x 12 (Stcrt.; 18 jr 575,50/mnd
  #     t/m 23 jr 1 264,80/mnd); elke waarde tot het drempelinkomen geeft
  #     dezelfde uitkomst.
  #
  # NIET getranscribeerd: tabel 3, leeftijden 19-23 (zorgtoeslag 886, 758,
  # 603, 428, 222). Daar ligt het gezamenlijke inkomen boven het
  # drempelinkomen en hangt de uitkomst af van een inkomensreeks die het
  # document nergens vermeldt; die kolommen zijn zonder aanname niet als
  # scenario te schrijven. Dat ontbrekende input is precies de
  # oppervlakkigheid die het paper in par. 8 meet.

  # --- Tabel 2: alleenstaand, minimumjeugdloon, zorgtoeslag 330 op elke leeftijd ---

  Scenario: Tabel 2, alleenstaande 18 jaar met minimumjeugdloon
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   |  103000 |
      | toetsingsinkomen  |  690600 |
      | heeft_partner     | false   |
      | partner_verzekerd | false   |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "normpremie" equals 70000
    Then output "hoogte_zorgtoeslag" equals 33000

  Scenario: Tabel 2, alleenstaande 19 jaar met minimumjeugdloon
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   |  103000 |
      | toetsingsinkomen  |  796800 |
      | heeft_partner     | false   |
      | partner_verzekerd | false   |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "hoogte_zorgtoeslag" equals 33000

  Scenario: Tabel 2, alleenstaande 20 jaar met minimumjeugdloon
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   |  103000 |
      | toetsingsinkomen  |  933420 |
      | heeft_partner     | false   |
      | partner_verzekerd | false   |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "hoogte_zorgtoeslag" equals 33000

  Scenario: Tabel 2, alleenstaande 21 jaar met minimumjeugdloon
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   |  103000 |
      | toetsingsinkomen  | 1100400 |
      | heeft_partner     | false   |
      | partner_verzekerd | false   |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "hoogte_zorgtoeslag" equals 33000

  Scenario: Tabel 2, alleenstaande 22 jaar met minimumjeugdloon
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   |  103000 |
      | toetsingsinkomen  | 1290120 |
      | heeft_partner     | false   |
      | partner_verzekerd | false   |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "hoogte_zorgtoeslag" equals 33000

  Scenario: Tabel 2, alleenstaande 23 jaar met minimumloon
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   |  103000 |
      | toetsingsinkomen  | 1517760 |
      | heeft_partner     | false   |
      | partner_verzekerd | false   |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "hoogte_zorgtoeslag" equals 33000

  # --- Tabel 3: samenwonend, 18 jaar (gezamenlijk jeugdloon onder de drempel) ---

  Scenario: Tabel 3, samenwonend paar van 18 jaar met minimumjeugdloon
    # Gezamenlijk toetsingsinkomen 2 x 6 906 = 13 812, onder het drempelinkomen.
    # Normpremie = 6,5% x 17 500 = 1 137,50, afgerond 1 138;
    # zorgtoeslag = 2 x 1 030 - 1 138 = 922 (tabelwaarde: 922).
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   |  103000 |
      | toetsingsinkomen  | 1381200 |
      | heeft_partner     | true    |
      | partner_verzekerd | true    |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "normpremie" equals 113800
    Then output "hoogte_zorgtoeslag" equals 92200

  # --- Tabel 4: uitkering op bijstandsniveau, vanaf 21 jaar ---

  Scenario: Tabel 4, alleenstaande 21-plusser op bijstandsniveau
    # De tabel noemt geen bedrag; de bijstandsnorm ligt onder het
    # drempelinkomen, waar de uitkomst inkomens-ongevoelig is.
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   |  103000 |
      | toetsingsinkomen  | 1000000 |
      | heeft_partner     | false   |
      | partner_verzekerd | false   |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "hoogte_zorgtoeslag" equals 33000

  Scenario: Tabel 4, samenwonend paar op bijstandsniveau
    Given the calculation date is "2006-01-01"
    Given the following parameters:
      | drempelinkomen    | 1750000 |
      | standaardpremie   |  103000 |
      | toetsingsinkomen  | 1400000 |
      | heeft_partner     | true    |
      | partner_verzekerd | true    |
    When I evaluate "hoogte_zorgtoeslag" of "test_zorgtoeslag_2006"
    Then the execution succeeds
    Then output "hoogte_zorgtoeslag" equals 92200
