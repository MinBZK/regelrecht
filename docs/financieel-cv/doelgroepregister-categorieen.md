# Het doelgroepregister banenafspraak: gronden, rechten en bedragen

Alle gronden voor opname in het doelgroepregister banenafspraak (Wfsv artikel
38b), met per grond de rechten die eraan vastzitten en de hoogte daarvan.
Afgeleid uit de `machine_readable`-blokken en de wetteksten op de branch
`traject/financieel-cv-validatie-df48ddd1`, peildatum Wfsv 2026-07-01, Wtl
2026-01-01, overige wetten 2026-07-01.

Hoort bij [`gegevensherkomst.md`](gegevensherkomst.md) en
[`juristsessie-voorbereiding.md`](juristsessie-voorbereiding.md).
De beslisbomen bij dit document staan in
[`beslisboom-doelgroepregister.md`](beslisboom-doelgroepregister.md).

## De kern in één alinea

Het doelgroepregister zelf opent twee deuren: het loonkostenvoordeel doelgroep
banenafspraak (Wtl artikel 2.10) en de no-riskpolis (Ziektewet artikel 29b lid
2). Alle overige instrumenten in dit dossier — loonkostensubsidie,
loondispensatie, jobcoaching, werkplekaanpassing, proefplaatsing — hangen aan de
onderliggende grond zelf, niet aan de registratie. Wie via de Wajong in het
register staat, ontleent loondispensatie aan het Wajong-recht; wie via de
Participatiewet in het register staat, ontleent loonkostensubsidie aan de
doelgroepvaststelling van het college. Dat onderscheid bepaalt wat een
regelhulp mag afleiden uit de enkele mededeling "u staat in het
doelgroepregister".

## Deel 1: De gronden voor opname

Artikel 38b kent zeven gronden in lid 1, een aanvullende grond in lid 2 en een
blijfgrond in lid 6. Het model werkt de gronden af in vaste volgorde en levert
de eerst passende op in `grond_opname_doelgroepregister`.

| Grond | Wettekst in het kort | Wie stelt vast | Parameter in het model | Enum-waarde |
|---|---|---|---|---|
| 38b.1.a | Pwet-toeleiding of loonkostensubsidie op grond van 10d lid 2, plus een UWV-vaststelling dat betrokkene het WML niet kan verdienen, dan wel een loonwaarde onder het WML vastgesteld door het college | UWV, op verzoek van het college; of het college zelf | `is_pwet_lks_toegeleid_met_uwv_loonwaarde_vaststelling` | `pwet_lks_uwv_loonwaarde` |
| 38b.1.b | Wsw-indicatie, of een nog geldende indicatiebeschikking op grond van Wsw artikel 11 zoals dat luidde op 31 december 2014 | UWV (Wsw-indicatie) | `is_wsw_geindiceerd_of_oude_indicatie` | `wsw` |
| 38b.1.c | Recht op Wajong-arbeidsondersteuning of Wajong-uitkering. Wie duurzaam geen mogelijkheden tot arbeidsparticipatie heeft, telt alleen mee zolang die arbeid in dienstbetrekking verricht | UWV | `heeft_wajong_arbeidsondersteuning_of_uitkering`, met `heeft_wajong_duurzaam_geen_mogelijkheden` en `verricht_arbeid_in_dienstbetrekking` | `wajong` |
| 38b.1.d | Voldoet aan een bij of krachtens AMvB vastgestelde indicatie | UWV, uitvoering AMvB | `voldoet_aan_amvb_indicatie_38b_1_d` | `amvb_38b_1_d` |
| 38b.1.e | Pwet-toeleiding plus een UWV-vaststelling op eigen verzoek dat betrokkene het WML niet kan verdienen | UWV, op eigen verzoek van betrokkene | `is_pwet_toegeleid_met_uwv_wml_vaststelling_eigen_verzoek` | `pwet_uwv_wml_eigen_verzoek` |
| 38b.1.f | Overgangsrecht: op of na 1 januari 2013 een persoon als bedoeld onder b of c, en op 1 mei 2015 niet meer | UWV, uit de historische registratie | `was_arbeidsbeperkte_lid_1_b_of_c_op_of_na_2013_01_01`, met uitzondering `was_lid_1_c_en_nu_wajong_duurzaam_geen_mogelijkheden` | `overgangsrecht_38b_1_f` |
| **38b.1.g** | **Recht op een WIA-uitkering uit hoofdstuk 6 (IVA), waarbij bij wijze van experiment op grond van Wet SUWI artikel 82a lid 1 loondispensatie is ingezet** | **UWV** | **ontbreekt** | **ontbreekt** |
| 38b.2 | UWV oordeelt dat betrokkene door ziekte of gebrek ontstaan voor het achttiende jaar, of tijdens de studie, een belemmering ondervindt en zonder voorziening het WML niet kan verdienen | UWV | `is_jonggehandicapt_uwv_oordeel_lid_2` | `jonggehandicapt_uwv_oordeel` |
| 38b.6 | Blijfgrond: betrokkene voldoet niet meer aan lid 1 of lid 2, maar de opname in de registratie van artikel 38d is nog niet geëindigd | UWV, uit de registratie | `was_arbeidsbeperkte_lid_1_of_2` en `registratie_nog_niet_geeindigd` | valt terug op de eerdere grond |

