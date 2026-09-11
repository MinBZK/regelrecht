# Audit machine-leesbare wetten: synthese

Bereik: 83 wetsbestanden in `corpus/regulation/` en `corpus/demo/regulation/`. De ruwe bevindingen zijn gededupliceerd (dezelfde afwijking in twee versies of in een zusterbestand telt als een punt), nagelezen tegen de YAML en, waar de bevinding zelf twijfelde, tegen de engine. Rangorde per wet: eerst wat een uitkomst voor een burger verandert (bedrag, datum, recht), dan wat een oordeel raakt (`voldoet`, `is_gerechtigd`, weigeringsgrond), dan de rest (grondslagverwijzing, documentatie, structuur).

Verworpen na nalezen, omdat er geen echte afwijking is:

- Archiefwet openbaarheid, art. 15 lid 6: de boolean `ministerraad_besluit_staatsbelang` dekt "voor zover de ministerraad niet anders beslist" correct. Geen punt.
- Awb bezwaar, het commentaar over FOREACH+ADD: correcte technische toelichting, geen afwijking.
- APV erfgrens Amsterdam, `postcode: required: true`: de bevinding stelde dat de heg-aanroep zonder postcode een `MissingParameter`-fout geeft. Dat klopt niet. De engine werkt met Kleene-logica (`operations.rs`: "AND with a false is false whatever the unknown"), de eerste IF-tak faalt al op `type_beplanting`, en het scenario "Heg in Amsterdam" slaagt. Wat overblijft is een te ruime declaratie (lid 2 heeft geen postcode nodig); die staat hieronder als restpunt, niet als uitkomstfout.
- WPM RVO, `aantal_werknemers`/`verstrekt_mobiliteitsvergoeding` niet nullable: de bevinding zegt zelf dat unknown-propagatie hier de juiste RFC-036-vorm is. Geen punt.
- Wet BRP terugmelding Belastingdienst, "geen eigen scenario's": proceskwestie, geen afwijking van de tekst.
- Zorgtoeslagwet 2025, "harde afkap boven drempelinkomen": `drempelinkomen_alleenstaande: 3971900` is de afbouwgrens (het inkomen waarboven standaardpremie minus normpremie nul wordt), niet het wettelijke drempelinkomen. De afkap is rekenkundig gelijk aan MAX(0, ...). De naam is misleidend en de vermogenstoets hoort bij art. 2a, niet art. 2; dat staat als restpunt.
- Besluit stralingsbescherming, required/nullable-asymmetrie tussen sub b en sub c: de bevinding acht dat zelf verdedigbaar. Als documentatiepunt meegenomen.

Gedupliceerd en samengevoegd:

- De verouderde untranslatable "FOREACH niet beschikbaar in v0.5.1" staat identiek in dertien bestanden (awb beroep en bezwaar, beschermingsbewind, curatele, gezag, volmacht, executeur, mentorschap, handelingsonbekwaamheid, wsnp, curator, machtigingenwet, kinderopvang, kindgebonden budget, SUWI, strafrecht). Een punt.
- De lege-string-tak `EQUALS $current.datum_einde ''` naast de null-toets staat identiek in negen delegatiebestanden (beschermingsbewind, curatele, executeur, gezag, handelingsonbekwaamheid, mentorschap, volmacht, curator, wsnp) plus SUWI (`end_date`). Een punt.
- `voldoet_aan_voorwaarden` dat alle registraties telt in plaats van de actieve: curator en wsnp. Een punt.
- `is_verzekerde_zorgtoeslag: true`: zorgtoeslagwet 2024 en 2025. Een punt.
- Alcoholwet levensgedrag: de bevinding op VWS (null wordt "voldoet") en de bevinding op Rotterdam (constante `false` doorgeven) zijn twee kanten van een afwijking en zijn samengenomen.

## Corpus (bucket A, echte corpus)

### Burgerlijk Wetboek Boek 5, art. 5:42 (`corpus/regulation/nl/wet/burgerlijk_wetboek_boek_5/2024-01-01.yaml`)

Uitkomst voor de burger

- Art. 42 lid 1, 3 en 4. Tekst: verbod binnen de afstand tenzij toestemming of openbare weg/water (lid 1); geen verzet bij beplanting niet hoger dan de scheidsmuur (lid 3); schadevergoeding pas na aanmaning (lid 4). YAML: alleen `minimale_afstand_cm`/`_m` (lid 2). Categorie: scenario. Getrouwe vorm: outputs `beplanting_geoorloofd` (AND van afstandstoets, NOT toestemming, NOT openbare weg of water), `nabuur_kan_zich_verzetten` (hoogte boven scheidsmuur) en `vergoedbare_schade` (schadedatum na aanmaningsdatum). Alle drie uitdrukbaar met AND/IF/DATE_DIFF. Kan nu.

Rest

- Lid 2, "of een plaatselijke gewoonte": alleen de verordeningstak is gemodelleerd (open_terms). Plaatselijke gewoonte is geen document dat een gemeente `implements`. Categorie: engine. Getrouwe vorm: untranslatable-vermelding "ongeschreven afwijkingsgrond, geen registreerbare implementatie". Kan nu als vermelding; het mechanisme zelf niet (RFC-kandidaat).

### APV erfgrens Amsterdam, art. 2.75 (`corpus/regulation/nl/gemeentelijke_verordening/amsterdam/apv_erfgrens/2024-01-01.yaml`)

Rest

- `postcode: required: true` terwijl lid 2 (heggen, heel Amsterdam) geen postcode nodig heeft. Categorie: schema. Uitkomst is ongewijzigd door Kleene-AND. Getrouwe vorm: `required: false`. Kan nu.

### Afstemmingsverordening Participatiewet Diemen, art. 9 (`corpus/regulation/nl/gemeentelijke_verordening/diemen/afstemmingsverordening_participatiewet/2015-01-01.yaml`)

Oordeel

- Gedragscategorie als los getal 0..3 met "0 = geen gedraging" als sentinel; art. 7 (de classificatie) heeft geen machine_readable. Categorie: scenario. Getrouwe vorm: art. 7 met eigen machine_readable die de categorie afleidt uit de feitelijke criteria, output `nullable: true` voor "geen verwijtbare gedraging", art. 9 leest die output. Kan nu.

Rest

- Lid 2, afwijkingsbevoegdheid van het college (5 tot 100 procent): nergens genoemd. Categorie: scenario. Getrouwe vorm: untranslatable op `verlaging_percentage` en `duur_maanden`. De bevoegdheid zelf is niet berekenbaar.

## Demo-corpus

### Alcoholwet vergunning VWS, art. 8 (`corpus/demo/regulation/nl/alcoholwet/vergunning/VWS-2024-01-01.yaml`) en Rotterdam (`.../gemeenten/GEMEENTE_ROTTERDAM-2024-01-01.yaml`)

Oordeel

- Lid 1 onder b, slecht levensgedrag. Tekst: positieve eis. YAML: `EQUALS $is_van_slecht_levensgedrag null THEN true` (voldoet), en Rotterdam geeft altijd de constante `geen_slecht_levensgedrag: false` door omdat er geen register is. Categorie: data. Getrouwe vorm: Rotterdam geeft geen constante door; VWS laat de `null -> true`-tak vallen zodat afwezigheid als "onbekend, ontbreekt: levensgedrag" uitkomt, met een untranslatable die het ontbrekende register benoemt. Kan nu; het demo-scenario krijgt dan "onbekend" in plaats van een verleende vergunning.
- Lid 3 en 4, Register sociale hygiëne. Tekst: inschrijvingseis (lid 3) met een uitzondering voor de risicodragende leidinggevende zonder bemoeienis, schriftelijk bevestigd (lid 4). YAML: `EQUALS $is_ingeschreven_svh_register null THEN true`; lid 4 nergens. Categorie: schema/data. Getrouwe vorm: `voldoet_aan_svh_eis = IF ingeschreven THEN true ELSE IF (NOT heeft_bemoeienis_bedrijfsvoering AND schriftelijke_verklaring_bevestigd) THEN true ELSE false`; afwezige registratie is "niet voldaan" of "onbekend", niet "voldoet". Kan nu, vergt twee nieuwe booleans.
- Lid 5, paracommerciële rechtspersoon: minstens twee leidinggevenden. Ontbreekt. Categorie: scenario. Getrouwe vorm: FOREACH over de leidinggevenden met telling >= 2 als `is_paracommercieel`. Vergt een collectie-input die nu niet bestaat.

Rest

- Lid 2 (AMvB nadere eisen zedelijk gedrag): geen open_terms, geen untranslatable. Getrouwe vorm: open_term of untranslatable. Kan nu.
- Art. 10 lid 2 (vloeroppervlakte) en art. 27 (weigeringsgronden) worden als legal_basis gebruikt zonder dat hun tekst in het corpus staat. Getrouwe vorm: artikelen opnemen als `articles`-item. Kan nu.

### Register sociale hygiëne SVH, art. 8 (`corpus/demo/regulation/nl/alcoholwet/register_sociale_hygiene/SVH-2024-01-01.yaml`)

Rest

- Alle vier acties citeren `paragraph: '4'` (de vrijstelling) terwijl zij de inschrijving van lid 3 raadplegen. Categorie: scenario. Getrouwe vorm: `paragraph: '3'`. Kan nu.
- `produces: BESCHIKKING/TOEKENNING` voor een registerraadpleging. Categorie: schema. Getrouwe vorm: `legal_character: TOETS`. Kan nu.
- Lid 4 en 5 niet gemodelleerd, geen untranslatable. Categorie: data (het register heeft geen veld voor de verklaring). Kan nu als vermelding.

### Algemene Kinderbijslagwet SVB, art. 6 (`corpus/demo/regulation/nl/algemene_kinderbijslagwet/SVB-2025-01-01.yaml`)

Rest

- Het enige artikel (verzekeringskring) draagt outputs die uit art. 7 komen (recht op kinderbijslag), zonder legal_basis naar art. 7. Categorie: scenario. Getrouwe vorm: art. 7 opnemen en de outputs daar onderbrengen. Kan nu.

### Algemene nabestaandenwet SVB, art. 14 (`corpus/demo/regulation/nl/algemene_nabestaandenwet/SVB-2026-01-01.yaml`)

Uitkomst voor de burger

- Lid 1 onder b. Tekst: arbeidsongeschikt sinds de dag van overlijden (of het einde van de maand waarin onderdeel a wegviel), ten minste drie maanden voortdurend of aannemelijk. YAML: `ao_percentage >= 45` met een verzonnen `ao_drempel: 45`. Categorie: scenario. Getrouwe vorm: ingangsdatum van de arbeidsongeschiktheid plus een DATE_DIFF-duurtoets; het aannemelijkheidsoordeel als untranslatable. Kan nu niet: het register levert een percentage, geen ingangsdatum.
- Lid 2 t/m 5, ingangsdatum en uitzonderingen: niet gemodelleerd. Getrouwe vorm: output `ingangsdatum` met IF-keten; lid 5 (SVB-discretie) untranslatable. Kan nu niet volledig (lid 5).

Oordeel

- `is_gerechtigd: value: true`, ongeconditioneerd (regel 173). Categorie: schema. Getrouwe vorm: `value: $voldoet_aan_voorwaarden`. Kan nu.
- Lid 1 onder a: drie cumulatieve eisen aan het kind (ongehuwd, jonger dan 18, niet in het huishouden van een ander). YAML: een boolean `heeft_kinderen_onder_18`. Categorie: scenario. Getrouwe vorm: FOREACH over de kinderen met alle drie voorwaarden. Kan nu, vergt kinderen als collectie.

Rest

- `partner_verzekerd` en overlijdensdatum onder legal_basis art. 14, terwijl dat art. 13/15 is. Kan nu.

### Algemene Ouderdomswet SVB, art. 7 t/m 11 (`corpus/demo/regulation/nl/algemene_ouderdomswet/SVB-2024-01-01.yaml`)

Uitkomst voor de burger

- Toeslagkorting (art. 10 jo. 11). Tekst: 15 procent van het bruto-minimumloon vrijgelaten, van het meerdere twee derde in mindering. YAML: lineaire aftopping met `kortingsdeler` (50 cent) en `inkomensgrens_partner` zonder herkomst; legal_basis alleen art. 9. Categorie: poc. Getrouwe vorm: `toeslag = MAX(0, toeslag_max - MAX(0, partner_inkomen - 0.15 * minimumloon) * 2/3)` met minimumloon als cross-law input. Kan nu, vergt een minimumloonbron.
- Art. 8 lid 1 en 2: toeslag alleen bij huwelijk en recht voor 1 januari 2015. YAML: geen huwelijksdatumvoorwaarde. Categorie: scenario. Getrouwe vorm: guard op huwelijksdatum en ingangsdatum recht. Kan nu niet: die datums zitten niet in de inputs.

Oordeel

- `is_gerechtigd: value: true` (regel 228), los van `voldoet_aan_voorwaarden`. Categorie: poc. Getrouwe vorm: `value: $voldoet_aan_voorwaarden`. Kan nu.

### AOW leeftijdsbepaling SVB, art. 7a (`corpus/demo/regulation/nl/algemene_ouderdomswet/leeftijdsbepaling/SVB-2024-01-01.yaml`)

Uitkomst voor de burger

- Lid 1 onder n (2025: 67 jaar). `verhoging_2025: 24` is gedefinieerd maar de laatste `when` is `< 1960-01-01`; geboortejaar 1960 valt door naar de formule-tak. Categorie: poc. Getrouwe vorm: extra `when: LESS_THAN $geboortedatum '1961-01-01' then: DIVIDE($verhoging_2025, 12)`. Kan nu.
- Lid 2, de formule. Tekst: `V = 2/3 * (L - 20,64) - (P - 67)`, V < 0,25 wordt 0, anders precies drie maanden. YAML: `MIN((L - 20.0) * 3/12, 2)`: geen P, verkeerde referentie, proportioneel in plaats van een stap. Categorie: poc. Getrouwe vorm: V berekenen zoals de tekst zegt, `IF V < 0.25 THEN 0 ELSE 3/12`, met de pensioenleeftijd van het voorafgaande jaar als jaargebonden gegeven. Kan nu.

