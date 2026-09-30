# Welke gegevens de berekening nodig heeft, en waar ze vandaan moeten komen

Alle invoer van de zeven gemodelleerde wetten van het Financieel CV, ingedeeld
naar herkomst. Afgeleid uit de `machine_readable`-blokken op de branch
`traject/financieel-cv-validatie-df48ddd1`, gemeten op 20 september 2026.

Hoort bij [`juristsessie-voorbereiding.md`](juristsessie-voorbereiding.md).

## De verdeling

| Herkomst | Aantal | Wat het betekent |
|---|---|---|
| Andere wet | 15 | De engine haalt het zelf op uit een andere wet |
| Uitvoerder | 78 | Een uitvoerder moet het vaststellen of geregistreerd hebben |
| Register | 11 | Het ligt vast in een basisregistratie |
| Werkgever | 10 | Het staat in het dienstverband of de loonaangifte |
| Burger of werkgever | 5 | Iemand moet iets aanvragen of verklaren |
| Wetsbedrag | 1 | Het staat in de wet of een AMvB |
| **Totaal** | **120** | |

En per concrete bron, want "uitvoerder" is te grof om iets mee te doen:

| Bron | Aantal |
|---|---|
| UWV | 52 |
| Gemeente of college | 18 |
| Werkgever of loonaangifte | 9 |
| Belastingdienst, uit de loonaangifte | 6 |
| Aanvraag of verzoek | 5 |
| BRP | 5 |
| Doelgroepregister (Wfsv 38b) | 3 |
| Justitiële registratie | 2 |
| SVB | 1 |
| AIVD-melding | 1 |
| Polisadministratie (UWV) | 1 |
| Werkgever of UWV | 1 |
| WML met AMvB-indexering | 1 |
| Andere wet in het corpus | 15 |

## Waarden die de engine zelf ophaalt (15)

Deze komen binnen via een `source`-aanroep naar een andere wet in het corpus. Ze zijn er dus al, en ze zijn de reden dat de zeven wetten samen een stelsel vormen in plaats van zeven losse rekenmachines.

| Gegeven | Type | Wetten | Bron | Waarvoor |
|---|---|---|---|---|
| `heeft_recht_op_lks` | boolean | ZW | Participatiewet | — |
| `verricht_arbeid_in_beschut_werk` | boolean | ZW | Participatiewet | — |
| `heeft_recht_op_arbeidsondersteuning_wajong` | boolean | WIA, Wajong, ZW | Wajong | Lid 1: de voorziening staat open voor "de jonggehandicapte". Doelgroepvaststelling loopt via ar… |
| `is_banenafspraak_doelgroep` | boolean | ZW | Wfsv | — |
| `is_doelgroep_banenafspraak` | boolean | Wtl | Wfsv | — |
| `is_arbeidsgehandicapte_werknemer` | boolean | Wtl | Wtl | — |
| `is_herplaatsen_arbeidsgehandicapte` | boolean | Wtl | Wtl | — |
| `arbeidsongeschiktheidspercentage` | number | ZW | Wet WIA | — |
| `heeft_recht_op_iva_uitkering` | boolean | ZW | Wet WIA | — |
| `heeft_recht_op_wga_uitkering` | boolean | ZW | Wet WIA | — |
| `is_gedeeltelijk_arbeidsgeschikt` | boolean | WIA | Wet WIA | — |
| `is_uitsluitingsgrond_van_toepassing` | boolean | WIA | Wet WIA | — |
| `is_volledig_en_duurzaam_arbeidsongeschikt` | boolean | WIA, Wajong | Wet WIA | Lid 3 onderdeel a: bij volledige en duurzame arbeidsongeschiktheid ontstaat het recht op de dag… |
| `wachttijd_doorlopen` | boolean | WIA | Wet WIA | — |
| `wachttijd_einddatum_wia` | date | ZW | Wet WIA | — |

## Oordeel of registratie van een uitvoerder (78)

Het grootste blok. Geen van deze gegevens kan een burger aanleveren: het zijn
beoordelingen (arbeidsdeskundig, medisch, loonwaarde) of administratieve
vaststellingen.

### De 78 uitgesplitst naar wat er moet gebeuren

Zonder die uitsplitsing is "een uitvoerder levert het" geen werkpakket. Deze
vijf groepen vragen elk iets anders, en ze verschillen sterk in hoe moeilijk ze
zijn.

