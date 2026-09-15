# Converted from overig/archiefwet_NATIONAAL_ARCHIEF-2024-06-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Archiefwet 1995 - Beheer en openbaarheid van archiefbescheiden
  Als archivaris
  Wil ik weten of archiefbescheiden overgebracht, openbaar of vernietigd moeten worden
  Zodat ik kan voldoen aan de verplichtingen uit de Archiefwet 1995

  Background:
    Given the calculation date is "2024-06-01"

  Scenario: Document van 21 jaar oud moet overgebracht worden
    Given the following parameters:
      | archiefstuk_id    | DOC-001    |
      | aanmaakdatum      | 2003-01-01 |
      | voor_vernietiging | false      |
    # POC: parameters "veelvuldig_gebruik", "opschortingsmachtiging" not provided by the scenario (None in the POC)
    And parameter "veelvuldig_gebruik" is "null"
    And parameter "opschortingsmachtiging" is "null"
    When I evaluate outputs "moet_overgebracht_worden, uiterste_overbrengdatum" of "archiefwet/overbrenging"
    Then output "moet_overgebracht_worden" is true
    And output "uiterste_overbrengdatum" equals "2033-01-01"

  Scenario: Document van 19 jaar oud hoeft nog niet overgebracht te worden
    Given the following parameters:
      | archiefstuk_id    | DOC-002    |
      | aanmaakdatum      | 2005-01-01 |
      | voor_vernietiging | false      |
    # POC: parameters "veelvuldig_gebruik", "opschortingsmachtiging" not provided by the scenario (None in the POC)
    And parameter "veelvuldig_gebruik" is "null"
    And parameter "opschortingsmachtiging" is "null"
    When I evaluate outputs "moet_overgebracht_worden" of "archiefwet/overbrenging"
    Then output "moet_overgebracht_worden" is false

  Scenario: Document voor vernietiging wordt niet overgebracht
    Given the following parameters:
      | archiefstuk_id    | DOC-003    |
      | aanmaakdatum      | 2000-01-01 |
      | voor_vernietiging | true       |
    # POC: parameters "veelvuldig_gebruik", "opschortingsmachtiging" not provided by the scenario (None in the POC)
    And parameter "veelvuldig_gebruik" is "null"
    And parameter "opschortingsmachtiging" is "null"
    When I evaluate outputs "moet_overgebracht_worden" of "archiefwet/overbrenging"
    Then output "moet_overgebracht_worden" is false

  Scenario: Overbrenging kan opgeschort worden bij veelvuldig gebruik met machtiging binnen de termijn
    Given the following parameters:
      | archiefstuk_id         | DOC-004    |
      | aanmaakdatum           | 2000-01-01 |
      | voor_vernietiging      | false      |
      | veelvuldig_gebruik     | true       |
      | opschortingsmachtiging | true       |
      | machtiging_datum       | 2020-01-01 |
    When I evaluate outputs "moet_overgebracht_worden" of "archiefwet/overbrenging"
    Then output "moet_overgebracht_worden" is false

  Scenario: Overbrenging kan niet opgeschort worden zonder machtiging
    Given the following parameters:
      | archiefstuk_id         | DOC-005    |
      | aanmaakdatum           | 2000-01-01 |
      | voor_vernietiging      | false      |
      | veelvuldig_gebruik     | true       |
      | opschortingsmachtiging | false      |
    # POC: parameter "machtiging_datum" not provided by the scenario (None in the POC)
    And parameter "machtiging_datum" is "null"
    When I evaluate outputs "moet_overgebracht_worden" of "archiefwet/overbrenging"
    Then output "moet_overgebracht_worden" is true

  Scenario: Overbrenging kan niet opgeschort worden als de machtiging ouder is dan tien jaar
    # Art. 13 lid 4: de machtiging geldt "voor een periode van ten hoogste tien jaar";
    # een machtiging van elf jaar oud is niet meer werkzaam.
    Given the following parameters:
      | archiefstuk_id         | DOC-006    |
      | aanmaakdatum           | 2000-01-01 |
      | voor_vernietiging      | false      |
      | veelvuldig_gebruik     | true       |
      | opschortingsmachtiging | true       |
      | machtiging_datum       | 2013-01-01 |
    When I evaluate outputs "moet_overgebracht_worden" of "archiefwet/overbrenging"
    Then output "moet_overgebracht_worden" is true

  Scenario: Overbrenging kan niet opgeschort worden als de machtiging geen verleningsdatum heeft
    # Art. 13 lid 4: zonder verleningsdatum is de tienjaarstermijn niet vast te stellen,
    # dus is de machtiging niet werkzaam (RFC-036 afwezigheidssemantiek).
    Given the following parameters:
      | archiefstuk_id         | DOC-007    |
      | aanmaakdatum           | 2000-01-01 |
      | voor_vernietiging      | false      |
      | veelvuldig_gebruik     | true       |
      | opschortingsmachtiging | true       |
    And parameter "machtiging_datum" is "null"
    When I evaluate outputs "moet_overgebracht_worden" of "archiefwet/overbrenging"
    Then output "moet_overgebracht_worden" is true

  Scenario: Overgebracht document zonder beperking is openbaar
    Given the following parameters:
      | archiefstuk_id | DOC-101    |
      | aanmaakdatum   | 2000-01-01 |
      | overbrengdatum | 2020-01-01 |
      | beperking_type | null       |
    # POC: parameters "beperking_termijn_jaren", "ministerraad_besluit_staatsbelang", "minister_of_gs_besluit_anders" not provided by the scenario (None in the POC)
    And parameter "beperking_termijn_jaren" is "null"
    And parameter "ministerraad_besluit_staatsbelang" is "null"
    And parameter "minister_of_gs_besluit_anders" is "null"
    When I evaluate outputs "is_openbaar" of "archiefwet/openbaarheid"
    Then output "is_openbaar" is true

  Scenario: Document met privacybeperking is niet openbaar binnen de termijn
    Given the following parameters:
      | archiefstuk_id          | DOC-102    |
      | aanmaakdatum            | 2000-01-01 |
      | overbrengdatum          | 2020-01-01 |
      | beperking_type          | PRIVACY    |
      | beperking_termijn_jaren | 75         |
    # POC: parameters "ministerraad_besluit_staatsbelang", "minister_of_gs_besluit_anders" not provided by the scenario (None in the POC)
    And parameter "ministerraad_besluit_staatsbelang" is "null"
    And parameter "minister_of_gs_besluit_anders" is "null"
    When I evaluate outputs "is_openbaar, beperking_reden, openbaar_vanaf_datum" of "archiefwet/openbaarheid"
    Then output "is_openbaar" is false
    And output "beperking_reden" equals "Beperkt vanwege eerbiediging van de persoonlijke levenssfeer"
    And output "openbaar_vanaf_datum" equals "2075-01-01"

  Scenario: Document met privacybeperking wordt openbaar na 75 jaar
    Given the calculation date is "2076-01-01"
    And the following parameters:
      | archiefstuk_id          | DOC-103    |
      | aanmaakdatum            | 2000-01-01 |
      | overbrengdatum          | 2020-01-01 |
      | beperking_type          | PRIVACY    |
      | beperking_termijn_jaren | 75         |
    # POC: parameters "ministerraad_besluit_staatsbelang", "minister_of_gs_besluit_anders" not provided by the scenario (None in the POC)
    And parameter "ministerraad_besluit_staatsbelang" is "null"
    And parameter "minister_of_gs_besluit_anders" is "null"
    When I evaluate outputs "is_openbaar" of "archiefwet/openbaarheid"
    Then output "is_openbaar" is true

  Scenario: Document met staatsbelang beperking en ministerraadsbesluit blijft beperkt na 75 jaar
    Given the calculation date is "2076-01-01"
    And the following parameters:
      | archiefstuk_id                    | DOC-104      |
      | aanmaakdatum                      | 2000-01-01   |
      | overbrengdatum                    | 2020-01-01   |
      | beperking_type                    | STAATSBELANG |
      | beperking_termijn_jaren           | 75           |
      | ministerraad_besluit_staatsbelang | true         |
    # POC: parameter "minister_of_gs_besluit_anders" not provided by the scenario (None in the POC)
    And parameter "minister_of_gs_besluit_anders" is "null"
    When I evaluate outputs "is_openbaar, beperking_reden" of "archiefwet/openbaarheid"
    Then output "is_openbaar" is false
    And output "beperking_reden" equals "Beperkt vanwege het belang van de Staat of zijn bondgenoten"

  Scenario: Document met staatsbelang beperking zonder ministerraadsbesluit wordt openbaar na 75 jaar
    Given the calculation date is "2076-01-01"
    And the following parameters:
      | archiefstuk_id                    | DOC-105      |
      | aanmaakdatum                      | 2000-01-01   |
      | overbrengdatum                    | 2020-01-01   |
      | beperking_type                    | STAATSBELANG |
      | beperking_termijn_jaren           | 75           |
      | ministerraad_besluit_staatsbelang | false        |
    # POC: parameter "minister_of_gs_besluit_anders" not provided by the scenario (None in the POC)
    And parameter "minister_of_gs_besluit_anders" is "null"
    When I evaluate outputs "is_openbaar" of "archiefwet/openbaarheid"
    Then output "is_openbaar" is true

  Scenario: Document met privacybeperking blijft beperkt na 75 jaar bij ministerbesluit
    Given the calculation date is "2076-01-01"
    And the following parameters:
      | archiefstuk_id                | DOC-107    |
      | aanmaakdatum                  | 2000-01-01 |
      | overbrengdatum                | 2020-01-01 |
      | beperking_type                | PRIVACY    |
      | beperking_termijn_jaren       | 75         |
      | minister_of_gs_besluit_anders | true       |
    # POC: parameter "ministerraad_besluit_staatsbelang" not provided by the scenario (None in the POC)
    And parameter "ministerraad_besluit_staatsbelang" is "null"
    When I evaluate outputs "is_openbaar, openbaar_vanaf_datum" of "archiefwet/openbaarheid"
    Then output "is_openbaar" is false
    And output "openbaar_vanaf_datum" equals "null"

  Scenario: Document met korte beperking wordt openbaar na termijn
    Given the calculation date is "2030-01-01"
    And the following parameters:
      | archiefstuk_id          | DOC-106               |
      | aanmaakdatum            | 2000-01-01            |
      | overbrengdatum          | 2020-01-01            |
      | beperking_type          | ONEVENREDIGE_GEVOLGEN |
      | beperking_termijn_jaren | 10                    |
    # POC: parameters "ministerraad_besluit_staatsbelang", "minister_of_gs_besluit_anders" not provided by the scenario (None in the POC)
    And parameter "ministerraad_besluit_staatsbelang" is "null"
    And parameter "minister_of_gs_besluit_anders" is "null"
    When I evaluate outputs "is_openbaar" of "archiefwet/openbaarheid"
    Then output "is_openbaar" is true

  Scenario: Document op selectielijst mag vernietigd worden na bewaartermijn
    Given the following parameters:
      | archiefstuk_id                | DOC-201    |
      | aanmaakdatum                  | 2015-01-01 |
      | documenttype                  | FACTUUR    |
      | op_selectielijst_vernietiging | true       |
      | bewaartermijn_jaren           | 7          |
      | selectielijst_vastgesteld     | true       |
      | selectielijst_gepubliceerd    | true       |
    When I evaluate outputs "mag_vernietigd_worden, vernietig_vanaf_datum" of "archiefwet/vernietiging"
    Then output "mag_vernietigd_worden" is true
    And output "vernietig_vanaf_datum" equals "2022-01-01"

  Scenario: Document op selectielijst mag nog niet vernietigd worden voor einde bewaartermijn
    Given the following parameters:
      | archiefstuk_id                | DOC-202    |
      | aanmaakdatum                  | 2020-01-01 |
      | documenttype                  | FACTUUR    |
      | op_selectielijst_vernietiging | true       |
      | bewaartermijn_jaren           | 7          |
      | selectielijst_vastgesteld     | true       |
      | selectielijst_gepubliceerd    | true       |
    When I evaluate outputs "mag_vernietigd_worden, reden_niet_vernietigen" of "archiefwet/vernietiging"
    Then output "mag_vernietigd_worden" is false
    And output "reden_niet_vernietigen" equals "Bewaartermijn is nog niet verstreken"

  Scenario: Document niet op selectielijst mag niet vernietigd worden
    Given the following parameters:
      | archiefstuk_id                | DOC-203         |
      | aanmaakdatum                  | 2000-01-01      |
      | documenttype                  | BELEIDSDOCUMENT |
      | op_selectielijst_vernietiging | false           |
      | selectielijst_vastgesteld     | true            |
      | selectielijst_gepubliceerd    | true            |
    # POC: parameter "bewaartermijn_jaren" not provided by the scenario (None in the POC)
    And parameter "bewaartermijn_jaren" is "null"
    When I evaluate outputs "mag_vernietigd_worden, reden_niet_vernietigen" of "archiefwet/vernietiging"
    Then output "mag_vernietigd_worden" is false
    And output "reden_niet_vernietigen" equals "Archiefstuk komt niet voor op een selectielijst voor vernietiging (mogelijk blijvend te bewaren)"

  Scenario: Document mag niet vernietigd worden als selectielijst niet is vastgesteld
    Given the following parameters:
      | archiefstuk_id                | DOC-204    |
      | aanmaakdatum                  | 2015-01-01 |
      | documenttype                  | FACTUUR    |
      | op_selectielijst_vernietiging | true       |
      | bewaartermijn_jaren           | 7          |
      | selectielijst_vastgesteld     | false      |
      | selectielijst_gepubliceerd    | true       |
    When I evaluate outputs "mag_vernietigd_worden, reden_niet_vernietigen" of "archiefwet/vernietiging"
    Then output "mag_vernietigd_worden" is false
    And output "reden_niet_vernietigen" equals "Selectielijst is niet formeel vastgesteld volgens artikel 5 lid 2 Archiefwet"

  Scenario: Document mag niet vernietigd worden als selectielijst niet is gepubliceerd
    Given the following parameters:
      | archiefstuk_id                | DOC-205    |
      | aanmaakdatum                  | 2015-01-01 |
      | documenttype                  | FACTUUR    |
      | op_selectielijst_vernietiging | true       |
      | bewaartermijn_jaren           | 7          |
      | selectielijst_vastgesteld     | true       |
      | selectielijst_gepubliceerd    | false      |
    When I evaluate outputs "mag_vernietigd_worden, reden_niet_vernietigen" of "archiefwet/vernietiging"
    Then output "mag_vernietigd_worden" is false
    And output "reden_niet_vernietigen" equals "Selectielijst is niet bekendgemaakt in de Staatscourant volgens artikel 5 lid 3 Archiefwet"

  Scenario: Document blijvend bewaren: niet voor vernietiging, moet overgebracht, wordt openbaar
    Given the following parameters:
      | archiefstuk_id                | DOC-301    |
      | aanmaakdatum                  | 2000-01-01 |
      | voor_vernietiging             | false      |
      | op_selectielijst_vernietiging | false      |
      | overbrengdatum                | 2020-01-01 |
      | beperking_type                | null       |
    # POC: parameters "veelvuldig_gebruik", "opschortingsmachtiging" not provided by the scenario (None in the POC)
    And parameter "veelvuldig_gebruik" is "null"
    And parameter "opschortingsmachtiging" is "null"
    When I evaluate outputs "moet_overgebracht_worden" of "archiefwet/overbrenging"
    Then output "moet_overgebracht_worden" is true
    # POC: parameters "beperking_termijn_jaren", "ministerraad_besluit_staatsbelang", "minister_of_gs_besluit_anders" not provided by the scenario (None in the POC)
    Given parameter "beperking_termijn_jaren" is "null"
    And parameter "ministerraad_besluit_staatsbelang" is "null"
    And parameter "minister_of_gs_besluit_anders" is "null"
    When I evaluate outputs "is_openbaar" of "archiefwet/openbaarheid"
    Then output "is_openbaar" is true
    # POC: parameters "documenttype", "bewaartermijn_jaren", "selectielijst_vastgesteld", "selectielijst_gepubliceerd" not provided by the scenario (None in the POC)
    Given parameter "documenttype" is "null"
    And parameter "bewaartermijn_jaren" is "null"
    And parameter "selectielijst_vastgesteld" is "null"
    And parameter "selectielijst_gepubliceerd" is "null"
    When I evaluate outputs "mag_vernietigd_worden" of "archiefwet/vernietiging"
    Then output "mag_vernietigd_worden" is false

  Scenario: Document tijdelijk bewaren: voor vernietiging na 10 jaar
    Given the calculation date is "2026-01-01"
    And the following parameters:
      | archiefstuk_id                | DOC-302         |
      | aanmaakdatum                  | 2015-01-01      |
      | documenttype                  | correspondentie |
      | voor_vernietiging             | true            |
      | op_selectielijst_vernietiging | true            |
      | bewaartermijn_jaren           | 10              |
      | selectielijst_vastgesteld     | true            |
      | selectielijst_gepubliceerd    | true            |
    # POC: parameters "veelvuldig_gebruik", "opschortingsmachtiging" not provided by the scenario (None in the POC)
    And parameter "veelvuldig_gebruik" is "null"
    And parameter "opschortingsmachtiging" is "null"
    When I evaluate outputs "moet_overgebracht_worden" of "archiefwet/overbrenging"
    Then output "moet_overgebracht_worden" is false
    When I evaluate outputs "mag_vernietigd_worden, vernietig_vanaf_datum" of "archiefwet/vernietiging"
    Then output "mag_vernietigd_worden" is true
    And output "vernietig_vanaf_datum" equals "2025-01-01"