Rest

- Lid 2 en 3: de verhoging is een landelijke, vijf jaar vooraf vastgestelde jaarwaarde, geen live per-persoon berekening op de individuele CBS-raming; `aankondigingsperiode_jaren` is ongebruikt. Categorie: poc. Getrouwe vorm: jaargebonden gegeven `pensioengerechtigde_leeftijd_kalenderjaar`. Kan nu niet zonder zo'n tabel.

### AOW gegevens SVB, art. 7a (`corpus/demo/regulation/nl/algemene_ouderdomswet_gegevens/SVB-2025-01-01.yaml`)

Oordeel

- De hele norm (tabel lid 1, formule lid 2, vijfjaarstermijn lid 3) is afwezig: `pensioenleeftijd` wordt uit een register gelezen. Categorie: scenario. Getrouwe vorm: tabel als IF-cascade op het jaar, formule met stapfunctie via IF tegen 0,25, CBS-raming als `source: {}`. Kan nu.

Rest

- `type_spec min: 65, max: 70` is een gok, geen wettelijke grens. Categorie: checker. Getrouwe vorm: verwijderen of als redelijkheidscontrole documenteren. Kan nu.
- Geen `nullable`/`absent:`-regel voor `pensioengegevens`. Vervalt zodra de norm zelf gemodelleerd is.

### APV Rotterdam exploitatievergunning, art. 2:28 (`corpus/demo/regulation/nl/algemene_plaatselijke_verordening/exploitatievergunning/gemeenten/GEMEENTE_ROTTERDAM-2024-01-01.yaml`)

Uitkomst voor de burger

- Lid 2: vijf jaar "tenzij bij de vergunning anders is bepaald". YAML: onvoorwaardelijk 5. Categorie: poc. Getrouwe vorm: nullable input `afwijkende_vergunningsduur` met IF. Kan nu.

Oordeel

- Lid 5 onder h, i, j, k (voorschriften overtreden, geen KvK-inschrijving, feitelijke toestand afwijkend, strijd met horecabeleid) ontbreken uit `harde_weigeringsgrond`. Categorie: poc. Getrouwe vorm: vier nullable booleans als extra `when`-cases. Kan nu, vergt registervelden.
- Lid 4 onder a en d koppelen 21 jaar en sociale hygiëne aan "een drank- en horecawetvergunning is verstrekt", niet aan `schenkt_alcohol`. Categorie: data. Getrouwe vorm: input `heeft_dhw_vergunning` (cross-law naar de Alcoholwet-vergunning). Kan nu.
- Lid 4 aanhef: de eisen gelden per beheerder. De `beheerders`-array bestaat, maar de aggregatie (`alle_hebben_vog` enz.) komt kant-en-klaar uit bindings.yaml. Categorie: schema. Getrouwe vorm: FOREACH over `$beheerders` met `combine: AND` in de wet zelf. Kan nu.
- Lid 6, elf facultatieve gronden: alleen een Bibob-casus die in de tekst niet voorkomt. Getrouwe vorm: registerbare gronden als cases, beoordelingsgronden (a, b, c) als untranslatable. Kan nu.

### APV Rotterdam ontheffingspas geluid, art. 4:2 (`corpus/demo/regulation/nl/algemene_plaatselijke_verordening/ontheffingspas_geluid/gemeenten/GEMEENTE_ROTTERDAM-2024-01-01.yaml`)

Oordeel

- Het hele machine_readable-blok (KvK-quotum van 12, aanvraagtermijn, klachtencheck, eindtijden) hoort niet bij art. 4:2 (aanwijzing collectieve festiviteiten door het college). Categorie: scenario. Getrouwe vorm: pas mogelijk als het juiste artikel is aangewezen. Kan nu niet.
- `komt_in_aanmerking_voor_geluidsontheffing: value: true` (regel 187), los van `voldoet_aan_voorwaarden`. Getrouwe vorm: `value: $voldoet_aan_voorwaarden`. Kan nu, ook binnen de huidige (verkeerde) constructie.

Rest

- `text` breekt af midden in een HTML-attribuut (scrape-artefact). Categorie: data. Getrouwe vorm: schone tekst opnieuw ophalen. Kan nu.

### APV Rotterdam terrassen, art. 2:30b (`corpus/demo/regulation/nl/algemene_plaatselijke_verordening/terrassen/GEMEENTE_ROTTERDAM-2024-01-01.yaml`)

Oordeel

- Datacontract. bindings.yaml zegt `absent: unknown` voor `beschikbare_oppervlakte`, `functie_oppervlak`, `max_sluitingstijd_*`, `tarief_per_m2`; de wet test `EQUALS $x null` (met RFC-036-commentaar). De null-tak wordt nooit geraakt, de uitkomst is "onbekend" in plaats van de beschreven afhandeling. Categorie: data. Getrouwe vorm: een van beide kiezen: `absent: null` in bindings als "geen BGT-locatie" een echte afwezigheid is, of de null-guards weghalen en de unknown-propagatie in de scenario's asserteren. Kan nu.
- De harde checklist (1,8 m, BGT-functies, sluitingstijden, alcoholvergunning) staat niet in 2:30b; de discretionaire gronden van lid 2 zijn niet gemodelleerd of als untranslatable benoemd. Getrouwe vorm: untranslatable op de weigeringsgrond, objectieve criteria met bron-vindplaats (Terrassenbeleid 2023, precarioverordening). Kan nu als documentatie.

Rest

- Lid 3, 4, 5 (verwijderplicht, verbod buiten het vergunde deel, opruimplicht) niet gemodelleerd. Untranslatable (gedragsnormen). Kan nu.
- Alcoholvergunning-voorwaarde zonder grondslag. Verwijderen of legal_basis. Kan nu.

### Awb art. 1:1 bestuursorgaan (`corpus/demo/regulation/nl/algemene_wet_bestuursrecht/artikel_1_1_bestuursorgaan/AWB-1994-01-01.yaml`)

Oordeel

- Lid 2 onder a, d, g, h, i (wetgevende macht, Raad van State, functionarissen van de uitgesloten organen, CTIVD, TIB) ontbreken; de wetgevende macht kan zo ten onrechte als bestuursorgaan kwalificeren. Categorie: scenario. Getrouwe vorm: vijf nullable booleans als OR-takken in `is_excluded` en `exclusion_reason`. Kan nu.
- Lid 3 (toch bestuursorgaan bij ambtenaarrechtelijke besluiten, met terug-uitzondering) ontbreekt. Getrouwe vorm: `OR(qualifies AND NOT excluded, excluded AND ambtenaarrechtelijk AND NOT voor_het_leven_benoemd)`. Kan nu.

Rest

- Het 66,67-procent-financieringscriterium komt uit jurisprudentie, niet uit de tekst; ongedocumenteerd. Commentaar met bron. Kan nu.
- Lid 4 (vermogensrechtelijke gevolgen) buiten scope, niet vermeld. Commentaar. Kan nu.

### Archiefwet openbaarheid, art. 15 (`corpus/demo/regulation/nl/archiefwet/NATIONAAL_ARCHIEF-openbaarheid-2024-01-01.yaml`)

Oordeel

- Lid 4: de 75-jaargrens geldt "tenzij Onze minister of GS anders beslist". YAML: onvoorwaardelijke doorbreking, alleen de staatsbelangtak (lid 6) heeft een schakelaar. Categorie: scenario. Getrouwe vorm: nullable boolean `minister_of_gs_besluit_anders` als guard op de 75-jaartak. Kan nu.
- Lid 2, 3, 5 (beperkingen achteraf, opheffing per verzoeker, uitzondering staatsbelang) ontbreken. Discretionaire bevoegdheden; getrouwe vorm is untranslatable, eventueel met parameter `beperking_ambtshalve_opgeheven`. Kan nu als vermelding.

Rest

- Lid 7 (Woo van toepassing bij staatsbelang): output `alternatieve_regeling_van_toepassing` of untranslatable. Kan nu.
- Default "Beperkt, reden onbekend": vangnet voor een niet-gesloten enum. Categorie: schema (geen enum-validatie op strings). Kan nu niet zonder enum.

### Archiefwet overbrenging, art. 12/13 (`corpus/demo/regulation/nl/archiefwet/NATIONAAL_ARCHIEF-overbrenging-2024-01-01.yaml`)

Oordeel

- Art. 13 lid 4: machtiging geldt ten hoogste tien jaar. YAML: `opschortingsmachtiging: true` is tijdloos. Categorie: scenario. Getrouwe vorm: machtigingsdatum plus DATE_DIFF < 10 jaar. Kan nu, vergt een datum-input.

Rest

- Opschortingslogica onder legal_basis art. 12 lid 1 in plaats van art. 13 lid 3/4; `uiterste_overbrengdatum` citeert Archiefbesluit art. 11 in plaats van art. 9. Kan nu.
- Art. 12 lid 2 (AMvB-delegatie) niet als open_terms. Kan nu.

### Archiefwet vernietiging, art. 5 (`corpus/demo/regulation/nl/archiefwet/NATIONAAL_ARCHIEF-vernietiging-2024-01-01.yaml`)

Oordeel

- `bewaartermijn_jaren` afwezig telt via `default: true` als "termijn verstreken" (regel 98 e.v.). Categorie: scenario. Getrouwe vorm: `default: false` of onbeslisbaar; of `bewaartermijn_jaren` verplicht zodra `op_selectielijst_vernietiging` waar is. Kan nu.
- `selectielijst_vastgesteld`, `selectielijst_gepubliceerd`, `bewaartermijn_jaren` zijn institutionele registerfeiten, geen aanvraaggegevens die een aanvrager kan weglaten. Getrouwe vorm: `required: true`, `nullable: false`, guards weg. Kan nu.

Rest

- `documenttype` en `minimale_bewaartermijn_jaren` ongebruikt; `mag_vernietigd_worden` citeert art. 3 waar art. 5 lid 2/3 wordt getoetst; de rekenregel "termijn vanaf aanmaakdatum" staat niet in de tekst. Kan nu (documentatie).

### Awb beroep JenV (`corpus/demo/regulation/nl/awb/beroep/JenV-2024-01-01.yaml`)

Oordeel

- `$zaak.status` en `$zaak.approved` worden gelezen maar `zaak` is geen gedeclareerde input (inputs: wet, adres, jurisdictie, gebeurtenissen). Categorie: scenario. Getrouwe vorm: `zaak` als input (object, `source: {}`). Kan nu.
- `type_rechter` mengt rechtertypen (RECHTBANK, GERECHTSHOF) met een concrete rechtbanknaam (`RECHTBANK_DEN_HAAG`), waardoor `bevoegde_rechtbank` een compensatietak nodig heeft. Categorie: schema. Getrouwe vorm: type en naam scheiden. Kan nu.

Rest

- Uitsluitingsgronden onder 8:1/8:3 in plaats van 1:3/8:2/8:5 en 7:1; `direct_beroep` citeert 3:11 in plaats van 7:1 lid 1 onder d. Kan nu.
- Null-toets op `$wet.beroepstermijn_weken`: een genest veld van een `object`-input kan niet `nullable` zijn. Categorie: schema (RFC-kandidaat).

### Awb bezwaar JenV (`corpus/demo/regulation/nl/awb/bezwaar/JenV-2024-01-01.yaml`)

Oordeel

- Art. 7:1 lid 1 onder a t/m g: geen van de zeven gronden gemodelleerd; alleen `decision_type`/`legal_character`. Categorie: scenario. Getrouwe vorm: per grond een conditie of een untranslatable. Kan nu niet volledig (brondata).
- `bezwaartermijn_weken` enz. als subvelden van het ongetypeerde object `wet`; de null-toets ontsnapt aan de typechecker (RFC-037 N1). Categorie: schema. Getrouwe vorm: losse getypeerde inputs met `nullable: true`. Kan nu.

Rest

- FOREACH-telling van eerdere bezwaren implementeert art. 6:17 zonder eigen legal_basis. Kan nu.

### Belastingdienst vermogen, art. 47 AWR (`corpus/demo/regulation/nl/belastingdienst_vermogen/BELASTINGDIENST-2025-01-01.yaml`)

Rest

- Art. 47 (informatieplicht) is geen norm over vermogen; het bestand is een registerwrapper zonder toelichting. Untranslatable of commentaar. Kan nu.
- `vermogen` niet nullable, terwijl het kindgebonden budget dezelfde output als `partner_vermogen` wel nullable afneemt. Categorie: schema. Getrouwe vorm: een contract kiezen en documenteren. Kan nu.

### Besluit basisveiligheidsnormen stralingsbescherming, art. 3.7 (`corpus/demo/regulation/nl/besluit_basisveiligheidsnormen_stralingsbescherming/ANVS-2018-01-01.yaml`)

Rest

- Zes van acht weigeringsgronden (a, d, e, f, g, h) niet gemodelleerd; "kan worden overschreden" niet apart. Untranslatables. Kan nu.
- De optelling over alle handelingen van de ondernemer (b) en andere handelingen (c) wordt bij de aanvrager verondersteld. Commentaar bij de parameters. Kan nu.

### Besluit bijstandverlening zelfstandigen, art. 2 (`corpus/demo/regulation/nl/besluit_bijstandverlening_zelfstandigen/SZW-2025-01-01.yaml`)

Oordeel

- Urencriterium (1225 uur) als blanket-voorwaarde op alle vier onderdelen; art. 2 noemt het niet. Categorie: scenario. Getrouwe vorm: verwijderen of legal_basis naar het artikel dat het stelt. Kan nu.
- Vermogenstoets als voorwaarde voor algemene bijstand; lid 2 beperkt alleen bedrijfskapitaal tot a, b, c. Getrouwe vorm: lid 2 als categorie-restrictie op `bedrijfskapitaal_max`. Kan nu.
- Onderdeel c mist "inkomen duurzaam ontoereikend"; `min_inkomen_ouder` ligt ongebruikt klaar. Getrouwe vorm: `LESS_THAN_OR_EQUAL $inkomen_uit_bedrijf $min_inkomen_ouder` of untranslatable voor "duurzaam". Kan nu.
- Onderdeel b mist de echtgenoot-variant (WW-uitkering van de echtgenoot). Getrouwe vorm: `OR(heeft_ww_uitkering, partner.heeft_ww_uitkering)`. Kan nu, vergt partnerdata.
- Minimum- en pensioenleeftijd zonder grondslag in art. 2. Verwijderen of legal_basis. Kan nu.

