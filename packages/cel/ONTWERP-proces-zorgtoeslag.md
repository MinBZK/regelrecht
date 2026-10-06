# Ontwerpnotitie: het hele zorgtoeslagproces in de kroniek

Status: concept, ter bespreking. Bouwt voort op #1679 (de aanvraag als chronolex-feit) en #1683 (de aanvraag en het besluit in de demo).

Deze notitie beschrijft hoe de cel van Toeslagen het hele proces van de zorgtoeslag vastlegt: de aanvraag, het voorschot, de maandelijkse betalingen, de definitieve toekenning na het berekeningsjaar en de verrekening. Ze beschrijft ook hoe de demo daarvoor de tijd vooruit laat lopen. Bij elke keuze staat waar die vandaan komt: **wet**, **RFC** (RFC-022 op main, RFC-044/045/047 op `poc/chronolex`), **oude PoC** (de chrono-poc-PR's #1466 t/m #1482) of **eigen keuze**.

## 1. Het proces volgens de wet

De Awir in de tekst die in 2025 gold. Waar 2026 anders is, staat dat erbij.

| Stap | Artikel | Wat ontstaat | Wanneer |
|---|---|---|---|
| Aanvraag | Awir 15 | indiening | tot 1 september (2026: 31 december) van het jaar na het berekeningsjaar; geldt ook voor volgende jaren (lid 5) |
| Voorschot | Awir 16 | beschikking, "tot het bedrag waarop de tegemoetkoming vermoedelijk zal worden vastgesteld" | binnen 13 weken na de aanvraag; vóór het jaar bij een doorlopende aanvraag (lid 2) |
| Betaling voorschot | Awir 22 | termijnen | ritme volgt uit de dagtekening van het voorschot: 12 termijnen vanaf december als het vóór het jaar verleend is, anders de resterende maanden plus een inhaalbedrag, of ineens na 31 oktober |
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
| voorschot verleend | Awir 16 | decretogram | `on_application` | het voorschotbedrag, de termijnen (§4) |
| voorschottermijn betaald | Awir 22 | executogram | `decision` → voorschot | bedrag, maand |
| toekenning | Awir 19 jo. Zorgtoeslagwet 2 | decretogram | `on_application` | het vastgestelde bedrag, het inkomensgegeven waarop het rust |
| verrekening | Awir 24 | decretogram | `decision` → toekenning | nog te betalen, onverschuldigd betaald |
| terugvordering | Awir 26 | decretogram | `decision` → toekenning | het bedrag (na 26a) |

Wat de cel vastlegt, blijft wat de wet zegt (`produces.extensions.chronolex.establishes`), zoals nu bij de aanvraag.

**Wijziging ten opzichte van #1679:** het huidige besluit "zorgtoeslag toegekend" (Zorgtoeslagwet art. 2, vastgelegd op de besluitdag) splitst in een **voorschot** (Awir 16) en een **toekenning** (Awir 19). Zorgtoeslagwet art. 2 blijft het bedrag uitrekenen; de Awir zegt welk besluit het draagt.

**Eigen keuze, nog open:** of Zorgtoeslagwet art. 2 zelf `decides_on` blijft houden of dat Awir 16 en 19 die rol overnemen. De wet zegt: de toeslag wordt toegekend door de Dienst Toeslagen op aanvraag (Awir 14), met het bedrag uit de Zorgtoeslagwet. Ik stel voor dat Awir 16 en 19 `decides_on: awir#15` krijgen en Zorgtoeslagwet 2 het bedrag levert. Dan geldt het voor elke toeslag.

## 4. Betalingen: de termijnen komen uit de wet

**Bron:** Awir 22 en de oude PoC (#1466, #1469, #1482). Die zette verplichtingen in het besluitartikel (`extensions.chronolex.verplichtingen` met bedrag, ritme en grondslag) en benaderde het ritme van art. 22 met `ritme: $betalingsritme`.

**Voorstel (eigen keuze):** art. 22 **letterlijk** modelleren als uitkomst van het voorschotbesluit: een lijst termijnen `{maand, bedrag}`, afgeleid van de dagtekening en het voorschotbedrag. Lid 1, 2, 4 en 5 zijn rekenregels over maanden. Lukt dat niet met de huidige operaties, dan is dat een bevinding voor de engine (een lijst maken met FOREACH of LIST), geen reden om het ritme in de cel te zetten.

- **Een termijn die nog moet komen is geen feit** (RFC-044: "wat nog moet gebeuren is geen feit"). De termijnen staan als *uitkomst* in het voorschotgram. Een betaling wordt pas een gram als de maand er is.
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
- **Gevolg:** de wetsversie volgt het berekeningsjaar. Een aanvraag voor 2025 die in 2026 wordt beslist, rekent met de wet van 2025, en de Awb-termijnen van de beslissing met de wet van de besluitdag. Dat zijn twee data, en de engine moet ze allebei kunnen krijgen: één om de versie te kiezen, één voor de procedure. Dat moet nog worden uitgezocht.

## 7. Voorschot op een geschat inkomen

- **Wet:** Awir 16 geeft een voorschot "tot het bedrag waarop de tegemoetkoming vermoedelijk zal worden vastgesteld". Dat rekent met een geschat inkomen, de toekenning met het definitieve.
- **Voorstel (eigen keuze):** het geschatte inkomen wordt een veld van de aanvraag (`geschat_toetsingsinkomen`, origin BELANGHEBBENDE). Zo gaat het bij Toeslagen in de praktijk ook. Awir 16 rekent ermee, Awir 19 met het inkomensgegeven uit de BRI. Het is één rekenregel (Zorgtoeslagwet 2) met twee bronnen voor het inkomen, en de herkomst zegt welke.
- **Demo:** de persona geeft zijn geschatte inkomen op in de aanvraag (via `demo-config.yaml`). Het definitieve inkomen per jaar komt uit `box1` met een jaarkolom en `select_on: year`. Dan krijgt de demo een verschil om te verrekenen.

## 8. De tijd vooruit in de demo

**Eén klok.** De peildatum van de demo (`state.referenceDate`) wordt de enige tijd. De bekendmaking, de bezwaartermijn en de zaakgebeurtenissen gebruiken die ook, in plaats van `nowIso()` (eigen keuze; nu lopen er twee tijdassen).

**Vooruit, niet terug.** Een gram mag niet in de toekomst liggen (RFC-044). Terugzetten zou de kroniek ongeldig maken. Terug kan dus alleen via "opnieuw beginnen" (de bestaande reset).

**"Naar het volgende moment".** In de Kroniek van een zaak toont de demo onder de feiten wat de wet als volgende moment geeft, als verwachting en niet als gram:
- de volgende voorschottermijn (uitkomst van het voorschot);
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
| 2 | Awir 16 voorschot met termijnen (art. 22 letterlijk), demo: voorschot op geschat inkomen | een à twee dagen |
| 3 | Lexostatus met peilmoment en `sum`; betaalde termijnen als grammen | een dag |
| 4 | Awir 19 toekenning en 24 verrekening, terugvordering met 26a | een à twee dagen |
| 5 | Demo: één klok, "naar het volgende moment", inkomen per jaar | een à twee dagen |

## 11. Vragen voor jou

1. Mag Zorgtoeslagwet art. 2 zijn `decides_on` kwijt aan Awir 16 en 19 (§3)? Dan is het patroon toeslag-onafhankelijk.
2. Art. 22 letterlijk modelleren (§4), ook als dat een engine-uitbreiding voor lijsten vraagt?
3. Het geschatte inkomen als veld van de aanvraag (§7)?
4. Eerst de Awir opnieuw harvesten (stap 0)?
