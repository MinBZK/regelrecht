# Gesprekspunten chronolexografie

Vijf vragen die de cel-runtime oproept en die de positionpaper open laat of
anders beantwoordt dan wij nu doen. Daarna een lijst keuzes die we zelf hebben
gemaakt en gebouwd, ter bevestiging. Ze zijn bedoeld voor een gesprek met de
bedenkers van chronolexografie. Paperverwijzingen noemen de sectie van de
[positionpaper](https://chronolexografie.nl/position-paper/). Waar de runtime
een keuze maakt die de paper niet draagt, staat dat als eigen keuze in
[RFC-044](../../docs/src/content/rfcs/rfc-044.md) en als afwijking in
[de docs van de cel](../../docs/src/content/docs/components/cel.md).

Stand: september 2026, branch `aanvraag-cel`.

## 1. Wat is het lexogram?

**De paper** ("Het chronolexogram"): een lexogram is "de vastlegging van een
(mogelijke, toekomstige) wijziging in wet- of regelgeving, bijvoorbeeld de
invoering of wijziging van een wet". Zoals elk chronolexogram ontstaat het in
een cel: "elk chronolexogram wordt in een specifiek domein gecreëerd (tot feit
gemaakt) en daar ook opgeslagen".

**Wat wij doen.** Het lexogram is de geconsolideerde regeling-YAML in het
corpus, bijvoorbeeld de Wet op de politieke partijen per 1 januari 2026. Dat
bestand geeft de tekst zoals die op een datum geldt. Het hoort bij geen enkele
cel: elke cel en elk proces leest het. Zo legt RFC-022 §1.1 het vast.

**Het verschil.** De paper legt de wijzigingshandeling vast: "per 1 januari
wordt artikel 106 gewijzigd", vastgelegd door wie de wijziging vaststelt. Wij
leggen de toestand vast die daaruit volgt. In de termen van de paper is ons
YAML-bestand eerder een lexostatus van een wetgevercel: het resultaat van een
reductie over lexogrammen.

**Waarom het ertoe doet.**