Rest

- Onderdeel a "redelijke termijn" als untranslatable. Dode definities (`vermogensgrens_laag`, `rente_percentage`, `belasting_percentage`). Kan nu.

### Besluit kerninstallaties, art. 1 (`corpus/demo/regulation/nl/besluit_kerninstallaties/ANVS-2024-01-01.yaml`)

Rest

- Het hele execution-blok (financiële zekerheid, beveiligingsplan, noodplan, deskundigen) hangt onder een zuiver begripsartikel; `minimaal_financiele_zekerheid` en `minimum_aantal_deskundigen` zijn ongemotiveerde constanten; legal_basis noemt geen artikelnummer. Categorie: poc. Getrouwe vorm: de eisen onderbrengen bij de artikelen die ze stellen, het bedrag als open_term. Kan nu niet zonder die artikelen op te zoeken.
- `heeft_beeindigingsplan` gedeclareerd, nooit gebruikt. Toevoegen aan `administratieve_eisen_voldaan` of verwijderen. Kan nu.

### Delegatieregisters BW (beschermingsbewind, curatele, executeur, gezag, handelingsonbekwaamheid, mentorschap, volmacht) en Faillissementswet (curator, wsnp)

Oordeel

- Lege-string-tak. Alle negen bestanden filteren "nog niet beëindigd" met `OR(EQUALS datum_einde null, EQUALS datum_einde '', GREATER_THAN datum_einde $referencedate)`. De `''`-tak bestaat omdat scenario-JSON `"datum_einde":""` schrijft terwijl profiles.yaml `null` gebruikt; RFC-036 kent alleen null. Categorie: data. Getrouwe vorm: `''`-tak verwijderen, scenario-fixtures naar null (`apply_absent_semantics.mjs` normaliseert velden in JSON-arraycellen niet). Kan nu. Onderliggend schemagat: `nullable` kan niet op een veld binnen een array-item worden gedeclareerd (RFC-kandidaat).
- Curator en wsnp: `voldoet_aan_voorwaarden` telt alle registraties, ook beëindigde (scenario "opgeheven faillissement" asserteert `voldoet_aan_voorwaarden is true`). Categorie: poc. Getrouwe vorm: tellen over `$actieve_faillissementen` resp. `$actieve_wsnp`, met legal_basis art. 68 jo. 193 resp. 316 jo. 356. Kan nu.
- Curator `subject_types`: `default: CITIZEN` voor alles wat niet `RECHTSPERSOON` is. Categorie: engine. Getrouwe vorm: twee expliciete cases zonder default. Kan nu.

Rest

- Verouderde untranslatable "FOREACH niet beschikbaar in v0.5.1" met `accepted: true` in alle bestanden die FOREACH inmiddels gebruiken (zie de lijst bovenaan). Verwijderen. Kan nu.
- Niet-gemodelleerde leden zonder vermelding: bewind 431 lid 2 t/m 5; curatele 378 lid 1 (materiële gronden), lid 2, lid 3; executeur 144 lid 2 (beloning 1 procent, wel berekenbaar) en lid 3; gezag 245 lid 5; handelingsonbekwaamheid 381 lid 3, 4, 5, 6; mentorschap 450 lid 1 (parameter heet `bsn` maar is `bsn_mentor`); curator 68 lid 2 t/m 4; wsnp 316 lid 2. Getrouwe vorm: untranslatable of commentaar dat het register de rechterlijke uitkomst levert, niet de gronden. Kan nu.
- Volmacht en executeur: de rechtensets (LEZEN, CLAIMS_INDIENEN, BESLUITEN_ONTVANGEN) zijn een portaalmodel met legal_basis art. 3:62 resp. 4:145; `subject_types: CITIZEN` met legal_basis art. 3:60. Legal_basis weglaten of untranslatable. Kan nu.
- Volmacht: art. 61, 62, 72 als legal_basis zonder tekst in het bestand. Kan nu.

### BW minderjarigheid RvIG, art. 1:234 (`corpus/demo/regulation/nl/burgerlijk_wetboek_minderjarigheid/RvIG-2024-01-01.yaml`)

Rest

- Zeven SELF-delegatie-outputs met legal_basis art. 234, dat niets over delegatie zegt. Legal_basis weglaten, commentaar "technisch contract delegation-provider". Kan nu.
- Lid 3 (veronderstelde toestemming) en "voor zover de wet niet anders bepaalt" niet gemarkeerd. Untranslatable. Kan nu.

### Handelsregisterwet KVK, art. 2/7/9 (`corpus/demo/regulation/nl/handelsregisterwet/KVK-2024-01-01.yaml`)

Uitkomst voor de burger

- `kvk_nummer` (type string) is een FOREACH zonder filter en zonder combine: levert een array van alle nummers, ook van beëindigde inschrijvingen. Categorie: schema (validate toetst het resultaattype niet). Getrouwe vorm: filter op actieve ondernemersvorm en status (dezelfde als `is_actieve_ondernemer`) en het outputtype op de uitkomst afstemmen (array, of een scalar via een enkelwaardige combinatie). Kan nu.

Rest

- Artikelblok 2 draagt logica van art. 7 en 9. Aparte blokken. Kan nu.

### Handelsregisterwet bedrijfsgegevens, art. 14 (`corpus/demo/regulation/nl/handelsregisterwet/bedrijfsgegevens/KVK-2024-01-01.yaml`)

Rest

- Lid 1 noemt nummer, naam, post- en bezoekadres, datum ingebruikname of beëindiging; de outputs zijn aantal_werknemers, status, rechtsvorm, datum_aanvang, vestigingsadres. Lid 2 (hoofdvestiging) en de scope (vestiging zonder onderneming) ontbreken. `produces: TOEKENNING` voor een registratieplicht. Getrouwe vorm: outputs hernoemen naar de tekst, `is_hoofdvestiging` nullable, scope-guard, ander legal_character. Kan nu.

### Handelsregisterwet jaarrekening, art. 2:394 (`corpus/demo/regulation/nl/handelsregisterwet/jaarrekening/KVK-2024-01-01.yaml`)

Uitkomst voor de burger

- Lid 3: twaalf maanden na afloop van het boekjaar. YAML: `volgende_deadline: $laatste_boekjaar_einde`, `deponerings_termijn_maanden: 12` ongebruikt. Categorie: schema. Getrouwe vorm: boekjaareinde plus twaalf maanden. Kan nu.

Oordeel

- `heeft_deponeringsplicht: value: true`, ook voor rechtsvormen waarvoor `voldoet_aan_voorwaarden` false is. Getrouwe vorm: `value: $voldoet_aan_voorwaarden`. Kan nu.
- `reden_vrijstelling: ''` met legal_basis art. 360; lid 5 (ontheffing) en lid 8 (AFM) worden nooit getoetst. Getrouwe vorm: IF op `ontheffing_verleend`/`afm_toezending_gedaan`, of untranslatable met lid 5/8. Kan nu.

Rest

- Lid 2, 4, 6, 7 niet gemodelleerd, niet gemarkeerd. Kan nu als vermelding.

### Kernenergiewet ANVS, art. 15/15b (`corpus/demo/regulation/nl/kernenergiewet/ANVS-2024-07-01.yaml`)

Oordeel

- Lid 1 onder c (beveiliging): `weigeringsgrond_beveiliging` is gedefinieerd, het besluit levert `beveiligingsplan_voldoet`, maar `weigeringsgronden` gebruikt het niet. Categorie: scenario. Getrouwe vorm: extra `when` op een input gebonden aan `besluit_kerninstallaties.beveiligingsplan_voldoet` (en `noodplan_voldoet`). Kan nu.
- Lid 1 onder a: de wet hardcodeert `dosis_limiet_bevolking: 1.0` en vergelijkt zelf, terwijl het besluit (art. 3.7) al `dosislimiet_overschreden` levert met drie limieten. Getrouwe vorm: die output afnemen. Kan nu.
- Lid 2 (verouderde techniek): parameters `is_nieuwe_installatie`, `technologie_beschrijving` en de definitie bestaan, geen actie gebruikt ze. Getrouwe vorm: case met het discretionaire oordeel als open term/untranslatable. Kan nu.

Rest

- Gronden b en e gedefinieerd zonder bron; untranslatable. `voldoet_aan_voorwaarden` zonder legal_basis en ongebruikt. Kan nu.

### Kieswet KIESRAAD, art. B1 (`corpus/demo/regulation/nl/kieswet/KIESRAAD-2024-01-01.yaml`)

Oordeel

- Lid 1 (uitzondering werkelijke woonplaats Aruba, Curaçao, Sint Maarten) en lid 2 (tien jaar ingezetene, openbare dienst en gezinsleden) ontbreken; wie in Aruba woont krijgt `heeft_stemrecht: true`. Categorie: scenario. Getrouwe vorm: input werkelijke woonplaats plus de lid-2-uitzonderingen als OR; zonder brondata untranslatable. Kan nu, vergt brondata.

Rest

- `gerechtelijke_uitsluiting` onder legal_basis B1, terwijl dat B3 is; B3 staat niet in het bestand. Kan nu.

### Machtigingenwet KVK, art. 2:240 (`corpus/demo/regulation/nl/machtigingenwet/KVK-2024-01-01.yaml`)

Oordeel

- `bevoegde_functies` mengt bestuurders (lid 2), gemachtigden (lid 4), een VOORZITTER zonder grondslag, en vennoten/maten (personenvennootschappen, buiten Boek 2) onder legal_basis lid 2. Categorie: data. Getrouwe vorm: splitsen per lid met eigen paragraph, VOORZITTER en `eigenaar_functies` verwijderen of apart onderbouwen. Kan nu.

Rest

- Lid 3 (derdenbescherming) en "voor zover uit de wet niet anders voortvloeit" niet vermeld. `valid_until_dates` heet null bij actieve functionaris maar is niet nullable; per-element-nullability in arrays bestaat niet (RFC-kandidaat). Verouderde FOREACH-untranslatable. Kan nu (vermeldingen).

### Omgevingswet energiebesparing informatieplicht RVO, art. 5.15/5.15a/5.15d (`corpus/demo/regulation/nl/omgevingswet/energiebesparing/informatieplicht/RVO-2024-01-01.yaml`)

Uitkomst voor de burger

- `volgende_deadline` is de vaste datum 2027-12-01; na die datum stilzwijgend fout. Categorie: data. Getrouwe vorm: berekenen uit basisdatum plus `rapportage_frequentie_jaren`, of de vaste datum in de tekst onderbouwen. Kan nu.

Oordeel

- `is_woonfunctie` als losse `source: {}` naast een ongebruikte cross-law keten (KVK adres, BAG gebruiksdoel) die dezelfde vraag beantwoordt. Categorie: scenario. Getrouwe vorm: `wet_bag.is_woonfunctie` afnemen, losse input weg. Kan nu.

Rest

- Alleen de overgangsbepaling van 5.15d staat als tekst; drempels, frequentie en methode zijn niet tegen tekst te toetsen. Artikelen opnemen. Kan nu.

### Omgevingswet werkgebonden personenmobiliteit RVO, art. 18.11 (`corpus/demo/regulation/nl/omgevingswet/werkgebonden_personenmobiliteit/RVO-2024-07-01.yaml`)

Oordeel

- `rapportageverplichting: value: true`, los van `voldoet_aan_voorwaarden` (organisatie met 40 werknemers krijgt toch een plicht). Categorie: scenario. Getrouwe vorm: `value: $voldoet_aan_voorwaarden`. Kan nu.

Rest

- Lid 2 onder a, e, f, g, h zijn ritclassificaties, niet ondernemingskenmerken; untranslatable. Lid 2 onder c en d (vestigingsland) samengevoegd tot een werknemersaantal; vergt aparte data. Kan nu als vermelding.

### WPM gegevens RVO, art. 18.14 (`corpus/demo/regulation/nl/omgevingswet/werkgebonden_personenmobiliteit/gegevens/RVO-2024-07-01.yaml`)

Uitkomst voor de burger

- Lid 1 (emissie per reizigerskilometer, gescheiden voor woon-werk en zakelijk) niet gemodelleerd. Categorie: scenario. Getrouwe vorm: twee DIVIDE-outputs. Kan nu.

Rest

- `voldoet_aan_voorwaarden = AND(EQUALS(true, true))`: tautologie zonder tekstgrond. Verwijderen. Kan nu.
- Emissiefactoren "bij ministeriële regeling vastgesteld" als vaste definitions. open_terms met default. Kan nu.

### Participatiewet AIO SVB, art. 47a (`corpus/demo/regulation/nl/participatiewet/aio/SVB-2026-01-01.yaml`)

Uitkomst voor de burger

- `sociaal_minimum` citeert art. 21 (normen tot pensioengerechtigde leeftijd) terwijl AIO-gerechtigden onder art. 22 (normen pensioengerechtigden) vallen; `norm_alleenstaand: 138000` en `norm_gehuwden: 197000` zijn niet de per 2026-01-01 geldende art. 22-bedragen. Categorie: data. Getrouwe vorm: `article: '22'`, bedragen conform de geldende tekst (de bevinding noemt 156469 en 214416; nalezen op wetten.overheid.nl, de tekst van art. 22 staat niet in het bestand). Kan nu.
- `totaal_inkomen = aow + (inkomen/12 - aow)` is algebraïsch `inkomen/12`; de AOW-term doet niets. Categorie: poc. Getrouwe vorm: als bedoeld is "maandinkomen plus AOW": `aow + inkomen/12`; de vrijlating van art. 33 lid 5 (26,50 / 53,00 per maand) apart. Kan nu.

