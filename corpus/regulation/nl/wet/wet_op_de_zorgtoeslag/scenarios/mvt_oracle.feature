Feature: Zorgtoeslag — de rekenvoorbeelden van de memorie van toelichting

  # Bron: Kamerstukken II 2003/04, 29 762, nr. 3 (memorie van toelichting bij de
  #       Wet op de zorgtoeslag), sectie "Voorbeeldberekening zorgtoeslag".
  #       De MvT rekent met eigen, gestileerde parameters (drempelinkomen 17 500,
  #       standaardpremie 1 000, toetsingsinkomen 22 000). De encoding in dit
  #       corpus neemt die parameters als constanten van het jaar, dus de
  #       huishoudens hieronder zijn omgerekend naar de parameters van 2025
  #       (drempelinkomen 39 719, standaardpremie 2 112) met dezelfde
  #       rekenwijze die de MvT toepast: normpremie = pct van het drempelinkomen
  #       plus 13,7% van het meerdere, toeslag = standaardpremie min normpremie.
  #       De verwachte waarden komen dus uit de rekenwijze van de wetgever en
  #       niet uit de encoding; dat onderscheid is de reden dat dit bestand
  #       naast eligibility.feature bestaat.

  Background:
    Given the calculation date is "2025-01-01"
    Given law "wet_basisregistratie_personen" is loaded
    Given law "zorgverzekeringswet" is loaded
    Given law "penitentiaire_beginselenwet" is loaded
    Given law "regeling_standaardpremie" is loaded
    Given law "algemene_wet_inkomensafhankelijke_regelingen" is loaded
    Given law "wet_inkomstenbelasting_2001" is loaded
    Given law "wet_forensische_zorg" is loaded

  # MvT-huishouden 1: verzekerde zonder partner, inkomen op het drempelinkomen.
  # Normpremie = 1,896% x 39 719 = 753,07; toeslag = 2 112 - 753,07 = 1 358,93.
  Scenario: MvT-huishouden 1, alleenstaande op het drempelinkomen
    Given the following "personal_data" data with key "bsn":
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 1980-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 3971900                   | 0                         | 0                     | 0                               | 0            | 0                   |
    Given the following "box2" data with key "bsn":
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "hoogte_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "hoogte_zorgtoeslag" equals 135893

  # MvT-huishouden 1 met inkomen boven de drempel: 22 000/17 500 in MvT-verhouding
  # geeft hier 49 934 eurocent-inkomen; normpremie = 753,07 + 13,7% x (49 934 - 39 719)
  # = 753,07 + 1 399,46 = 2 152,53, hoger dan de standaardpremie: geen toeslag.
  Scenario: MvT-huishouden 1 boven de drempel, normpremie overtreft standaardpremie
    Given the following "personal_data" data with key "bsn":
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 1980-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | buitenlands_inkomen |
      | 999993653 | 4993400                   | 0                         | 0                     | 0                               | 0            | 0                   |
    Given the following "box2" data with key "bsn":
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "hoogte_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "hoogte_zorgtoeslag" equals 0