### De uitsluiting in de chapeau

Het chapeau van lid 1 en de aanhef van lid 6 sluiten één groep uit: de persoon
van wie het college heeft vastgesteld dat die uitsluitend in een beschutte
omgeving onder aangepaste omstandigheden mogelijkheden tot arbeidsparticipatie
heeft (Participatiewet artikel 10b lid 1). In het model is dat de parameter
`is_uitgesloten_beschut_werk_pwet_10b`. Beschut werk en het doelgroepregister
sluiten elkaar uit.

### Twee bevindingen op dit artikel

**Onderdeel g ontbreekt in het model.** De geldende tekst van 38b lid 1 kent
zeven onderdelen. Het model kent er zes: er is geen parameter, geen
`voldoet_aan_grond_38b_1_g` en geen enum-waarde voor de IVA-gerechtigde bij wie
op grond van Wet SUWI artikel 82a experimenteel loondispensatie is ingezet. De
tekstcontrole uit deel 1 van de voorbereiding vergelijkt de wettekst en signaleert
dit daarom niet: de tekst klopt, de modellering dekt die niet volledig af.

**De gelijkstelling van 38f lid 5 ontbreekt.** Wtl artikel 2.10 lid 1 geeft
recht op het loonkostenvoordeel aan de arbeidsbeperkte van 38b "of daarmee
gelijkgesteld op grond van artikel 38f, vijfde lid". Die ministeriële
gelijkstelling van bepaalde soorten dienstbetrekkingen is in het model afwezig.
Hetzelfde geldt voor Ziektewet 29b lid 2 onderdeel d, dat er ook naar verwijst.

## Deel 2: Welke rechten aan de registratie hangen

Twee instrumenten toetsen rechtstreeks op de registerstatus.

| Recht | Grondslag | Aan wie | Voorwaarde | Gemodelleerd |
|---|---|---|---|---|
| Loonkostenvoordeel doelgroep banenafspraak | Wtl 2.10, 2.12, 2.13 | Werkgever | Arbeidsbeperkte in de zin van 38b in het aangiftetijdvak, plus een verzoek in de loonaangifte (2.1) | Ja, via `is_doelgroep_banenafspraak` ← `behoort_tot_doelgroepregister_banenafspraak` |
| No-riskpolis | Ziektewet 29b lid 2 onderdeel e | Werkgever | Pwet-toeleiding of loonkostensubsidie 10d lid 2, met UWV- of collegevaststelling; het recht ontstaat niet eerder dan het moment waarop betrokkene arbeidsbeperkte wordt in de zin van 38b lid 1 of 2 | Ja, via `is_banenafspraak_doelgroep` ← `behoort_tot_doelgroepregister_banenafspraak` |

### Hoogte en duur

| | Loonkostenvoordeel banenafspraak | No-riskpolis |
|---|---|---|
| Bedrag | € 1,01 per verloond uur (Wtl 2.13) | 70% van het dagloon (ZW 29b lid 5) |
| Maximum | € 2.000 per werknemer per kalenderjaar (Wtl 2.13) | Eerste 52 weken op verzoek van de werkgever 100% van het dagloon, tot ten hoogste het loon dat de werkgever verschuldigd zou zijn (ZW 29b lid 6) |
| Duur | Zolang de dienstbetrekking voortduurt en aan de voorwaarden van 2.10 wordt voldaan (Wtl 2.12) | Onbeperkt zolang de dienstbetrekking voortduurt (lid 2) |
| Eindigt bij | Pensioengerechtigde leeftijd, of Wsw-dienstbetrekking artikel 2 zonder detachering (Wtl 2.10 lid 2) | Wsw-dienstbetrekking artikel 2 (ZW 29b lid 8) |
| Correctie | Anticumulatie: bij samenloop met een ander loonkostenvoordeel telt uitsluitend het hoogste bedrag (Wtl 4.1 lid 3) | Bij een Wsw artikel 7-arbeidsovereenkomst wordt het dagloon verminderd met het naar werkdagen herleide subsidiebedrag (ZW 29b lid 7) |