- De paper noemt als voordeel dat chronolexografie de ontstaanscontext
  vastlegt en niet de resulterende toestand ("Precieze vastlegging van
  ontstaanscontext", het voorbeeld van de verhuizing). Voor de wet doen wij het
  omgekeerde.
- "Welke tekst gold op moment T, en wie besloot dat wanneer" kan nu alleen via
  `valid_from` op versiebestanden, niet via een kroniek met twee tijden.

**Vragen**

1. Moet een uitvoerende cel de wet kunnen lezen als lexogrammen in een cel van
   de wetgever? Of is een geconsolideerde stand van de wet een legitieme bron,
   net als een lexostatus van een andere cel?
2. Zo ja: wie is de cel van de wet? De wetgever, de officiële bekendmaking
   (Staatsblad), of het corpus dat de consolidatie bijhoudt?

**Ons voorstel.** Het corpus behandelen als de lexostatus van een (virtuele)
wetgevercel. Het receipt van een besluit verwijst dan naar die lexostatus, met
de hash van elke geladen regeling; dat doet het receipt al. Echte lexogrammen,
zoals een bekendmaking als gram, blijven buiten deze proof of concept.

Zie RFC-044 Open Question 2 en afwijking 45 in de docs van de cel.

## 2. Mag een cel een concept op proef reduceren?

**Wat wij doen.** Vóór het indienen toetst het portaal de aanvraag: is ze
volledig, en heeft deze aanvrager recht? Daarvoor stuurt het proces het concept
naar de cel (`POST /cellen/<cel>/api/lexostatus/<naam>/proef`). De cel bouwt het
gram in het geheugen, reduceert haar kroniek alsof het gram vastligt en geeft
die lexostatus terug. Er wordt niets vastgelegd. De proef van een handeling
(proefbesluit, proefbetaling) werkt hetzelfde.

**De paper.** Reductie is "de (inherent destructieve) activiteit van het
hergebruik van feiten" ("Reductie & Lexostatus"). Een concept is geen feit:
"wanneer een chronolexogram is vastgelegd, is hetgeen is vastgelegd 'tot feit
gemaakt'". De paper stelt de vraag zelf ook, als onderzoeksvraag ("Omgaan met
inconsistenties en voorgenomen besluiten"): "zijn 'voorgenomen' besluiten
reeds besluiten? … de juridische status van concepten en voornemens in de
chronolexografische vastlegging".

**Twee lezingen**

- **A. Proefreductie in de cel (huidige keuze).** Alleen de cel reduceert,
  zoals de paper eist ("Reductie vindt altijd plaats ín de cel"). Het begrip
  reductie wordt dan wel opgerekt naar iets dat geen feit is.
- **B. Toetsen is concluderen in het proces.** Het proces heeft het concept in
  handen en concludeert erover (de stap "concluderen"). De cel levert alleen de
  lexostatus van de echte feiten. Dan moet het proces de afleiding van de cel
  kunnen toepassen op feiten plus concept, zonder zelf grammen te zien.

**Vragen**

1. Is "wat zou de lexostatus zijn als dit een feit werd" een legitieme vraag
   aan een cel? Of hoort het bij het concluderen van het proces?
2. Hoe verhoudt dit zich tot het voorgenomen besluit: is een proefbesluit dat
   niet wordt vastgelegd een voornemen in de zin van de paper?

**Ons voorstel.** Lezing A houden als expliciet benoemde celfunctie,
"hypothetische reductie". Het antwoord is uitdrukkelijk geen lexostatus van
feiten, en kan nooit worden vastgelegd of doorgegeven. Lezing B wordt
haalbaar als reductie een engine-regel wordt (zie de engine-RFC bij RFC-044
Open Question 1): dan draait het proces dezelfde regel over feiten plus
concept, zonder dat de cel iets fictiefs reduceert.

Zie RFC-044 Open Question 4 en afwijking 32 in de docs van de cel.

## 3. Wie bepaalt wat vastlegbaar is?

**De paper** geeft geen antwoord, maar stelt een onderzoeksvraag ("Afspraken-
en governancemodel voor vastlegging"): "Hoe komen we tot afspraken over welke
feiten vastlegbaar zijn, en hoe wordt geborgd dat deze afspraken zowel
disciplinerend als open zijn? … eisen aan de vorm, zonder de inhoud te
beperken."

**Wat wij doen (eigen keuze)**

- **De cel dwingt de vorm af.** Een gram voldoet aan het schema en aan de
  stroomdefinitie van de cel, met een grondslag per event. De handelende
  actor is de `recording_actor` van de stroom. Een gram dat een zaak volgt,
  volgt een zaak die de cel kent. Een stage ligt één keer vast per besluit.
  Het rechtsmoment ligt niet na het vastleggen en niet vóór de zaak. De cel
  toetst dit onder hetzelfde slot als het schrijven.
- **Het proces beslist over de inhoud.** Het handelt niet uit zichzelf als de
  wet dat niet toelaat. Het betaalt niet meer dan de subsidievaststelling
  (Awb 4:52). Het maakt niet bekend op een wijze die Awb 3:41 niet kent.
- **Wat gebeurd is, wordt vastgelegd.** Meldt een behandelaar dat zo'n
  handeling toch heeft plaatsgevonden, dan legt de cel haar vast, met de reden
  als waarschuwing. De gevolgen volgen uit de reductie: onverschuldigd betaald
  (Awb 4:57), of een bekendmaking waarop geen bezwaartermijn loopt.

**Waar het wringt**

- "Eén stage per besluit" leiden we af uit RFC-022 §1.2 (de stages van één
  besluit delen een zaakkenmerk). Het houdt ook een feitelijk tweede besluit
  tegen: dat moet als wijziging met een eigen grondslag (bijvoorbeeld Awb
  4:49). Is dat een vormeis of een inhoudelijke beperking?
- De stroomdefinitie bepaalt welke events er bestaan. Wie de stroom schrijft,
  bepaalt dus wat vastlegbaar is. Nu is dat de configuratie van de cel, met een
  grondslag per event.
- Een besluit kan niet worden gemeld als gebeurd; het proces moet het nemen.
  Ook dat is een keuze tussen vorm en inhoud.

**Vragen**

1. Is de stroomdefinitie met een grondslag per event de afspraak over
   vastlegbaarheid die de paper bedoelt?
2. Hoort die afspraak bij de cel, bij de wet, of bij een gezamenlijk orgaan dat
   de afspraken voor een chronolexosfeer beheert?
3. Klopt de verdeling "vorm bij de cel, inhoud bij het proces, en wat gebeurd is
   wordt altijd vastgelegd"?

**Ons voorstel.** Deze driedeling als standpunt hanteren. RFC-044 §1 legt haar
nu vast als eigen keuze, niet als lezing van de paper.

## 4. Welke soorten chronolexogrammen zijn er?

**De paper** ("Het chronolexogram"): "we zien tenminste drie klassen (typen)
van chronolexogrammen voor ons": het lexogram, het decretogram ("de
vastlegging van een concreet besluit of beschikking") en het executogram ("de
vastlegging van daadwerkelijke levering of afhandeling (fulfillment) van een
zaak of dienst"). Een chronolexogram is "de vastlegging van één handeling of
besluit in de tijd". De paper vraagt zelf ("Typering en modelering van
chronolexogrammen"): "Is deze indeling exclusief of zijn er andere relevante
typeringen?"

**Wat wij doen (eigen keuze).** Twee typen erbij, binnen het open vocabulaire
van RFC-022 §1 (RFC-044 §3):

- **indiening**: wat een ander bij de vastleggende actor indient, zoals een
  aanvraag, een aanvulling of bescheiden. Vastgelegd wordt de ontvangst, in de
  vorm van het voorbeeld uit de paper: "Op 3 april heeft de gemeente-ambtenaar
  vastgesteld dat diezelfde dag door Lotje aangifte is gedaan". De inhoud is
  een bewering van de indiener; een besluit rekent met het feit uit de bron.
- **handeling**: wat de vastleggende actor zelf doet en geen besluit en geen
  levering is, zoals een verzoek om aanvulling, een bekendmaking, een
  mededeling of een statistiek.

Het decretogram is alleen een besluit in de zin van de Awb (1:3), het
executogram alleen een levering, zoals een betaling. Een gebeurtenis die een
handeling en een besluit tegelijk droeg, is gesplitst in twee grammen.

**Waar het wringt**

- De bekendmaking van een besluit is hier een handeling, geen executogram:
  er wordt niets geleverd. Maar zij brengt wel een stage van het besluit tot
  stand (RFC-008 BEKENDMAKING) en laat de bezwaartermijn lopen.
- Een indiening ligt in de cel van wie ontvangt. De paper zegt "elke actor
  houdt eigen feiten bij in een eigen cel". Dat de aanvrager iets indiende, is
  ook een feit van de aanvrager.

**Vragen**

1. Zijn indiening en handeling een legitieme uitbreiding van de drie klassen,
   of zijn het soorten van het executogram ("afhandeling")?
2. Hoort een indiening ook in een cel van de indiener, en hoe verhouden de twee
   grammen zich dan?

**Ons voorstel.** De twee typen houden als open uitbreiding, met `soort` voor
de fijnere indeling. De cel van de indiener blijft buiten deze proof of
concept.

## 5. Zaak, besluit en kroniek: hoe groepeer je feiten?

**De paper** ("Cel & Kroniek"): "Een chronolexocel kan één of meerdere
chronolexokronieken beheren, wat chronolexocellen in staat stelt feiten te
groeperen (vergelijk het bijhouden van meerdere ordners in een kast of meerdere
tabellen in een database). Bovendien bepaalt een chronolexokroniek de
segmentering van de tijdsas waarin chronolexogrammen elkaar opvolgen. Elke
chronolexokroniek heeft een eigen tijdsas."

**Wat wij doen (eigen keuze).** Een cel heeft een kroniek per soort feiten,
bijvoorbeeld een kroniek voor alle aanvragen en hun verloop. Binnen die kroniek
groeperen twee kenmerken (RFC-044 §4):

- het **zaakkenmerk**: de aanvraag opent een zaak, en alles wat volgt
  (aanvulling, besluit, bekendmaking, betaling) draagt het;
- het **besluitkenmerk**: een zaak kan meer besluiten hebben, zoals een
  voorschot, een vaststelling en een terugvordering, elk met eigen stages en
  een eigen bezwaartermijn.

Beide zijn een groepering in de registratie, geen toestand. De levensloop van
een besluit staat in de Awb (RFC-008, de procedure van een beschikking), de
stand van een zaak is een lexostatus van de cel (`zaakstand`). Binnen een zaak
gaat de tijd vooruit: een gram mag niet vóór het laatste rechtsmoment in die
zaak liggen.

**Waar het wringt**

- Een zaak heeft in de runtime een eigen ordening (de tijd gaat vooruit), maar
  is geen kroniek. In de paper bepaalt juist de kroniek de tijdsas.
- De paper kent geen zaak. RFC-008 wijst een aparte zaak als toestandsdrager
  af; het besluit is dat.

**Vragen**

1. Is een zaak in de termen van de paper een kroniek (een ordner per zaak, met
   een eigen tijdsas), of een groepering binnen een kroniek?
2. Moet de ordening binnen een zaak dan ook een eis van de kroniek zijn, of
   blijft het een regel van de runtime?

**Ons voorstel.** De zaak als groepering binnen een kroniek houden: dan blijft
reductie over alle zaken heen mogelijk (een werkvoorraad), en de kroniek
behoudt één tijdsas.

## Zelf besloten, ter bevestiging

Deze keuzes hebben we gemaakt en gebouwd. Ze staan in RFC-043, RFC-044 en de
docs van de cel. We leggen ze voor om te horen of ze in de geest van de paper
zijn, niet om er lang over te praten.

- **Cel en proces gescheiden.** De cel legt vast, bewaart en reduceert; het
  proces informeert, concludeert en laat vastleggen ("reductie vindt altijd
  plaats ín de cel", "synthese gebeurt bij de afnemer").
- **De wet noemt geen cel.** Een regeling zegt per parameter wie hem levert en
  onder welke regeling een register wordt bijgehouden (`origin`, RFC-043). Welke
  cel dat register bijhoudt, is configuratie van het proces.
- **Twee tijden per gram.** `op_moment` (wanneer het feit rechtens geldt) en
  `vastgelegd_op` (wanneer de cel het vastlegde), met peilen op beide: het
  "tijdreizen" van de paper, en de vorm van het voorbeeld van Lotje ("op 3
  april … per 2 april").
- **De levensloop van een besluit komt uit de Awb.** De stages AANVRAAG,
  BESLUIT, BEKENDMAKING en BEZWAAR staan als procedure in de Awb-regeling
  (RFC-008). De cel legt per stage een gram vast; de bezwaartermijn rekent de
  Awb uit.
- **Een fout wordt niet gewist.** Een correctie of een wijziging is een nieuw
  gram, een wijziging van een besluit een nieuw besluit met een eigen
  grondslag ("feiten worden nooit verwijderd, ook niet bij correcties").
- **Eén regeltaal is een wens, geen eis van de paper.** Reductie en synthese
  hebben nu een eigen configuratietaal. RFC-045 stelt voor ze als gewone
  engine-run te draaien; dat is een RegelRecht-vraag, geen chronolexografie.