Rest

- URL-ankers `Hoofdstuk3_Paragraaf3.4_Artikel47a`; art. 47a staat in hoofdstuk 5, paragraaf 5.4. Kan nu.

### Participatiewet bijstand SZW, art. 11/20/22a (`corpus/demo/regulation/nl/participatiewet/bijstand/SZW-2023-01-01.yaml`)

Uitkomst voor de burger

- Kostendelersnorm: de IF-keten herhaalt 1.0/0.5/0.43/0.4 als literals en heeft `default: 0.38` voor vijf of meer, een waarde die niet in `kostendelersnorm_factoren` staat en niet met de bekende reeks strookt. Categorie: data. Getrouwe vorm: de definitie gebruiken en de factor voor vijf of meer nalezen in art. 22a lid 1 (tekst niet in het bestand). Kan nu.

Oordeel

- Lid 4: recht komt echtgenoten gezamenlijk toe, tenzij een van hen geen recht heeft. Partnerinkomen telt mee, maar of de partner zelf aan lid 1 t/m 3 voldoet wordt niet getoetst. Categorie: scenario. Getrouwe vorm: dit artikel voor `$partner_bsn` aanroepen (cross-law op zichzelf) en meenemen in `voldoet_aan_voorwaarden`. Kan nu.
- Lid 3 (AMvB-gelijkstelling) als open_term met lege default. Kan nu.

Rest

- `voldoet_aan_voorwaarden` zonder legal_basis terwijl het criteria uit art. 7, 9, 11, 13, 31/32, 34 samenvoegt; art. 20 en 22a zonder tekst. Kan nu.

### Participatiewet bijstand Amsterdam, art. 11 (`corpus/demo/regulation/nl/participatiewet/bijstand/gemeenten/GEMEENTE_AMSTERDAM-2023-01-01.yaml`)

Oordeel

- `is_gerechtigd: true` ongeconditioneerd, terwijl `voldoet_aan_landelijke_voorwaarden` als input klaarligt. Categorie: scenario. Getrouwe vorm: `value: $voldoet_aan_landelijke_voorwaarden`. Kan nu.

Rest

- Lid 4, lid 2/3 zonder vermelding; de arbeidsverplichting/ontheffing-guard hoort bij art. 9, niet 11. Kan nu als documentatie.

### Penitentiaire beginselenwet DJI, art. 2 (`corpus/demo/regulation/nl/penitentiaire_beginselenwet/DJI-2022-01-01.yaml`)

Rest

- "dan wel door diens deelname aan een penitentiair programma" niet gemodelleerd; vergt een registerveld dat er niet is. Explanation noemt art. 3 als definitiebron, ten onrechte. Kan nu (explanation).

### Pensioenwet PENSIOENFONDS (`corpus/demo/regulation/nl/pensioenwet/PENSIOENFONDS-2026-01-01.yaml`)

Rest

- Opbouwpercentages 1,75/1,67 en omrekenfactor 0,05 met legal_basis art. 10 (een definitiebepaling); dat is reglement, geen wet. open_terms gedelegeerd aan het reglement of expliciet als demo-parameter. Alleen art. 1 staat als tekst. `pensioenkapitaal` met legal_basis art. 51 (informatieplicht). Kan nu (documentatie en legal_basis).
- De toelichting bij `voldoet_aan_voorwaarden` presenteert "geen registratie is geen recht" als wettelijke voorwaarde; het is een modelleerkeuze. Kan nu.

### UWV toetsingsinkomen en werkgegevens, art. 54 Wet SUWI (`corpus/demo/regulation/nl/uwv_toetsingsinkomen/UWV-2025-01-01.yaml`, `corpus/demo/regulation/nl/uwv_werkgegevens/UWV-2025-01-01.yaml`)

Rest

- Beide bestanden zijn registerwrappers opgehangen aan een gegevensverstrekkingsplicht die niets over toetsingsinkomen of werkgegevens zegt; `produces: BESCHIKKING/TOEKENNING`. Getrouwe vorm: als wrapper documenteren, ander legal_character. Kan nu als documentatie; het juiste artikel vinden niet.

### Verordening precariobelasting Rotterdam, art. 2 (`corpus/demo/regulation/nl/verordening_precariobelasting/gemeenten/GEMEENTE_ROTTERDAM-2024-01-01.yaml`)

Oordeel

- `is_belastingplichtig: value: true` (regel 105), ook zonder terras of vergunning; de scenario's testen alleen `voldoet_aan_voorwaarden`. Categorie: schema. Getrouwe vorm: `value: $voldoet_aan_voorwaarden`. Kan nu.

Rest

- Lid 2 (reclamebelasting) buiten scope zonder vermelding; tarief 36,10 en drempel 50 m2 uit CVDR704478 art. 5 zonder tekst. Kan nu.

### Vreemdelingenwet IND, art. 8 (`corpus/demo/regulation/nl/vreemdelingenwet/IND-2024-01-01.yaml`)

Rest

- Gronden f t/m m ontbreken, b en d zijn samengevoegd zonder de aantekening van art. 45c. Vergt registervelden; untranslatable per grond. Kan nu als vermelding.

### Warenwet HACCP NVWA, art. 2/3 (`corpus/demo/regulation/nl/warenwet/haccp/NVWA-2024-01-01.yaml`)

Rest

- Art. 2 is een verbod met verwijzing naar EU-verordeningen; de gemodelleerde begrippen (levensmiddelenbedrijf via SBI, registratie, HACCP-systeem) staan er niet in; art. 3 wordt geciteerd zonder tekst; `voldoet_aan_voorwaarden` zonder legal_basis. Untranslatable of de EU-artikelen opnemen. Kan nu als documentatie.

### Warenwet meldplicht NVWA, art. 21 (`corpus/demo/regulation/nl/warenwet/meldplicht/NVWA-2024-01-01.yaml`)

Oordeel

- `heeft_meldplicht: value: true` zodra levensmiddelenbedrijf, terwijl de eigen `is_event_driven`-uitleg zegt dat de plicht pas bij een incident ontstaat en lid 1 een ministeriële last na een gevaaroordeel vereist. Categorie: scenario. Getrouwe vorm: minimaal `AND(is_levensmiddelenbedrijf, heeft_actief_incident)` met null-guard; volledig getrouw is een last-object. Kan nu (de minimale vorm).

Rest

- `melding_deadline_uren: 4` kwantificeert "onverwijld" zonder bron; `rapportage_methode` (NVWA-meldformulier) en `is_event_driven` zijn systeemfeiten met legal_basis art. 21. Untranslatable of verwijderen. Lid 2 en 3 niet gemodelleerd. Kan nu.

### Werkloosheidswet UWV, art. 16 (`corpus/demo/regulation/nl/werkloosheidswet/UWV-2025-01-01.yaml`)

Oordeel

- Lid 1 onder b (beschikbaar om arbeid te aanvaarden) ontbreekt volledig; de overige voorwaarden (nationaliteit, verblijf, ZW/WIA, detentie) staan niet in art. 16. Categorie: scenario. Getrouwe vorm: input `beschikbaar_om_arbeid_te_aanvaarden` als AND-conditie met eigen legal_basis; de andere voorwaarden hun eigen artikel. Kan nu, vergt een registerveld.

Rest

- Lid 2 t/m 8 (26-wekenberekening, scholing, opzegtermijn, WIA/ZW-correctie) niet gemodelleerd; dagloon/percentage/duur citeren art. 1b, 47, 42 zonder tekst. Kan nu als vermelding.

### Wet adviescollege ICT-toetsing ACICT, art. 1 (`corpus/demo/regulation/nl/wet_adviescollege_ict_toetsing/ACICT-2024-07-01.yaml`)

Rest

- Beslislogica citeert art. 7 en 14 die niet in het bestand staan; `project_kosten` "voor transparantie" met legal_basis art. 1. Artikelen opnemen, legal_basis herschrijven. Kan nu.

### Wet BAG Kadaster, art. 1/2 (`corpus/demo/regulation/nl/wet_bag/KADASTER-2018-07-28.yaml`)

Rest

- gebruiksdoel/oppervlakte/status met legal_basis art. 1 (definities); `is_woonfunctie` citeert art. 2 met een omschrijving van art. 21; de gebruiksfunctie-taxonomie komt uit het Bouwbesluit. Kan nu (legal_basis 21, commentaar met bron); de Catalogus BAG als bron niet.

### Wet Bibob LBB, art. 9 (`corpus/demo/regulation/nl/wet_bibob/LBB-2024-01-01.yaml`)

Rest

- Kernoutputs (mate van gevaar, weigering, voorschriften) hangen aan art. 3, dat niet in het bestand staat; `beschikking_type`, `bsn`, `adviestermijn_weken` ongebruikt; lid 5 niet vermeld. Art. 3 opnemen, dode elementen weg of gebruiken. Kan nu.

### Wet BRP RvIG (`corpus/demo/regulation/nl/wet_brp/RvIG-2020-01-01.yaml`)

Oordeel

- `heeft_kinderen_onder_12` telt alleen of er kinderen zijn; `kind_max_leeftijd_combinatiekorting: 12` wordt nergens gebruikt, terwijl `kinderen_gegevens` een geboortedatum draagt. Categorie: scenario. Getrouwe vorm: FOREACH met filter `AGE(referentiedatum, geboortedatum) < 12`, `combine: ADD`, `> 0`. Kan nu.

Rest

- `address_match_distance_m` ongebruikt (exacte adresmatch); `verblijfsadres`, `huishoudgrootte`, `postadres` zonder legal_basis. Kan nu.

### Wet BRP LAA RvIG, art. 28a (`corpus/demo/regulation/nl/wet_brp/laa/RvIG-2023-05-15.yaml`)

Rest

- Art. 28a (opsomming analysemethoden) draagt signaallogica en termijnen met legal_basis 28f en 28g; drempel `hoog_bewoners_drempel: 5` zonder bron. Splitsen per artikel met tekst; drempel onderbouwen. Kan nu.

### Wet BRP terugmelding Belastingdienst, art. 2.34 (`corpus/demo/regulation/nl/wet_brp/terugmelding/BELASTINGDIENST-2023-05-15.yaml`)

Oordeel

- `EQUALS $belasting_adres null THEN true`: geen Belastingdienst-adres telt als gerede twijfel over het BRP-adres. Twijfel vereist een aanwijzing van onjuistheid, niet het ontbreken van vergelijkingsmateriaal; CJIB en Toeslagen doen het andersom. Het commentaar rechtvaardigt het met "zoals in de POC". Categorie: scenario. Getrouwe vorm: bij `null` geen twijfel (false), of unknown laten propageren; alleen een afwijkend adres geeft twijfel. Kan nu; het LAA-scenario "Belastingdienst meldt twijfel" verandert dan.

### Wet BRP terugmelding CJIB en Toeslagen

Rest

- CJIB: `obr_drempel_herhaald: 2` met legal_basis art. 28 Besluit BRP zonder tekst; niet te verifiëren. Toeslagen: de invulling van "gerede twijfel" is uitvoeringsbeleid, niet gemarkeerd; leden 2 t/m 5 buiten scope zonder vermelding. Commentaar. Kan nu.

### Wet forensische zorg DJI, art. 1.1 (`corpus/demo/regulation/nl/wet_forensische_zorg/DJI-2022-01-01.yaml`)

Rest

- `bwb_id: BWBR0040635` (dat is de Wvggz; de Wfz is BWBR0040634) op bestandsniveau en in legal_basis; legal_basis wijst naar art. 2.1 met een explanation die bij art. 1.1 lid 2 past. `geldige_juridische_titels` zijn DJI-codes die niet op de gronden van lid 2 (sepot, schorsing, gratie, strafbeschikking) aansluiten. Categorie: data. Getrouwe vorm: id en artikel corrigeren; de codering als untranslatable-vertaalslag benoemen. Kan nu (metadata); de codering vergt registerkennis.

### Wet inkomstenbelasting Belastingdienst (`corpus/demo/regulation/nl/wet_inkomstenbelasting/BELASTINGDIENST-2001-01-01.yaml`)

Uitkomst voor de burger

- Box 3 bij partner: `box3_bezittingen` trekt al de dubbele heffingsvrije voet (115.458) af, en `partner_box3_inkomen` trekt op het partnervermogen nog eens `heffingsvrij_vermogen` (57.200, een derde constante) af. Dubbele vrijstelling. Categorie: data. Getrouwe vorm: de tweede aftrek verwijderen, `heffingsvrij_vermogen` weg. Kan nu.

Rest

- Alle logica (box 1/2/3, kortingen, partnertoerekening) onder art. 2.1 (belastingplicht) zonder de onderliggende artikelen; `min_personal_income` dood; peildatum AOW-leeftijd niet toetsbaar. Kan nu (artikelen opnemen, dode definitie weg).

### Wet inkomstenbelasting UWV toetsingsinkomen, art. 2.11 (`corpus/demo/regulation/nl/wet_inkomstenbelasting/UWV-2020-01-01.yaml`)

Rest

- Volledige logica hangt aan een vervallen artikel ("Vervallen"). Verplaatsen naar Awir art. 7/8 of expliciet als hulpbestand markeren. Kan nu.
- bindings.yaml zet `absent: 0` voor `buitenlands_inkomen` en `partner_buitenlands_inkomen`, dus de `nullable: true` plus null-guard in de wet is dode code in de demo. Categorie: data. Bindings naar null of documenteren. Kan nu.

### Wet kinderopvang Toeslagen, art. 1.5 (`corpus/demo/regulation/nl/wet_kinderopvang/TOESLAGEN-2024-01-01.yaml`)

Oordeel