| Groep | Aantal | Wat het is | Wat het vraagt |
|---|---|---|---|
| A | 26 | Een besluit dat al genomen is | Besluiten ontsluiten, of de producerende wet modelleren |
| B | 23 | Een registratiefeit | Gegevenslevering uit een bestaande administratie |
| C | 16 | Een professioneel oordeel | Blijft mensenwerk; vindplaats en aanname expliciet maken |
| D | 5 | Een open norm die lagere regelgeving invult | AMvB of verordening inwinnen en modelleren |
| E | 8 | Een procesfeit of een termijn | Uit het zaaksysteem of de aanvraag |

Twee van de vijf groepen, A en B samen 49 van de 78, zijn een kwestie van
ontsluiten: het antwoord bestaat al ergens. Groep C, 16 stuks, bestaat nog niet
en gaat ook niet bestaan zonder dat een mens kijkt. Dat onderscheid is het
belangrijkste dat deze lijst oplevert, want het scheidt een koppelvlakvraag van
een ontwerpvraag.


#### A. Een besluit dat al genomen is (26)

Een toekenning, indicatie of verklaring die ergens als beschikking ligt, met een datum en een grondslag. Niets hoeft opnieuw te worden beoordeeld; het moet ontsloten worden.

**Wat ervoor nodig is:** besluiten opvraagbaar maken per burger en peildatum, of de wet modelleren die het besluit produceert zodat de engine het zelf uitrekent. Voor de WIA- en Wajong-rechten is dat tweede al half gebeurd: ze bestaan als output, maar worden hier nog als parameter ingevoerd.