**De driejaarstermijn in het model is achterhaald.** De omschrijving van
`datum_opname_doelgroepregister` in het Wfsv-model noemt "3 jaar vanaf opname,
Wtl 2.10-2.13". De geldende tekst van Wtl artikel 2.12 kent die termijn niet
meer: het loonkostenvoordeel doelgroep banenafspraak loopt door zolang de
dienstbetrekking en de voorwaarden bestaan. Artikel 6.2 regelt het
overgangsrecht voor rechten die vóór de inwerkingtreding van de Wet
banenafspraak zijn aangevangen. De omschrijving in de YAML moet worden
bijgewerkt.

## Deel 3: Rechten die aan de onderliggende grond hangen

Deze instrumenten toetsen op de grond zelf. Een registratie is er noch nodig
noch voldoende voor.

| Recht | Grondslag | Hangt aan | Aan wie | Hoogte | Gemodelleerd |
|---|---|---|---|---|---|
| Loonkostensubsidie | Pwet 10c, 10d | Doelgroepvaststelling door het college | Werkgever | Verschil tussen WML plus vakantiebijslag en de vastgestelde loonwaarde plus vakantiebijslag, ten hoogste 70% van WML plus vakantiebijslag, op de normbasis van 36 uur per week en daarna naar evenredigheid van de overeengekomen arbeidsduur | Ja, met bedragen |
| Begeleiding op de werkplek | Pwet 10da | Doelgroep loonkostensubsidie | Werknemer | Geen bedrag in de wet | Ja, als aanspraak |
| Loondispensatie | Wajong 2:20 | Recht op arbeidsondersteuning | Werkgever, via een lagere beloningsaanspraak | Vermindering naar evenredigheid van de arbeidsprestatie, in afwijking van de WML. Een lagere beloning overeenkomen is nietig (lid 2) | Alleen als recht; geen bedrag |
| No-riskpolis, overige gronden | ZW 29b lid 1, lid 2 a t/m d en f, lid 4 | WIA-recht, Wajong-recht, Wsw, beschut werk, onderwijsbelemmering | Werkgever | 70% van het dagloon, met de afwijking van lid 6 | Ja, als recht en duur; geen bedrag |
| Jobcoaching | WIA 35 lid 2 onderdeel d; Wajong 2:22 lid 2 onderdeel d; Pwet 10 lid 1 slotzin | WIA-recht met structurele functionele beperking; Wajong-recht; Pwet-doelgroep | Werknemer | Geen bedrag in de wet; het Reïntegratiebesluit vult de open norm | Ja, als aanspraak |
| Werkplekaanpassing | WIA 35 lid 2 onderdeel c; Wajong 2:22 lid 2 onderdeel c; Pwet 10 lid 1 en 10e lid 2 onderdeel c | Zelfde | Werknemer | Geen bedrag in de wet | Ja, als aanspraak |
| Proefplaatsing | WW 76a, WIA 37, Wajong 2:24, Pwet 8a lid 2 onderdeel d | WW-, WIA- of Wajong-recht; of algemene bijstand | Werkgever, in de vorm van onbeloonde arbeid | Geen bedrag. De uitkering loopt door | Ja, met duur |

### Duur van de proefplaatsing

| Regeling | Maximale duur |
|---|---|
| WW 76a lid 1 | Zes kalendermaanden |
| Wet WIA 37 lid 1 | Zes maanden |
| Wajong 2:24 lid 1 | Zes maanden |
| Participatiewet 8a lid 2 onderdeel d | Twee maanden, met verlenging van ten hoogste vier maanden, dus ten hoogste zes maanden bij volledige verlenging |

## Deel 4: De bedragen op een rij