- Lid 1: "geregistreerd kindercentrum" (LRK) wordt niet getoetst; `kinderopvang_kvk` is alleen een KvK-nummer. Categorie: scenario. Getrouwe vorm: input `is_geregistreerd` als voorwaarde, of untranslatable "geen LRK-bron". Kan nu, vergt data.
- Lid 2 (ouderparticipatiecrèche in de aanloopperiode): geen aanspraak. Ontbreekt. Getrouwe vorm: AND-voorwaarde op `soort_opvang` en `binnen_aanloopperiode`. Kan nu, vergt data.
- `gewerkte_uren` niet nullable met directe `GREATER_THAN 0`, terwijl `partner_gewerkte_uren` wel nullable is met guard. Categorie: schema. Getrouwe vorm: dezelfde guard, of documenteren waarom eigen uren altijd aanwezig zijn. Kan nu.

Rest

- Partnervoorwaarde zonder legal_basis (Besluit kinderopvangtoeslag); legal_basis per rekenstap in `jaarbedrag` verdwenen; verouderde FOREACH-untranslatable. Kan nu.

### Wet op de huurtoeslag Toeslagen (`corpus/demo/regulation/nl/wet_op_de_huurtoeslag/TOESLAGEN-2025-01-01.yaml`)

Uitkomst voor de burger

- Art. 19 lid 3: normhuur naar boven afronden op hele eurocenten. YAML rondt niet af; `CEIL` bestaat in het schema. Categorie: schema. Getrouwe vorm: `CEIL` met `precision: 0` om de default-tak. Kan nu.
- Art. 19 lid 2: kwadratische normhuurformule `a*Y^2 + b*Y`. YAML: lineaire interpolatie tussen twee ankerpunten. Categorie: scenario. Getrouwe vorm: open_terms a en b en een kwadraat. Kan nu niet: geen POWER-operatie (RFC-kandidaat; MULTIPLY(Y, Y) is een omweg die het commentaar dan moet benoemen).
- Art. 13 jo. 5: rekenhuur met per-categorie gemaximeerde servicekosten; YAML gebruikt een totaalplafond. Vergt per-categorie parameters. Kan nu niet zonder die data.

Rest

- Art. 21 lid 1 onder b/c: percentages "bij AMvB vast te stellen" als vaste definitions. open_terms. Kan nu.
- Leeftijdstoets (art. 8) onder legal_basis art. 7; `kind_vrijstelling`, `leeftijdsgrens_kind_inkomen` dood; alleen art. 1 als tekst. Kan nu.

### Wet op het CBS, art. 3 (`corpus/demo/regulation/nl/wet_op_het_centraal_bureau_voor_de_statistiek/CBS-2024-01-01.yaml`)

Rest

- Art. 3 (taakstelling) draagt een BSN-geparametriseerde BESCHIKKING voor `levensverwachting_65`; de grootheid is een macrocijfer per jaar (AOW art. 7a lid 2/4), bindings selecteren al op `jaar`. Categorie: engine (cross-law parameterconventie). Getrouwe vorm: `bsn` niet verplicht, AOW geeft hem niet door, legal_character als gegevenslevering. Kan nu.

### Wet op het kindgebonden budget Toeslagen, art. 2 (`corpus/demo/regulation/nl/wet_op_het_kindgebonden_budget/TOESLAGEN-2025-01-01.yaml`)

Uitkomst voor de burger

- Lid 6: tekst (in het bestand en op wetten.overheid.nl) zegt 3.389 euro; `alo_kop: 348000` (3.480 euro) en de explanation herhaalt 3.480. Categorie: data. Getrouwe vorm: `alo_kop: 338900`. Kan nu.
- Lid 12, 13, 14 (woonlandfactor bij kind buiten Nederland/EU, uitzondering bij drie maanden verblijf): niet gemodelleerd. Categorie: scenario. Getrouwe vorm: open_term `woonlandpercentage` (default 100) per kind, woonland per kind als input, lid 14 als guard. Kan nu, vergt woonland per kind.

Rest

- Commentaar citeert lid 1 voor de partnerinkomenstelling (lid 7/8); verouderde FOREACH-untranslatable; lid 9/10 via AKW verondersteld zonder vermelding. Kan nu.

### Wet SUWI UWV, art. 33 (`corpus/demo/regulation/nl/wet_structuur_uitvoeringsorganisatie_werk_en_inkomen/UWV-2024-01-01.yaml`)

Rest

- Art. 33 regelt de polisadministratie; de berekening van verzekerde jaren en gewerkte uren heeft er geen grondslag. De peildatum-substitutie bij lopende periodes is een engine-noodzaak (DATE_DIFF eist een datum), de null-toets zelf is correct RFC-036. De `''`-tak op `end_date` (zie delegatieregisters). `nullable` op een veld binnen een array-item niet declareerbaar (RFC-kandidaat). Verouderde FOREACH-untranslatable. Kan nu: `''`-tak weg, untranslatable weg; de grondslag niet.

### Wet studiefinanciering DUO, art. 3.1 (`corpus/demo/regulation/nl/wet_studiefinanciering/DUO-2024-01-01.yaml`)

Rest

- Bedragen, inkomensgrens en afbouw hangen aan art. 3.1 (opsomming van vormen); het commentaar noemt zelf art. 3.9. Lid 1/2 (gift/prestatiebeurs/lening), lid 3 (levenlanglerenkrediet) en lid 5 niet gemodelleerd. Art. 3.9 opnemen; untranslatables. Kan nu (legal_basis); lid 3 niet zonder aanspraakvoorwaarden.

### WIA UWV, art. 54 (`corpus/demo/regulation/nl/wet_werk_en_inkomen_naar_arbeidsvermogen/UWV-2025-01-01.yaml`)

Oordeel

- Lid 1 (wachttijd, gedeeltelijk arbeidsgeschikt, geen uitsluitingsgrond) is vervangen door `status == ACTIEF` uit een register. Categorie: scenario. Getrouwe vorm: drie inputs met AND, of untranslatables per criterium met een commentaar dat het bestand een registerstatus doorgeeft. Kan nu als vermelding; de criteria vergen data.
- Lid 2 (ingangsdatum), lid 3/4 (uitkeringsvorm via referte-eis art. 58) ontbreken. `decision_type: TOEKENNING` vast, ook bij false. Kan nu als vermelding.

### Wetboek van Strafrecht JUSTID, art. 28 (`corpus/demo/regulation/nl/wetboek_van_strafrecht/JUSTID-2023-01-01.yaml`)

Rest

- De geldigheidstoets (start/einddatum) staat in art. 31, niet 28; de POC-md had die legal_basis per tak. Verouderde FOREACH-untranslatable. bindings-entry `stemrecht_uitsluitingen` zonder `absent:`. Categorie: data. Getrouwe vorm: legal_basis per operand, `absent` toevoegen. Kan nu.

### WGBO vertegenwoordiger RvIG, art. 7:465 (`corpus/demo/regulation/nl/wgbo_vertegenwoordiger/RvIG-2024-01-01.yaml`)

Oordeel

- Lid 3 rangorde: schriftelijk gemachtigde gaat voor, dan partnercategorie, dan familiecategorie, elk met "tenzij deze persoon dat niet wenst". YAML: een platte IN-lijst zonder gemachtigde, zonder rangorde, zonder wens-vlag; meerdere gelijktijdige vertegenwoordigers mogelijk. Categorie: engine. Getrouwe vorm: nullable input `gemachtigde`, dan twee FOREACH-stappen met prioriteit en `wenst_niet_op_te_treden`. Kan nu.
- Lid 3: GROOTOUDER en KLEINKIND ontbreken in `wgbo_relatie_hierarchie`. Categorie: data. Toevoegen, met eigen delegation_types. Kan nu.
- Lid 1 (patiënt onder 12: ouders met gezag of voogd) en lid 2 (curator/mentor bij curatele/mentorschap) ontbreken; de BW-registers bestaan in het corpus maar worden niet aangeroepen. Getrouwe vorm: leeftijd-input en cross-law naar curatele/mentorschap. Kan nu.

Rest

- Lid 4, 5, 6 (goed hulpverlener, goed vertegenwoordiger, verzetsrecht) zijn beoordelingsnormen; untranslatable. `permissions` onvoorwaardelijk als gevolg. Kan nu als vermelding.

### Ziektewet UWV, art. 29 (`corpus/demo/regulation/nl/ziektewet/UWV-2025-01-01.yaml`)

Rest

- Veertien leden, geen enkele gemodelleerd; een boolean uit een register wordt doorgegeven. Commentaar dat dit een registerstatus is. Kan nu.

### Zorgtoeslagwet Toeslagen 2024 (`corpus/demo/regulation/nl/zorgtoeslagwet/TOESLAGEN-2024-01-01.yaml`)

Uitkomst voor de burger

- `percentage_drempelinkomen_met_partner` en `_alleenstaand` zijn beide 0.0486; art. 2 lid 3 (2024) kent 4,256 en 1,879 procent, zoals `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2024-01-01.yaml` al doet. Categorie: data. Getrouwe vorm: 0.04256 en 0.01879. Kan nu.

Oordeel

- Vermogensgrens alleen in `hoogte_toeslag` (bedrag 0), niet in `voldoet_aan_voorwaarden`: "voldoet, bedrag nul" in plaats van "geen aanspraak". Getrouwe vorm: vermogenstoets als eigen conditie. Kan nu.
- `is_verzekerde_zorgtoeslag: value: true` in 2024 en 2025, terwijl de BDD-conversie het als recht-op-zorgtoeslag-assertie gebruikt. Getrouwe vorm: `value: $voldoet_aan_voorwaarden`. Kan nu.

Rest

- Alle logica onder art. 1 (definities); 2025 en de referentiecorpus gebruiken art. 2/3. Kan nu.

### Zorgtoeslagwet Toeslagen 2025 (`corpus/demo/regulation/nl/zorgtoeslagwet/TOESLAGEN-2025-01-01.yaml`)

Uitkomst voor de burger

- Lid 4: partner die geen verzekerde is geeft vijftig procent. Ontbreekt, geen input `partner_is_verzekerde`. Categorie: scenario. Getrouwe vorm: IF op `heeft_partner AND NOT partner_is_verzekerde` dan MULTIPLY 0.5. Kan nu, vergt de partnerstatus uit zvw.

Rest

- `drempelinkomen_*` is de afbouwgrens, niet het wettelijke drempelinkomen; de vermogenstoets hoort bij art. 2a, niet art. 2 lid 1. Naamgeving en legal_basis. Lid 5 (per kalendermaand) niet als legal_basis bij de temporal-declaratie. Regeling standaardpremie: alleen art. 1, de bestuursrechtelijke premies niet vermeld. Kan nu.

### Zorgverzekeringswet bijdrage Belastingdienst, art. 41 t/m 45 (`corpus/demo/regulation/nl/zorgverzekeringswet/bijdrage/BELASTINGDIENST-2026-01-01.yaml`)

Uitkomst voor de burger

- `bijdrage_inkomen = $inkomen_loon` (= `box1_inkomen` uit de IB, inclusief eigenwoningforfait). Art. 43 lid 1 kent vier bestanddelen (loon, winst, resultaat overige werkzaamheden, periodieke uitkeringen), geen eigen woning. legal_basis art. 42 in plaats van 43 lid 1. Categorie: schema. Getrouwe vorm: de vier IB-componenten optellen. Kan nu.

Oordeel

- `voldoet_aan_voorwaarden = inkomen_loon > 0` zonder legal_basis; art. 41 stelt de bijdrageplicht onvoorwaardelijk. Getrouwe vorm: als procesveld documenteren of verwijderen. Kan nu.

Rest

- `text` van art. 41 voegt "De inhoudingsplichtige en" toe; de tekst noemt alleen de verzekeringsplichtige. Categorie: data. Corrigeren. Kan nu.
- `is_aow_gerechtigd` met legal_basis art. 43 lid 2 (maximering); HOOG/LAAG en 6,5/5,25 procent zijn gedelegeerd (art. 45 lid 2/3), als vaste definitions met legal_basis art. 43. open_terms, legal_basis 45. Lid 3 en 4 niet vermeld. Kan nu.

### Zvw verzekeringsstatus RVZ, art. 1/2/9 (`corpus/demo/regulation/nl/zvw/RVZ-2024-01-01.yaml`)

Rest

- `heeft_verdragsverzekering` citeert art. 9 (zorgpolis); verdragsverzekering is art. 69. `polis_status`/`registratie`-waarden en de landenlijst zijn demo-abstracties zonder markering. legal_basis 69, commentaar. Kan nu.

### Zvw werkgeversbijdrage Belastingdienst, art. 42 (`corpus/demo/regulation/nl/zvw/werkgeversbijdrage/BELASTINGDIENST-2024-01-01.yaml`)

Rest

- Lid 2: maximum "door Onze Minister vastgesteld" als vaste definition zonder open_terms; legal_basis art. 43. Lid 1 (loonbegrip met uitzonderingen), lid 3 t/m 5 (cumulatief per loontijdvak) en lid 6 niet vermeld. open_terms; untranslatables (tijdvakcumulatie vergt state). Kan nu (vermeldingen).

## Tabel per categorie

| Categorie | Aantal punten (na dedup) | Kern | Voorbeelden |
|---|---|---|---|
| schema | 16 | Declaraties die het schema toelaat maar die de tekst niet dekt, of die het schema niet kan uitdrukken | `is_gerechtigd: true` (Anw), `heeft_deponeringsplicht: true`, FOREACH zonder combine op een string-output, nullable op object-subvelden, TOEKENNING voor registerraadpleging, ontbrekende CEIL |
| engine | 6 | Gedrag dat door engine-conventies is ingegeven | default-tak als vangnet (curator CITIZEN), bsn verplicht voor een macrocijfer (CBS), plaatselijke gewoonte niet als implementatie, WGBO rangorde |
| checker | 1 | Grenzen voor de validator zonder tekstbasis | `type_spec 65..70` op pensioenleeftijd |
| scenario | 60 | Modellering gebogen naar wat het scenario of de POC meegaf, of leden weggelaten zonder vermelding | Anw AO-drempel 45, urencriterium in Bbz, terugmelding null is twijfel, Awb 7:1 gronden, Kieswet Caribisch gebied, ontbrekende leden zonder untranslatable |
| data | 22 | Registercontract of brondata die de norm vervormt | lege string als afwezigheid (9 bestanden), `absent: unknown` versus null-guards (terrassen), constante `false` voor levensgedrag, dubbele box-3-vrijstelling, ALO-kop 3.480, zorgtoeslagpercentages 2024, AIO art. 21/22 |
| poc | 17 | Restanten van de POC-migratie | verouderde FOREACH-untranslatable (13 bestanden), `voldoet_aan_voorwaarden` over alle registraties, AOW-formule en kortingsdeler, `default: 0.38`, dode definities, kapstok op vervallen art. 2.11 |
| onbekend | 6 | Grondslagverwijzing die niet bij de tekst hoort, zonder aanwijsbare technische reden | Archiefwet art. 12 vs 13 en Archiefbesluit 9 vs 11, twee-derde-criterium zonder bron, LAA-drempel 5 |