| Gegeven | Wetten | Wie levert | Waarvoor |
|---|---|---|---|
| `heeft_geldige_doelgroepverklaring_2_15` | Wtl | Belastingdienst, uit de loonaangifte | Lid 1 onderdeel b: geldige doelgroepverklaring als bedoeld in artikel 2.15, verstrekt aan… |
| `heeft_geldige_doelgroepverklaring_2_7` | Wtl | Belastingdienst, uit de loonaangifte | Lid 1 onderdeel c: de werknemer heeft een geldige doelgroepverklaring als bedoeld in artik… |
| `behoort_tot_doelgroep_lks` | Pwet, ZW | Gemeente of college | Persoon behoort tot de doelgroep loonkostensubsidie, bedoeld in artikel 6 lid 1 onderdeel… |
| `college_heeft_vastgesteld_uitsluitend_beschut_werk` | Pwet | Gemeente of college | Lid 1: het college heeft vastgesteld dat de persoon uitsluitend in een beschutte omgeving… |
| `is_uitgesloten_beschut_werk_pwet_10b` | Wfsv, Wtl, ZW | Gemeente of college | Chapeau-uitsluiting: door college vastgesteld dat persoon uitsluitend in beschutte omgevin… |
| `had_of_heeft_wajong_arbeidsongeschiktheidsuitkering` | ZW | UWV | Lid 2 onderdeel a: recht had of heeft gehad op een arbeidsongeschiktheidsuitkering op gron… |
| `had_wia_recht_maand_voor_aanvang` | Wtl | UWV | Lid 1 onderdeel a onder 1°: de werknemer had in de maand voorafgaand aan de aanvang van de… |
| `had_wsw_dienstbetrekking_of_indicatie_voorafgaand` | ZW | UWV | Lid 2 onderdeel d: had onmiddellijk voorafgaand aan de dienstbetrekking een Wsw-dienstbetr… |
| `heeft_of_krijgt_wia_recht_bij_hervatting` | Wtl | UWV | Lid 1 onderdeel a onder 1°: de werknemer heeft op dat moment recht op een WIA-uitkering, o… |
| `heeft_recht_op_arbeidsondersteuning` | Wajong | UWV | Lid 1: de jonggehandicapte heeft recht op arbeidsondersteuning op grond van deze wet. Bron… |
| `heeft_recht_op_uitkering_hoofdstuk_6_of_7` | WIA, ZW | UWV | Onderdeel a onder 1°: er bestaat al recht op een IVA-uitkering (hoofdstuk 6) of WGA-uitker… |
| `heeft_recht_op_wia_uitkering` | WIA | UWV | Lid 1: de gedeeltelijk arbeidsgeschikte heeft recht op een uitkering op grond van deze wet… |
| `heeft_recht_op_ww_uitkering` | WW | UWV | Lid 1: werknemer heeft recht op een uitkering op grond van hoofdstuk II van de Werklooshei… |
| `heeft_wajong_arbeidsondersteuning_of_uitkering` | Wfsv, Wtl, ZW | UWV | 38b.1.c: recht op arbeidsondersteuning of arbeidsongeschiktheids- uitkering Wajong. Uitslu… |
| `heeft_wajong_duurzaam_geen_mogelijkheden` | Wfsv, Wtl, ZW | UWV | 38b.1.c: vastgesteld op grond van Wajong 1a:1 lid 1, 2:4 lid 1 of 3:8a lid 1 dat persoon d… |
| `is_jonggehandicapt_uwv_oordeel_lid_2` | Wfsv, Wtl, ZW | UWV | 38b.2: UWV oordeelt dat persoon wegens ziekte/gebrek ontstaan voor 18e levensjaar of als s… |
| `is_jonggehandicapte_hoofdstuk_2` | Wajong | UWV | Jonggehandicapte in de zin van artikel 2:3: op de dag van zeventien worden (of daarna, na… |
| `is_jonggehandicapte_hoofdstuk_2_wajong` | ZW | UWV | Jonggehandicapte in de zin van Wajong 2:3. Bron: UWV-beoordeling. |
| `is_uitsluitingsgrond_2_11_van_toepassing` | Wajong | UWV | Lid 1 onderdeel b: een uitsluitingsgrond van artikel 2:11 (o.a. detentie, niet in Nederlan… |
| `is_uitsluitingsgrond_2_11_wajong_van_toepassing` | ZW | UWV | Wajong 2:15 lid 1 onderdeel b (artikel 2:11). Bron: UWV. |
| `is_volledig_en_duurzaam_arbeidsongeschikt_wajong` | ZW | UWV | Wajong 2:15 lid 3 onderdeel a. Bron: UWV-beoordeling. |
| `is_wia_uitstromer_artikel_34a_35_36` | Pwet | UWV | Lid 1, tweede categorie: persoon als bedoeld in Wet WIA artikel 34a lid 5 b, 35 lid 4 b of… |
| `is_wsw_geindiceerd_of_oude_indicatie` | Wfsv, Wtl, ZW | UWV | 38b.1.b: geïndiceerd in zin van Wet sociale werkvoorziening, of nog geldende indicatiebesc… |
| `recht_herleeft_op_grond_van_2_17` | Wajong | UWV | Lid 3 onderdeel b en lid 5: het recht herleeft op grond van artikel 2:17 (na eerdere beëin… |
| `recht_op_arbeidsondersteuning_herleeft_2_17` | ZW | UWV | Wajong 2:15 lid 3 onderdeel b en lid 5 (artikel 2:17). Bron: UWV. |
| `was_rea_arbeidsgehandicapte_voor_2006` | Wtl | UWV | Lid 1 onderdeel a onder 2°: de werknemer zou arbeidsgehandicapte zijn geweest in de zin va… |

#### B. Een registratiefeit (23)

Een feit dat in een administratie staat zonder dat er een oordeel aan te pas komt: verloonde uren, een lopende registratie, een uitkering die iemand ontvangt, de eerste ziektedag.

**Wat ervoor nodig is:** gegevenslevering uit de polisadministratie, het doelgroepregister en de gemeentelijke uitkeringsadministratie, met de grondslag daarvoor in de Wet SUWI. Dit is het blok waar een koppeling het werk doet.

| Gegeven | Wetten | Wie levert | Waarvoor |
|---|---|---|---|
| `verloonde_uren` | Wtl | Belastingdienst, uit de loonaangifte | Verloonde uren in kalenderjaar (loonaangifte). |
| `was_in_dienst_bij_werkgever_binnen_zes_maanden` | Wtl | Belastingdienst, uit de loonaangifte | Lid 1 onderdeel b: de werknemer is in de zes maanden voor de indiensttreding op enig momen… |
| `heeft_dienstbetrekking_beschut_werk` | Pwet, ZW | Gemeente of college | Lid 1, slot: de persoon verricht werkzaamheden in een dienstbetrekking in een beschutte om… |
| `is_niet_uitkeringsgerechtigde` | Pwet | Gemeente of college | Lid 1, vierde categorie: niet-uitkeringsgerechtigde. |
| `is_pwet_lks_toegeleid_met_uwv_loonwaarde_vaststelling` | Wfsv, Wtl, ZW | Gemeente of college | 38b.1.a: persoon is met collegale ondersteuning (Pwet 7 lid 1.a) toegeleid naar dienstbetr… |
| `is_wsw_of_beschut_werk_dienstbetrekking` | Wtl | Gemeente of college | Lid 3 onderdeel b: het verzoek betreft een dienstbetrekking als bedoeld in artikel 2 Wet s… |
| `is_wsw_werknemer` | WIA, Wajong, ZW | Gemeente of college | Lid 2 onderdeel b: heeft een arbeidsovereenkomst gesloten met een werkgever als bedoeld in… |
| `ontvangt_algemene_bijstand` | Pwet | Gemeente of college | Lid 2 onderdeel d stelt de proefplaatsing uitsluitend open voor wie algemene bijstand ontv… |
| `is_verzekerde_wia` | WIA, ZW | Polisadministratie (UWV) | Verzekerde in de zin van artikel 7 tot en met 9 (werknemer in de zin van de Ziektewet, dan… |
| `heeft_nabestaandenuitkering_anw` | Pwet | SVB | Lid 1, derde categorie: nabestaandenuitkering Anw. |
| `arbeidskundig_onderzoek_verricht` | ZW | UWV | Lid 1 onderdeel b, aanhef: in een arbeidskundig onderzoek is de mate van arbeidsongeschikt… |
| `dienstbetrekking_aangevangen_voor_achttien_en_voor_wajong_recht` | ZW | UWV | Lid 2 onderdeel c: de dienstbetrekking is aangevangen voordat de werknemer achttien werd e… |
| `dienstbetrekking_voortgezet_na_vaststelling_wia_recht` | ZW | UWV | Lid 4: de dienstbetrekking bij de werkgever is voortgezet nadat het recht op WIA-uitkering… |
| `eerste_dag_wachttijd` | WIA | UWV | Lid 2: de eerste werkdag waarop wegens ziekte niet is gewerkt of het werken is gestaakt. A… |
| `eerste_dag_wachttijd_wia` | ZW | UWV | Eerste dag van de WIA-wachttijd (artikel 23 lid 2 Wet WIA). Alleen vereist als de werkneme… |
| `geen_dienstbetrekking_elders_elf_weken_voor_einde_wachttijd` | ZW | UWV | Lid 1 onderdeel b onder 2°: op de eerste dag van elf weken voorafgaand aan het einde van d… |
| `is_pwet_toegeleid_met_uwv_wml_vaststelling_eigen_verzoek` | Wfsv, Wtl, ZW | UWV | 38b.1.e: persoon is met collegale ondersteuning (Pwet 7 lid 1.a) toegeleid naar dienstbetr… |
| `is_wsw_dienstbetrekking` | Pwet, ZW | UWV | Artikel 10d lid 3: uitsluitingsgrond — dienstbetrekking op grond van artikelen 2 of 7 Wsw. |
| `is_ziek_geworden` | WIA | UWV | De verzekerde is wegens ziekte uitgevallen (artikel 47 en 54: "de verzekerde die ziek word… |
| `is_ziek_geworden_wia` | ZW | UWV | De werknemer is wegens ziekte uitgevallen zodat een WIA-wachttijd is gaan lopen. Bron: UWV… |
| `ontvangt_wazo_uitkering_wegens_ziekte_door_zwangerschap` | WIA, ZW | UWV | Onderdeel a onder 2°: uitkering op grond van hoofdstuk 3, afdeling 2, paragraaf 1 Wet arbe… |
| `registratie_nog_niet_geeindigd` | Wfsv, Wtl, ZW | UWV | 38b.6 blijfgrond: de opname van de persoon in de registratie van arbeidsbeperkten (artikel… |
| `was_lid_1_c_en_nu_wajong_duurzaam_geen_mogelijkheden` | Wfsv, Wtl, ZW | UWV | 38b.1.f uitsluiting: persoon valt onder onderdeel c-historie (Wajong) maar heeft inmiddels… |

#### C. Een professioneel oordeel (16)

Medisch, arbeidskundig of bestuurlijk. Of een beperking structureel functioneel is, of er reëel uitzicht op werk bestaat, wat de loonwaarde is, of het college een voorziening noodzakelijk acht.

**Wat ervoor nodig is:** niets van dit blok wordt ooit automatisch. Twee dingen kunnen wel. Het oordeel dat al geveld is, ligt vast in een rapport of beschikking en valt dan onder A. Voor het oordeel dat nog niet geveld is, moet de regelhulp eerlijk tonen dat hier een aanname zit, en moet de vindplaats bekend zijn: welke beleidsregel of welk protocol stuurt het. Het corpus bevat vandaag geen enkele UWV-beleidsregel.

| Gegeven | Wetten | Wie levert | Waarvoor |
|---|---|---|---|
| `college_acht_voorziening_noodzakelijk` | Pwet | Gemeente of college | Lid 1: de voorziening moet naar het oordeel van het college noodzakelijk zijn. Discretiona… |
| `kan_minimumloon_niet_verdienen` | Pwet, ZW | Gemeente of college | Artikel 10d lid 2: college heeft vastgesteld dat persoon met voltijdse arbeid niet in staa… |
| `kan_taken_niet_verrichten_zonder_ondersteuning` | Pwet | Gemeente of college | Lid 1, slotzin: persoonlijke ondersteuning bestaat alleen indien de persoon zonder die ond… |
| `loonwaarde_eurocent_per_maand` | Pwet, ZW | Gemeente of college | Vastgestelde loonwaarde van de persoon in eurocent per maand. Voor lid 4 gebruikt om subsi… |
| `valt_onder_lid_2_wegens_voorziening` | Pwet | Gemeente of college | Lid 2: persoon behoort door een voorziening gericht op arbeidsinschakeling niet meer tot e… |
| `arbeidsprestatie_duidelijk_minder_dan_minimumloon` | Wajong | UWV | Lid 1: UWV heeft vastgesteld dat de arbeidsprestatie van werknemer in deze functie wegens… |
| `heeft_belemmering_bij_onderwijs_door_ziekte_of_gebrek` | ZW | UWV | Lid 1 onderdeel c en d: ondervindt of ondervond in verband met ziekte of gebrek een belemm… |
| `heeft_structurele_functionele_beperking` | WIA | UWV | Lid 1: UWV oordeelt dat persoon een structurele functionele beperking heeft. Discretionair… |
| `in_staat_tot_werkzaamheden` | WIA, WW, Wajong | UWV | Lid 2 onderdeel a: werkzaamheden waartoe de gedeeltelijk arbeidsgeschikte met zijn krachte… |
| `is_medisch_duurzaam` | WIA | UWV | Lid 2 en 3: de medische situatie is stabiel of verslechterend, dan wel kent op lange termi… |
| `is_medisch_duurzaam_wia` | ZW | UWV | Duurzaamheid (artikel 4 lid 2 en 3 Wet WIA). Bron: verzekeringsarts UWV. |
| `niet_in_staat_meer_dan_75_procent_maatmaninkomen` | Wajong, ZW | UWV | Lid 1 onderdeel a: sinds de dag waarop hij jonggehandicapte werd niet in staat gebleven me… |
| `niet_in_staat_tot_eigen_of_passende_arbeid_bij_eigen_werkgever` | ZW | UWV | Lid 1 onderdeel b onder 3°: niet in staat tot het verrichten van eigen of andere passende… |
| `reeel_uitzicht_op_dienstbetrekking_zes_maanden` | WIA, WW, Wajong | UWV | Lid 2 onderdeel d: naar het oordeel van het UWV of de eigenrisicodrager is er reeel uitzic… |
| `valt_onder_uitzondering_lid_2` | Wtl | UWV | Lid 2: de werknemer gaat binnen vijf jaar na afloop van de wachttijd werken, is volgens ar… |
| `verdiencapaciteit_percentage_maatmaninkomen` | WIA, ZW | UWV | Het percentage van het maatmaninkomen per uur dat de verzekerde met arbeid nog kan verdien… |

#### D. Een open norm die lagere regelgeving invult (5)

De wet delegeert en de invulling staat in een AMvB of een gemeentelijke verordening.

**Wat ervoor nodig is:** die regelgeving inwinnen en modelleren. Dan verschuift het gegeven vanzelf naar A of B. Voor de verordening betekent het bovendien een keuze welke gemeente, want het antwoord verschilt per gemeente.

| Gegeven | Wetten | Wie levert | Waarvoor |
|---|---|---|---|
| `behoort_tot_doelgroep_10b_lid_1` | Pwet, ZW | Gemeente of college | Lid 1: persoon als bedoeld in artikel 7 lid 1 onderdeel a of 7a lid 1 onderdeel a, dan wel… |
| `behoort_tot_doelgroep_artikel_7_lid_1_a` | Pwet | Gemeente of college | Lid 2 onderdeel d: persoon als bedoeld in artikel 7 lid 1 onderdeel a — de groep waarvoor… |
| `college_verleent_toestemming_proefplaatsing` | Pwet | Gemeente of college | Lid 2 onderdeel d: het college verleent toestemming onder de voorwaarden die de gemeentera… |
| `pwet_college_draagt_zorg_uitsluiting` | WIA | Gemeente of college | Lid 4 onderdeel b: indien het college van b&w op grond van Pwet artikel 7 lid 1 a zorg dra… |
| `voldoet_aan_amvb_indicatie_38b_1_d` | Wfsv, Wtl, ZW | UWV | 38b.1.d: voldoet aan bij of krachtens AMvB vastgestelde indicatie. Bron: UWV (uitvoering A… |

#### E. Een procesfeit of een termijn (8)

Is er een aanvraag, wanneer is die gedaan, is een periode verstreken, over welk kalenderjaar gaat het.

**Wat ervoor nodig is:** uit het zaaksysteem van de uitvoerder, of uit de aanvraag zelf. Klein blok, maar het bepaalt wel of er überhaupt recht bestaat: zonder aanvraag geen arbeidsondersteuning.

| Gegeven | Wetten | Wie levert | Waarvoor |
|---|---|---|---|
| `heeft_loonaangifte_verzoek_ingediend` | Wtl | Belastingdienst, uit de loonaangifte | Art. 2.1: werkgever heeft in de loonaangifte verzoek gedaan voor het loonkostenvoordeel. |
| `periode_2_16_is_verstreken` | Wtl | Belastingdienst, uit de loonaangifte | Lid 2 onderdeel c: de periode van artikel 2.16 is voor deze werknemer verstreken. Lid 4 te… |
| `aanvraag_arbeidsondersteuning_ingediend` | Wajong | UWV | Lid 1, aanhef: het recht bestaat "op aanvraag". Zonder aanvraag is er geen recht en geen i… |
| `aanvraag_arbeidsondersteuning_wajong_ingediend` | ZW | UWV | Wajong 2:15 lid 1, aanhef: arbeidsondersteuning is aangevraagd. Bron: UWV. |
| `datum_aanvraag_arbeidsondersteuning` | Wajong | UWV | Datum waarop de aanvraag om arbeidsondersteuning is ingediend. Lid 1 onderdeel d toetst de… |
| `datum_aanvraag_arbeidsondersteuning_wajong` | ZW | UWV | Datum van de aanvraag om arbeidsondersteuning (Wajong 2:15 lid 1 onderdeel d en lid 2). Al… |
| `kalenderjaar_quotumtekort` | Wfsv | UWV | Kalenderjaar t waarover het quotumtekort (38g) wordt bepaald. De vaststelling vindt plaats… |
| `periode_zonder_ziekengeld_29_lid_11_loopt_nog` | WIA, ZW | UWV | Onderdeel c: de periode waarin op grond van Ziektewet 29 lid 11 geen ziekengeld wordt uitg… |

## Uit een basisregistratie (11)

Feiten die al ergens vastliggen. Voor een echte regelhulp is dit het laaghangend fruit: opvragen in plaats van uitvragen.

| Gegeven | Type | Wetten | Bron | Waarvoor |
|---|---|---|---|---|
| `is_uitreiziger` | boolean | WIA, ZW | AIVD-melding | Onderdeel i: is een uitreiziger. Bron: UWV / AIVD-melding. |
| `geboortedatum` | date | Wajong, ZW | BRP | Lid 1 onderdeel c en d: jonger dan achttien, respectievelijk achttien of ouder. Bron: BRP. |
| `is_overleden` | boolean | WIA, ZW | BRP | Onderdeel h: overlijden van de verzekerde. Bron: BRP. |
| `woont_niet_in_nederland` | boolean | WIA, ZW | BRP | Onderdeel f: woont niet in Nederland. Bron: BRP. |
| `heeft_pensioengerechtigde_leeftijd_bereikt` | boolean | WIA, Wtl, ZW | BRP of AOW-leeftijd | Uitsluitingsgrond voor alle drie categorieën (art. 2.6 lid 3.a, 2.10 lid 2.a, 2.14 lid 2.a): we… |
| `bsn` | string | Pwet, WIA, WW, Wajong, Wfsv, Wtl, ZW | BRP, via inloggen | BSN van de persoon waarvan doelgroep-status wordt bepaald. |
| `datum_eerste_opname_doelgroepregister` | date | Wfsv, Wtl, ZW | Doelgroepregister (Wfsv 38b) | Datum van eerste opname in doelgroepregister banenafspraak door UWV (datasource UWV). Wordt geb… |
| `was_arbeidsbeperkte_lid_1_b_of_c_op_of_na_2013_01_01` | boolean | Wfsv, Wtl, ZW | Doelgroepregister (Wfsv 38b) | 38b.1.f overgangsrecht: persoon was op of na 1-1-2013 een persoon als bedoeld in lid 1 onderdee… |
| `was_arbeidsbeperkte_lid_1_of_2` | boolean | Wfsv, Wtl, ZW | Doelgroepregister (Wfsv 38b) | 38b.6 blijfgrond: persoon voldeed eerder aan een grond van het eerste of tweede lid (was opgeno… |
| `is_rechtens_vrijheid_ontnomen` | boolean | WIA, ZW | Justitiële registratie | Onderdeel d: rechtens zijn vrijheid ontnomen. Bron: Justid/UWV. |
| `onttrekt_zich_aan_vrijheidsstraf` | boolean | WIA, ZW | Justitiële registratie | Onderdeel e: onttrekt zich aan de tenuitvoerlegging van een vrijheidsstraf of vrijheidsbenemend… |

## Van de werkgever of uit de loonaangifte (10)

Gegevens over het dienstverband. In de praktijk staan ze in de loonaangifte en daarmee in de polisadministratie, dus ook hier geldt: opvragen in plaats van uitvragen.

| Gegeven | Type | Wetten | Bron | Waarvoor |
|---|---|---|---|---|
| `loondoorbetalings_of_ziekengeldtijdvak_loopt_nog` | boolean | WIA, ZW | Werkgever of UWV | Onderdeel b: het tijdvak van loondoorbetaling (BW 7:629 lid 11), bezoldiging (Ziektewet 76a lid… |
| `aansprakelijkheidsverzekering_aanwezig` | boolean | WIA, WW, Wajong | Werkgever of loonaangifte | Lid 2 onderdeel b: de werkgever bij wie de proefplaatsing geschiedt heeft een aansprakelijkheid… |
| `datum_aanvang_dienstbetrekking` | date | ZW | Werkgever of loonaangifte | Aanvang van de dienstbetrekking (artikel 3, 4 of 5) waarvoor de no-riskpolis wordt getoetst. Ge… |
| `datum_afronding_onderwijs` | date | ZW | Werkgever of loonaangifte | Lid 1 onderdeel c en d: datum waarop het onderwijs is afgerond; de dienstbetrekking moet binnen… |
| `heeft_arbeidsverhouding_of_voorbereiding` | boolean | WIA, Wajong | Werkgever of loonaangifte | Lid 1: verricht arbeid in dienstbetrekking of gaat die verrichten, volgt scholing of opleiding… |
| `hervat_eigen_arbeid_of_andere_functie` | boolean | Wtl | Werkgever of loonaangifte | Lid 1 onderdeel a, aanhef: de werknemer hervat zijn eigen arbeid geheel of gedeeltelijk, of gaa… |
| `niet_eerder_proefplaatsing_zelfde_werkgever` | boolean | WIA, WW, Wajong | Werkgever of loonaangifte | Lid 3 onderdeel c: de jonggehandicapte heeft de werkzaamheden niet reeds eerder onbeloond op ee… |
| `overeengekomen_arbeidsduur_uren_per_week` | number | Pwet, ZW | Werkgever of loonaangifte | Lid 4, tweede zin: de overeengekomen arbeidsduur per week. De subsidie wordt naar evenredigheid… |
| `verricht_arbeid_in_dienstbetrekking` | boolean | Wfsv, Wtl, ZW | Werkgever of loonaangifte | 38b.1.c slotzin: wie duurzaam geen mogelijkheden tot arbeidsparticipatie heeft wordt "slechts a… |
| `voorafgaand_relevante_onderwijsroute_of_doelgroep` | boolean | Pwet, ZW | Werkgever of loonaangifte | Artikel 10d lid 2, onderdelen a, b en c — één boolean over de drie routes, omdat lid 2 ze als a… |

## Een aanvraag of een wilsuiting (5)

Het enige blok dat echt van de burger of de werkgever moet komen, want het is geen feit maar een handeling.

| Gegeven | Type | Wetten | Bron | Waarvoor |
|---|---|---|---|---|
| `aanvraag_ingediend` | boolean | Pwet | Aanvraag of verzoek | Lid 5: de persoon of de werkgever heeft bij het college een aanvraag ingediend om gevolg te gev… |
| `aanvraag_jobcoaching_ingediend` | boolean | WIA, Wajong | Aanvraag of verzoek | Lid 1 + 2.d: aanvraag bij UWV voor noodzakelijke persoonlijke ondersteuning bij het verrichten… |
| `aanvraag_lks_ingediend_binnen_zes_maanden` | boolean | Pwet, ZW | Aanvraag of verzoek | Artikel 10d lid 2: aanvraag werkgever of werknemer is binnen 6 maanden na begin dienstbetrekkin… |
| `aanvraag_loondispensatie_ingediend` | boolean | Wajong | Aanvraag of verzoek | Lid 1: het UWV vermindert "op verzoek van de betrokken werkgever of werknemer". Een aanvraag is… |
| `aanvraag_werkplekaanpassing_ingediend` | boolean | WIA, Wajong | Aanvraag of verzoek | Lid 1 + 2.c: aanvraag voor meeneembare voorzieningen ten behoeve van de inrichting van de arbei… |

## Een bedrag dat in de wet of een AMvB staat (1)

Hoort niet als parameter te worden ingevoerd maar uit de WML en de indexerings-AMvB te komen.

| Gegeven | Type | Wetten | Bron | Waarvoor |
|---|---|---|---|---|
| `minimumloon_plus_vakantiebijslag_eurocent_per_maand` | amount | Pwet, ZW | WML met AMvB-indexering | WML + 8% vakantiebijslag (artikel 15 Wml) per maand, afhankelijk van leeftijd en arbeidsduur. B… |

## Wat dit zegt

**Vijf van de 120 gegevens zijn een handeling van de burger of de werkgever.**
Al het andere is een feit dat elders vastligt of een oordeel dat een uitvoerder
heeft geveld. Een regelhulp die deze lijst als vragenlijst voorschotelt, vraagt
naar dingen die de overheid al weet, en vraagt bovendien om oordelen die de
burger niet kán geven: of een beperking structureel functioneel is, of er reëel
uitzicht op een dienstbetrekking bestaat, wat de loonwaarde is.

**Eén organisatie draagt 52 van de 120.** UWV stelt vast, beoordeelt of
registreert bijna de helft van alles wat de berekening nodig heeft. Gemeenten
volgen met 18, de Belastingdienst met 6. Dat is geen verdeling die met een
koppeling is op te lossen: het is de reden dat een burger vandaag langs drie
loketten moet.

**Vijftien waarden komen al uit een andere wet.** Dat deel werkt, en het laat
zien wat de rest zou kunnen zijn: geen invoer maar een aanroep.

**Het model kan de herkomst nergens vastleggen.** Er is geen veld waarin staat
dat `loonwaarde_eurocent_per_maand` van de gemeente komt en
`verdiencapaciteit_percentage_maatmaninkomen` van UWV. De indeling hierboven
komt deels uit `description`-teksten waarin de modelleur het toevallig heeft
opgeschreven (42 van de 120 noemen zelf een bron) en deels uit de naam.
RFC-017 (Native Data Source Metadata) beschrijft precies dat veld en staat op
concept, niet geïmplementeerd. Zolang dat zo is, is dit document de enige plek
waar de herkomst staat, en veroudert het vanaf de dag dat iemand een parameter
toevoegt.

## Wat er moet gebeuren om dit echt te maken

Zes stappen, op volgorde van wat het meeste oplevert per eenheid werk.

1. **Model de wetten die de besluiten van groep A produceren.** Acht van die 26
   zijn een WIA-, WW- of Wajong-recht, en die wetten zitten al in het dossier.
   Dat een recht hier als parameter binnenkomt terwijl de engine het zelf kan
   uitrekenen, is de goedkoopste winst in de lijst. Drie gegevens dragen zelfs
   letterlijk de naam van een bestaande output.
2. **Regel gegevenslevering voor groep B.** 23 registratiefeiten uit de
   polisadministratie, het doelgroepregister en de gemeentelijke administratie.
   Dit is koppelvlakwerk met een grondslag in de Wet SUWI, geen modelleerwerk.
3. **Win de lagere regelgeving van groep D in en modelleer die.** Vijf
   gegevens, en het maakt er meteen meer los: het Besluit loonkostensubsidie,
   het Besluit loondispensatie Wajong en het Besluit SUWI staan al als tekst in
   het corpus. Voor de gemeentelijke verordening moet eerst een gemeente worden
   gekozen.
4. **Ontwerp hoe groep C eruitziet in een regelhulp.** Zestien professionele
   oordelen die niet bestaan tot iemand ze velt. Een regelhulp kan daar twee
   dingen mee: tonen dat hier een aanname zit en welke, of de vraag doorleiden
   naar degene die het oordeel mag geven. Wat niet kan, is er een invoerveld van
   maken en het antwoord als feit behandelen.
5. **Zoek de vindplaatsen bij groep C.** Welke beleidsregel of welk protocol
   stuurt "reëel uitzicht", "structurele functionele beperking" en de
   loonwaardebepaling. Het corpus heeft geen enkele UWV-beleidsregel, dus dit
   is de vraag waar een jurist het meeste verschil maakt.
6. **Leg de herkomst vast in het model, niet in dit document.** Zolang RFC-017
   concept blijft, is deze indeling een losse bijlage die veroudert. Met dat
   veld wordt het een eigenschap van de parameter zelf, en kan een controle
   afdwingen dat elke nieuwe parameter een herkomst draagt.

De eerste drie stappen halen samen 54 van de 120 gegevens weg bij de invuller.
Wat overblijft is vijf handelingen van de burger of de werkgever, zestien
oordelen die mensenwerk blijven, en de rest die al uit een register of een
andere wet komt.

## Hoe deze lijst is gemaakt

De gegevens komen uit de `parameters`- en `input`-velden van alle
`machine_readable`-blokken in de zeven wetten. Een gegeven dat in meerdere
wetten voorkomt staat hier één keer, met alle wetten erachter. De indeling volgt
in de eerste plaats de `Bron:`-aantekening in de omschrijving, en waar die
ontbreekt de naam en de omschrijving. Veertien gevallen zijn met de hand
ingedeeld. De kolom Bron is dus een voorstel, geen vaststelling, zolang RFC-017
er niet is.