| Instrument | Bedrag | Maximum | Grondslag | In het model |
|---|---|---|---|---|
| LKV doelgroep banenafspraak | € 1,01 per verloond uur | € 2.000 per werknemer per kalenderjaar | Wtl 2.13 | `tegemoetkoming_banenafspraak_eurocent` |
| LKV arbeidsgehandicapte werknemer | € 3,05 per verloond uur | € 6.000 per werknemer per kalenderjaar, gedurende ten hoogste drie aaneengesloten jaren (Wtl 2.8) | Wtl 2.9 | `tegemoetkoming_arbeidsgehandicapte_eurocent` |
| LKV herplaatsen arbeidsgehandicapte werknemer | € 3,05 per verloond uur | € 6.000 per werknemer per kalenderjaar, gedurende ten hoogste één jaar (Wtl 2.16) | Wtl 2.17 | `tegemoetkoming_herplaatsen_eurocent` |
| Anticumulatie LKV | Uitsluitend het hoogste van de berekende bedragen; bij gelijke hoogte het eerstgenoemde in de wet | — | Wtl 4.1 lid 3 | `hoogte_lkv_per_jaar_eurocent`, `categorie_lkv` |
| Loonkostensubsidie | WML plus vakantiebijslag minus loonwaarde plus vakantiebijslag | 70% van WML plus vakantiebijslag | Pwet 10d lid 4 | `hoogte_lks_eurocent_per_maand` |
| Ziekengeld no-riskpolis | 70% van het dagloon | Eerste 52 weken op verzoek 100% van het dagloon | ZW 29b lid 5 en 6 | ontbreekt |
| Loondispensatie | Vermindering naar evenredigheid van de arbeidsprestatie | — | Wajong 2:20 lid 1 | ontbreekt |

## Deel 5: Wat er ontbreekt om dit door te rekenen

1. **Het WML-bedrag is invoer.** `minimumloon_plus_vakantiebijslag_eurocent_per_maand`
   is de enige parameter van de 120 die een wetsbedrag draagt. Zonder een
   gemodelleerde WML met artikel 15 vakantiebijslag en de indexerings-AMvB rust
   de hele loonkostensubsidieberekening op een handmatig ingevoerd getal. In
   `corpus-poc/terugbetaalregimes` ligt WML artikel 8 al gemodelleerd, met
   peildatums 2025 en 2026.
2. **Het dagloon is nergens gemodelleerd.** Ziekengeld, WIA- en WW-uitkering
   rusten er alle drie op. Het Dagloonbesluit werknemersverzekeringen staat als
   tekst in het corpus en heeft geen `machine_readable`. Zolang dat zo blijft,
   levert de no-riskpolis wel een recht en een duur op, en geen bedrag.
3. **De loonwaarde is een professioneel oordeel zonder methode.** De
   loonkostensubsidieberekening leunt op `loonwaarde_eurocent_per_maand`. Het
   Besluit loonkostensubsidie Participatiewet, dat de methode voorschrijft,
   staat als tekst in het corpus en is niet gemodelleerd.
4. **Loondispensatie levert geen bedrag.** Wajong 2:20 spreekt van vermindering
   naar evenredigheid. Het Besluit loondispensatie Wajong vult dat in en staat
   als tekst in het corpus.
5. **De loonkostensubsidieberekening hangt aan het verkeerde artikel.** De
   outputs `bruto_subsidie_eurocent_per_maand`, `maximum_subsidie_eurocent_per_maand`,
   `hoogte_lks_voltijd_eurocent_per_maand` en `hoogte_lks_eurocent_per_maand`
   staan in het `machine_readable`-blok van Participatiewet artikel 10c, en hun
   omschrijvingen verwijzen naar "lid 4". Artikel 10c gaat over de vaststelling
   wie tot de doelgroep behoort en kent twee leden. De berekening staat in
   artikel 10d lid 4, dat geen `machine_readable`-blok heeft. Een jurist die de
   verwijzing natrekt, komt bij het verkeerde artikel uit.
6. **Wtl artikel 2.10 is niet gemodelleerd.** De voorwaarden voor het
   loonkostenvoordeel doelgroep banenafspraak, inclusief de uitsluitingen van
   lid 2 en de duur van 2.12, worden in artikel 2.1 afgevangen met de
   binnenkomende waarde `is_doelgroep_banenafspraak`. De artikelen 2.6 en 2.14
   zijn voor de twee andere categorieën wel volledig gemodelleerd. De
   asymmetrie verklaart waarom de pensioengerechtigde leeftijd en de
   Wsw-uitsluiting voor deze categorie nergens worden getoetst.

## Hoe deze lijst is gemaakt

De gronden komen uit de tekst van Wfsv artikel 38b en uit de outputs
`voldoet_aan_grond_38b_*` en `grond_opname_doelgroepregister` van hetzelfde
artikel. De rechten komen uit de `source`-aanroepen die naar
`behoort_tot_doelgroepregister_banenafspraak` verwijzen, aangevuld met de
outputs van de overige zes wetten. De bedragen komen uit de wetteksten van
Wtl 2.9, 2.13 en 2.17, Participatiewet 10d lid 4 en Ziektewet 29b lid 5 tot en
met 7. Waar het model en de wettekst uiteenlopen, staat dat erbij.