## Wat de taal nog niet kan (RFC-kandidaten)

1. Nullable op een veld binnen een array-item of een object-input. Negen delegatieregisters en SUWI testen `datum_einde`/`end_date` op null zonder dat het veld nullable kan worden gedeclareerd; awb beroep en bezwaar testen `$wet.*_weken` op null. De typechecker (RFC-037 N1) ziet deze velden niet. Vereist een item- of veldschema voor `type: array` en `type: object`.
2. Per-element-nullability in array-outputs. `valid_until_dates` (machtigingenwet, mentorschap, gezag) bevat null voor lopende relaties; `nullable` geldt alleen voor het veld als geheel.
3. Een gesloten waardenbereik (enum) voor string-parameters, zodat `beperking_type` (Archiefwet) of `type_rechter` (Awb) geen "reden onbekend"-vangnet nodig heeft.
4. Een machtsverheffing of kwadraat (`POWER`), nodig voor art. 19 lid 2 Wet op de huurtoeslag.
5. Een implementatiemechanisme voor ongeschreven recht: "plaatselijke gewoonte" (BW 5:42 lid 2) als afwijkingsgrond naast een verordening.
6. Een jaargebonden, landelijk vastgestelde grootheid met aankondigingstermijn (AOW-leeftijd, art. 7a lid 2/3), los van per-persoon-parameters; en, verwant, een cross-law aanroep zonder verplichte bsn voor macrocijfers (CBS).
7. Een uitkomstafhankelijk `decision_type` (TOEKENNING/AFWIJZING op basis van de output) of een neutraal `produces` voor registerwrappers (SVH, KVK bedrijfsgegevens, UWV, WIA).
8. Stateful cumulatie over tijdvakken (Zvw art. 42 lid 3 t/m 5), en een last- of beschikkingsobject als input voor bevoegdheidsnormen (Warenwet art. 21).
9. Normalisatie van afwezigheid in JSON-arraycellen van scenario-tabellen (`apply_absent_semantics.mjs` raakt alleen scalaire kolommen), zodat lege strings niet als tweede afwezigheidswaarde de wet binnenkomen.
10. Een cross-law aanroep van de eigen wet met andere parameters (art. 11 lid 4 Participatiewet: het recht van de echtgenoot via `source: {regulation: participatiewet/bijstand, output: voldoet_aan_voorwaarden, parameters: {bsn: $partner_bsn}}`). De cyclusdetectie sleutelt op `regulation#output` zonder parameters en ziet dit als kringverwijzing.
11. Een cross-law aanroep per FOREACH-item met een parameter uit het item zelf (WGBO art. 7:465 lid 1/2: per kandidaat-vertegenwoordiger curatele of mentorschap opvragen op zijn eigen bsn). `source.parameters` wordt een keer opgelost bij het binden van de input, niet per item.
12. Een keuze per groep in FOREACH (winnaar per patiënt): de rangorde van art. 7:465 lid 3 BW tussen een gemachtigde en een familielid van dezelfde patiënt vraagt group-by, dat de taal niet heeft.
13. Een opzoeking in een map-definitie (`kostendelersnorm_factoren: {'1': 1.0, ...}`); nu vier losse scalaire definities en een IF-keten (art. 22a lid 1 Participatiewet).

## Rechtgetrokken in deze PR

Controles na afloop van de tweede ronde: `validate` over beide corpora 114 bestanden OK (0 bevindingen); `yamllint` schoon; `just bdd` 18 features / 156 scenario's / 1236 stappen groen; `just bdd-demo` 49 features / 392 scenario's / 3140 stappen groen (0 overgeslagen); `frontend-demo` vitest 11 bestanden / 97 tests groen; `just wasm-build` slaagt. De eerste ronde stond op 137 en 331 scenario's.

De PR bestaat uit twee rondes. De eerste ronde trok de aantoonbare afwijkingen recht (hieronder onder "Eerste ronde"), de tweede ronde de punten uit de categorieën scenario, poc, data en onbekend die zonder nieuwe taalconstructen te doen waren ("Tweede ronde"). De diff over `corpus/` bevat precies een gewijzigde `text:`-regel, en dat is een toevoeging: artikel 13 Archiefwet (zie daar). Geen bestaand `text:`-veld is gewijzigd.

### Eerste ronde: corpus (bucket A)

- APV erfgrens Amsterdam, art. 2.75 lid 2: parameter `postcode` van `required: true` naar `required: false`. Lid 2 (heggen en heesters, heel Amsterdam) gebruikt de postcode niet; bij een afwezige postcode faalt in lid 1 alleen de centrum-voorwaarde (Kleene-AND), de uitkomst van lid 2 verandert niet.

### Eerste ronde: demo-corpus

- Alcoholwet vergunning VWS, art. 8 lid 1 onder b en lid 3/4: de takken `EQUALS … null -> true` uit `voldoet_aan_levensgedrag_eis` en `voldoet_aan_svh_eis` verwijderd. Een onbekend levensgedrag of een onbekende SVH-inschrijving is geen "voldoet".
- Alcoholwet Rotterdam, art. 8 lid 1 onder b: de constante `geen_slecht_levensgedrag: false` is weg. In plaats daarvan een ongevoed nullable input `levensgedrag_onbekend` (altijd null) dat als `is_van_slecht_levensgedrag` aan VWS wordt doorgegeven; de engine eist een expliciete binding voor een nullable parameter.
- Algemene nabestaandenwet, art. 14 lid 1: `is_gerechtigd` was constant `true`, is nu `$voldoet_aan_voorwaarden`.
- Algemene Ouderdomswet, art. 7 lid 1: `is_gerechtigd` was constant `true`, is nu `$voldoet_aan_voorwaarden` (leeftijd plus verzekeringsperiode).
- APV Rotterdam ontheffingspas geluid, art. 4:2: `komt_in_aanmerking_voor_geluidsontheffing` was constant `true`, is nu `$voldoet_aan_voorwaarden`.
- APV Rotterdam terrassen, art. 2:30b lid 1/2: in `bindings.yaml` gaan `beschikbare_oppervlakte`, `max_sluitingstijd_doordeweeks`, `max_sluitingstijd_weekend` en `tarief_per_m2` van `absent: unknown` naar `absent: null`. Het register is gezaghebbend; geen rij is een echte afwezigheid, en de wet had al `nullable: true` met null-guards op precies deze vier inputs.
- Archiefwet vernietiging, art. 5: `default: true` bij een onbekende bewaartermijn verwijderd. De false-gronden (`op_selectielijst_vernietiging`, `selectielijst_vastgesteld`, `selectielijst_gepubliceerd`) gaan voor; daarna geeft een onbekende `bewaartermijn_jaren` een onbekende uitkomst. Output `mag_vernietigd_worden` is daarom `nullable: true`.
- Awb bezwaar, art. 6:7 en 7:10: `bezwaartermijn_weken`, `beslistermijn_weken` en `verdagingstermijn_weken` zijn losse `number`-inputs met `nullable: true` (waren subvelden van het object-input `wet`, waar nullable niet declareerbaar is); in `bindings.yaml` drie losse bindingen op dezelfde tabelrij.
- Delegatieregisters BW (beschermingsbewind art. 1:449, curatele en handelingsonbekwaamheid art. 1:389, executeur art. 4:149, gezag art. 1:245, mentorschap art. 1:462, volmacht art. 3:72), Faillissementswet (curator art. 193, wsnp art. 356) en Wet SUWI art. 33 (drie plekken): de lege-string-tak `EQUALS $current.datum_einde ''` (SUWI: `$end_date`) naast de null-toets verwijderd. Null is de enige afwezigheidswaarde.
- Handelsregisterwet KVK, art. 9 lid 1: output `kvk_nummer` (gedeclareerd als string, gevuld met een ongefilterde FOREACH die een array oplevert) hernoemd naar `kvk_nummers_actieve_inschrijvingen`, type `array`, gefilterd op de criteria van art. 7 (rechtsvorm en status).
- Handelsregisterwet jaarrekening, art. 2:394 lid 1: `heeft_deponeringsplicht` was constant `true`, is nu `$voldoet_aan_voorwaarden` (rechtsvormtoets).
- Omgevingswet WPM RVO, art. 18.11 lid 1/2: `rapportageverplichting` was constant `true`, is nu `$voldoet_aan_voorwaarden`.
- Omgevingswet WPM gegevens RVO, art. 18.14 lid 2: de constante output `voldoet_aan_voorwaarden` (`AND(EQUALS true true)`) verwijderd; lid 2 beschrijft alleen de CO2-berekening.
- Participatiewet bijstand Amsterdam, art. 11 lid 1: `is_gerechtigd` was constant `true`, is nu `$voldoet_aan_voorwaarden`.
- Verordening precariobelasting Rotterdam, art. 2 lid 1: `is_belastingplichtig` was constant `true`, is nu `$voldoet_aan_voorwaarden` (terras op openbare grond met vergunning).
- Warenwet meldplicht NVWA, art. 21 lid 1: `heeft_meldplicht` was constant `true`, volgt nu `heeft_actief_incident`; een afwezig incidentgegeven geeft een onbekende uitkomst in plaats van een stilzwijgend oordeel. Het ministeriële oordeel ("naar het oordeel van Onze Minister") staat als geaccepteerde untranslatable.
- Wet BRP terugmelding Belastingdienst, art. 2.34 lid 1: een afwezig Belastingdienst-adres (`$belasting_adres = null`) is geen gerede twijfel meer (was `IF null -> true`), zowel in `heeft_gerede_twijfel_adres` als in de redenbepaling. Twijfel vereist een concreet afwijkend gegeven.
- Wet IB 2001 UWV toetsingsinkomen, art. 2.17: in `bindings.yaml` gaat `partner_buitenlands_inkomen.absent` van `0` naar `null`, zodat de null-guard in de wet daadwerkelijk wordt geraakt; `buitenlands_inkomen.absent` blijft `0` (niet nullable, geen guard), nu met toelichting.
- Wet op het CBS, art. 3: parameter `bsn` van `required: true` naar `required: false`; art. 3 lid 1 noemt geen individuele burger en de binding selecteert alleen op `jaar`. De parameter blijft bestaan omdat de AOW-leeftijdsbepaling hem doorgeeft.
- Zorgtoeslagwet 2024, art. 1 en 2 lid 1: `is_verzekerde_zorgtoeslag` was constant `true`, is nu `OR(heeft_verzekering, heeft_verdragsverzekering)`. Zorgtoeslagwet 2025, art. 2 lid 1: idem, nu `$is_verzekerde`.
- Alleen toelichting, geen gedragswijziging: Wet kinderopvang art. 1.5 (waarom `gewerkte_uren` niet nullable is: array-input zonder rijen is 0, geen null); Wet op het kindgebonden budget art. 2 lid 1 onderdeel b (waarom `partner_vermogen` nullable is: overgeslagen aanroep bij `partner_bsn = null`, niet een register dat afwezigheid kan leveren).

### Eerste ronde: gewijzigde scenario-verwachtingen

- `alcoholwet/vergunning/gemeenten/scenarios/alcoholwet_GEMEENTE_ROTTERDAM-2024-01-01.feature`: in drie scenario's gaan `voldoet_aan_voorwaarden` en `heeft_recht_op_vergunning` van `true` naar `unknown` (art. 8 lid 1 onder b Alcoholwet: zonder levensgedrag-register is het levensgedrag onbekend, en daarmee de uitkomst). Commentaar met artikel staat boven elke gewijzigde verwachting.
- `omgevingswet/werkgebonden_personenmobiliteit/scenarios/omgevingswet_werkgebonden_personenmobiliteit_RVO-2024-07-01.feature`: `voldoet_aan_voorwaarden` van de gegevens-wet wordt niet meer opgevraagd of geasserteerd (art. 18.14 lid 2, output bestaat niet meer). De eerste CO2-assertie is een `Then` geworden; door de verwijdering was het een `And` direct achter een `When`, waardoor die en de vijf volgende asserties door cucumber stil werden overgeslagen (de run meldde 1 skipped, 0 failed).

