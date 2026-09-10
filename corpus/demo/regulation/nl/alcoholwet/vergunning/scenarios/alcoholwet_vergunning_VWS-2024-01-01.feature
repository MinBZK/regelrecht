Feature: Eis sociale hygiëne Alcoholwet artikel 8 lid 3 en 4
  Als leidinggevende van een horeca- of slijtersbedrijf
  Wil ik weten of ik voldoe aan de eis van sociale hygiëne
  Zodat duidelijk is of ik in aanmerking kom voor een Alcoholwetvergunning

  Background:
    Given the calculation date is "2024-06-01"

  Scenario: Ingeschreven in het Register sociale hygiëne (artikel 8 lid 3)
    Given the following parameters:
      | leeftijd_leidinggevende            | 30            |
      | is_onder_curatele                  | false         |
      | is_van_slecht_levensgedrag          | false         |
      | is_ingeschreven_svh_register        | true          |
      | heeft_bemoeienis_bedrijfsvoering    | null          |
      | schriftelijke_verklaring_bevestigd  | null          |
      | vloeroppervlakte                   | 50            |
      | type_bedrijf                       | horecabedrijf |
    When I evaluate outputs "voldoet_aan_svh_eis" of "alcoholwet/vergunning"
    Then output "voldoet_aan_svh_eis" is true

  Scenario: Niet ingeschreven, maar uitzondering lid 4 van toepassing
    Given the following parameters:
      | leeftijd_leidinggevende            | 30            |
      | is_onder_curatele                  | false         |
      | is_van_slecht_levensgedrag          | false         |
      | is_ingeschreven_svh_register        | false         |
      | heeft_bemoeienis_bedrijfsvoering    | false         |
      | schriftelijke_verklaring_bevestigd  | true          |
      | vloeroppervlakte                   | 50            |
      | type_bedrijf                       | horecabedrijf |
    When I evaluate outputs "voldoet_aan_svh_eis" of "alcoholwet/vergunning"
    # Art. 8 lid 4: exploitant voor eigen rekening en risico, geen bemoeienis
    # met bedrijfsvoering/exploitatie, schriftelijk bevestigd door de
    # vergunninghouder -> de inschrijvingseis van lid 3 vervalt.
    Then output "voldoet_aan_svh_eis" is true

  Scenario: Niet ingeschreven en wel bemoeienis met de bedrijfsvoering (uitzondering geldt niet)
    Given the following parameters:
      | leeftijd_leidinggevende            | 30            |
      | is_onder_curatele                  | false         |
      | is_van_slecht_levensgedrag          | false         |
      | is_ingeschreven_svh_register        | false         |
      | heeft_bemoeienis_bedrijfsvoering    | true          |
      | schriftelijke_verklaring_bevestigd  | true          |
      | vloeroppervlakte                   | 50            |
      | type_bedrijf                       | horecabedrijf |
    When I evaluate outputs "voldoet_aan_svh_eis" of "alcoholwet/vergunning"
    # Art. 8 lid 4: de uitzondering geldt alleen zonder bemoeienis met de
    # bedrijfsvoering. Is die bemoeienis er wel, dan blijft de
    # inschrijvingseis van lid 3 gelden en is niet aan voldaan.
    Then output "voldoet_aan_svh_eis" is false

  Scenario: Niet ingeschreven, geen schriftelijke verklaring (uitzondering geldt niet)
    Given the following parameters:
      | leeftijd_leidinggevende            | 30            |
      | is_onder_curatele                  | false         |
      | is_van_slecht_levensgedrag          | false         |
      | is_ingeschreven_svh_register        | false         |
      | heeft_bemoeienis_bedrijfsvoering    | false         |
      | schriftelijke_verklaring_bevestigd  | false         |
      | vloeroppervlakte                   | 50            |
      | type_bedrijf                       | horecabedrijf |
    When I evaluate outputs "voldoet_aan_svh_eis" of "alcoholwet/vergunning"
    # Art. 8 lid 4 vereist ook dat "de vergunninghouder dit in een
    # schriftelijke verklaring bevestigt". Zonder die bevestiging geldt de
    # uitzondering niet, dus blijft de inschrijvingseis van lid 3 gelden.
    Then output "voldoet_aan_svh_eis" is false

# Een vijfde tak (niet ingeschreven, lid 4-voorwaarden onbekend -> uitkomst
# onbekend) is in deze feature niet op te schrijven: dit is de wet zelf die
# alleen platte "parameter"-stappen kent, en de canonieke grammatica heeft
# geen stap om een losse parameter als onbekend te zetten (dat kan alleen via
# een ontbrekende celwaarde in een "the following ... data"-tabel, en dat
# bestaat hier niet omdat deze wet geen source: {} inputs heeft). De
# EQUALS-null-tak in de actie is wel expliciet als commentaar toegelicht, en
# is verder gedekt door hetzelfde patroon bij is_ingeschreven_svh_register en
# is_van_slecht_levensgedrag hoger in dit bestand.
