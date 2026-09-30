Feature: Op welke producten de verordening van toepassing is (artikel 2 en artikel 3)

  Artikel 2, lid 1, noemt de producten waarop de verordening van toepassing
  is, met de begrippen van artikel 3. Lid 2 somt in de onderdelen a tot en met
  q op waarop zij niet van toepassing is. Artikel 25, lid 1, leest beide.

  Het uitgangspunt is een hefbrug voor voertuigen: een samenstel met een
  aandrijfsysteem en bewegende onderdelen, dat onder geen enkel onderdeel van
  lid 2 valt. Elk scenario zet één feit om en kijkt wat er verandert.

  De onderdelen f, g, h, i en p lezen het toepassingsgebied van een andere
  EU-regeling die nog niet in het corpus staat. Die binding wordt pas gelezen
  als de feiten van het onderdeel zelf vaststaan. Een scenario dat daar
  belandt, geeft de uitkomst mee onder de naam van de input, of laat zien dat
  de engine zonder die regeling weigert ("Law not found"). Dat laatste
  scenario wordt rood zodra Verordening (EU) 2018/858 in het corpus staat, en
  moet dan een scenario worden dat de uitkomst uit die verordening leest.
  Een product dat al onder een onderdeel zonder andere regeling valt, zoals
  een raceauto onder j, vraagt niet om die regeling.

  Lid 2 draagt twee markeringen en artikel 3 één. De scenario's draaien daarom
  in de modus "warn"; het laatste laat zien wat de markeringen in de
  standaardmodus doen.

  Background:
    Given the calculation date is "2027-01-14"
    Given the untranslatable mode is "warn"
    Given the following parameters:
      | samenstel_met_ten_minste_een_beweegbaar_onderdeel                                   | true  |
      | voorzien_van_of_bestemd_voor_aandrijfsysteem                                        | true  |
      | aandrijfsysteem_op_basis_van_rechtstreekse_spierkracht                              | false |
      | samengevoegd_voor_bepaalde_toepassing                                               | true  |
      | kan_niet_zelfstandig_bepaalde_toepassing_realiseren                                 | false |
      | verwijderbare_component_voor_krachtoverbrenging                                     | false |
      | bestemd_om_identieke_componenten_te_vervangen                                       | false |
      | specifiek_bestemd_voor_kermissen_of_amusementsparken                                | false |
      | speciaal_ontworpen_voor_gebruik_in_kerninstallatie                                  | false |
      | gebruikt_in_kerninstallatie                                                         | false |
      | is_wapen                                                                            | false |
      | is_middel_voor_vervoer_door_lucht_over_water_of_via_spoor                           | false |
      | is_luchtvaartproduct_onderdeel_of_apparatuur                                        | false |
      | is_motorvoertuig_of_aanhangwagen                                                    | false |
      | is_voertuigdeel_voor_motorvoertuig_of_aanhangwagen                                  | false |
      | is_twee_of_driewielig_voertuig_of_vierwieler                                        | false |
      | is_voertuigdeel_voor_twee_of_driewielig_voertuig_of_vierwieler                      | false |
      | is_landbouw_of_bosbouwtrekker                                                       | false |
      | is_voertuigdeel_voor_landbouw_of_bosbouwtrekker                                     | false |
      | is_motorvoertuig_uitsluitend_bestemd_voor_wedstrijden                               | false |
      | is_zeeschip_of_mobiele_offshore_eenheid                                             | false |
      | geinstalleerd_aan_boord_van_zeeschip_of_offshore_eenheid                            | false |
      | specifiek_ontworpen_en_gebouwd_voor_militaire_of_politiedoeleinden                  | false |
      | specifiek_ontworpen_en_gebouwd_voor_onderzoek_voor_tijdelijk_gebruik_in_laboratoria | false |
      | is_mijnlift                                                                         | false |
      | voor_verplaatsen_van_kunstenaars_tijdens_optreden                                   | false |
      | is_huishoudelijk_apparaat_voor_huishoudelijk_gebruik                                | false |
      | is_audio_of_videoapparatuur                                                         | false |
      | is_apparatuur_voor_informatietechnologie                                            | false |
      | is_gewone_kantoormachine                                                            | false |
      | is_schakelmaterieel_of_besturingsapparatuur                                         | false |
      | is_elektromotor                                                                     | false |
      | is_elektrisch_hoogspanningsproduct                                                  | false |

  Scenario: Een hefbrug is een machine (artikel 3, punt 1, onder a)
    When I evaluate "is_machine" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "is_machine" is true

  Scenario: Een samenstel dat op rechtstreekse spierkracht draait valt niet onder punt 1, onder a
    Given parameter "aandrijfsysteem_op_basis_van_rechtstreekse_spierkracht" is "true"
    When I evaluate "machine_onder_a" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "machine_onder_a" is false

  Scenario: Op een hefbrug is de verordening van toepassing
    When I evaluate "verordening_van_toepassing" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "verordening_van_toepassing" is true

  Scenario: Op een wapen is de verordening niet van toepassing
    Given parameter "is_wapen" is "true"
    When I evaluate "verordening_van_toepassing" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "verordening_van_toepassing" is false

  Scenario: Een uitgesloten wapen heet nog steeds een product binnen het toepassingsgebied
    Given parameter "is_wapen" is "true"
    When I evaluate "is_product_binnen_toepassingsgebied" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "is_product_binnen_toepassingsgebied" is true

  Scenario: Een wapen krijgt geen conformiteitsbeoordelingsprocedure
    Given parameter "is_wapen" is "true"
    Given the following parameters:
      | categorie_bijlage_i                                  | A.3   |
      | vervaardigd_volgens_normen_die_alle_eisen_bestrijken | false |
    When I evaluate "toegestane_conformiteitsbeoordelingsprocedures" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "toegestane_conformiteitsbeoordelingsprocedures" is absent

  Scenario: Voor een hefbrug rekent artikel 25 de procedures van lid 2 uit
    Given the following parameters:
      | categorie_bijlage_i                                  | A.3   |
      | vervaardigd_volgens_normen_die_alle_eisen_bestrijken | false |
    When I evaluate "toegestane_conformiteitsbeoordelingsprocedures" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "toegestane_conformiteitsbeoordelingsprocedures" equals '["B+C","H","G"]'

  Scenario: Een machine buiten bijlage I krijgt interne productiecontrole
    Given the following parameters:
      | categorie_bijlage_i                                  | geen  |
      | vervaardigd_volgens_normen_die_alle_eisen_bestrijken | false |
    When I evaluate "toegestane_conformiteitsbeoordelingsprocedures" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "toegestane_conformiteitsbeoordelingsprocedures" equals '["A"]'

  Scenario: Een cirkelzaag uit deel B volgens de normen mag ook interne productiecontrole kiezen
    Given the following parameters:
      | categorie_bijlage_i                                  | B.1.1 |
      | vervaardigd_volgens_normen_die_alle_eisen_bestrijken | true  |
    When I evaluate "toegestane_conformiteitsbeoordelingsprocedures" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "toegestane_conformiteitsbeoordelingsprocedures" equals '["A","B+C","H","G"]'

  Scenario: Een lintzaag uit deel B die niet volgens de normen is gebouwd, niet
    Given the following parameters:
      | categorie_bijlage_i                                  | B.4.1 |
      | vervaardigd_volgens_normen_die_alle_eisen_bestrijken | false |
    When I evaluate "toegestane_conformiteitsbeoordelingsprocedures" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "toegestane_conformiteitsbeoordelingsprocedures" equals '["B+C","H","G"]'

  Scenario: Een machine aan boord van een zeeschip valt onder onderdeel k
    Given parameter "geinstalleerd_aan_boord_van_zeeschip_of_offshore_eenheid" is "true"
    When I evaluate "uitgesloten_onderdeel_k" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "uitgesloten_onderdeel_k" is true

  Scenario: Een machine die op een motorvoertuig is gemonteerd valt niet onder onderdeel g
    Given the following parameters:
      | is_voertuigdeel_voor_motorvoertuig_of_aanhangwagen | true |
      | gemonteerd_op_motorvoertuig_of_aanhangwagen        | true |
    When I evaluate "uitgesloten_onderdeel_g" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "uitgesloten_onderdeel_g" is false

  Scenario: Een motorvoertuig binnen Verordening (EU) 2018/858 valt onder onderdeel g
    Given the following parameters:
      | is_motorvoertuig_of_aanhangwagen                   | true  |
      | gemonteerd_op_motorvoertuig_of_aanhangwagen        | false |
      | valt_binnen_toepassingsgebied_verordening_2018_858 | true  |
    When I evaluate "verordening_van_toepassing" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "verordening_van_toepassing" is false

  Scenario: Zonder Verordening (EU) 2018/858 weigert de engine een motorvoertuig
    Given the following parameters:
      | is_motorvoertuig_of_aanhangwagen            | true  |
      | gemonteerd_op_motorvoertuig_of_aanhangwagen | false |
    When I evaluate "uitgesloten_onderdeel_g" of "verordening_eu_2023_1230"
    Then the execution fails with "Law not found: verordening_eu_2018_858"

  Scenario: Een raceauto valt onder onderdeel j, zonder dat Verordening (EU) 2018/858 wordt gelezen
    Given the following parameters:
      | is_motorvoertuig_of_aanhangwagen                      | true  |
      | gemonteerd_op_motorvoertuig_of_aanhangwagen           | false |
      | is_motorvoertuig_uitsluitend_bestemd_voor_wedstrijden | true  |
    When I evaluate "verordening_van_toepassing" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "verordening_van_toepassing" is false

  Scenario: Een samenstel dat zijn toepassing niet zelfstandig kan realiseren, is geen machine onder a
    Given parameter "kan_niet_zelfstandig_bepaalde_toepassing_realiseren" is "true"
    When I evaluate "machine_onder_a" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "machine_onder_a" is false

  Scenario: Een aftakas tussen trekker en machine is een verwant product (onderdeel e, artikel 3, punt 9)
    Given the following parameters:
      | samenstel_met_ten_minste_een_beweegbaar_onderdeel                                         | false |
      | verwijderbare_component_voor_krachtoverbrenging                                           | true  |
      | tussen_machine_met_eigen_aandrijving_of_trekker_en_andere_machine_of_verwant_product     | true  |
      | verbindt_bij_eerste_vaste_aslager                                                         | true  |
    When I evaluate "is_verwant_product" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "is_verwant_product" is true

  Scenario: Een luchtvaartmachine die Verordening (EU) 2018/1139 maar gedeeltelijk dekt, blijft onder de verordening
    Given the following parameters:
      | is_luchtvaartproduct_onderdeel_of_apparatuur                   | true  |
      | verordening_2018_1139_heeft_betrekking_op_alle_relevante_eisen | false |
    When I evaluate "verordening_van_toepassing" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "verordening_van_toepassing" is true

  Scenario: Een kantoormachine binnen Richtlijn 2014/35/EU valt onder onderdeel p
    Given the following parameters:
      | is_gewone_kantoormachine                                 | true  |
      | is_additieve_drukmachine_voor_driedimensionale_producten | false |
      | valt_binnen_toepassingsgebied_richtlijn_2014_35          | true  |
    When I evaluate "uitgesloten_onderdeel_p" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "uitgesloten_onderdeel_p" is true

  Scenario: Een 3D-printer valt niet onder onderdeel p, zonder dat een richtlijn wordt gelezen
    Given the following parameters:
      | is_gewone_kantoormachine                                 | true |
      | is_additieve_drukmachine_voor_driedimensionale_producten | true |
    When I evaluate "uitgesloten_onderdeel_p" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "uitgesloten_onderdeel_p" is false

  Scenario: Een hoogspanningstransformator valt onder onderdeel q
    Given the following parameters:
      | is_elektrisch_hoogspanningsproduct | true |
      | is_transformator                   | true |
    When I evaluate "uitgesloten_onderdeel_q" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "uitgesloten_onderdeel_q" is true

  Scenario: Een niet voltooide machine voor militaire doeleinden valt niet onder onderdeel l
    Given the following parameters:
      | voorzien_van_of_bestemd_voor_aandrijfsysteem                                 | false |
      | samenstel_als_a_waaraan_slechts_montage_of_aansluitcomponenten_ontbreken     | false |
      | samenstel_van_machines_dat_als_een_geheel_functioneert                       | false |
      | in_samenhang_bestemd_voor_heffen_van_lasten                                  | false |
      | samenstel_als_a_tot_en_met_e_waarop_enkel_software_ontbreekt                 | false |
      | kan_niet_zelfstandig_bepaalde_toepassing_realiseren                          | true  |
      | slechts_bedoeld_om_te_worden_ingebouwd_om_machine_te_vormen                  | true  |
      | door_bediener_gekoppeld_aan_machine_of_trekker_voor_andere_of_nieuwe_functie | false |
      | component_van_product_binnen_toepassingsgebied                               | false |
      | niet_vast_verbonden_onderdeel_voor_hijsen_of_heffen_van_last                 | false |
      | is_strop_of_component_daarvan                                                | false |
      | is_ketting_als_voorwerp                                                      | false |
      | is_kabel_als_voorwerp                                                        | false |
      | is_band_als_voorwerp                                                         | false |
      | verwijderbare_component_voor_krachtoverbrenging                              | false |
      | specifiek_ontworpen_en_gebouwd_voor_militaire_of_politiedoeleinden           | true  |
    When I evaluate "verordening_van_toepassing" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "verordening_van_toepassing" is true

  Scenario: Een niet voltooide machine krijgt geen procedure uit artikel 25
    Given the following parameters:
      | voorzien_van_of_bestemd_voor_aandrijfsysteem                                 | false |
      | samenstel_als_a_waaraan_slechts_montage_of_aansluitcomponenten_ontbreken     | false |
      | samenstel_van_machines_dat_als_een_geheel_functioneert                       | false |
      | in_samenhang_bestemd_voor_heffen_van_lasten                                  | false |
      | samenstel_als_a_tot_en_met_e_waarop_enkel_software_ontbreekt                 | false |
      | kan_niet_zelfstandig_bepaalde_toepassing_realiseren                          | true  |
      | slechts_bedoeld_om_te_worden_ingebouwd_om_machine_te_vormen                  | true  |
      | door_bediener_gekoppeld_aan_machine_of_trekker_voor_andere_of_nieuwe_functie | false |
      | component_van_product_binnen_toepassingsgebied                               | false |
      | niet_vast_verbonden_onderdeel_voor_hijsen_of_heffen_van_last                 | false |
      | is_strop_of_component_daarvan                                                | false |
      | is_ketting_als_voorwerp                                                      | false |
      | is_kabel_als_voorwerp                                                        | false |
      | is_band_als_voorwerp                                                         | false |
      | verwijderbare_component_voor_krachtoverbrenging                              | false |
      | categorie_bijlage_i                                                          | geen  |
      | vervaardigd_volgens_normen_die_alle_eisen_bestrijken                         | false |
    When I evaluate "toegestane_conformiteitsbeoordelingsprocedures" of "verordening_eu_2023_1230"
    Then the execution succeeds
    Then output "toegestane_conformiteitsbeoordelingsprocedures" is absent

  Scenario: Zonder acceptatie van de markeringen weigert de engine lid 2
    Given the untranslatable mode is "error"
    When I evaluate "verordening_van_toepassing" of "verordening_eu_2023_1230"
    Then the execution fails with "Untranslatable construct"