Geen verwachting gewijzigd, wel invoerdata: de POC-lege tekst `""` als einddatum is overal `null` geworden, omdat de wetten die lege string niet meer als afwezigheid lezen. Dat raakt de eigen features van de negen delegatieregisters, plus twee cross-law-consumenten die de per-wet-agents misten en die na hun wijziging faalden: `wet_kinderopvang_TOESLAGEN-2024-01-01.feature` (zes UWV-rijen `end_date`, art. 33 Wet SUWI; vier scenario's faalden op "Failed to parse date ''") en `apv_exploitatievergunning_GEMEENTE_ROTTERDAM-2024-01-01.feature` (een RECHTSPRAAK-rij `datum_einde`, art. 1:389 BW; een scenario faalde op "Type mismatch: expected number or date, got string"). De persona-generatoren in `frontend-demo/src/simulation/population.js` en `corpus/demo/profiles.yaml` gebruikten al `null`.

### Tweede ronde: corpus (bucket A)

- Burgerlijk Wetboek Boek 5, art. 5:42 lid 1, 3 en 4: drie nieuwe nullable outputs. `beplanting_geoorloofd` (lid 1: geoorloofd bij toestemming van de nabuur of als het naburige erf een openbare weg of openbaar water is, onbekend zolang die gegevens ontbreken), `nabuur_kan_zich_verzetten` (lid 3: geen verzetsrecht bij beplanting niet hoger dan de scheidsmuur) en `vergoedbare_schade` (lid 4: alleen schade ontstaan na de aanmaning, `DATE_DIFF` in dagen groter dan 0). Vijf nullable parameters (`toestemming_nabuur_gegeven`, `naburig_erf_is_openbare_weg_of_water`, `beplanting_hoger_dan_scheidsmuur`, `datum_aanmaning_opheffing`, `datum_ontstaan_schade`); `$schema` van v0.5.0 naar v0.5.7 omdat v0.5.0 `DATE_DIFF` en `untranslatables` niet kent. Scenario's: elf nieuw in `erfgrensbeplanting.feature` (geoorloofd met toestemming, geoorloofd naast openbare weg, ongeoorloofd, onbekend zonder gegevens; verzet wel, niet, onbekend; schade na en voor de aanmaning, onbekend zonder aanmaning, onbekend zonder schadedatum).
- Afstemmingsverordening Participatiewet Diemen, art. 7 en 9: art. 7 heeft nu een eigen `machine_readable` (`legal_character: INFORMATIEF`) met zes booleans, een per gedraging (onderdeel a, b 1° t/m 4°, c), en de nullable output `gedragscategorie`; bij samenloop geldt de zwaarste categorie, met commentaar dat de tekst zelf geen samenloopregel geeft. Art. 9 leest `gedragscategorie` als intra-wet input uit art. 7 (`source: {output: gedragscategorie}`) in plaats van de sentinel `0`; de optionele parameter blijft bestaan omdat `participatiewet/scenarios/bijstand.feature` de categorie rechtstreeks meegeeft en een parameter een afgeleide input overschrijft. `verlaging_percentage` heeft geen 0-default meer en `duur_maanden` is null zonder categorie. Scenario's: nieuw `scenarios/verlaging.feature` met acht scenario's (categorie 1, 2, 2 via tegenprestatie, 3, samenloop, geen gedraging, art. 9 via art. 7, rechtstreekse parameter overschrijft).

### Tweede ronde: demo-corpus

- Alcoholwet vergunning VWS, art. 8 lid 3 en 4: `voldoet_aan_svh_eis` toetste alleen de inschrijving in het register; de uitzondering van lid 4 (geen bemoeienis met de bedrijfsvoering en een schriftelijke verklaring) is toegevoegd via `heeft_bemoeienis_bedrijfsvoering` en `schriftelijke_verklaring_bevestigd` (nullable, niet verplicht). `legal_basis.paragraph` van `'1'` naar `'3'`. Scenario's: nieuw `alcoholwet_vergunning_VWS-2024-01-01.feature` met vier scenario's (ingeschreven; niet ingeschreven met geldige uitzondering; wel bemoeienis; geen verklaring).
- Algemene nabestaandenwet, art. 14 lid 1 onder a: de SVB-boolean `heeft_kinderen_onder_18` is vervangen door de collectie `kinderen` uit `wet_brp` en de output `heeft_kind_onder_18` (`FOREACH` met `AGE(kind.geboortedatum) < 18`, `combine: OR`). De binding `heeft_kinderen_onder_18` is uit `bindings.yaml`. Scenario's: "Nabestaande met kind onder 18 jaar zonder inkomen" geeft nu een echt kind via `kinderen_gegevens`; de kolom `heeft_kinderen_onder_18` is uit alle zes SVB-tabellen; "Nabestaande met inkomen boven vrijlating" had die boolean op `true` en kreeg het kind (300000103, al aanwezig als tweede rij) in `kinderen_gegevens` terug, anders faalde de voorwaarde terecht. Het itemveld heet `bsn`, zoals `wet_brp.kinderen_bsns` het leest.
- Algemene Ouderdomswet, art. 9 (alleen toelichting): de toeslagkorting van art. 10/11 (vrijlating 15% van het minimumloon, twee derde van het meerdere) is niet gewijzigd. De tekst van art. 10/11 staat niet in dit bestand en het corpus heeft geen minimumloonregeling om als bron te dienen; de constanten `inkomensgrens_partner` en `kortingsdeler` dragen nu een commentaar dat zegt dat ze geen brontekst hebben, en de `explanation` bij art. 9 verwijst daarnaar.
- AOW leeftijdsbepaling, art. 7a lid 1 onder n en lid 2: geboren in 1960 valt onder lid 1 onder n (67 jaar) als extra IF-tak. De formule van lid 2 is herschreven naar precies `V = 2/3 × (L − 20,64) − (P − 67)` als stapfunctie (0 of drie maanden), met de definities `levensverwachting_constante: 20.64`, `pensioenleeftijd_constante: 67`, `verhouding_levensverwachting`, `verhogingsdrempel: 0.25` en `verhoging_bij_overschrijding: 3`; de oude `referentie_levensverwachting: 20.0` en `maanden_verhoging_per_jaar` (geen P, verkeerde referentie) zijn weg. P staat als `pensioenleeftijd_voorafgaand_jaar: 67`, juist voor de eerste ronde (2026), vereenvoudiging daarna (zie "Buiten deze PR gelaten"). Een dubbele YAML-sleutel `aankondigingsperiode_jaren` is opgeruimd. Scenario's: nieuw `algemene_ouderdomswet_leeftijdsbepaling_SVB-2024-01-01.feature` met drie scenario's (1960, V onder 0,25, V vanaf 0,25).
- AOW gegevens, art. 7a lid 1 onder a t/m m: IF-cascade op `$referencedate.year` met de vaste leeftijden 65, 66 en 67 uit de tekst, tot en met 2024. Lid 1 onder n (2025 en later) is untranslatable (RFC-012, `accepted: true`): de formule van lid 2 is recursief in P zonder dat de tekst een jaarreeks geeft; het register blijft de default-tak, nu met toelichting. `type_spec min: 65, max: 70` is weg (geen tekstbasis). Scenario's: nieuw `algemene_ouderdomswet_gegevens_SVB-2025-01-01.feature` met drie scenario's (2020, 2024, 2025 via register).
- APV Rotterdam exploitatievergunning, art. 2:28 lid 2, lid 4 aanhef, lid 4 onder a en d, lid 5 onder h t/m k: `afwijkende_vergunningsduur` (nullable) gaat voor de standaardduur in `vergunningsduur`; `heeft_dhw_vergunning` (cross-law uit `alcoholwet/vergunning/rotterdam`, output `heeft_actieve_vergunning`) vervangt `schenkt_alcohol` op precies de twee plekken die de tekst aan een verleende vergunning koppelt (leeftijdseis en SVH-eis), terwijl `verstrekt_alcoholhoudende_dranken` aan `schenkt_alcohol` gebonden blijft (art. 3 Alcoholwet); de beheerders zijn een collectie waarover de wet zelf met `FOREACH` toetst op VOG, leeftijd en curatele, in plaats van drie kant-en-klare booleans; de vier gronden van lid 5 onder h t/m k (`voorschriften_overtreden`, `kvk_inschrijving_geldig`, `feitelijke_toestand_conform_aanvraag`, `voldoet_aan_horecabeleid`) zijn inputs met `absent: unknown` (geen register in de demo). In de afsluitronde is `nullable: true` van die vier af: een binding met `absent: unknown` levert nooit null, en de vitest `bindings.test.js` bewaakt dat. Scenario's: twee hernoemd (alcoholverstrekking naar drank- en horecawetvergunning), drie nieuw (afwijkende vergunningsduur, een beheerder voldoet niet, weigeringsgrond onbekend door ontbrekend register), en elk bestaand scenario kreeg een GEMEENTE_ROTTERDAM-tabel voor deze wet.
- Awb art. 1:1 lid 2 onder a, d, g, h en i, en lid 3: vijf nullable inputs (`is_wetgevende_macht`, `is_raad_van_state`, `is_functionaris_uitgezonderd_orgaan`, `is_ctivd`, `is_tib`) als extra takken in `is_excluded` en `exclusion_reason`; onderdeel g is een functionarisgrond omdat de tekst al die ambten aan hetzelfde gevolg koppelt. Lid 3 (terug-uitzondering voor ambtenarenrechtelijke besluiten over eigen personeel, behalve voor het leven benoemde ambtenaren bij Raad van State en Rekenkamer) via `is_ambtenarenrechtelijk_besluit_eigen_personeel` en `is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer`. Scenario's: zeven nieuw (ORG060 t/m ORG064 voor de vijf gronden, twee voor lid 3). In de afsluitronde zijn vijf asserties `output "exclusion_reason" is "..."` herschreven naar `equals "..."`: de vorm `is "..."` staat niet in `bdd/grammar.yaml` en cucumber sloeg die stappen stil over.
- Archiefwet openbaarheid, art. 15 lid 4: nullable parameter `minister_of_gs_besluit_anders`; de 75-jaar-doorbreking in `is_openbaar` geldt niet als de minister of GS anders besliste, en `openbaar_vanaf_datum` geeft dan null. Scenario's: DOC-107 nieuw; de zeven bestaande kregen de null-marker voor de nieuwe parameter, geen verwachting gewijzigd.
- Archiefwet overbrenging, art. 13 lid 4: `machtiging_datum` (nullable) en `maximale_termijn_machtiging_jaren: 10`; de opschorting in `moet_overgebracht_worden` vereist nu machtiging, verleningsdatum en minder dan tien jaar sinds die datum. Artikel 13 stond niet in het bestand en is als artikel toegevoegd met de tekst van BWB `BWBR0007376` (expressie 2022-05-01, geldig op 2024-01-01); dat is de enige `text:`-regel in de diff en het is een toevoeging, geen wijziging. Scenario's: "met machtiging" hernoemd naar "binnen de termijn" en voorzien van `machtiging_datum`; twee nieuw (machtiging ouder dan tien jaar, machtiging zonder verleningsdatum).
- Awb beroep, art. 8:1, 8:3 en 8:7: `zaak` is een `object`-input met `source: {}` en een binding `kind: cases` (service JenV); `type_rechter` geeft voor de vreemdelingenzaak de categorie `RECHTBANK` (was de naam `RECHTBANK_DEN_HAAG`), waardoor de compensatietak in `bevoegde_rechtbank` weg kon en de default terugvalt op `$wet.competent_court`. Scenario's: nieuw `awb/beroep/scenarios/JenV-2024-01-01.feature` met vier scenario's. In de afsluitronde zijn vier asserties `is "..."` naar `equals "..."` en een `is null` naar `is absent` gezet (buiten de grammatica, werden overgeslagen).
- Besluit bijstandverlening zelfstandigen, art. 2 lid 1 en 2: het urencriterium en de vermogenstoets zijn uit `voldoet_aan_voorwaarden` (art. 2 stelt ze niet; lid 2 beperkt met de vermogensgrens alleen de categorie bedrijfskapitaal, wat `bedrijfskapitaal_max` al deed); `vermogen`, `vermogensgrens_algemeen`, `vermogensgrens_ouder` en `min_inkomen_ouder` zijn weg; onderdeel b dekt ook de echtgenoot met WW-uitkering; onderdeel c ("duurzaam ontoereikend inkomen") is untranslatable. Scenario's: "Oudere zelfstandige met te hoog vermogen" en "niet aan urencriterium" omgezet naar "krijgt bijstand ongeacht" (verwachting `true`), nieuw "Beginnende zelfstandige zonder eigen WW maar met echtgenoot met WW". In de afsluitronde is ook "Gevestigde zelfstandige met te hoog vermogen krijgt geen bijstand" omgezet (was gemist, faalde na de wijziging).
- Besluit kerninstallaties, art. 1: parameter `heeft_beeindigingsplan` verwijderd; de tekst kent alleen een ontmantelingsplan en gebruikt geen van beide als voorwaarde. Geen scenario raakte de parameter.
- Faillissementswet curator (art. 68 jo. 193) en wsnp-bewindvoerder (art. 316 jo. 356): `voldoet_aan_voorwaarden` telt alleen actieve procedures (zoals `heeft_delegaties` al deed) in plaats van alle registraties; curator art. 1: `subject_types` zonder `default: CITIZEN`, twee expliciete takken (RECHTSPERSOON, NATUURLIJK_PERSOON). Scenario's: "Curator met opgeheven faillissement", "Voltooide WSNP met schone lei" en "WSNP beeindigd zonder schone lei" verwachten `voldoet_aan_voorwaarden` `false` (was `true`).
- Handelsregisterwet jaarrekening, art. 2:394 lid 1, 3, 5 en 8: `volgende_deadline` is `DATE_ADD` van twaalf maanden op het boekjaareinde (de definitie werd niet gebruikt), met null-guard; `reden_vrijstelling` toetst `ontheffing_verleend` (lid 5) en `afm_toezending_gedaan` (lid 8), beide `absent: unknown`. In de afsluitronde is `nullable: true` van die twee af (zie exploitatievergunning). Scenario's: nieuw `handelsregisterwet_jaarrekening_KVK-2024-01-01.feature` met vijf scenario's; in de afsluitronde zijn drie asserties `is "..."` naar `equals` gezet, waarna het AFM-scenario bleek te falen omdat de lid-5-kolom ontbrak (onbekend gaat voor in de IF); die kolom staat er nu op `false`.
- Kernenergiewet, art. 15b lid 1 onder a en c, lid 2: `dosislimiet_overschreden` komt uit `besluit_basisveiligheidsnormen_stralingsbescherming` art. 3.7 (eigen constante 1,0 mSv en `verwachte_stralingsdosis` weg); `beveiligingsplan_voldoet` en `noodplan_voldoet` uit `besluit_kerninstallaties` als weigeringsgrond; nieuwe output `verouderde_technologie_te_beoordelen` (lid 2). Scenario's: drie nieuw (beveiliging, verouderde technologie bij oprichting, geen beoordeling zonder technologiebeschrijving).
- Kieswet, art. B1 lid 1 en 2: `land_van_verblijf` uit `wet_brp`; `ingezetenschapsduur_jaren` en `werkzaam_in_nederlandse_openbare_dienst` als claim-bindingen (geen register); output `uitzondering_caribisch_nederland_van_toepassing` (Aruba, Curaçao, Sint Maarten zonder tien jaar ingezetenschap of openbare dienst) als extra voorwaarde in `heeft_stemrecht`. Scenario's: vier nieuw.
- Machtigingenwet, art. 2:240 lid 2 en 4: `bestuurder_functies` (lid 2) en `gemachtigde_functies` (lid 4, met VOORZITTER) gescheiden; `eigenaar_functies` valt buiten art. 2:240 en staat als untranslatable (Boek 7A niet in het corpus). `legal_basis` en `explanation` per lid. Geen scenario gewijzigd.
- Omgevingswet energiebesparing informatieplicht, art. 5.15 en 5.15d: `is_woonfunctie` komt uit `wet_bag` op `$adres` (nullable, want het KVK-adres kan ontbreken); de dode input `bag_gebruiksdoel` is weg; bij `volgende_deadline` staat dat 1 december 2027 de vaste einddatum uit de tekst is, geen berekening. Scenario's: nieuw `informatieplicht_RVO-2024-01-01.feature` met twee scenario's.
- Omgevingswet WPM gegevens, art. 18.14 lid 1: outputs `emissie_per_reizigerskilometer_woon_werk` en `emissie_per_reizigerskilometer_zakelijk` (CO2-subtotaal gedeeld door reizigerskilometers, 0 bij nul kilometer). Scenario's: twee nieuw.
- Participatiewet AIO, art. 22 en art. 32 jo. 33 lid 5: `legal_basis` van art. 21 naar art. 22 (norm voor pensioengerechtigden), `norm_alleenstaand`/`norm_gehuwden` op de bedragen uit de tekst in `corpus/regulation/.../participatiewet/2022-03-15.yaml` (121306 en 164254 eurocent); in `totaal_inkomen` is de term weg die de AOW er weer aftrok. De vrijlating van art. 33 lid 5 is untranslatable (bedragen staan nergens als tekst in het corpus). Scenario's: nieuw `participatiewet_aio_SVB-2026-01-01.feature` met een scenario.
- Participatiewet bijstand, art. 22a lid 1: `kostendelersnorm_factor_1` t/m `_4` als definities in plaats van de ongebruikte map en herhaalde literals; `default: 0.38` is weg en `kostendelersnorm` is nullable (vijf of meer personen: null, want geen bron; de formule staat in `corpus/regulation` als afbeelding). Amsterdam art. 11 (afsluitronde): de input `kostendelersnorm` is nullable en `uitkeringsbedrag` en `startkapitaal` geven null zonder norm, anders faalde de validator (N5). Scenario's: nieuw `participatiewet_bijstand_SZW-2023-01-01.feature` met vier scenario's (factor 1.0, 0.43, vijf of meer afwezig, vreemdeling zonder gelijkstelling); in de afsluitronde kreeg de vreemdeling een bekende niet-Nederlandse nationaliteit, want een lege cel is onbekend en maakt de uitkomst onbekend in plaats van `false`.
- Wet BRP, art. 2.7: `heeft_kinderen_onder_12` telde alleen of er kinderen waren; nu `FOREACH` met filter `AGE(current.geboortedatum) < kind_max_leeftijd_combinatiekorting`. Scenario's (afsluitronde): de zeven RvIG-rijen in `wet_kinderopvang_TOESLAGEN-2024-01-01.feature` met kinderen zonder `geboortedatum` kregen er een, anders faalde elke kinderopvangtoeslag-berekening op het ontbrekende veld.
- Wet inkomstenbelasting 2001, art. 2.17: `partner_box3_inkomen` trok bovenop de gezamenlijke partnervoet nog eens `heffingsvrij_vermogen` af; die aftrek en de dode definitie zijn weg. Scenario's: een nieuw ("box 3 partnerinkomen zonder dubbele heffingsvrije voet").
- Wet kinderopvang, art. 1.5 lid 1 en 2: `is_geregistreerd` (LRK, claim) als voorwaarde; `aanvraag_soort_opvang` en `binnen_aanloopperiode` (claim) voor de uitzondering van de ouderparticipatiecrèche in de aanloopperiode. Scenario's: twee nieuw.
- Wet op de huurtoeslag, art. 19 lid 3: de interpolatietak van `basishuur` is gewrapt in `CEIL` met `precision: -2` (eurocent naar hele euro's, RFC-024). Scenario's: een nieuw (355,64 wordt 356 euro).
- Wet op het kindgebonden budget, art. 2 lid 6 en lid 12 t/m 14: `alo_kop` van 348000 naar 338900, want de artikeltekst in het bestand zegt € 3.389; `open_terms.woonlandpercentage` (ministeriële regeling, default 100 uit "maximaal 100" in lid 12/13), input `kinderen_woonlanden` en output `woonlandfactor` (`MIN` over de kinderen). Scenario's: vier verwachtingen herberekend (`alo_kop_bedrag` 348000 naar 338900, `kindgebonden_budget_jaar` 599100 naar 590000), drie nieuw (onbekend woonland, Nederland, buitenland zonder regeling).
- WGBO vertegenwoordiger, art. 7:465 lid 3: het filter eist een schriftelijke machtiging of een familierelatie zonder `wenst_niet_op_te_treden`; GROOTOUDER en KLEINKIND in de hiërarchie en in `delegation_types`, SIBLING los van de default. Scenario's: vier nieuw (gemachtigde, echtgenoot die niet wenst op te treden, grootouder, kleinkind).
- Zorgtoeslagwet 2024, art. 2 lid 3 en art. 2a: `percentage_drempelinkomen_met_partner` 0,04256 en `_alleenstaand` 0,01879 (waren beide 0,0486) uit `corpus/regulation/.../wet_op_de_zorgtoeslag/2024-01-01.yaml`; de vermogenstoets van art. 2a staat in `voldoet_aan_voorwaarden`. Scenario's: drie verwachtingen `hoogte_toeslag` (194834 naar 197205, 197728 naar 198324, 197971 naar 198418), een nieuw (vermogen boven de grens).
- Zorgtoeslagwet 2025, art. 2 lid 4: `partner_bsn` (wet_brp) en `partner_is_verzekerde` (zvw, op de partner-bsn); bij een bevestigd niet-verzekerde partner de helft van het lid-1-bedrag, bij onbekend het volle bedrag. Scenario's: een nieuw (210773).
- Zorgverzekeringswet bijdrage, art. 41 en 43 lid 1: de drempel `inkomen_loon > 0` in `voldoet_aan_voorwaarden` is weg (art. 41 is onvoorwaardelijk); `legal_basis` van `bijdrage_inkomen` van art. 42 naar art. 43 lid 1, met untranslatable voor de vier IB-componenten. Scenario's: "Persoon zonder inkomen voldoet niet aan voorwaarden" hernoemd naar "is toch bijdrageplichtig", verwachting `false` naar `true`.

### Afsluitronde: wat de gezamenlijke run nog opleverde

De per-wet-wijzigingen zijn los van elkaar gemaakt; de gezamenlijke run over het hele corpus vond vier soorten samenloop, alle hierboven bij de wet vermeld: een consument die een nullable geworden output niet nullable las (Amsterdam), scenariodata die een verwijderd invoerveld nog nodig had (Anw, kinderopvang, vreemdeling), een scenario dat de oude lezing nog verwachtte (Bbz), en twaalf asserties in drie nieuwe feature-bestanden die buiten `bdd/grammar.yaml` vielen (`output "x" is "..."` en `is null`) en daardoor door cucumber als overgeslagen werden gemeld in plaats van als fout. Na herstel van die stappen bleek een ervan (jaarrekening, lid 8) inhoudelijk te falen. Een overgeslagen stap in een run die groen meldt, is een testfout die de suite hoort af te vangen; dat staat bij de RFC-kandidaten niet apart omdat het geen taalkwestie is maar een instelling van de runner.

## Buiten deze PR gelaten

Alleen wat niet kon, met de reden, en de RFC-kandidaten. Alles wat hier niet staat en wel in de per-wet secties, is in een van de twee rondes rechtgetrokken.

Geen brontekst in het corpus (de tekst is de bron, een getal wordt niet verzonnen):

- Algemene Ouderdomswet art. 10/11: de toeslagkorting bij partnerinkomen (vrijlating 15% van het bruto-minimumloon, twee derde van het meerdere). De tekst van art. 10/11 staat niet in het bestand en er is geen minimumloonregeling in het corpus; `inkomensgrens_partner` en `kortingsdeler` blijven constanten met een commentaar dat ze geen bron hebben.
- AOW art. 7a lid 1 onder n jo. lid 2 en 3, voor 2025 en later: P (pensioengerechtigde leeftijd van het voorafgaande jaar) is recursief en de tekst geeft geen jaarreeks; de vijfjaarscyclus van lid 3 is niet gemodelleerd. AOW gegevens: untranslatable, register als default. Leeftijdsbepaling: `pensioenleeftijd_voorafgaand_jaar: 67` is juist voor 2026 en een vereenvoudiging daarna.
- Participatiewet art. 22a lid 1, vijf of meer kostendelende personen: de formule staat in `corpus/regulation` als afbeelding, niet als tekst; `kostendelersnorm` is dan null.
- Participatiewet art. 33 lid 5 (AIO): de vrijlatingsbedragen staan nergens als tekst in het corpus; untranslatable.
- Besluit bijstandverlening zelfstandigen art. 2 lid 1 onder c: "duurzaam ontoereikend inkomen" heeft geen grens of berekeningswijze in de tekst; untranslatable, discretionair oordeel van het college.
- Algemene nabestaandenwet art. 14 lid 1 onder a: "ongehuwd" en "niet tot het huishouden van een ander behoort" hebben geen bronveld in de BRP-uitvoer of elders; alleen de leeftijd wordt getoetst, untranslatable voor de rest.
- Machtigingenwet: de vertegenwoordigingsbevoegdheid van eigenaar, vennoot, maat en beherend vennoot berust op Boek 7A BW, dat niet in het corpus staat; untranslatable.
- Zorgverzekeringswet art. 43 lid 1: de vier IB-componenten (loon, winst, resultaat overige werkzaamheden, periodieke uitkeringen) zijn in `wet_inkomstenbelasting` alleen inputs, geen outputs; `bijdrage_inkomen` blijft `$inkomen_loon` met een untranslatable die aangeeft welke outputs daar zouden moeten komen.
- Omgevingswet informatieplicht: `rapportage_frequentie_jaren: 4` heeft geen tekstbasis in het bestand (art. 5.15d geeft alleen de vaste einddatum); blijft staan met die constatering, omdat er geen tekst is om de waarde door te vervangen.
- Kernenergiewet: de weigeringsgronden b en e stonden al zonder bron in het bestand en worden nergens gebruikt; ongewijzigd, zelfde reden.
- Alcoholwet vergunning VWS art. 8 lid 4: de tak "niet ingeschreven en lid-4-voorwaarden onbekend geeft onbekend" is niet als scenario te schrijven, omdat deze wet platte parameters gebruikt en de scenariotaal een parameter niet onbekend kan maken.
- Awb beroep: de binding van `zaak` heeft `kind: cases`, dat een lijst levert en geen object, en de materialiser heeft geen sleutel om de zaak te selecteren (de zaak is de zaak). Genoteerd in `bindings.yaml`, niet stilzwijgend omzeild.

Nog niet nagelopen, gevonden bij de accijnswet (september 2026):

- **Afrondingsvoorschriften in de wettekst die het model niet uitvoert.** De Wet op de accijns art. 7 lid 1 en art. 13 dragen dezelfde slotzin met twee afrondingen die de andere kant op gaan: de hectoliter "rekenkundig" (half-up), het volumeprocent alcohol "naar beneden" op één decimaal. Het model deed alleen de eerste, waardoor bier van 5,28%vol € 0,65 per hectoliter te duur werd en gedistilleerd van 40,37%vol € 1,28, altijd ten nadele van de belastingplichtige. Rechtgezet met `FLOOR precision: 1` vóór de vermenigvuldiging, met scenario's die het vastleggen.

  Het punt is algemener dan deze wet. Schema-validatie ziet zo'n afwijking niet, de scenario's zagen hem niet omdat ze allemaal ronde percentages gebruikten, en het commentaar boven de regel ging over precies die zin maar sloeg de tweede helft over. Twaalf andere bestanden in de demo-corpus noemen "afgerond" of "afronding" in hun artikeltekst (zorgtoeslagwet 2024 en 2025, AOW en AOW-gegevens, Wet IB, SUWI, huurtoeslag, bijstand Amsterdam, WW, pensioenwet, Zvw werkgeversbijdrage). Of die de voorgeschreven afronding werkelijk uitvoeren, en in de juiste richting en volgorde, is niet nagelopen. Dat vraagt de wettekst zin voor zin naast het model leggen; de skill `law-letter-fidelity-audit` is daarvoor bedoeld.

Wat de taal niet kan (zie de nummers onder "Wat de taal nog niet kan"):

- Participatiewet art. 11 lid 4 (partnerrecht): kandidaat 10, de aanroep van de eigen wet op de partner-bsn wordt als kringverwijzing geweigerd. Empirisch geverifieerd en teruggedraaid.
- WGBO art. 7:465 lid 1 en 2 (minderjarige onder twaalf, curatele, mentorschap): kandidaat 11, geen cross-law aanroep per kandidaat op zijn eigen bsn. Lid 3, de rangorde tussen een gemachtigde en een familielid van dezelfde patiënt: kandidaat 12, geen keuze per groep; als untranslatable vastgelegd in plaats van een vangnet.
- Kandidaten 1 t/m 9 uit de eerste ronde staan nog open. Twee raken deze PR nog steeds direct: de delegatieregisters en SUWI toetsen `datum_einde`/`end_date` op null zonder dat het veld nullable kan worden gedeclareerd (1), en de lege-string-normalisatie in JSON-arraycellen van scenario-tabellen (9) is met de hand gedaan. Kandidaat 13 (map-lookup) is in deze ronde omzeild met vier scalaire definities.
