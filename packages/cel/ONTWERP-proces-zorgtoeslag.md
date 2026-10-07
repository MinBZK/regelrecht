# Ontwerpnotitie: het hele zorgtoeslagproces in de kroniek

Status: concept, ter bespreking. Bouwt voort op #1679 (de aanvraag als chronolex-feit) en #1683 (de aanvraag en het besluit in de demo).

Deze notitie beschrijft hoe de cel van Toeslagen het hele proces van de zorgtoeslag vastlegt: de aanvraag, het voorschot, de maandelijkse betalingen, de definitieve toekenning na het berekeningsjaar en de verrekening. Ze beschrijft ook hoe de demo daarvoor de tijd vooruit laat lopen. Bij elke keuze staat waar die vandaan komt: **wet**, **RFC** (RFC-022 op main, RFC-044/045/047 op `poc/chronolex`), **oude PoC** (de chrono-poc-PR's #1466 t/m #1482) of **eigen keuze**.

## 1. Het proces volgens de wet

De Awir in de tekst die in 2025 gold. Waar 2026 anders is, staat dat erbij.

| Stap | Artikel | Wat ontstaat | Wanneer |
|---|---|---|---|
| Aanvraag | Awir 15 | indiening | tot 1 september (2026: 31 december) van het jaar na het berekeningsjaar; geldt ook voor volgende jaren (lid 5) |
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

Wat de cel vastlegt, blijft wat de wet zegt (`produces.extensions.chronolex.establishes`), zoals nu bij de aanvraag.

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

- **Een termijn die nog moet komen is geen feit** (RFC-044: "wat nog moet gebeuren is geen feit"). Het voorschotgram legt geen toekomstige betalingen vast, alleen het bedrag en het moment van het besluit. Dat volgt de tekst ("elke volgende termijn telkens een maand later") en sluit aan bij het vooruitspoelen (§8).
- **De hoogte van een termijn** regelt de wet niet. Voorstel: het voorschotbedrag gedeeld door het aantal termijnen, met het restant in de laatste termijn (zoals de oude PoC, #1466). Dat is een uitvoeringskeuze en hoort in het uitvoeringsbeleid van Toeslagen, niet in de Awir. Zolang er geen beleid is, staat het als gemarkeerde keuze in het model.
- **Wie betaalt, zegt de wet niet**, maar het uitvoeringsbeleid of de cel (oude PoC: "betaler uit het wereldbestand"). Voor deze stap blijft het bij één cel: Toeslagen legt zelf vast dat de termijn betaald is. Een aparte betaalcel is een latere stap.

## 5. Verrekening: een reductie over de eigen kroniek

**Bron:** RFC-045 §2 en de oude PoC (#1472, #1479).

- **De toekenning (Awir 19)** rekent op het definitieve inkomensgegeven (Awir 8, AWR 21).
- **De verrekening (Awir 24 lid 2)** is toegekend bedrag minus **uitbetaalde** voorschotten (#1472: niet de verleende, want wat vervalt wordt nooit betaald). Dat "uitbetaald" leest de cel terug uit haar eigen kroniek, met een lexostatus `betaald_voorschot` die de betaalde termijnen telt. Dat is een reductie bij de bron, zoals nu `aanvraag`.
- **Openstaand per richting, niet netto** (#1479). Nabetaling en terugvordering zijn twee uitkomsten, want verrekenen is een rechtsvraag (Awir 24 lid 2 en 30).
- **Na de toekenning vervallen de nog niet betaalde voorschottermijnen.** Dat volgt uit de verwijzing en wordt geen eigen gram (#1472).
- **Terugvordering (Awir 26)**, met de drempel van 26a. De zienswijze (26b) is een Awb-achtige stap; die volgt later.

**Wat de lexostatus daarvoor nodig heeft:** optellen over meerdere grammen (een `sum` naast `pick: latest`) en lezen **op een moment**: `peilmoment` (effective_at ≤ t) en `bekend_op` (recorded_at ≤ t), gesorteerd op effective_at en daarna recorded_at (RFC-044 en RFC-045 §7). De compacte cel kent dat nog niet.

## 6. Het berekeningsjaar en de geldende wet

Dit is nu het grootste gat. Het staat in `Cell::decide` en in de review van #1683, en de oude PoC vond het al (#1472).

- **Wet:** Awir 2 definieert het berekeningsjaar als kalenderjaar, Awir 8 neemt het inkomen over dat jaar. Het corpus modelleert `berekeningsjaar = $referencedate.year`: het jaar van de rekendatum, niet dat van de aanvraag.
- **Voorstel (eigen keuze):** het berekeningsjaar is een gegeven van de aanvraag (`aangevraagd_berekeningsjaar`, origin BELANGHEBBENDE, staat er al). Awir 2 leest het als parameter in plaats van uit de rekendatum. De cel voert de besluiten uit op een rekendatum in dat jaar: de rekendatum is **wat het besluit betreft**, niet de dag waarop het wordt genomen. De wet bepaalt welk moment dat is; voor een jaarbedrag ligt 1 januari van het berekeningsjaar voor de hand.
- **Gevolg:** de wetsversie volgt het berekeningsjaar. Een aanvraag voor 2025 die in 2026 wordt beslist, rekent met de wet van 2025, en de Awb-termijnen van de beslissing met de wet van de besluitdag. Dat zijn twee data. Opgelost in stap 1: de wet zegt bij het besluit over welke periode het gaat (`period`), de cel rekent op 1 januari van dat jaar en legt het besluit vast op de besluitdag.

## 7. Voorschot op een geschat inkomen

- **Wet:** Awir 16 geeft een voorschot "tot het bedrag waarop de tegemoetkoming vermoedelijk zal worden vastgesteld". Dat rekent met een geschat inkomen, de toekenning met het definitieve.
- **Voorstel (eigen keuze):** het geschatte inkomen wordt een veld van de aanvraag (`geschat_toetsingsinkomen`, origin BELANGHEBBENDE). Zo gaat het bij Toeslagen in de praktijk ook. Awir 16 rekent ermee, Awir 19 met het inkomensgegeven uit de BRI. Het is één rekenregel (Zorgtoeslagwet 2) met twee bronnen voor het inkomen, en de herkomst zegt welke.
- **Demo:** de persona geeft zijn geschatte inkomen op in de aanvraag (via `demo-config.yaml`). Het definitieve inkomen per jaar komt uit `box1` met een jaarkolom en `select_on: year`. Dan krijgt de demo een verschil om te verrekenen.

## 8. De tijd vooruit in de demo

**Eén klok.** De peildatum van de demo (`state.referenceDate`) wordt de enige tijd. De bekendmaking, de bezwaartermijn en de zaakgebeurtenissen gebruiken die ook, in plaats van `nowIso()` (eigen keuze; nu lopen er twee tijdassen).

**Vooruit, niet terug.** Een gram mag niet in de toekomst liggen (RFC-044). Terugzetten zou de kroniek ongeldig maken. Terug kan dus alleen via "opnieuw beginnen" (de bestaande reset).

**"Naar het volgende moment".** In de Kroniek van een zaak toont de demo onder de feiten wat de wet als volgende moment geeft, als verwachting en niet als gram:
- de volgende voorschottermijn (art. 22, gevraagd voor de volgende maand);
- het einde van het berekeningsjaar;
- de aanslag IB en daarmee de termijn van Awir 19;
- de betaaldatum van een nabetaling (Awir 24, vier weken) of een terugvordering (Awir 28, zes weken).

Eén knop zet de peildatum op het eerstvolgende moment. De cel legt vast wat dan ontstaat (een betaalde termijn, een toekenning), en het portaal toont aan de burgerkant hetzelfde: ontvangen voorschotten, de toekenning en wat er nog komt of terug moet. Wat het volgende moment is, leest de demo uit de uitkomsten van de grammen, niet uit een eigen kalender.

**Bron:** de oude PoC had een logische klok per wereld met een wachtrij van (datum, trigger). Hier wordt dat de peildatum, met de volgende momenten afgeleid uit de kroniek.

**Pagina:** de bestaande pagina's (Portaal, Zaaksysteem met Kroniek), zoals afgesproken. De tijdsbediening hoort bij de zaak, niet bij een losse pagina.

## 9. Wat bewust later komt

- de aanspraak per kalendermaand (Zorgtoeslagwet 2 lid 5); eerst een jaarbedrag over de termijnen
- herziening van het voorschot na een wijziging (Awir 16 lid 5, 17), herziening na de toekenning (20, 21, 21a)
- rente (27, alleen 2025), invordering (28, 29) en verrekening over regelingen heen (30)
- een aparte betaalcel en kanalen
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
