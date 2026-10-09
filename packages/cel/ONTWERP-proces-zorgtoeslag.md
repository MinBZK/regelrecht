# Ontwerpnotitie: het hele zorgtoeslagproces in de kroniek

Status: concept, ter bespreking. Bouwt voort op #1679 (de aanvraag als chronolex-feit) en #1683 (de aanvraag en het besluit in de demo).

Deze notitie beschrijft hoe de cel van Toeslagen het hele proces van de zorgtoeslag vastlegt: de aanvraag, het voorschot, de maandelijkse betalingen, de definitieve toekenning na het berekeningsjaar en de verrekening. Ze beschrijft ook hoe de demo daarvoor de tijd vooruit laat lopen. Bij elke keuze staat waar die vandaan komt: **wet**, **RFC** (RFC-022, en RFC-045 tot en met 050 in deze PR), **oude PoC** (de chrono-poc-PR's #1466 t/m #1482) of **eigen keuze**.

## 1. Het proces volgens de wet

De Awir in de tekst die in 2025 gold. Waar 2026 anders is, staat dat erbij.

| Stap | Artikel | Wat ontstaat | Wanneer |
|---|---|---|---|
| Aanvraag | Awir 15 | indiening | tot 1 september (2026: 31 december) van het jaar na het berekeningsjaar; geldt ook voor volgende jaren (lid 5, zie §8a) |
| Voorschot | Awir 16 | beschikking, "tot het bedrag waarop de tegemoetkoming vermoedelijk zal worden vastgesteld" | binnen 13 weken na de aanvraag; vóór het jaar bij een doorlopende aanvraag (lid 2) |
| Betaling voorschot | Awir 22 | termijnen | per maand een termijn, volgend uit de dagtekening van het voorschot: 12 vanaf december als het vóór het jaar verleend is, anders de resterende maanden plus een bedrag ineens voor de verstreken maanden, of ineens na 31 oktober |
| Wijziging | Awir 17, 16 lid 5 | melding, dan herziening voorschot | tijdens het jaar |
| Toekenning | Awir 19 jo. 14, Zorgtoeslagwet 2 | beschikking op het inkomensgegeven (Awir 8, AWR 21) | binnen zes maanden na de aanslag IB; anders uiterlijk 31 december van het jaar erna |
| Uitbetaling, verrekening | Awir 24 | nabetaling binnen vier weken; voorschotten verrekend (lid 2), of terugvordering (lid 3) | na de toekenning |
| Terugvordering | Awir 26, 26a, 26b, 28 | beschikking; tot € 121 geen terugvordering; zienswijze vanaf € 1.500 | betalen binnen zes weken |

Zorgtoeslagwet art. 2 lid 5: de aanspraak wordt **per kalendermaand** bepaald. Het corpus rekent nu alleen een jaarbedrag.

## 2. Wat er al is, en wat eerst moet

- **Awir 16 tot en met 29 zijn nergens machineleesbaar.** Alleen art. 2, 3, 8 en 15 zijn dat.
- **De geharveste Awir-tekst is een redactie van rond 2010.** Een paar voorbeelden: 8 weken in plaats van 13 in art. 16, 8 weken in plaats van zes maanden in art. 19, en art. 22 met 3 leden in plaats van 8. Het model van art. 15 spreekt de eigen tekst al tegen. Er is ook geen versie voor 2026.
- **De demo heeft één inkomen voor alle jaren** (`box1` zonder jaarkolom).
- **De demo heeft geen bediening voor de tijd.** `setReferenceDate` bestaat, maar geen scherm roept hem aan. De Awb-stages (bekendmaking, bezwaartermijn) en de zaakgebeurtenissen gebruiken de wandklok (`nowIso()`), de grammen de peildatum: twee tijdassen naast elkaar.

**Voorstel, stap 0 (eigen keuze):** eerst de Awir opnieuw harvesten, voor 2025 en 2026, in een eigen PR. Zonder geldende tekst modelleren we een wet die er niet meer is.

## 3. Elk besluit een eigen artikel en een eigen gram

**Bron:** RFC-022 (elke stage een eigen decretogram) en de opvolger in `poc/chronolex` (`testregeling_toeslag`: voorschot, vaststelling, wijziging en terugvordering elk als eigen artikel en event).

| Gram | Vestigend artikel | Type | Verwijst naar | Velden |
|---|---|---|---|---|
| aanvraag | Awir 15 | submission | — | wat de wet vraagt (bestaat al) |
| voorschot verleend | Awir 16 | decretogram | `on_application` | het voorschotbedrag; de dagtekening is het moment dat telt (§4) |
| voorschottermijn betaald | Awir 22 | executogram | `decision` → voorschot | bedrag, maand |
| toekenning | Awir 19 jo. Zorgtoeslagwet 2 | decretogram | `on_application` | het vastgestelde bedrag, het inkomensgegeven waarop het rust |
| verrekening | Awir 24 | decretogram | `decision` → toekenning | nog te betalen, onverschuldigd betaald |
| terugvordering | Awir 26 | decretogram | `decision` → toekenning | het bedrag (na 26a) |

Wat de cel vastlegt, blijft wat de wet zegt, zoals nu bij de aanvraag. Sinds 7 oktober 2026 zegt de wet dat in eigen woorden en niet meer in een blok `produces.extensions.chronolex`: `produces.submission` (hier ontstaat een aanvraag), `produces: BESCHIKKING` met `decides_on` (hierop wordt besloten, in elke fase van de procedure die `is: BESLUIT`), de haken met `applies_to.submission`, `origin` op de parameters (met `rol: TIJDVAK` voor het berekeningsjaar) en `produces.moment` (Awb 4:13: de ontvangst telt). De cel leidt de vastlegging daaruit af (`extension::derive`); een stroom die een artikel met twee besluitfasen vastlegt zegt per event de `stage`. Alleen het fictieve uitvoeringsbeleid (een executogram) draagt nog een eigen blok.

**Wijziging ten opzichte van #1679:** het huidige besluit "zorgtoeslag toegekend" (Zorgtoeslagwet art. 2, vastgelegd op de besluitdag) splitst in een **voorschot** (Awir 16) en een **toekenning** (Awir 19). Zorgtoeslagwet art. 2 blijft het bedrag uitrekenen; de Awir zegt welk besluit het draagt.

**Besloten (7 oktober 2026): de Awir als "Awb van de toeslagen".** Zorgtoeslagwet art. 2 neemt het besluit, in twee soorten: voorschot en toekenning. Awir 16, 19, 22 en 24 haken erop, zoals Awb 4:2 en 4:13 op elke aanvraag haken. Zo noemt de Awir geen enkele toeslag bij naam en geldt ze voor elke inkomensafhankelijke regeling. (Eerder besloten was dat Awir 16 en 19 het besluit zouden dragen. Dat vroeg dat de Awir het bedrag uit de Zorgtoeslagwet ophaalt, en delegatie loopt in regelrecht alleen naar een lagere laag.)

### Uitwerking na de spike (7 oktober 2026)

Een prototype op de engine laat zien hoe het past:

- **De Awir krijgt een eigen procedure** `tegemoetkoming` (Awir 1: geldt voor inkomensafhankelijke regelingen), met de fasen AANVRAAG, VOORSCHOT en TOEKENNING. Zorgtoeslagwet 2 kiest die procedure met `procedure_id` (Wzt 1 onder e: "zorgtoeslag: een tegemoetkoming"). De Awir-haken vuren alleen in die fasen, dus niet op elke beschikking.
- **Het geschatte inkomen:** Awir 16 is een pre-haak op VOORSCHOT met als uitvoer `toetsingsinkomen`, een term die Awir 2 lid 1 onder i definieert "in deze wet … alsmede in inkomensafhankelijke regelingen". Die vervangt bij het voorschot de invoer die bij de toekenning uit Awir 8 komt. Geen van beide wetten noemt de andere.
- **De engine moet eerst drie dingen goed doen:**
  1. twee artikelen met dezelfde uitvoer (Awir 8 en 16) mogen niet stil op het laatst geladen artikel uitkomen;
  2. een fase VOORSCHOT of TOEKENNING is een besluitfase, zodat Awb 3:46 en 6:7 er ook op haken (een stage krijgt `is: BESLUIT`);
  3. de cel voert precies één fase uit op een verse toestand, zodat het geschatte inkomen niet doorlekt naar de toekenning.
- **Awir 22** wordt een gewoon artikel dat de cel per maand uitvoert. De hoogte van een termijn komt uit een gemarkeerde beleidsregel van Toeslagen, niet uit de Awir.

## 4. Betalingen: de termijnen komen uit de wet

**Bron:** Awir 22 en de oude PoC (#1466, #1469, #1482). Die zette verplichtingen in het besluitartikel (`extensions.chronolex.verplichtingen` met bedrag, ritme en grondslag) en benaderde het ritme van art. 22 met `ritme: $betalingsritme`.

**Wat art. 22 zegt** (tekst van 2026-01-01): het voorschot wordt "uitbetaald in 12 termijnen", de eerste in december vóór het berekeningsjaar "en elke volgende termijn telkens een maand later" (lid 1). Wie in de loop van het jaar een voorschot krijgt, krijgt "zoveel termijnen als er na de maand van dagtekening nog kalendermaanden van dat jaar overblijven" (lid 2). Bij een aanspraak voor een deel van het jaar is het aantal termijnen het aantal kalendermaanden met aanspraak (lid 3). Over al verstreken maanden volgt een bedrag ineens in de maand van dagtekening (lid 4). Na 31 oktober is het één bedrag in de maand van dagtekening (lid 5). Over de hoogte van een termijn zegt het artikel niets: het noemt geen "gelijke termijnen" meer.

Art. 22 spreekt dus niet van een lijst of van betalingsopdrachten. Het zegt per maand of er een termijn valt, en dat volgt uit de dagtekening van het voorschotbesluit.

**Voorstel (eigen keuze):** art. 22 wordt een **regel per maand**, geen lijst. Het artikel krijgt als invoer het voorschotbedrag, de dagtekening van het voorschot, de maanden met aanspraak en een maand. Het antwoordt: valt er in deze maand een termijn, en zo ja welk bedrag. Elke keer dat de tijd een maand verder gaat, vraagt de cel dat aan de wet; pas een betaalde termijn wordt een gram (§5).

- **Een termijn die nog moet komen is geen feit** (RFC-050: "wat nog moet gebeuren is geen feit"). Het voorschotgram legt geen toekomstige betalingen vast, alleen het bedrag en het moment van het besluit. Dat volgt de tekst ("elke volgende termijn telkens een maand later") en sluit aan bij het vooruitspoelen (§8).
- **De hoogte van een termijn** regelt de wet niet. Voorstel: het voorschotbedrag gedeeld door het aantal termijnen, met het restant in de laatste termijn (zoals de oude PoC, #1466). Dat is een uitvoeringskeuze en hoort in het uitvoeringsbeleid van Toeslagen, niet in de Awir. Zolang er geen beleid is, staat het als gemarkeerde keuze in het model.
- **Wie betaalt, zegt de wet niet**, maar het uitvoeringsbeleid of de cel (oude PoC: "betaler uit het wereldbestand"). Voor deze stap blijft het bij één cel: Toeslagen legt zelf vast dat de termijn betaald is. Een aparte betaalcel is een latere stap.

## 5. Verrekening: een reductie over de eigen kroniek

**Bron:** RFC-045 §2 en de oude PoC (#1472, #1479).

- **De toekenning (Awir 19)** rekent op het definitieve inkomensgegeven (Awir 8, AWR 21).
- **De verrekening (Awir 24 lid 2)** is toegekend bedrag minus **uitbetaalde** voorschotten (#1472: niet de verleende, want wat vervalt wordt nooit betaald). Dat "uitbetaald" leest de cel terug uit haar eigen kroniek, met een lexostatus `betaald_voorschot` die de betaalde termijnen telt. Dat is een reductie bij de bron, zoals nu `aanvraag`.
- **Openstaand per richting, niet netto** (#1479). Nabetaling en terugvordering zijn twee uitkomsten, want verrekenen is een rechtsvraag (Awir 24 lid 2 en 30).
- **Na de toekenning vervallen de nog niet betaalde voorschottermijnen.** Dat volgt uit de verwijzing en wordt geen eigen gram (#1472).
- **Terugvordering (Awir 26)**, met de drempel van 26a. De zienswijze (26b) is een Awb-achtige stap; die volgt later.

**Wat de lexostatus daarvoor nodig heeft:** optellen over meerdere grammen (een `sum` naast `pick: latest`) en lezen **op een moment**: `peilmoment` (effective_at ≤ t) en `bekend_op` (recorded_at ≤ t), gesorteerd op effective_at en daarna recorded_at (RFC-050 en RFC-045 §7). De compacte cel kent dat nog niet.

**Gebouwd (8 oktober 2026): het voorschot leest de cel terug met een beleidsartikel.** Besloten op 6 en 7 oktober: de reductie is bedrijfslogica van de houder en staat in dezelfde regeltaal als de wet, als artikel in het beleid van de houder (RFC-045); wat nergens staat, schrijven we namens de houder en markeren we als **aanname**. De lexostatus `voorschot` is daarom vervangen door `fictief_beleid_kroniek_toeslagen` art. 1, een fictief uitvoeringsbeleid van de Dienst Toeslagen, gemarkeerd als aanname. Het artikel krijgt de aanvraag als parameter `root` en de kroniek als invoer zonder bron (`source: {}`), en geeft het voorschotbedrag (Awir 16 lid 1), de dagtekening van het voorschot (Awir 22 lid 1) en het berekeningsjaar. Het wetsformaat kent geen LAST: het laatste voorschot is het besluit met de hoogste `sequence` (MAX), en één waarde lees je met ADD over dat ene besluit.

- **Welke kroniek** die invoer is, zegt alleen de celconfiguratie: `registers: {fictief_beleid_kroniek_toeslagen#kroniek: {chronicle: toeslagen}}` in `cell.yaml` (zoals `deployment/registers.yaml` in het NAPP-corpus). Het beleid noemt geen systeem.
- **De cel** registreert per register een gegevensbron bij de engine (`register::bind`), met het beleid als scope. Bij het lezen geeft ze de grammen die op het leesmoment gelden, in de volgorde waarin ze gelden (`effective_at`, dan `recorded_at`), één rij per gram: de velden plat, met `id`, `event`, `type`, `stage`, `root`, `sequence`, `effective_at`, `effective_date`, `recorded_at`, `period` en een paar andere ernaast. Een gram dat pas later geldt, staat er niet in.
- **Een stroom** leest zo'n beleid met `reads: [{regulation: fictief_beleid_kroniek_toeslagen}]`, of één artikel ervan. De cel voert dan elke uitvoer van het beleid (of van dat artikel) uit met `{root}` en neemt wat het artikel vraagt. De herkomst zegt de lexostatus (het `endpoint` van het artikel), het register en het artikel dat de engine uitvoerde (`{source: lexostatus, lexostatus: voorschot, register: fictief_beleid_kroniek_toeslagen#kroniek, article: fictief_beleid_kroniek_toeslagen#1}`).
- **Eigen keuze:** het beleid geldt vanaf 1 januari 2024, zodat een voorschot dat in december vóór het berekeningsjaar wordt betaald, het kan lezen.
- **Geen lexostatus-configuratie meer (9 oktober 2026).** Zie hieronder.

**Gebouwd (9 oktober 2026): de wet bepaalt de interface, de cel de implementatie.** Officieel (de chronolexografie zoals de paper haar beschrijft) is de reductie van een cel haar eigen zaak. Regelrecht wijkt daar bewust een beetje van af, omdat het een systeem voor wetsuitvoering wil zijn: de **wet** bepaalt de interface van een cel, welke waarden (naam, type) zij moet kunnen leveren, namelijk de parameters van de artikelen die aan een besluit meedoen, met hun `origin`. Hoe een cel tot die antwoorden reduceert, bepaalt zij zelf. De feitenreductie van regelrecht, in het wetsformaat, is één manier om dat te implementeren; een cel mag het ook anders doen. Binnen regelrecht is de volgorde (Tim, 9 oktober): zo veel mogelijk afgeleid uit de wet; een beleidsartikel van de houder waar de wet het echt niet zegt; celconfiguratie alleen als laatste redmiddel. Wat een lexostatus teruggeeft (namen, typen) moet herleidbaar zijn uit een wets- of beleidsartikel, "anders verdwijnt een groot deel van de werking naar de binnenkant van het zaaksysteem".

- **`aanvraag` uit de wet.** Een besluit op een aanvraag (`decides_on`) leest wat het vraagt en de aanvraag bevat uit die aanvraag, zonder `reads` in zijn stroom. De velden van de aanvraag zegt de wet al (Awir 15 en de haken erop, parameters met origin `BELANGHEBBENDE`/`KANAAL`); de dag van ontvangst is de datumparameter van Awir 15 (`datum_ontvangst`) waarvan de `origin` rust op dezelfde bepaling als de `produces.moment` van de haak (Awb 4:13 lid 1). Wat een beleidsartikel dat het besluit leest ook geeft, leest de cel uit dat artikel (`vermoedelijk_toetsingsinkomen` uit art. 4); de periode geeft de cel. De lexostatus heet naar de soort aanvraag (`produces.submission.kind`). Code: `Cell::submission_of`, `Cell::read_submission`, `Shape::moment` (`shape::submission_moment`).
- **`uitbetaald` als beleidsartikel.** `fictief_beleid_kroniek_toeslagen` art. 3a (in `corpus/regulation` en `corpus/demo`): de som van wat de bank op de termijnen van het voorschot voor een berekeningsjaar bijschreef, Awir 24 lid 2, gemarkeerd als aanname. De toekenning leest `{regulation: fictief_beleid_kroniek_toeslagen, article: 3a}`.
- **Elk beleidsartikel is een lexostatus**, genoemd naar zijn `endpoint` (een bestaande sleutel van het wetsformaat, "named endpoint for this article"). `Cell::lexostatuses` geeft per lexostatus de soort (`submission` of `policy`), de bepaling waarin haar vorm ligt, en de gegevens die zij geeft met type en eenheid (`fields`): wat de gebeurtenissen die haar lezen vragen, niet de hulpuitvoer van een artikel.
- **Weg:** `lexostatuses.yaml` (demo en testfixture), de reductie-DSL (`filter`, `pick`, `derivations`) in `config.rs` en `lexostatus.rs`, `Cell::lexostatus_fields` en `WasmCell.lexostatusFields`. `lexostatus.rs` houdt alleen het lezen op een moment (`in_force`).
- **Verlies:** een lezing van een beleidsartikel noemt alle grammen van de zaak in het register, niet de rijen die het artikel gebruikte; de engine zegt dat niet. De oude `uitbetaald` noemde precies de betaalde termijnen.

**Gebouwd (8 oktober 2026): een rekening waarop de betalingen echt binnenkomen.** De betaalcel uit §9 is er, als fictieve bank in een tweede cel.

- **Toeslagen geeft een betaalopdracht.** Het executogram van het fictieve beleid (art. 1) heet nu `betaalopdracht_gegeven`: het bedrag, de rekening van de aanvrager en de uitvoerdatum (de dag zelf), met een verwijzing naar het voorschot. De rekening vraagt het fictieve beleid van Toeslagen bij de aanvraag (art. 3, een haak op de aanvraag van Awir 15, origin BELANGHEBBENDE; niet de Awir). De cel leest haar terug met `fictief_beleid_kroniek_toeslagen` art. 2. De persona geeft een duidelijk verzonnen nummer op (`NL00TEST0123456789`, `application` in `demo-config.yaml`).
- **De bank is een eigen cel** (`corpus/demo/cells/bank`, `recording_actor: fictieve_bank`, kroniek `rekeningen`) met eigen regels: `fictieve_bankvoorwaarden` art. 1, "op de uitvoerdatum bijgeschreven, tenzij de rekening onbekend of geblokkeerd is". Het schema kent geen laag voor regels van een private partij; het regelwerk staat als UITVOERINGSBELEID, met die markering. Het staat alleen in het democorpus. Wat de bank van een rekening weet (geblokkeerd, beginsaldo) staat bij de persona onder `BANK` in `profiles.yaml` en komt via `bindings.yaml` als gegevensbron binnen.
- **Bij ontvangst** (eigen keuze, generiek): een executogram met `record_when` maar zonder `executed_on` ontstaat niet op een dag maar wanneer er iets binnenkomt. `Cell::receive` voert het ontvangende artikel één keer uit met wat er binnenkwam en legt elk event van de cel vast dat dat artikel vestigt en waarvan `record_when` waar is: bij de bank `overboeking_bijgeschreven` of `overboeking_geweigerd`, bij Toeslagen `voorschottermijn_betaald` of `betaling_mislukt` (art. 2 van het fictieve beleid, met het antwoord van de bank als parameters met origin KANAAL, en een verwijzing naar de betaalopdracht). Een veld van zo'n gram mag ook een parameter zijn (het kenmerk van de opdracht). Het antwoord op een gram die een periode betreft, rekent met de wet van die periode.
- **Het transport** doet de demo, generiek: `channels` in `demo-config.yaml` zegt welke gram van welke cel naar welk artikel van welke cel gaat, welk veld welke parameter vult en welke verwijzing het antwoord krijgt. De wet zegt niet wie de bank is of hoe een bericht reist. In JavaScript staat geen eventnaam.
- **De verrekening telt alleen wat de bank bijschreef.** `uitbetaald` telt `betaald_bedrag` van `voorschottermijn_betaald`, niet de opdrachten.
- **Een mislukte termijn blijft open** (eigen keuze, gemarkeerd in het beleid). `once_per: month` blijft streng: één opdracht per maand. Wat de bank weigerde, gaat mee met de opdracht van de volgende uitvoeringsdag (`meegenomen_achterstand`), ook in een maand zonder termijn, tot de toekenning. Wat er achterstallig is, leest de cel met `fictief_beleid_kroniek_toeslagen` art. 3: geweigerd min al opnieuw opgedragen. Zo blokkeert `once_per` geen nieuwe poging, en hoeft de cel niets over mislukken te weten.
- **In het portaal** staat "Mijn rekening (fictieve bank)": saldo en overboekingen uit de kroniek van de bankcel en het beginsaldo van de persona (`account` in `demo-config.yaml`). Het saldo is beginsaldo plus wat de bank bijschreef: een afschrift, geen wet. Een rekening blokkeren kan nu alleen in `profiles.yaml` (`geblokkeerd: true`).
- **Open:** een rekening die de bank helemaal niet kent, geeft in de engine een onbekende waarde en dus een fout, tenzij de gegevens expliciet null zeggen; het antwoord van de bank geldt vanaf het moment van ontvangst, niet vanaf de uitvoerdatum.

## 5b. De afrekening als feiten: aanslag, nabetaling en terugvordering

**Gebouwd (9 oktober 2026).** Tot nu toe rekende de toekenning met een datum en een inkomen uit de gegevens van de persona (`dossier` in `demo-config.yaml`), en werd er na de toekenning niets betaald of teruggevorderd. Nu is de aanslag een besluit van de Belastingdienst, komt het inkomensgegeven als feit bij Toeslagen binnen, en volgen nabetaling en terugvordering via de bank.

**De wet.** AWR 11 lid 1: de inspecteur stelt de aanslag vast. Art. 21 onder e, 1°: is er een aanslag, dan is het inkomensgegeven het verzamelinkomen; art. 21c lid 4: het aanslagbiljet vermeldt het; art. 21e lid 1: de inspecteur verstrekt het aan een afnemer. Awir 8 lid 1: het toetsingsinkomen is het inkomensgegeven over het berekeningsjaar. Awir 19 lid 1: toekennen binnen zes maanden na de aanslag. Awir 24 lid 1: uitbetalen binnen vier weken na de dagtekening. Awir 26: het terug te vorderen bedrag is in zijn geheel verschuldigd en wordt volledig teruggevorderd; 26a: tot € 118 (2025) niet. Awir 28 lid 1: betalen binnen zes weken na de dagtekening van de terugvordering.

- **De cel van de Belastingdienst** (`corpus/demo/cells/belastingdienst`, kroniek `aanslagen`) legt de aanslag vast als decretogram. Het democorpus heeft daarvoor een eigen AWR (`algemene_wet_inzake_rijksbelastingen`, art. 11, 21, 21c en 21e, tekst 2024 = 2025). Art. 11 is een `BESCHIKKING` zonder `decides_on`: een besluit op geen aanvraag. **Eigen keuze (generiek, cel):** zo'n besluit leidt de cel af als decretogram zonder verwijzing (`extension::derive`). Het verzamelinkomen rekent de aanslag niet zelf uit (Wet IB 2.18 staat niet in het democorpus): het is per jaar een gegeven van de inspecteur, **fictief**, in de tabel `aanslagen_inkomstenbelasting` bij de persona, die alleen deze cel leest.
- **Welk jaar.** Het schema laat de rol TIJDVAK alleen toe bij een tijdvak dat de aanvrager kiest (`waarde: BELANGHEBBENDE`). Een aanslag vraagt niemand aan. **Eigen keuze:** de stroom noemt de parameter die het jaar geeft (`period: {parameter: belastingjaar, unit: year}`); de cel weigert dat als de wet zelf een tijdvak noemt of als het besluit op een aanvraag is. Open voor het schema: een tijdvak bij een besluit op geen aanvraag.
- **Wanneer.** Hetzelfde mechanisme als het voorschot voor een volgend jaar: `decided_on` wijst een artikel aan dat de dag geeft. `fictief_beleid_aanslagregeling` art. 1 (**fictief**): de dag uit de planning van de inspecteur, per persoon en jaar (dezelfde tabel, kolom `datum_vaststelling`). `Cell::due_ex_officio` geeft per onderwerp (de BSN) het jaar en de dag: elk jaar zonder aanslag over dat onderwerp, vanaf de eerste aanslag erover (of, vóór de eerste, vanaf het jaar vóór dat van nu; **eigen keuze**: de inspecteur besluit over voorbije jaren) tot en met het jaar van nu. Een jaar dat werd overgeslagen terwijl een later jaar een aanslag kreeg, komt zo opnieuw aan de beurt. Een stroom kan het eerste jaar vastzetten (`first_period`); de demo doet dat niet. Over wie het besluit gaat, noemt de stroom (`subject: [bsn]`, **generiek, cel**): de cel weigert een tweede aanslag over hetzelfde jaar over dezelfde BSN, wat er verder ook wordt meegegeven, en een aanslag vóór zijn dag of zonder dag. Het jaar is het kalenderjaar (**eigen keuze**, naar Wet IB 2001 art. 2.3: de belasting wordt geheven over het kalenderjaar; AWR 11 noemt zelf geen jaar).
- **Voor wie (eigen keuze, demo).** `ex_officio` in `demo-config.yaml` noemt het besluit en het onderwerp (`bsn: $bsn`). De demo vraagt het alleen voor de persoon van een open zaak met een aanvraag in een kroniek; zonder zo'n zaak besluit de cel over niemand. Zo merkt de rest van de demo niets van deze cel: geen andere wet leest de tabel `aanslagen_inkomstenbelasting`, de inkomens in `box1` blijven zoals ze waren, en de AWR staat in `hidden_laws`.
- **Het kanaal.** De aanslag gaat naar `fictief_beleid_toekenning_toeslagen` art. 1, een ontvangst bij Toeslagen (`inkomensgegeven_ontvangen`, `identified_by` het kenmerk van de aanslag). **Eigen keuze:** Toeslagen doet het verzoek van art. 21e ("op zijn verzoek") eens, als staand verzoek voor elke betrokkene en elk jaar; daarop verstrekt de inspecteur het inkomensgegeven zodra de aanslag er is. Nieuw in de kanalen: `$period` (de periode van de gram) en `$input.<naam>` (de parameter waarmee de cel haar vulde, de BSN).
- **De toekenning leest het feit.** `fictief_beleid_kroniek_toeslagen` art. 5 (**aanname**) geeft het laatste inkomensgegeven over het berekeningsjaar voor de BSN uit de aanvraag, met de dag van de aanslag. De dag gaat naar Awir 19 (`datum_vaststelling_aanslag`, voorheen uit het dossier); het inkomensgegeven naar `fictief_beleid_toekenning_toeslagen` art. 2, een haak vóór de beschikking in de fase TOEKENNING die het toetsingsinkomen (Awir 8 lid 1) het verstrekte inkomensgegeven maakt (parameter met origin REGISTER, grondslag AWR 21e lid 1). **Eigen keuze, fictief beleid:** dat Toeslagen bij de toekenning het verstrekte gegeven neemt is uitvoering van art. 8, niet de wet zelf; in Awir 8 kan het niet, omdat dat artikel ook de bron is van het toetsingsinkomen buiten de procedure (de tegel op het portaal), en daar zou een niet doorgegeven parameter het onbekend maken. Wanneer: art. 3 van hetzelfde beleid (**fictief**), op de dag van de aanslag, zodra het inkomensgegeven er is (`decided_on`). Zonder inkomensgegeven is die dag onbekend, en dan geeft de cel geen dag (een onbekende dag is nog geen dag). Het democorpus heeft geen `dossier`-regel meer.
- **De nabetaling.** `fictief_beleid_toekenning_toeslagen` art. 4 (**fictief**): een executogram `nabetaling_opgedragen` dat naar de toekenning verwijst (fase TOEKENNING), per berekeningsjaar, op de dag van de toekenning (binnen de vier weken van Awir 24 lid 1), met wat er nog uit te betalen is minus wat al is opgedragen en niet geweigerd (art. 7 van het teruglezen). Zelfde velden als een betaalopdracht, dus dezelfde bankvoorwaarden (art. 1) en hetzelfde kanaal; het antwoord legt art. 5 vast (`nabetaling_betaald`, `nabetaling_mislukt`). Een nabetaling telt niet als uitbetaald voorschot. Een geweigerde nabetaling gaat mee met de opdracht van de eerste van de volgende maand.
- **Eén antwoord, twee ontvangers (generiek, cel en demo).** De bank antwoordt op een termijn en op een nabetaling met hetzelfde artikel. Beide kanalen bieden het antwoord aan; een ontvangend artikel dat de gram waarnaar het verwijst niet beantwoordt, weigert met de soort `not_addressed` (`Error::NotAddressed`), en de demo telt dat niet als fout. Een bericht dat voor geen enkel artikel is, komt in de outbox.
- **De terugvordering is een eigen besluit.** Awir 26 in het democorpus (2025 en 2026), met een eigen procedure `terugvordering` in de Awir (fasen TERUGVORDERING `is: BESLUIT` en TERUGVORDERING_BEKENDMAKING, **eigen keuze** zoals de procedure `tegemoetkoming`). Zo loopt de levensloop van de zaak niet door de terugvordering heen, en haken Awb 3:46 en 6:7 erop. Awir 28 lid 1 is een haak in die fase (de uiterste betaaldatum, zes weken). Awir 26 besluit `decides_on` de aanvraag van Awir 15 (**eigen keuze**: de wet noemt de terugvordering geen besluit op de aanvraag, maar zij hoort bij de tegemoetkoming die op die aanvraag is toegekend). Het besluit dat de aanvrager vraagt, is het besluit op de aanvraag waarvan de procedure een fase voor de aanvraag heeft; de terugvordering heeft die niet (`shape::submission_fields`). Wanneer: `fictief_beleid_toekenning_toeslagen` art. 6 (**fictief**), op de dag van de toekenning als die iets terug te vorderen laat (na 26a); anders geen dag. Het bedrag leest art. 6 van het teruglezen uit de toekenning. **Eigen keuze:** art. 26a lid 1 laat de beschikking tot terugvordering bij een klein bedrag "op nihil" vaststellen; de demo legt alleen een terugvordering vast die iets terugvordert.
- **De incasso.** `fictief_beleid_toekenning_toeslagen` art. 7 (**fictief, aanname**: de belanghebbende gaf toestemming): een executogram `incasso_opgedragen` dat naar de terugvordering verwijst, op de eerste van de maand na de dagtekening (binnen de zes weken van Awir 28). De bank voert haar voorwaarden uit (`fictieve_bankvoorwaarden` art. 2, **fictief**): afschrijven, tenzij de rekening onbekend of geblokkeerd is of het saldo te laag. Het saldo leest de bank uit haar eigen kroniek met `fictief_beleid_kroniek_bank` art. 1 (bijgeschreven min afgeschreven) plus het beginsaldo; daarvoor mag een ontvangst nu ook `reads` hebben (generiek: de cel voert het beleid uit met het bericht als parameters). Het antwoord legt Toeslagen vast met art. 8 (`terugvordering_geind`, `incasso_mislukt`). Een geweigerde incasso blijft openstaan; de eerste van de volgende maand volgt een nieuwe opdracht. Op het scherm Gevolgen gaat het saldo omlaag.
- **Wanneer de klok een besluit neemt (generiek).** `Cell::due_decision` kijkt bij een besluit met `decided_on` en een periode naar elke periode van de zaak zonder dat besluit, vanaf de aangevraagde: de eerste waarvoor het beleid een dag geeft. Een jaar zonder dag (niets terug te vorderen, nog geen inkomensgegeven) houdt een later jaar niet op (**eigen keuze**).
- **Geen dag is niet verschuldigd (generiek, cel; review 9 oktober 2026).** Voor `decided_on` kent de cel één betekenis: een dag is wanneer het besluit verschuldigd is, geen dag (null, of een dag die rust op een gegeven dat er nog niet is) is nog niet verschuldigd. `due_decision` en `due_ex_officio` geven dan geen dag, en `decide` weigert het besluit vóór zijn dag én zonder dag; `preview_decision` vraagt dat niet (een voorbeeld laat zien wat de wet zou besluiten, ook vóór de dag; review 9 oktober 2026, ronde 2). Eén uitzondering: het besluit dat de aanvraag beantwoordt, voor het jaar waarvoor zij is gedaan. Dat is het besluit in de eerste besluitfase (`is: BESLUIT`) na de fase van de aanvraag in zijn procedure (`shape::answered_by`; zonder eigen procedure elk besluit van het artikel): bij de Awir het voorschot (VOORSCHOT), niet de toekenning (TOEKENNING), hoewel beide Zorgtoeslagwet art. 2 vestigen. Wie de aanvraag beantwoordt (de behandelaar, de levensloop van de zaak) neemt het; zo blijft het eerste voorschot (art. 4 geeft voor dat jaar geen dag) en de handmatige beoordeling werken. De toekenning over het aangevraagde jaar, een volgend jaar (Awir 15 lid 5), de terugvordering (eigen procedure) en een aanslag (geen aanvraag) nemen alleen de dag van de houder; de demo zet een besluit met `decided_on` zonder dag dus ook niet klaar voor de behandelaar. `due_ex_officio` vraagt bij een `first_period` na het jaar van nu nog niets, tot dat jaar er is; een reeks van meer dan `MAX_EX_OFFICIO_PERIODS` (50) jaren (**eigen keuze**) is een fout in plaats van een leeg antwoord.
- **Een afwijzing is ook een besluit (Awb 1:3; demo, review 9 oktober 2026).** Een volgend besluit dat niets toekent (`voldoet_aan_voorwaarden` onwaar) legt de demo vast met die uitkomst. Zo is het jaar beslist en volgt het volgende; zonder gram bleef de cel hetzelfde jaar geven, en kwamen latere jaren en de terugvordering nooit. Het eerste besluit dat de behandelaar afwijst, eindigt de zaak en komt niet in de kroniek, zoals voorheen. Kan de wet een volgend jaar nog niet beslissen, dan meldt de zaak dat per jaar. De demo neemt een besluit van de cel dat de levensloop niet neemt (de terugvordering, een ander artikel in een eigen procedure) op de dag die de cel geeft, ook het eerste; en per stap van de klok eerst de ambtshalve besluiten (de aanslag), dan de zaken, en na een besluit wat op dezelfde dag volgt.

Het gevolg voor Merijn: zie §8b.

## 6. Het berekeningsjaar en de geldende wet

Dit is nu het grootste gat. Het staat in `Cell::decide` en in de review van #1683, en de oude PoC vond het al (#1472).

- **Wet:** Awir 2 definieert het berekeningsjaar als kalenderjaar, Awir 8 neemt het inkomen over dat jaar. Het corpus modelleert `berekeningsjaar = $referencedate.year`: het jaar van de rekendatum, niet dat van de aanvraag.
- **Voorstel (eigen keuze):** het berekeningsjaar is een gegeven van de aanvraag (`aangevraagd_berekeningsjaar`, origin BELANGHEBBENDE, staat er al). Awir 2 leest het als parameter in plaats van uit de rekendatum. De cel voert de besluiten uit op een rekendatum in dat jaar: de rekendatum is **wat het besluit betreft**, niet de dag waarop het wordt genomen. De wet bepaalt welk moment dat is; voor een jaarbedrag ligt 1 januari van het berekeningsjaar voor de hand.
- **Gevolg:** de wetsversie volgt het berekeningsjaar. Een aanvraag voor 2025 die in 2026 wordt beslist, rekent met de wet van 2025, en de Awb-termijnen van de beslissing met de wet van de besluitdag. Dat zijn twee data. Opgelost in stap 1: de wet zegt bij het besluit over welke periode het gaat (`period`), de cel rekent op 1 januari van dat jaar en legt het besluit vast op de besluitdag. Dat de wet van een jaar de wet op 1 januari van dat jaar is, is wat `period.unit: year` betekent, en geen aanname van de cel: een wet die een andere dag bedoelt, heeft een andere eenheid nodig.

## 7. Voorschot op een geschat inkomen

- **Wet:** Awir 16 geeft een voorschot "tot het bedrag waarop de tegemoetkoming vermoedelijk zal worden vastgesteld". Dat rekent met een geschat inkomen, de toekenning met het definitieve.
- **Voorstel (eigen keuze):** het geschatte inkomen wordt een veld van de aanvraag (`geschat_toetsingsinkomen`, origin BELANGHEBBENDE). Zo gaat het bij Toeslagen in de praktijk ook. Awir 16 rekent ermee, Awir 19 met het inkomensgegeven uit de BRI. Het is één rekenregel (Zorgtoeslagwet 2) met twee bronnen voor het inkomen, en de herkomst zegt welke.
- **Demo:** de persona geeft zijn geschatte inkomen op in de aanvraag (via `demo-config.yaml`). Het definitieve inkomen per jaar komt uit `box1` met een jaarkolom en `select_on: year`. Dan krijgt de demo een verschil om te verrekenen.

## 8. De tijd vooruit in de demo

**Eén klok.** De peildatum van de demo (`state.referenceDate`) wordt de enige tijd. De bekendmaking, de bezwaartermijn en de zaakgebeurtenissen gebruiken die ook, in plaats van `nowIso()` (eigen keuze; nu lopen er twee tijdassen).

**Vooruit, niet terug.** Een gram mag niet in de toekomst liggen (RFC-050). Terugzetten zou de kroniek ongeldig maken. Terug kan dus alleen via "opnieuw beginnen" (de bestaande reset).

**"Naar het volgende moment".** In de Kroniek van een zaak toont de demo onder de feiten wat de wet als volgende moment geeft, als verwachting en niet als gram:
- de volgende voorschottermijn (art. 22, gevraagd voor de volgende maand);
- het einde van het berekeningsjaar;
- de aanslag IB en daarmee de termijn van Awir 19;
- de betaaldatum van een nabetaling (Awir 24, vier weken) of een terugvordering (Awir 28, zes weken).

Eén knop zet de peildatum op het eerstvolgende moment. De cel legt vast wat dan ontstaat (een betaalde termijn, een toekenning), en het portaal toont aan de burgerkant hetzelfde: ontvangen voorschotten, de toekenning en wat er nog komt of terug moet. Wat het volgende moment is, leest de demo uit de uitkomsten van de grammen, niet uit een eigen kalender.

**Bron:** de oude PoC had een logische klok per wereld met een wachtrij van (datum, trigger). Hier wordt dat de peildatum, met de volgende momenten afgeleid uit de kroniek.

**Pagina:** de bestaande pagina's (Portaal, Zaaksysteem met Kroniek), zoals afgesproken. De tijdsbediening hoort bij de zaak, niet bij een losse pagina.

**Gebouwd (stap 5, 7 oktober 2026).** De demo heeft één klok en de knop "Naar het volgende moment" in de Kroniek. Wat de volgende momenten zijn, haalt de demo uit voorbeelden van de cel (`preview_execution`, `preview_decision`): de wet uitgevoerd zonder vast te leggen. De datum van de aanslag (Awir 19, herkomst DOSSIER) kwam toen uit de gegevens van de persona, via `dossier` in `demo-config.yaml`; sinds 9 oktober 2026 is de aanslag een feit (§5b). Elk besluit heeft een eigen bekendmaking (fasen VOORSCHOT_BEKENDMAKING en TOEKENNING_BEKENDMAKING, `is: BEKENDMAKING`), zodat Awb 6:8 per besluit vuurt (eigen keuze, naar Awb 3:40 en 3:41). Het berekeningsjaar van de aanvraag is in de demo nog het jaar van de peildatum, dus een voorschot vóór het berekeningsjaar is in de demo niet te bereiken.

**Na de review (7 oktober 2026).** Wat de demo nog zelf uitrekende, komt nu uit de wet en de cel:

- **De dagtekening van een besluit** zegt de procedure van de Awir: de ene datum die de fase vraagt (`requires` van VOORSCHOT: `dagtekening_voorschot`; van TOEKENNING: `dagtekening_toekenning`). De cel leidt daar `dated_by` uit af; een wet die het anders zegt, zegt het met `produces.moment.parameter`. De cel vult alleen die parameter met de besluitdag; wat de fase verder vraagt en niemand geeft, weigert ze.
- **Op welke dagen een termijn valt**, zegt de cel (`Cell::due_executions`) uit `executed_on` (met `once_per` en `day`) en `until`. De dag in de maand (`day: 1`) is een keuze van het fictieve uitvoeringsbeleid en staat daar, gemarkeerd; de demo heeft geen eigen kalender meer. Eens per maand telt per zaak, ook na een herziening van het voorschot.
- **Wat er is ontvangen**, leest de demo met de lexostatus die de toekenning ook leest (`received` in `demo-config.yaml` wijst haar aan); de demo telt niet zelf op.
- **Vaste datums van een besluit dat nog komt** (de uiterste toekenningsdatum) vindt de demo met een heuristiek: een datum die gelijk is in twee voorbeelden van het besluit in verschillende kalendermaanden (vandaag en een maand later) telt als vast. Dat is een keuze van de demo, geen regel van de wet; dat de wet zelf een datum als vast markeert, is een latere stap.
- **De tijd gaat niet terug**: een kroniek weigert een gram die eerder is vastgelegd dan haar laatste.

## 8a. Een aanvraag voor meer jaren (Awir 15 lid 5)

**Gebouwd (9 oktober 2026).** De demo eindigde met "Zorgtoeslag toegekend, 15 april 2027" en liet het jaar erna leeg. Awir 15 lid 5: "Een aanvraag wordt geacht mede te zijn gedaan voor op het berekeningsjaar volgende berekeningsjaren." Er komt dus geen nieuwe aanvraag: op dezelfde aanvraag volgt elk jaar een voorschot, verleend vóór het jaar (Awir 16 lid 2), met de eerste termijn in december ervoor (Awir 22 lid 1), en later een toekenning.

- **Wet.** Awir 15 geeft `aanvraag_geldt_voor_berekeningsjaar`: het aangevraagde jaar en de jaren erna. Lid 6 (de Dienst deelt mee dat lid 5 eindigt) is niet gemodelleerd. Awir 16 en 19 rekenen met `berekeningsjaar`, het jaar van de tegemoetkoming (art. 2 lid 1 onder b), met rol TIJDVAK en grondslag lid 5: de belanghebbende wordt geacht ook voor dat jaar te hebben aangevraagd. Awir 16 verleent alleen een voorschot voor een jaar waarvoor de aanvraag geldt; die regel staat er uitgeschreven, omdat een bron naar art. 15 vanuit een haak op de aanvraag van art. 15 een kringverwijzing in de engine geeft.
- **Cel (eigen keuze, generiek).** Een besluit betreft één periode, en die geeft de cel: het eerste besluit van een gebeurtenis het jaar van de aanvraag (haar TIJDVAK), het volgende het jaar erna. Een gegeven jaar vóór dat van de aanvraag weigert ze. De zaak wordt voor die periode gelezen: een beleid krijgt `berekeningsjaar` als parameter. Een uitvoering met een periode loopt per periode van de besluiten waarnaar ze verwijst: verwijzingen, `once_per`, `until` en de verschuldigde dagen gelden per jaar. Zo betaalt december 2025 de laatste termijn van 2025 (of het bedrag ineens) naast de eerste van 2026, en beëindigt de toekenning over 2025 alleen de termijnen van 2025. De eigen periode van een haak op de aanvraag (Awir 16) is geen veld van de aanvraag.
- **Wanneer (eigen keuze, fictief beleid).** De wet geeft geen dag. `fictief_beleid_termijnbedrag_voorschot` art. 4: het voorschot voor een volgend jaar op 1 november ervoor. De stroom noemt dat artikel (`decided_on`); `Cell::due_decision` geeft periode en dag, en de cel neemt het besluit niet eerder. Valt 1 november al voor het eerste voorschot (een aanvraag in december), dan volgt het volgende voorschot dezelfde dag.
- **Schatting (aanname).** `fictief_beleid_kroniek_toeslagen` art. 4: zonder nieuwe schatting geldt de schatting uit de aanvraag ook voor een volgend jaar. Het voorschot leest alleen dat artikel (`reads: {regulation, article}`). Art. 1 en 3 (voorschot, achterstand) lezen per berekeningsjaar.
- **Wetsversies (eigen keuze).** De cel neemt de versie die op 1 januari van het jaar geldt. Het democorpus heeft geen Zorgtoeslagwet en geen standaardpremie voor 2026; voor 2026 geldt dus die van 2025 en is het voorschot even hoog. Het gram noemt de versie (`regulation_valid_from`). Een jaar zonder enige versie weigert de engine.
- **Demo.** Het eerste besluit van een gebeurtenis neemt de levensloop van de zaak, zoals voorheen. Elk volgend vraagt de klok aan de cel (`dueDecision`): op de dag van het beleid, of op de datum die het dossier voor dat jaar geeft (de aanslag). De demo neemt het zoals de wet het neemt, zonder behandelaar en zonder eigen bekendmaking (open punt). "Ontvangen" staat per jaar.

Open: lid 6 (beëindiging), een herziening van het voorschot bij een nieuwe schatting (Awir 16 lid 5, 17), bekendmaking en bezwaar van de besluiten na het eerste, een Zorgtoeslagwet en standaardpremie voor 2026 in het democorpus.

## 8b. Merijn, met aanslag en afrekening

Het verzamelinkomen in de (fictieve) aanslag wijkt bewust af van de schatting in de aanvraag (€ 22.000), zodat de demo beide richtingen toont: over 2025 lager (€ 16.000), over 2026 hoger (€ 32.000). Over 2024 staat er € 24.150 (een aanslag zonder zaak bij Toeslagen). Met de peildatum op 2 januari 2025 en de aanvraag die dag:

| Jaar | Voorschot | Betaald | Aanslag | Toekenning | Afrekening |
|---|---|---|---|---|---|
| 2025 | € 1.695 (2 januari 2025, 11 termijnen) | € 1.695 | 15 april 2026, € 16.000 | 15 april 2026, € 1.809 | nabetaling € 114 op 15 april 2026, bijgeschreven |
| 2026 | € 1.695 (1 november 2025, 12 termijnen vanaf december) | € 1.695 | 15 april 2027, € 32.000 | 15 april 2027, € 1.505 | terugvordering € 190 op 15 april 2027 (betalen vóór 27 mei 2027), afgeschreven op 1 mei 2027 |

De democorpus heeft voor 2026 geen eigen Zorgtoeslagwet of standaardpremie; het jaar 2026 rekent met die van 2025. Op 1 juli 2027 staat er € 4.867,50 op de rekening: het beginsaldo van € 423,50, plus alle termijnen en de nabetaling, min de terugvordering.

## 9. Wat bewust later komt

- de aanspraak per kalendermaand (Zorgtoeslagwet 2 lid 5); eerst een jaarbedrag over de termijnen
- herziening van het voorschot na een wijziging (Awir 16 lid 5, 17), herziening na de toekenning (20, 21, 21a)
- rente (27, alleen 2025), invordering (28, 29) en verrekening over regelingen heen (30)
- kanalen buiten de demo (nu draagt de demo de berichten tussen de cellen)
- de zienswijze bij terugvordering (26b)

## 10. Volgorde van bouwen

| Stap | Wat | Schatting |
|---|---|---|
| 0 | Awir opnieuw harvesten (2025, 2026), eigen PR | een halve dag, plus het nalopen van bestaande modellering |
| 1 | Berekeningsjaar uit de aanvraag (§6), rekendatum per besluit in de cel | een dag |
| 2 | Awir 16 voorschot en art. 22 als regel per maand, demo: voorschot op geschat inkomen | een à twee dagen |
| 3 | Lexostatus met peilmoment en `sum`; betaalde termijnen als grammen | een dag |
| 4 | Awir 19 toekenning en 24 verrekening, terugvordering met 26a | een à twee dagen |
| 5 | Demo: één klok, "naar het volgende moment", inkomen per jaar | een à twee dagen |

## 11. Besluiten en open vragen

Besloten (6 oktober 2026):

1. ~~Zorgtoeslagwet art. 2 draagt `decides_on` over aan Awir 16 en 19.~~ Vervangen op 7 oktober: de Awir haakt op het besluit van Zorgtoeslagwet 2 (§3).
2. Art. 22 wordt een regel per maand, geen lijst (§4).
3. Het geschatte inkomen wordt een veld van de aanvraag (§7).
4. Eerst de Awir opnieuw harvesten, voor 2025 en 2026 (stap 0).

Open:

- De hoogte van een termijn (§4): gelijke delen met het restant in de laatste termijn, als gemarkeerde uitvoeringskeuze tot er beleid van Toeslagen is?
- Het schema kent geen tijdvak voor een besluit op geen aanvraag (TIJDVAK alleen met BELANGHEBBENDE); de stroom van de Belastingdienst noemt het nu (§5b). Een voorstel voor het schema volgt.
- Awir 19 lid 2 (geen aanslag: uiterlijk 31 december van het jaar erna) en het inkomensgegeven zonder aanslag (het belastbare loon, AWR 21 onder e, 2°) zijn niet gemodelleerd: de Belastingdienst-cel stelt alleen aanslagen vast, en `fictief_beleid_toekenning_toeslagen` art. 3 kent alleen een toekenning na een aanslag (gemarkeerd als **aanname**). Zonder aanslag komt er dus geen toekenning. Volledig zou zijn: de Belastingdienst-cel legt ook een inkomensgegeven zonder aanslag vast uit haar eigen (fictieve) gegevens, en art. 3 valt zonder aanslag terug op de dag van lid 2.
- De beschikking tot terugvordering "op nihil" (Awir 26a lid 1, tweede volzin) legt de demo niet vast; de bekendmaking van de terugvordering (fase TERUGVORDERING_BEKENDMAKING) neemt de demo niet, zoals bij elk besluit na het eerste.
- De eerste aanslag die de Belastingdienst over een persoon neemt, is over het jaar vóór de peildatum waarop de demo hem gaat volgen; bij Merijn dus ook over 2024, zonder zaak bij Toeslagen.
