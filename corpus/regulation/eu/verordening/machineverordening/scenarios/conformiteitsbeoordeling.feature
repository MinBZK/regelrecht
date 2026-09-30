Feature: Welke conformiteitsbeoordelingsprocedure geldt (artikel 25 en bijlage I)

  Artikel 25 kiest het lid aan de hand van de plaats van de categorie in
  bijlage I: deel A is lid 2, deel B is lid 3, en een categorie die niet in
  bijlage I staat valt onder lid 4.

  Bijlage I, deel A, draagt een markering: punt 6 legt de procedure alleen op
  voor de systemen met zelfontwikkelend gedrag, en het model deelt een product
  niet op. Zolang een mens die markering niet heeft geaccepteerd, weigert de
  engine het artikel in de standaardmodus. De scenario's over de indeling
  draaien daarom in de modus "warn", die de uitdrukbare logica uitvoert; de
  laatste twee laten zien wat de markering in de andere modi doet.

  Background:
    Given the calculation date is "2027-01-14"
    Given the untranslatable mode is "warn"

  Scenario: Hefbrug voor voertuigen valt onder bijlage I, deel A
    Given parameter "categorie_bijlage_i" is "A.3"
    When I evaluate "lid_2_van_toepassing" of "machineverordening"
    Then the execution succeeds
    Then output "lid_2_van_toepassing" is true

  Scenario: Veiligheidscomponent met machinaal leren valt onder bijlage I, deel A
    Given parameter "categorie_bijlage_i" is "A.5"
    When I evaluate "in_bijlage_i_deel_a" of "machineverordening"
    Then the execution succeeds
    Then output "in_bijlage_i_deel_a" is true

  Scenario: Cirkelzaag met vast zaagblad en vast tafelblad valt onder deel B, lid 3
    Given parameter "categorie_bijlage_i" is "B.1.1"
    When I evaluate "lid_3_van_toepassing" of "machineverordening"
    Then the execution succeeds
    Then output "lid_3_van_toepassing" is true

  Scenario: Punt 1 van deel B is zelf geen categorie, alleen de subpunten
    Given parameter "categorie_bijlage_i" is "B.1"
    When I evaluate "in_bijlage_i_deel_b" of "machineverordening"
    Then the execution succeeds
    Then output "in_bijlage_i_deel_b" is false

  Scenario: Een hefbrug valt niet onder lid 3
    Given parameter "categorie_bijlage_i" is "A.3"
    When I evaluate "lid_3_van_toepassing" of "machineverordening"
    Then the execution succeeds
    Then output "lid_3_van_toepassing" is false

  Scenario: Een categorie buiten bijlage I valt onder lid 4
    Given parameter "categorie_bijlage_i" is "geen"
    When I evaluate "lid_4_van_toepassing" of "machineverordening"
    Then the execution succeeds
    Then output "lid_4_van_toepassing" is true

  Scenario: Een categorie uit deel B valt niet onder lid 4
    Given parameter "categorie_bijlage_i" is "B.14"
    When I evaluate "lid_4_van_toepassing" of "machineverordening"
    Then the execution succeeds
    Then output "lid_4_van_toepassing" is false

  Scenario: Een tikfout in de categorie leidt niet tot interne productiecontrole
    Given parameter "categorie_bijlage_i" is "A3"
    When I evaluate "lid_4_van_toepassing" of "machineverordening"
    Then the execution succeeds
    Then output "lid_4_van_toepassing" is false

  Scenario: Een onbekende categorie geeft geen procedure
    Given the following parameters:
      | categorie_bijlage_i                                  | A3    |
      | vervaardigd_volgens_normen_die_alle_eisen_bestrijken | false |
    When I evaluate "toegestane_conformiteitsbeoordelingsprocedures" of "machineverordening"
    Then the execution succeeds
    Then output "toegestane_conformiteitsbeoordelingsprocedures" is absent

  Scenario: Zonder acceptatie van de markering weigert de engine deel A
    Given the untranslatable mode is "error"
    Given parameter "categorie_bijlage_i" is "A.3"
    When I evaluate "in_bijlage_i_deel_a" of "machineverordening"
    Then the execution fails with "Untranslatable construct"

  Scenario: De markering op deel A werkt door in de keuze van het lid
    Given the untranslatable mode is "propagate"
    Given parameter "categorie_bijlage_i" is "A.3"
    When I evaluate "lid_2_van_toepassing" of "machineverordening"
    Then the execution succeeds
    Then output "lid_2_van_toepassing" is tainted as untranslatable
