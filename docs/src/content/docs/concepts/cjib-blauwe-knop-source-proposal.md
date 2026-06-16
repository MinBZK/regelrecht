---
title: "Voorstel: RegelRecht als engine achter de Blauwe-Knop-source van CJIB"
lang: nl
---

*Auteur: Anne Schuth · Datum: 2026-05-27 · Status: concept*

## Aanleiding

Mijn Betaaloverzicht (MBO, voorheen Vorderingenoverzicht Rijk) draait op een patroon dat in Nederland nog jong is maar principieel klopt: data blijft bij de bron, aggregatie gebeurt on-device in de burger-client, geen enkele overheidsorganisatie ziet het totaalbeeld. De onderliggende standaard is [Blauwe Knop Connect](https://vorijk.nl/standaard/connect/draft-bk-connect-00.html). De burger logt in via DigiD, krijgt een korte sessie, en haalt zelf bij elke aangesloten bronorganisatie zijn vorderingen op in het FCID-formaat. Voor zover wij uit publieke bronnen kunnen opmaken draait CJIB deze rol sinds 2025, net als de Belastingdienst, en zijn eind 2025 naar verwachting acht rijksorganisaties als Blauwe-Knop-source actief. Welke vorderingen CJIB vandaag precies via die source ontsluit (alleen Wahv, of ook mandaat-vorderingen) is nog te bevestigen; zie bijlage A, open vraag 2, en vraag 3 aan CJIB.

Wat aan dat patroon ontbreekt is juridische provenance per vordering. Een FCID-response van CJIB zegt vandaag "u heeft een vordering van €X". Wat ze niet zegt: op grond van welk artikel, berekend uit welke invoer, met welke bezwaartermijn die op het moment van bekendmaking daadwerkelijk is uitgerekend. Dat is de motiveringsplicht uit Awb 3:46, en een onderdeel van wat Nieuwland §5.4 met "samen zien" bedoelt.

RegelRecht heeft sinds 2024 een raamwerk opgebouwd waarmee een wetsartikel een uitvoerbare specificatie wordt: `legal_character` en `decision_type` voor besluiten, AWB-lifecycle als first-class construct (RFC-008), executie-trace per beschikking (RFC-013), federatie tussen organisaties via FSC (RFC-009). De wetten die nu in machine-leesbare vorm in het corpus staan dienen vooral als bewijs dat het raamwerk werkt; nieuwe wetten worden opgenomen op het moment dat een cel ze nodig heeft.

In december 2025 publiceerde de Denktank Achterkant van de Overheid het ontwerp [Nieuwland](https://achterkantvandeoverheid.nl/) en het [Chronolexografie-position paper](https://chronolexografie.nl/position-paper/). Daarin staat een coherent begrippenkader voor het digitaal vastleggen van de rechtstoestand: chronolexocellen, kronieken, en drie typen vastlegging (lexogram, decretogram, executogram). Het begrippenkader sluit aan op het werk aan VORIJK/MBO en op recente gesprekken met BZK.

Het idee van dit voorstel: **een RegelRecht-engine achter de Blauwe-Knop-source van CJIB zetten, te beginnen bij de Wahv**. CJIB blijft de cel die de source aanbiedt; RegelRecht is de engine die de inhoud van de FCID-response uitrekent. Niet als vervanger van wat CJIB nu draait, maar er naast. De source geeft op een burger-pull een FCID-response terug die op bedrag, termijn en rechtsmiddel-route gelijk *beoogt* te zijn aan CJIB's eigen systeem (of dat klopt, is precies wat de pilot meet), plus de juridische onderbouwing per vordering, omdat de engine die uit de wet afleidt. Het hele MBO-patroon (data-bij-de-bron, on-device aggregatie, geen centrale stapel) blijft volledig intact. We dragen ertoe bij, we breken er niets aan.

## Wat staat er niet in dit voorstel

Voor de zekerheid, omdat dit makkelijk verkeerd valt:

- **Geen vervanging.** Het lexogram dat we bouwen draait naast CJIB's huidige Wahv-uitvoering. Het doel van de pilot is dat de twee voor dezelfde casuïstiek tot hetzelfde bedrag, dezelfde termijn en dezelfde bezwaarroute komen. Pas als die matching klopt, is een vervolggesprek over rolverdeling op zijn plek. Eerder niet.
- **Geen centrale aggregatie.** MBO werkt by design on-device; de cel houdt haar eigen data en stelt die beschikbaar op pull-request van een burger-client, MBO aggregeert pas in die client. Niets in dit voorstel verandert dat. Het Blauwe-Knop-patroon is wat we ondersteunen, niet iets dat we hervormen.
- **Geen nieuw broker-mechanisme.** We hergebruiken Blauwe Knop Connect zoals het is. RegelRecht voegt onder de motorkap juridische provenance toe; het transport, de authenticatie en de aggregatie blijven Blauwe Knop.
- **Geen wederkerige samen-zien-implementatie.** Dit voorstel adresseert één kant van Nieuwland §5.4: de burger ziet meer dan vandaag. De wederkerige kant (de cel of een derde ziet, met machtiging, wat de burger via MBO geaggregeerd voor zichzelf ziet) valt buiten scope van deze pilot. Die kant raakt DigiD Machtigen, de gemachtigden-flow van Blauwe Knop Connect en de positie van schuldhulpverleners, en is eigen vraagstuk.

## Woordenlijst

Een korte vertaling van de Chronolexografie- en Blauwe-Knop-begrippen die in dit voorstel terugkomen, in CJIB-taal:

Chronolexografie onderscheidt drie typen vastlegging die in de rechtsstaat alle drie nodig zijn (lexogram, decretogram, executogram). De eerste drie termen hieronder zijn die typen:

- **Lexogram**: een wet of regeling in machine-leesbare vorm. Vergelijkbaar met "de regelingstekst zoals jullie compliance-team die interpreteert", maar dan in YAML die een engine direct kan uitvoeren. Voorbeeld: de Wahv zoals die geldt sinds 1 januari 2025.
- **Decretogram**: een concreet besluit. Bij CJIB: een Wahv-sanctie, een OM-strafbeschikking, of een bestuurlijke boete die CJIB namens een toezichthouder int. In RegelRecht-termen: een engine-output met `legal_character: BESCHIKKING`. FCID is daar één serialisatie van; het decretogram zelf is het primaire artefact. Voorbeeld: een Wahv-sanctie van €X die op datum Y aan kentekenhouder Z wordt opgelegd. (Let op: een door de rechter opgelegde schadevergoedings- of ontnemingsmaatregel die CJIB int, is géén decretogram in deze zin maar een rechterlijke uitspraak; zie bijlage A.)
- **Executogram**: een feit dat de afhandeling registreert. Bij CJIB: een binnengekomen betaling, een verleende kwijtschelding, een gestart deurwaardertraject. Concreet: een entry in een chronicle-stream-bestand. FCID is daar weer een serialisatie van. Voorbeeld: een betaling van €X die op datum Z bij CJIB binnenkomt onder zaakkenmerk Y.
- **Chronicle / chronolexochronicle**: een tijdlijn van vastleggingen die een cel bijhoudt. Een cel kan meerdere chronicles bijhouden (bijvoorbeeld één per regelingsgebied), elk een geordende reeks van haar decretogrammen en/of executogrammen.
- **Chronolexoreductie**: de afleiding van een lexostatus uit één of meer chronicles. Een filter-en-aggregatie-bewerking: "alle Wahv-besluiten in onze chronicle, voor BSN X, in of voorbij BEKENDMAKING, minus intrekkingen". Dit is wat een Blauwe-Knop-source op pull-moment uitvoert.
- **Lexostatus**: het *resultaat* van een chronolexoreductie. Bij CJIB: `openstaande_vorderingen` als lijst van vordering-records. De cel declareert welke lexostatussen ze aanbiedt en hoe ze elk uit haar chronicles worden afgeleid.
- **Cel (chronolexocell)**: een organisatie die kronieken bijhoudt, sleutels beheert en bevoegd gezag draagt. CJIB is een cel. NVWA, OM, DUO, CAK ook. Dit is niet nieuw maar dezelfde notie als wat RegelRecht eerder al `competent_authority` noemde.
- **Engine**: één van de componenten die in een cel kan draaien. Een cel kan één engine bevatten, meerdere, of een engine plus een legacy-systeem.
- **Blauwe-Knop-source**: een endpoint dat een bronorganisatie aanbiedt waar een geauthenticeerde burger-client (via DigiD + App Manager) zijn eigen FCID-records ophaalt. Geen push, geen centrale opslag.
- **FCID (Financial Claims Information Document)**: het JSON-formaat waarin een Blauwe-Knop-source vorderingen, betalingen en intrekkingen teruggeeft. Per response ondertekend door de bron-cel.
- **FSC (Federatieve Service Connectivity)**: het standaardmechanisme voor server-naar-server federatie tussen overheidsorganisaties. Naast Blauwe Knop, niet in plaats daarvan: Blauwe Knop is burger-naar-bron, FSC is bron-naar-bron (bijvoorbeeld een bevoegd schuldhulpverlener met machtiging).

In de huidige situatie wonen lexogram, decretogram en executogram in gescheiden systemen, met telkens een verlies aan context op de overgangen. De burger ziet via Blauwe Knop wel het bedrag, maar niet de beschikking of het artikel. De gevolgen daarvan zijn beschreven in Nieuwland en in eerdere publicaties van Kafkabrigade. De pilot die hieronder volgt sluit deze keten voor één wet bij één cel, zonder het Blauwe-Knop-patroon zelf te veranderen.

## Wat er nu klaar ligt

De [voorgestelde RFC-022](/rfcs/rfc-022) doet het architecturele werk. Generiek, niet CJIB-specifiek: een andere uitvoeringsorganisatie die morgen aansluit hoeft RFC-022 niet open te trekken.

Korte samenvatting van wat de RFC voorstelt:

- Lexogrammen (regelingen) blijven in `corpus/regulation/`. Decretogrammen zijn engine-output met `BESCHIKKING`. Executogrammen krijgen een eigen top-level directory `chronicles/`, naast het corpus, omdat een registratie-specificatie geen wet is.
- Het `chronicles/`-mechanisme is ook ontworpen als architectuurvoorbereiding op een toekomstige Wet gegevensboekhouding (Nieuwland §7.3.2): per cel verifieerbaar wat er feitelijk geregistreerd is, op grond van welke grondslag, op welk moment.
- De cel is geen nieuw concept maar hetzelfde als wat RFC-002 al `competent_authority` noemt en wat RFC-009 als `EngineIdentity` aan de engine-kant beschrijft.
- `decision_type` wordt voorgesteld om uitgebreid te worden met drie financiële-domein waarden (BETALINGSVERPLICHTING, STRAFBESCHIKKING, BESTUURLIJKE_BOETE).
- Intrekkingen zijn een nested besluit in de zin van RFC-008 (eigen AWB-lifecycle); `modality.is_intrekking_van` is alleen een backlink-veld naar het origineel besluit, geen nieuwe semantiek.
- Integraties hangen in een namespaced `extensions`-blok. Het `extensions.blauwe_knop`-blok markeert dat een rule of een chronicle-event in de FCID-response van de cel-als-Blauwe-Knop-source zichtbaar wordt. Activatie gebeurt in de cel-configuratie, niet in de wet zelf.
- Cross-cell queries hergebruiken het bestaande `source`-blok uit RFC-007. De cel-runtime kiest het juiste federatie-mechanisme: Blauwe Knop wanneer de cel een burger-client is, FSC wanneer de cel een bevoegde-instantie server is. Dezelfde regeling-YAML werkt in beide contexten.
- Rechtsbescherming wordt niet als nieuw veld geïntroduceerd: de cel leidt de rechtsmiddel-route af uit de procedure (`procedure_id`) van het decretogram, op het juiste moment, zodat de werkelijke einddatum meereist en niet een statische hint. Let op: dat is niet altijd Awb-*bezwaar*. De Wahv is bijvoorbeeld *lex specialis* met een eigen rechtsmiddel; het concrete Wahv-geval staat uitgewerkt onder "Het Wahv-artikel in YAML". RFC-022 §3.3 beschrijft de drie route-families.

RFC-022 raakt drie bestaande RFCs op punten die separaat aandacht vragen en in follow-up amendementen worden opgelost: RFC-007 (cel-resolutie in `source.regulation` en transport-keuze per cel-context), RFC-009 (sleutelhergebruik tussen FSC-signing en Blauwe-Knop-response-signing) en RFC-013 (de Execution Receipt's `loaded_regulations`-array wordt veralgemeniseerd naar alle geladen artefacten: ook chronicle-streams en lexostatus-definities). De RFC-009-vraag is de enige met een echt open beveiligingspunt; de RFC-007- en RFC-013-wijzigingen zijn additief. Voor de pilot maken we gebruik van de voorgestelde uitbreiding; de canonieke updates van die RFCs volgen na de werksessie.

Voor de werksessie hieronder is het niet nodig RFC-022 helemaal door te lezen. Dit voorstel is zelfstandig leesbaar. De RFC is er voor de IT-lead die wil zien hoe de mapping er onder de motorkap uitziet.

## Wat de pilot inhoudt

Voor één pilotwet (voorkeur: Wahv) leveren we drie samenhangende artefacten op.

**Een lexogram.** Een YAML-bestand `corpus/regulation/nl/wet/wet_administratiefrechtelijke_handhaving_verkeersvoorschriften/<valid_from>.yaml`. Dit is de Wahv in machine-leesbare vorm conform het RegelRecht-schema. Eén artikel produceert een `BESCHIKKING` met `decision_type: BETALINGSVERPLICHTING` en een `extensions.blauwe_knop`-hint die zegt: deze vordering hoort in de FCID-response zichtbaar te zijn zodra de procedure de bekendmaking-equivalente stage heeft bereikt. De rechtsmiddel-weg zit niet in de regeling, maar wordt uit de procedure afgeleid. Belangrijk: de Wahv volgt **niet** de standaard Awb-bezwaarprocedure. Het is lex specialis met een eigen rechtsmiddel: administratief beroep bij de officier van justitie (Wahv art. 6), daarna beroep bij de kantonrechter (Wahv art. 9). Het lexogram benoemt daarom een Wahv-specifieke `procedure_id` (niet de default `beschikking`), zodat de cel een `beroep_route` afleidt in plaats van een `bezwaar_route`. Dit Wahv-rechtsmiddel-model is een eigen stuk werk binnen de pilot; het is precies het soort detail dat de domeinexpert-validatie moet bevestigen.

**Een chronicle-stream.** Een YAML-bestand `chronicles/cjib_wahv_betalingen.yaml` met minstens drie events: `payment_received`, `kwijtschelding_verleend`, `deurwaardertraject_gestart`. Per event de juiste FCID-mapping in `extensions.blauwe_knop`. `kwijtschelding_verleend` declareert `references_decision: <kwijtschelding-besluit-id>` zodat de cel de bezwaarweg via dat besluit kan afleiden. `payment_received` en `deurwaardertraject_gestart` zijn feiten zonder bezwaar.

**Een werkende Blauwe-Knop-source.** Een RegelRecht-engine draait binnen een afgeschermde CJIB-pilot-omgeving met de Wahv-lexogram en de chronicle-stream geladen. De cel-configuratie activeert het `blauwe_knop_source`-blok en publiceert een Blauwe-Knop-source-endpoint. Wanneer een burger-client (MBO-app, of een test-client in de pilot) een geauthenticeerde pull doet voor deze burger, voert de cel een chronolexoreductie uit: ze loopt door de relevante chronicles, filtert op BSN en op `current_stage >= visible_from_stage`, voegt betalingen en kwijtscheldingen toe, en serialiseert het resultaat als FCID-response. De response is ondertekend met de Blauwe-Knop-signing-sleutel van de cel (voor de pilot een dedicated JWS-sleutel, zie "Vertrouwen en signing") en bevat per vordering de juiste rechtsmiddel-route met de werkelijke einddatum: voor de Wahv een `beroep_route` (Wahv art. 6, intake OM/CVOM), voor een gewone Awb-beschikking een `bezwaar_route` waarvan de einddatum door de AWB-6:8-hook op de BEKENDMAKING-stage is berekend.

Aan burger-zijde, on-device: een Wahv-vordering verschijnt in de MBO-app met een directe link naar het artikel, een verifieerbare verwijzing naar de executie-trace, een rechtsmiddel-knop met de juiste route (voor de Wahv: administratief beroep bij de OvJ) en de werkelijke einddatum, en, na betaling, een gekoppeld BetalingVerwerkt-event onder hetzelfde zaakkenmerk. De MBO-app voegt deze response samen met die van andere bronorganisaties in de burger-client. Er is geen centraal punt waar dat geheel bestaat.

### Drie casuïstiek-klassen in de pilot

De Wahv-baseline alleen test eigen-uitvoering door CJIB. Maar CJIB int ook namens andere bestuursorganen, en juist die mandaat-vorderingen zijn de moeilijke gevallen voor bezwaar-routing en cel-topologie. De pilot dekt daarom drie klassen:

1. **Wahv-sanctie** (CJIB is primair bestuursorgaan). Doel: baseline vergelijken tegen de bestaande Blauwe-Knop-source van CJIB.
2. **Eén CAK-eigen-bijdrage-zaak** (CAK is primair, CJIB voert uit onder mandaat). Doel: bezwaar-routing-mapping testen. De `bezwaar_route` wijst juridisch naar CAK, terwijl de burger operationeel CJIB belt. Vergelijken met wat de huidige Blauwe-Knop-source van CJIB hier doet, geeft de eerste echte test van de mandaat-keten.
3. **Optioneel: één OM-strafbeschikking** (Sv 257a). Doel: aantonen dat een niet-bestuursrechtelijk besluit als `decision_type: STRAFBESCHIKKING` in de FCID-response past zonder dat de Awb-bezwaar-derivatie erop wordt losgelaten. Een strafbeschikking kent geen Awb-bezwaar/beroep maar *verzet* bij de strafrechter (Sv 257e); de rechtsbescherming-route wordt dus uit het strafprocesrechtelijke model afgeleid, niet uit RFC-008. De UOV-uitzondering (Awb afdeling 3.4) hoort hier nadrukkelijk **niet** bij: UOV geldt nooit voor een strafbeschikking. Wie de UOV-`beroep_route`-afleiding wil testen, gebruikt daarvoor een echte UOV-Awb-beschikking, niet deze klasse.

Klasse 2 (CAK) hoort niet bij de Wahv-baseline van week 1, maar is de go/no-go-gate voor het uitbreiden voorbij de Wahv: een fundamentele mismatch tussen RegelRecht en CJIB-huidig in de CAK-bezwaarroute zou betekenen dat de mandaat-keten nog niet gemodelleerd kan worden. Dat moet als kennisvraag in de werksessie worden uitgezocht (hoe bepaalt CJIB nu de bezwaarroute voor mandaat-vorderingen, en hoe modelleert RegelRecht dat) vóórdat klasse 2 in de drie-maanden-vergelijking meedraait. Het zit bewust niet in de Wahv-lexogram; het hoort bij het mandaat-convenant of een aparte beleidsregel die mee-geladen wordt.

### Het Wahv-artikel in YAML

Het Wahv-eigen rechtsmiddelregime (hierboven beschreven) moet als RFC-008-procedure gemodelleerd worden: stages tot en met de uitreiking/bekendmaking, daarna de beroep-stage bij de OvJ. Het lexogram benoemt die procedure via een Wahv-specifieke `procedure_id`. Dat modelleerwerk is onderdeel van de pilot.

```yaml
# Primaire sanctie
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: BETALINGSVERPLICHTING
    procedure_id: wahv_sanctie           # Wahv-eigen regime, NIET de Awb-default
    extensions:
      blauwe_knop:
        payload: fcid
        category: ALGEMEEN
        visible_from_stage: BEKENDMAKING  # bekendmaking-equivalente stage van de Wahv-procedure

# Intrekking (bv. na gegrond beroep of administratieve correctie):
# een nieuwe BESCHIKKING met dezelfde decision_type, plus modality.
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: BETALINGSVERPLICHTING
    procedure_id: wahv_sanctie
    modality:
      is_intrekking_van: $oorspronkelijke_beschikking_id
    extensions:
      blauwe_knop:
        payload: fcid
        category: ALGEMEEN
        visible_from_stage: BEKENDMAKING
```

De cel ziet de `modality.is_intrekking_van` en serialiseert de intrekking-instance als `BetalingsverplichtingIngetrokken` in plaats van `BetalingsverplichtingOpgelegd`. Beide events delen het `zaakkenmerk` met de oorspronkelijke beschikking, zodat de burger ze in MBO in één tijdlijn ziet.

CJIB hoeft deze YAML niet zelf te schrijven; ons team doet de eerste versie. Wat we van CJIB nodig hebben is dat een domeinexpert verifieert dat het klopt: dat de bedragen, termijnen en de bezwaarweg overeenkomen met wat het bestaande Wahv-systeem produceert voor dezelfde casuïstiek.

## Wat de pilot CJIB oplevert

**Een tweede Blauwe-Knop-source naast de bestaande**, voor dezelfde Wahv-vorderingen, waarvan de output op bedrag- en termijn-niveau wordt *vergeleken* met de bestaande source (gelijkheid is de te toetsen hypothese, niet een gegeven), met daar bovenop volledige juridische provenance per vordering: per vordering de grondslag, de invoer waaruit het bedrag is berekend, en de rechtsmiddel-route met werkelijke einddatum. Die provenance-laag is wat de pilot hoe dan ook oplevert, is wat Nieuwland §7.2.1 vraagt, en is wat een FCID-response vandaag niet bevat. Dat twee sources naast elkaar kunnen draaien zonder dubbele vorderingen voor de burger, is een voorwaarde die in de werksessie afgesproken moet worden (deduplicatie-strategie met het MBO-team, vraag 6 aan CJIB); lukt dat niet, dan draait de pilot als enige source en vervalt alleen de naast-elkaar-vergelijking, niet de provenance-opbrengst.

**Eén bron voor norm, besluit en feit.** Het lexogram zit in het corpus; het besluit komt uit de engine; het feit komt uit de chronicle-stream. Wijzigt de wet, dan beweegt de FCID-response mee zonder aparte release in een tweede systeem. Compliance-werk en uitvoering komen samen onder hetzelfde artefact.

**Rechtsbescherming als ontwerp.** De procedure levert de termijn op het juiste moment (de notificatie-stage, niet het besluit). De cel pakt die termijn op en stuurt 'm mee als rechtsmiddel-route in elke FCID-response, met het juiste rechtsmiddel per regeling: voor de Wahv administratief beroep bij de OvJ (Wahv art. 6), voor een gewone Awb-beschikking bezwaar (Awb 6:7/6:8). Een Wahv-sanctie met automatische ophoging die voor iemand met laag inkomen disproportioneel uitwerkt, krijgt vanaf het moment van bekendmaking een zichtbare beroep-knop in MBO met de werkelijke einddatum, niet pas na een aanmaning. Dit is de operationalisering van Nieuwland §7.2.1, ingebed in het Blauwe-Knop-patroon dat MBO al draait.

**Een directe invulling van de Chronolexografie-architectuur, met behoud van organisatie-autonomie.** CJIB is een cel met eigen kronieken en eigen sleutels. NVWA, NEa, DUO, CAK kunnen straks elk hun eigen cel zijn, met dezelfde mappingsregels. Geen centraal systeem, geen vendor lock-in, en het Blauwe-Knop-patroon blijft het orchestratieprincipe.

**Voorspelbare schaalbaarheid voor nieuwe Blauwe-Knop-sources.** Sectorale toezichthouders die instromen krijgen `decision_type: BESTUURLIJKE_BOETE`, het juiste `procedure_id` per RFC-008, en een `extensions.blauwe_knop.category`. Geen schemawijziging per opdrachtgever, geen forks van regelingen alleen voor verschillen in MBO-aansluiting. Een nieuwe source wordt een lexogram plus een chronicle-stream plus een cel-config-toggle, niet een nieuw stuk integratie-infrastructuur.

## Wat wij van onze kant inbrengen

Concrete tegenprestatie, geen open einde aan de CJIB-kant:

- **Het lexogram en de chronicle-stream** worden door ons geschreven, op basis van CJIB's bestaande Wahv-uitvoering en de wetstekst. Eerste versie binnen twee weken na vaststelling van de pilotwet.
- **Een pilot-sandbox** waarin de RegelRecht-engine een tweede Blauwe-Knop-source aandrijft, naast de bestaande, gevoed door dezelfde input als CJIB's huidige systeem zou krijgen. CJIB hoeft geen productie-IT te raken voor de pilot.
- **Een koppelteam** van twee mensen: ik (Anne) als architect en één engineer voor de implementatie van de chronicle-stream, de cel-config en het Blauwe-Knop-source-endpoint. Aan CJIB-kant is één domeinexpert en eventueel één IT-contact genoeg om in een wekelijks ritme te valideren.
- **Volledige documentatie** van de mapping per artikel, controleerbaar tegen het bestaande Wahv-systeem. Mismatches zijn data voor het volgende gesprek, niet een falen.
- **Geen leveringsverplichting** als de pilot niet werkt. Na drie maanden Wahv-vergelijking is een no-go besluit een legitieme uitkomst, geen mislukking.

## Wat we van CJIB nodig hebben

Zes dingen, geen open einde.

1. **Validatie van het uitvoeringslandschap** (zie bijlage A). Het overzicht is opgebouwd uit publieke bronnen. Welke regelingen ontbreken of zijn fout toegewezen?
2. **Bevestiging of bijstelling van de pilotwet.** Wahv is een goede pilot omdat het juridisch kader helder is (enkelvoudig artikel, weinig nesting), CJIB de wet al uitvoert (mismatches zijn meetbaar tegen een bestaande baseline), en de foutmarge bij een afwijking laag is (parkeerboete, geen bijstand). De vergelijking-naast-de-bestaande-source veronderstelt wel dat CJIB de Wahv vandaag al via een Blauwe-Knop-source ontsluit; dat is open vraag 2 (bijlage A) en moet in de werksessie eerst bevestigd worden. Zo niet, dan draait de pilot als eerste source in plaats van als tweede, en vervalt alleen het naast-elkaar-vergelijken, niet de pilot zelf. Liever iets anders? OM-strafbeschikking voor één feitcode is een optie. NVWA-bestuurlijke boetes zouden de schaalbaarheid scherper testen omdat het sectoraal is.
3. **FCID-versie en status van jullie huidige Blauwe-Knop-source.** Volgens vorijk.nl is v3.0.0 momenteel de stabiele lijn en is v4.x experimenteel (te verifiëren). Welke draait nu in jullie pilot of productie, op welke endpoints, en welke FCID-event-typen ondersteunt jullie source vandaag? Welke vorderingen zijn al ontsloten (alleen Wahv, of ook CAK/OM-vorderingen)?
4. **Knelpunten in de mapping.** Voor `zaakkenmerk` geldt CJIB's eigen zaaknummer-systematiek als leidend. Voor signing van de FCID-response gaan we uit van de FSC-key uit RFC-009. Botst dit met de sleutels die jullie nu al voor de bestaande Blauwe-Knop-source gebruiken, of kunnen we dezelfde sleutel hergebruiken? Plus: hoe verloopt de bedrag-afronding op centen in jullie bestaande uitvoering (een beleidsregel-detail dat we in het lexogram moeten codificeren)?
5. **Cel-topologie en bezwaar-routing.** Hoeveel cellen zou CJIB draaien (één centraal, één per opdrachtgever, één per regelinggebied), hoeveel chronicles per cel, en hoe verhoudt dat zich tot de bestaande Blauwe-Knop-source (één source voor alles, of meerdere)? En per type vordering: waar landt het bezwaar formeel, en waar landt het in de praktijk? Een CAK-eigen-bijdrage-vordering staat onder CJIB-zaaknummer maar het bezwaar gaat juridisch naar CAK; in de operationele realiteit belt de burger naar CJIB. De `bezwaar_route` in de FCID-response moet kloppen met beide werelden. Dit punt wil ik graag samen met het VORIJK/MBO-team uitwerken.
6. **Deduplicatie bij twee parallel-lopende sources.** Tijdens de pilot draait CJIB voor de Wahv twee Blauwe-Knop-sources naast elkaar: de bestaande en de RegelRecht-aangedreven. De MBO-app krijgt voor dezelfde vordering twee FCID-records. Hoe wil het MBO/VORIJK-team dat de app dedupliceert (op `zaakkenmerk`, op `trace_id`-origin, op een ander criterium)? Het antwoord hierop bepaalt of we naast elkaar kunnen draaien zonder dat de burger dubbele vorderingen ziet.

## Volgende stap

Een werksessie van een dagdeel met CJIB, het VORIJK/MBO-team en BZK. Agenda: het uitvoeringslandschap valideren, de pilotwet vastpinnen, de cel-topologie schetsen, de bezwaar-routing per type vordering uitwerken, deduplicatie-strategie met MBO-team afspreken, knelpunten benoemen, en bevestigen dat de nieuwe Blauwe-Knop-source naast de bestaande kan draaien. Daarna kan de voorgestelde RFC-022 verder, kan de bijbehorende schema-bump (v0.5.4 → een nieuwe v0.6.0) worden voorbereid, en kunnen we beginnen met het Wahv-lexogram en de eerste chronicle-stream.

Doel: binnen één maand na de werksessie een werkende Blauwe-Knop-source in een pilot-omgeving, met één Wahv-beschikking die op een geauthenticeerde burger-pull in het MBO-pilotportaal verschijnt, een rechtsmiddel-knop bevat met de juiste route en termijn (voor de Wahv: administratief beroep bij de OvJ), en teruggetraceerd kan worden naar het wetsartikel. Daarna drie maanden vergelijking tegen de bestaande CJIB-Blauwe-Knop-source over alle drie casuïstiek-klassen. Pas daarna een gesprek over wat volgt. Geen jaartallen op het pad daarna; eerst dit laten kloppen.

## Bijlagen

- [Bijlage A: CJIB-uitvoeringslandschap](#bijlage-a-cjib-uitvoeringslandschap)
- [Bijlage B: een door RegelRecht aangedreven Blauwe-Knop-source in detail](#bijlage-b-een-door-regelrecht-aangedreven-blauwe-knop-source-in-detail)
- [RFC-022: Chronolexogram types in the schema and the cell model](/rfcs/rfc-022)
- [RFC-009: Multi-Organisation Execution](/rfcs/rfc-009)
- [Chronolexografie-position paper](https://chronolexografie.nl/position-paper/) van de Denktank Achterkant van de Overheid, december 2025
- [Nieuwland, een ontwerp voor een digitale rechtsstaat](https://achterkantvandeoverheid.nl/) van Denktank Achterkant van de Overheid, 15 december 2025
- [Blauwe Knop Connect-standaard](https://vorijk.nl/standaard/connect/draft-bk-connect-00.html)
- [FCID-spec op vorijk.nl](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/)

---

## Bijlage A: CJIB-uitvoeringslandschap

Deze bijlage inventariseert wat CJIB feitelijk doet: welke regelingen het zelf uitvoert, welke het namens andere organisaties uitvoert, en het beleidskader daaromheen. Niet normatief; achtergrondmateriaal voor de werksessie. Onzekerheden zijn met `[onzeker]` gemarkeerd.

### Waarom CJIB centraal staat

CJIB is een *zelfstandig bestuursorgaan* (ZBO) onder het ministerie van Justitie en Veiligheid. Het is een centraal financiële-handhavingsknooppunt van de Nederlandse staat: een groot deel van de administratiefrechtelijke en strafrechtelijke financiële verplichtingen komt hier terecht wanneer een burger niet vrijwillig betaalt (voor wat er *niet* via CJIB loopt, zie "Wat CJIB niet doet"). De onderstaande tabellen geven dertien opdrachtgevers; interne USB-lijsten noemen er naar verluidt meer (zie open vraag 1). Hoeveel het er precies zijn, is een van de dingen die we in de werksessie willen valideren.

Voor zover uit publieke bronnen is op te maken behoort CJIB tot de eerste Blauwe-Knop-sources, samen met de Belastingdienst; tegen eind 2025 zouden naar verwachting de acht oorspronkelijke CRI-rijksorganisaties als Blauwe-Knop-source actief zijn (Belastingdienst, Dienst Toeslagen, CJIB, DUO, SVB, CAK, UWV, RVO), met een uitgebreide kring (NVWA, RDI, RDW e-Tol, Inspectie JenV, NEa, DFEI, ATKM) in de pijplijn. Wat de status van CJIB's bestaande source precies is (welke FCID-versie, welke vorderingen al ontsloten zijn) weten we niet uit publieke bronnen; dat is open vraag 2 en vraag 3 aan CJIB. Wat CJIB hoe dan ook de logische eerste cel maakt is de breedte van de portfolio (het centrale incasso-knooppunt voor de meeste opdrachtgevers), niet een claim over wie de meest gevorderde implementatie heeft.

### Mapping op de drie chronolexogram-typen

CJIB's dagelijkse werk raakt alle drie de vastleggingstypen:

- CJIB *voert lexogrammen uit*: de wetten en beleidsregels onder welke het werkt.
- CJIB *produceert decretogrammen*: Wahv-sancties, OM-strafbeschikkingen die het uitvoert.
- CJIB *registreert executogrammen*: binnengekomen betalingen, verleende kwijtscheldingen, gestarte deurwaardertrajecten.

RFC-022 plaatst elk van deze in zijn juiste plek in de repository-layout: lexogrammen in `corpus/regulation/`, chronicle-stream-definities (die declareren welke executogrammen een cel registreert) in `chronicles/`.

### CJIB's eigen wettelijke taken

| Regeling | Grondslag | BWB-ID | Wat het dekt |
|---|---|---|---|
| Wahv (Wet Mulder) | Wet administratiefrechtelijke handhaving verkeersvoorschriften | [BWBR0004581](https://wetten.overheid.nl/BWBR0004581) | Bestuursrechtelijke afdoening lichte verkeersovertredingen |
| OM-strafbeschikking | Wetboek van Strafvordering art. 257a–257h | [BWBR0001903](https://wetten.overheid.nl/BWBR0001903) | Afdoeningshandeling door OM, uitgevoerd door CJIB |
| Schadevergoedingsmaatregel | Wetboek van Strafrecht art. 36f | [BWBR0001854](https://wetten.overheid.nl/BWBR0001854) | Vergoeding aan slachtoffer, geïnd door CJIB |
| Voorschotregeling slachtoffer | Sr art. 36f lid 7 | [BWBR0001854](https://wetten.overheid.nl/BWBR0001854) | Staat schiet voor na 8 maanden; CJIB verhaalt op dader |
| Ontnemingsmaatregel ("Pluk-ze") | Wetboek van Strafrecht art. 36e | [BWBR0001854](https://wetten.overheid.nl/BWBR0001854) | Ontneming van wederrechtelijk verkregen voordeel |
| Wet DNA-V | Wet DNA-onderzoek bij veroordeelden | [BWBR0017212](https://wetten.overheid.nl/BWBR0017212) | DNA-afname veroordeelden. Kostenverhaal via beleidsregels `[onzeker]` |
| Tenuitvoerlegging strafrechtelijke beslissingen (Wet USB) | Wetboek van Strafvordering Boek 6, in werking 2020-01-01 | [BWBR0001903](https://wetten.overheid.nl/BWBR0001903/Boek6) | Verantwoordelijkheid voor executie van OM naar Minister; CJIB/AICE coördineert operationele keten |
| EU wederzijdse erkenning geldelijke sancties | Wet wederzijdse erkenning en tenuitvoerlegging geldelijke sancties en beslissingen tot confiscatie | [BWBR0022604](https://wetten.overheid.nl/BWBR0022604) | Grensoverschrijdende erkenning en inning van boetes/confiscatiebeschikkingen |

### CJIB als uitvoerder voor andere cellen

CJIB int namens een reeks opdrachtgevers; de tabel hieronder geeft er dertien op basis van publieke bronnen (het werkelijke aantal kan hoger liggen, zie open vraag 1). De juridische grondslag verschilt per geval: sommige zijn sectorale wetten die de minister van JenV of CJIB direct aanwijzen; andere zijn mandaatconstructies onder de Algemene wet bestuursrecht. Het Clustering Rijksincasso (CRI) programma, geformaliseerd via [eenoverheidsincasso.nl](https://www.eenoverheidsincasso.nl/onze-partners), structureert deze samenwerking.

In Chronolexografie-termen: de cel van de *opdrachtgever* produceert de primaire decretogram (de inhoudelijke beschikking); CJIB's cel registreert de executogrammen (betaling, kwijtschelding) namens die opdrachtgever. Of CJIB ook een eigen vervolg-decretogram produceert (bijvoorbeeld een dwangbevel, gedefinieerd in Awb 4:115, dat onder Awb 4:116 een executoriale titel oplevert) hangt af van de regeling en het convenant. Voor de Blauwe-Knop-presentatie betekent dit dat één FCID-record over een CAK-vordering meerdere bron-relaties heeft: CAK heeft het primaire besluit genomen, CJIB beheert de inning. De `bezwaar_route` moet die werkelijkheid weerspiegelen (zie ook open vraag 8 in bijlage A, en vraag 5 aan CJIB).

| Opdrachtgever | Type vordering | Grondslag (best available) | Decretogram-cel | Executogram-cel | Past in voorgestelde schema-uitbreiding? |
|---|---|---|---|---|---|
| OM | Strafbeschikking | Sv 257a | OM | CJIB | Nieuw `decision_type: STRAFBESCHIKKING` |
| Rechter (OM int) | Schadevergoedingsmaatregel, ontnemingsmaatregel | Sr 36f, Sr 36e (ontneming via Sv 511b-procedure) | rechter | CJIB | Géén `STRAFBESCHIKKING`: dit zijn rechterlijke maatregelen uit een vonnis/arrest, geen OM-beschikkingen. Rechtsmiddel is hoger beroep/cassatie, niet verzet; en `legal_character: BESCHIKKING` dekt ze niet. Apart te modelleren (rechterlijke-uitspraak-categorie), buiten scope van deze pilot |
| DUO | Studieschuld, lesgeld-achterstanden | [Wet studiefinanciering 2000 (BWBR0011453)](https://wetten.overheid.nl/BWBR0011453), Les- en cursusgeldwet `[BWB-ID te verifiëren]` | DUO | CJIB | Nieuw `decision_type: BETALINGSVERPLICHTING` |
| CAK | Eigen bijdrage Wmo/Wlz, wanbetalersregeling Zvw | [Zvw art. 18a–18d (BWBR0018450)](https://wetten.overheid.nl/BWBR0018450) | CAK | CJIB | Gedeeltelijk: `wet_langdurige_zorg` zit al in corpus |
| UWV | Terugvorderingen WW/WIA/Wajong | Sectorale werknemersverzekeringswetten + Awb titel 4.4 | UWV | CJIB | Nieuw `decision_type: BETALINGSVERPLICHTING` |
| RVO | Subsidie-terugvorderingen, agrarische boetes | Awb 4.4 + sectorale LNV/EZK-wetten | RVO | CJIB | Nieuw |
| NVWA | Bestuurlijke boetes voedselveiligheid, tabak, dier | [Warenwet](https://wetten.overheid.nl/BWBR0001969), Wet dieren, Tabaks- en rookwarenwet | NVWA | CJIB | Nieuw `decision_type: BESTUURLIJKE_BOETE` |
| RDW / e-Tol | Onverzekerd voertuig, MRB-schorsing | [WAM art. 30/34](https://wetten.overheid.nl/BWBR0002326), Wet MRB | RDW | CJIB | Nieuw |
| NEa | Emissiehandel-boetes | [Wet milieubeheer hoofdstuk 18 (BWBR0003245)](https://wetten.overheid.nl/BWBR0003245) | NEa | CJIB | Nieuw |
| Inspectie JenV | Bestuurlijke boetes | Per sectorale wet `[onzeker]` | Inspectie JenV | CJIB | Nieuw |
| RDI | Telecom-boetes | [Telecommunicatiewet (BWBR0009950)](https://wetten.overheid.nl/BWBR0009950) | RDI | CJIB | Nieuw |
| ATKM | Boete tot 6e categorie of 4% jaaromzet | Uitvoeringswet Verordening terroristische online-inhoud art. 12–13 | ATKM | CJIB | Nieuw |
| DFEI | Diverse | `[onzeker, CRI noemt "Dienst Financiële en Economische Integriteit"]` | onduidelijk | CJIB | Nieuw |
| Gemeenten | Via mandaat | Gemeentewet + lokale verordeningen | gemeente | CJIB | Gedeeltelijk: `participatiewet` en `apv_erfgrens` bestaan |

De acht oorspronkelijke CRI-rijksorganisaties zijn: Belastingdienst, Dienst Toeslagen, CJIB, DUO, SVB, CAK, UWV, RVO. Sinds 2024 is de Betalingsregeling Rijk uitgebreid naar onder andere NVWA, RDI, RDW e-Tol, Inspectie JenV, NEa, DFEI en ATKM.

### Wat CJIB niet doet

Het is goed dit expliciet te maken omdat lezers het vaak verkeerd attribueren. CJIB int **niet**:

- *Fiscale naheffingsaanslagen parkeerbelasting* (Gemeentewet art. 234). Dit zijn gemeentelijke belastingsancties; gemeenten innen via Cocensus, belastingsamenwerkingen, of in-house. Let op het onderscheid: een Wahv/RVV-*parkeerfeit* (fout parkeren, Mulder-feit) loopt wél via CJIB; alleen de fiscale naheffing voor het niet-betalen van parkeerbelasting loopt via de gemeente. Waar dit voorstel de Wahv-baseline kortweg "parkeerboete" noemt, gaat het om het Mulder-feit, niet om de fiscale naheffing.
- *Fiscale aanslagen* (inkomstenbelasting, BTW, etc.). De Belastingdienst voert zijn eigen invordering uit op grond van de Invorderingswet 1990. De Belastingdienst draait wel een eigen Blauwe-Knop-source voor die aanslagen.
- Gemeentelijke leges en lokale heffingen, zelfde reden als parkeerboetes.
- *Civielrechtelijke vorderingen* tussen particulieren. Die lopen via gerechtsdeurwaarders.
- Deurwaardersbeslag in privaatrechtelijke geschillen.

De lijn is grofweg: CJIB doet door de staat opgelegde financiële verplichtingen onder publiek recht (strafrechtelijk, bestuursrechtelijk, of specifieke civielrechtelijke slachtoffermaatregelen), specifiek wanneer de inning centraal op rijksniveau is belegd. Voor wat CJIB niet doet maar wel in MBO verschijnt, geldt: andere bronorganisaties draaien hun eigen Blauwe-Knop-source.

### Beleidskader

| Instrument | Jaar | Bron |
|---|---|---|
| Beleidsregels tenuitvoerlegging strafrechtelijke en administratiefrechtelijke beslissingen (USB 2021) | 2021 | [Stcrt 2021, 33851](https://zoek.officielebekendmakingen.nl/stcrt-2021-33851.html) |
| Wet USB + Invoeringswet USB | Stb 2017, 82; Stb 2019, 504; in werking 2020-01-01 | Boek 6 Sv |
| Aanwijzing OM-strafbeschikking | 2022A003 | [OM publicatie](https://www.om.nl/onderwerpen/beleidsregels/aanwijzingen/executie/aanwijzing-om-strafbeschikking-2022a003) |
| Algemene wet bestuursrecht titel 4.4 (Bestuursrechtelijke geldschulden) | In werking 2009-07-01 | [BWBR0005537 art. 4:85–4:125](https://wetten.overheid.nl/BWBR0005537) |
| Evaluatiewet bestuursrechtelijke geldschuldenregeling Awb (35.477) | In behandeling/aangenomen | [Eerste Kamer dossier](https://www.eerstekamer.nl/wetsvoorstel/35477_evaluatiewet) |
| CRI-programma (Clustering Rijksincasso) | Lopend | [eenoverheidsincasso.nl](https://www.eenoverheidsincasso.nl/) |
| Blauwe Knop Connect-standaard | Concept-versie 00, lopend | [vorijk.nl/standaard/connect](https://vorijk.nl/standaard/connect/draft-bk-connect-00.html) |

### Voorgestelde uitbreiding op het RegelRecht-schema

Het huidige `produces.decision_type` enum heeft negen waarden (TOEKENNING, AFWIJZING, GOEDKEURING, GEEN_BESLUIT, ALGEMEEN_VERBINDEND_VOORSCHRIFT, BELEIDSREGEL, VOORBEREIDINGSBESLUIT, ANDERE_HANDELING, AANSLAG). Geen daarvan beschrijft het financiële handhavingsdomein.

De voorgestelde RFC-022 voegt drie waarden toe, elk een afzonderlijk type besluit:

- `BETALINGSVERPLICHTING`: generieke financiële verplichting opgelegd door een bestuursorgaan
- `STRAFBESCHIKKING`: strafrechtelijke afdoening onder Sv 257a
- `BESTUURLIJKE_BOETE`: sectorale bestuurlijke boete

*Intrekkingen* van een eerdere beschikking zijn geen apart type: ze zijn dezelfde `decision_type` met `modality.is_intrekking_van: <id>`. Dit volgt de Awb-praktijk: een intrekking is een handeling op een bestaand besluit, niet een nieuw type besluit. `legal_character: BESCHIKKING` dekt al deze gevallen.

### Open vragen en data-gaten

De volgende items konden niet uit publieke bronnen worden geverifieerd en hebben input van CJIB of zijn opdrachtgevers nodig:

1. **Volledige CJIB-portfolio.** Interne USB-lijsten bestaan maar zijn niet publiek geïndexeerd.
2. **Status van CJIB's bestaande Blauwe-Knop-source.** Welke FCID-versie draait nu (v3.0.0 of v4.x), welke event-typen worden ondersteund, en welke vorderingen zijn al ontsloten? De pilot draait een tweede source naast deze; afstemming op FCID-versie en signing-sleutel is nodig.
3. **DFEI-scope.** CRI noemt "Dienst Financiële en Economische Integriteit", maar de exacte overdracht naar CJIB is onduidelijk.
4. **Cel-topologie bij CJIB.** Draait CJIB één cel per opdrachtgever, één per regelingstype, of één centraal? Hoe veel chronicles per cel (één per regelingsgebied, of één centrale)? Hoe verhoudt dat zich tot de bestaande Blauwe-Knop-source (één source voor alles, of meerdere)? RFC-009 en RFC-022 ondersteunen elke keuze; de keuze beïnvloedt chronicle-ordering en signing-sleutels.
5. **Bilaterale convenanten** die niet in Staatscourant gepubliceerd worden: er kunnen extra opdrachtgevers zijn die niet via publieke bronnen zichtbaar zijn.
6. **DNA-V kostenverhaalgrondslag.** BWBR0017212 koppelt kostenverhaal niet rechtstreeks aan CJIB; dit loopt waarschijnlijk via beleidsregels die nog geverifieerd moeten worden.
7. **Per-opdrachtgever BWB-IDs** met `[onzeker]` in de tabel.
8. **Rechtsmiddel-routing in de FCID-response.** Elke FCID-vordering draagt op het moment van pull de juiste rechtsmiddel-route, afgeleid uit de procedure van het decretogram. Welk rechtsmiddel geldt verschilt per regeling: de Wahv-sanctie die CJIB zelf produceert kent géén Awb-bezwaar maar administratief beroep bij de officier van justitie (Wahv art. 6) en daarna de kantonrechter (Wahv art. 9), dus de route is een `beroep_route` naar het OM/CVOM, niet een `bezwaar_route`. Een CAK-eigen-bijdrage-besluit dat CJIB namens CAK int, is wél een Awb-beschikking met een `bezwaar_route` die formeel naar CAK wijst, terwijl de burger in de praktijk vaak CJIB belt. Een OM-strafbeschikking kent verzet (Sv 257e). De routing per regeling moet per geval gevalideerd worden. Hoe bepaalt CJIB's bestaande Blauwe-Knop-source vandaag de route voor een CAK-vordering (hardcoded, of dynamisch)? Het antwoord bepaalt of de RegelRecht-cel dit uit een mandaat-convenant-modellering of uit een aparte beleidsregel-laag moet afleiden.
9. **Wet gegevensboekhouding-interactie.** Nieuwland §7.3.2 schetst een Wet gegevensboekhouding die de executogram-zijdige registratie een wettelijke basis zou geven. De huidige grondslag van CJIB is impliciet in Awb 4.4 + sectorale wetten; een expliciete wet zou het beeld wijzigen. De chronicle-stream-architectuur is bewust ontworpen om met die toekomstige wet mee te kunnen bewegen: per cel verifieerbaar wat er feitelijk geregistreerd is, op grond van welke grondslag, op welk moment.
10. **Burger-machtigingen.** Het Blauwe-Knop-patroon is burger-geïnitieerd via DigiD. Voor de schuldhulpverleningspraktijk en voor sommige sectorale uitvoeringscontexten is er behoefte aan een derde die met expliciete machtiging op een burger toegang krijgt. Of dat via DigiD Machtigen via dezelfde Blauwe-Knop-flow loopt of via FSC met een aparte machtigingscontext is uit publieke bronnen niet ondubbelzinnig af te leiden.
11. **Deduplicatie bij parallel-lopende sources** (zie ook vraag 6 aan CJIB). Tijdens de pilot leveren de bestaande en de RegelRecht-aangedreven source beide FCID-responses voor dezelfde Wahv-vorderingen. Welke sleutel hanteert de MBO-app voor deduplicatie? Mogelijke kandidaten: `zaakkenmerk` (mits beide sources hetzelfde zaaknummer hanteren), of `(zaakkenmerk, decision_type, bedrag)`-triplet plus `trace_id`-origin (de RegelRecht-aangedreven source levert `trace_id`, de oude bron mogelijk niet). De keuze hoort thuis bij MBO-team.

---

## Bijlage B: een door RegelRecht aangedreven Blauwe-Knop-source in detail

Deze bijlage specificeert hoe een cel met een RegelRecht-engine een Blauwe-Knop-source aandrijft en hoe diezelfde cel-mechaniek gebruikt kan worden voor cross-cel queries in beide voorkomende contexten (burger-client en bevoegde-instantie). Inhoud is technisch; bedoeld voor de IT-lead die de pilot begeleidt.

### Doelversie en transport

FCID-baseline: **v3.0.0** (volgens vorijk.nl de huidige stabiele versie; release-datum en de exacte status van de v4.x-lijn te verifiëren tegen vorijk.nl). De architectuur is voorbereid op v4.x zodra die productie-rijp is. De integratie-spec is herzienbaar zonder dat RFC-022 opnieuw opengaat. Welke versie de pilot daadwerkelijk gebruikt, wordt bepaald door wat CJIB's bestaande Blauwe-Knop-source draait (vraag 3 aan CJIB).

Transport en authenticatie volgen [Blauwe Knop Connect](https://vorijk.nl/standaard/connect/draft-bk-connect-00.html) ongewijzigd. De burger authenticeert via DigiD en de App Manager; de cel valideert de aangeboden authorization-token; de cel antwoordt met een FCID-response ondertekend met haar eigen sleutel (JWS). Niets in dit voorstel verandert het protocol; we leveren een tweede source-implementatie naast de bestaande.

### Wat de cel doet op een pull

Een cel die een RegelRecht-engine draait en het `blauwe_knop_source`-blok in haar cel-config activeert, gedraagt zich als een Blauwe-Knop-source. Op een geauthenticeerde pull-request voor een specifieke burger voert de cel een **chronolexoreductie** uit op haar eigen chronicles:

1. Filter alle decretogrammen in de relevante chronicles op `subject_bsn == <burger>`.
2. Hou alleen decretogrammen waarvoor `current_stage >= visible_from_stage` per regel-niveau (default `BEKENDMAKING`). Een decretogram dat dat niet is, valt af; geen lege FCID-records, geen halfslachtige zichtbaarheid.
3. Voeg de relevante executogram-events toe (betalingen, kwijtscheldingen) uit de chronicles waarin die voorkomen.
4. Serialiseer het resultaat als FCID-records, conform de veld-afleidingen verderop.
5. Onderteken de hele response met de cel-eigen signing-sleutel (één signature per response, niet per record).

De response is per cel ondertekend. Er is geen push, geen centraal endpoint, geen kopie buiten de cel. De burger-client (MBO-app) verifieert de handtekening en aggregeert lokaal met de responses van andere sources. FCID is een serialisatie van het decretogram/executogram, geen aparte FCID-state in de cel (zie RFC-022 §1.2).

### Cell-capabilities: welke lexostatussen biedt de cel aan

Een cel declareert welke lexostatussen ze aan derden ontsluit (RFC-022 §4.1); dat is een cel-eigen capability-declaratie, geen hardcoded eigenschap van de engine. Schets voor CJIB:

```yaml
# cells/cjib/capabilities.yaml (sketch; volledig formaat in latere RFC)
lexostatus_definitions:
  - name: openstaande_vorderingen
    description: >
      Alle openstaande betalingsverplichtingen voor de gegeven BSN, ontleend
      aan de Wahv-chronicle, de OM-uitvoerings-chronicle en de
      mandaat-uitvoeringschronicles, gefilterd op stage >= BEKENDMAKING
      en minus intrekkingen.
    inputs:
      - name: bsn
        type: string
    reduction:
      sources:
        - chronicle: cjib_wahv_decisions
          select_where: "decision_type IN (BETALINGSVERPLICHTING) AND current_stage >= BEKENDMAKING"
        - chronicle: cjib_om_executions
          # STRAFBESCHIKKING volgt het strafvorderlijke model, niet de Awb-lifecycle: de
          # zichtbaarheidsdrempel is uitreiking/betekening (Sv 257d), niet Awb-BEKENDMAKING.
          select_where: "decision_type IN (STRAFBESCHIKKING) AND current_stage >= UITGEREIKT"
        - chronicle: cjib_mandate_executions
          select_where: "decision_type IN (BETALINGSVERPLICHTING, BESTUURLIJKE_BOETE) AND current_stage >= BEKENDMAKING"
      exclude:
        - "modality.is_intrekking_van IS NOT NULL OR has_intrekking_in_chronicle = true"
      group_by: zaakkenmerk
      order_by: bekendmaking_datum DESC
```

Het CJIB-specifieke hier is uit welke chronicles de reductie samenvalt (Wahv-, OM-uitvoerings- en mandaat-chronicles), welke filters gelden en hoe duplicaten worden gegroepeerd. Een consument die `openstaande_vorderingen` opvraagt (via Blauwe Knop in een burger-context, of via FSC in een bevoegde-instantie-context, zie verderop) krijgt dezelfde reductie, geserialiseerd in het transport-eigen formaat. De Blauwe-Knop-source-response zelf is conceptueel ook een lexostatus (`blauwe_knop_fcid_response`), die grotendeels overlapt met `openstaande_vorderingen` plus enkele FCID-specifieke velden (`gebeurtenis_kenmerk`, `signature`-block). De exacte vorm van `capabilities.yaml` wordt in een vervolg-RFC vastgepind.

### Activatie van het `blauwe_knop_source`-blok

Een cel beslist zelf of ze als Blauwe-Knop-source draait; activatie is een feature-block in de cel-config, geen toggleable "rol" (RFC-022 §3.2). Het blok koppelt het endpoint, de FCID-versie, de signing-sleutel en de capabilities aan elkaar:

```yaml
# cel-config (schets; volledig cel-config formaat in latere RFC)
blauwe_knop_source:
  enabled: true
  endpoint: https://cjib.example/blauwe-knop/fcid   # cel-eigen, geen centrale endpoint
  fcid_version: 3.0.0
  signing_key_ref: cjib-blauwe-knop-2026            # mag dezelfde zijn als bestaande source
  serves_lexostatus: blauwe_knop_fcid_response      # verwijst naar capabilities.yaml
```

Een gemeente die het Wahv-lexogram draait maar geen Blauwe-Knop-source wil zijn, laat het `blauwe_knop_source`-blok in haar cel-config gewoon weg. Dezelfde regeling-YAML werkt in beide cellen.

### FCID-event-typen en chronolexogram-mapping

FCID definieert (voor zover uit de spec af te leiden) de volgende event-typen. Op één na (zie hieronder) is elk de serialisatie van precies één chronolexogram-type. De event-type-namen en hun precieze velden hieronder zijn ontleend aan de [FCID-spec op vorijk.nl](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/) en moeten tegen de definitieve spec-versie worden geverifieerd (vraag 3 aan CJIB); een afwijkende naam of veldnaam in de FCID-versie die CJIB draait, werkt door in deze hele mapping.

| FCID `event_type` | Chronolexogram-type | Brongegeven in de cel |
|---|---|---|
| `FinancieleVerplichtingOpgelegd` | decretogram | engine-output met `decision_type: STRAFBESCHIKKING` (totaalbedrag) |
| `BetalingsverplichtingOpgelegd` | decretogram | engine-output met `decision_type: BETALINGSVERPLICHTING` / `BESTUURLIJKE_BOETE` |
| `BetalingsverplichtingIngetrokken` | decretogram (intrekking-modaliteit) | engine-output, zelfde `decision_type` als origineel, met `produces.modality.is_intrekking_van` gezet |
| `BetalingsverplichtingIngetrokken` | executogram dat een intrekking-besluit referenceert | chronicle-stream-event (bv. `kwijtschelding_verleend`) met `references_decision` gezet (zie "Rechtsbescherming op executogram-records") |
| `BetalingVerwerkt` | executogram | chronicle-stream-event, getriggerd door intake vanuit incasso-systeem |

`BetalingsverplichtingIngetrokken` is bewust het enige event-type met twee bronnen: een intrekking kan in de cel wonen als een eigen decretogram (de cel nam zelf het intrekking-besluit) óf als een executogram dat een elders genomen intrekking-besluit registreert (de cel voert alleen de afhandeling uit, bv. een `kwijtschelding_verleend` namens een opdrachtgever). Beide serialiseren naar hetzelfde FCID-event omdat de burger in beide gevallen één feit ziet: zijn verplichting is ingetrokken. Welke bron geldt, leest de cel af aan welk veld gezet is (`modality.is_intrekking_van` voor het decretogram-pad, `references_decision` voor het executogram-pad); de `event_type`-afleiding verschilt navenant per pad (zie de twee veld-afleidingstabellen verderop).

Een intrekking is een nested besluit in de zin van RFC-008 OQ5 (zie RFC-022 §3.1). In het decretogram-pad herkent de cel haar via `produces.modality.is_intrekking_van: <oorspronkelijke-id>` en serialiseert haar als `BetalingsverplichtingIngetrokken`. Intrekking en origineel delen een `zaakkenmerk` zodat de burger-client ze als één tijdlijn presenteert.

De tabel dekt de intrekking van een `STRAFBESCHIKKING` (bijvoorbeeld nietigverklaring na geslaagd *verzet*, Sv 257e) bewust **niet**: dat hoort bij het strafprocesrechtelijke model dat samen met de `verzet_route`-afleiding is uitgesteld (zie RFC-022 §1.2). Of FCID daarvoor een `FinancieleVerplichtingIngetrokken`-event kent (spiegelbeeld van `FinancieleVerplichtingOpgelegd`) is onderdeel van vraag 3 aan CJIB; bestaat dat event, dan is de mapping triviaal, zo niet dan is er nog geen FCID-pad voor dit geval. Tot dat model er is, surfacet de pilot geen STRAFBESCHIKKING-intrekking.

### Producer-zijde: hoe een lexogram-regel zichtbaar wordt in de FCID-response

Een regel waarvan het decretogram in de FCID-response moet verschijnen, declareert dat in het `extensions.blauwe_knop`-blok:

```yaml
execution:
  produces:
    legal_character: BESCHIKKING
    # Generiek voorbeeld: een gewone Awb-beschikking (bv. een bestuurlijke boete),
    # die wél de Awb-default-procedure gebruikt. NIET de Wahv: die kent een eigen
    # regime en een eigen procedure_id (wahv_sanctie), zie "Het Wahv-artikel in YAML".
    decision_type: BESTUURLIJKE_BOETE
    procedure_id: beschikking         # RFC-008 procedure-selectie; Awb-default => bezwaar_route
    extensions:
      blauwe_knop:
        payload: fcid                 # voor nu altijd fcid; future-proof
        category: ALGEMEEN
        visible_from_stage: BEKENDMAKING   # default; overrijdbaar
```

`category` is hier voorgesteld als een van `ALGEMEEN`, `ADMINISTRATIEKOSTEN`, `VERHOGING`, `RENTE`. Deze waardenlijst is een RegelRecht-voorstel binnen de `extensions.blauwe_knop`-namespace, te toetsen tegen de FCID-spec (vraag 3 aan CJIB): als FCID een eigen category-vocabulaire kent, volgen we dat. Een regeling die meerdere FCID-records uit één beschikking produceert (hoofdsom + administratiekosten + verhoging) declareert die als aparte artikelen of aparte `produces`-blokken, elk met zijn eigen `extensions.blauwe_knop.category`. Alle records delen hetzelfde `zaakkenmerk`; de burger-client gebruikt `category` om ze in de UI samen te groeperen.

`visible_from_stage` selecteert de procedure-stage vanaf wanneer de vordering zichtbaar is; default `BEKENDMAKING` (of de bekendmaking-equivalente stage van de betreffende procedure). De cel evalueert per pull, per kandidaat-decretogram, of `current_stage >= visible_from_stage`; vorderingen die nog niet zo ver zijn vallen weg (geen null, geen placeholder). Het is bewust `visible_from_stage` en niet `emit_at_stage`: Blauwe Knop kent geen emit-moment, alleen een pull-moment (zie RFC-022 §3.2). De rechtsmiddel-route-velden zijn pas vanaf de notificatie-stage correct (voor Awb-beschikkingen schrijft de AWB 6:8-hook de `bezwaartermijn_einddatum` op BEKENDMAKING; voor de Wahv geldt het analoge moment van de eigen procedure), dus verlagen naar de besluit-stage is niet toegestaan voor decretogrammen die een route vereisen.

#### Veld-afleiding per FCID-record

| FCID-veld | Afleiding |
|---|---|
| `event_type` | uit `decision_type` plus `modality.is_intrekking_van` per de tabel hierboven |
| `category` | uit `extensions.blauwe_knop.category` |
| `juridische_grondslag_omschrijving` | bondige verwijzing naar het artikel, bijvoorbeeld `"Wahv art. 2 + bijbehorende beleidsregel"`. Niet de eerste zin van `article.text`: dat parafraseert de inhoud en past zelden in FCID's tekstveld-limiet (de exacte limiet is spec-afhankelijk en te verifiëren tegen de FCID-versie die CJIB draait). De cel-config bepaalt de exacte vorm per regel, default is `<lexogram-naam> art. <nr>`. |
| `juridische_grondslag_bron` | `article.url` (canonieke wetten.overheid.nl-link) |
| `zaakkenmerk` | de cel's bestaande zaaknummer-systematiek; anders deterministische hash van `(cell.id, beschikking_id)`. Voor één verplichting met meerdere FCID-records (hoofdsom + admin + verhoging): hetzelfde `zaakkenmerk`, onderscheiden door `category`. |
| `gebeurtenis_kenmerk` | UUID v7, gegenereerd op pull-tijdstip |
| `bedrag` | currency-getypeerde output × 100 (FCID vereist centen als integer). Afrondingsregels (per record afronden, of totaal afronden) volgen de bestaande CJIB-uitvoering en worden in het lexogram gecodificeerd; mismatch hierop is meetbaar in de drie maanden vergelijking. |
| `bezwaar_route` / `beroep_route` | de rechtsmiddel-route, afgeleid uit de procedure van het decretogram op pull-tijdstip; welk veld (bezwaar vs. beroep) hangt af van de procedure (zie hieronder). Voor de Wahv is dit een `beroep_route`, niet een `bezwaar_route` |
| `signature` | de Blauwe-Knop-signing-sleutel van de cel; voor de pilot een dedicated JWS-sleutel (los van de FSC-mTLS-sleutel, zie "Vertrouwen en signing"). Per response één signature (BK-spec), niet per record. Of de FSC-sleutel later óók als JWS-sleutel mag dienen is een open beveiligingsvraag aan RFC-009 die separaat geamendeerd wordt; de pilot wacht daar niet op. |
| `trace_id` | W3C Trace Context `trace_id` uit de executie-trace van het decretogram. Identificeert deze record als afkomstig uit de RegelRecht-aangedreven source, en maakt deduplicatie tegen de bestaande Blauwe-Knop-source mogelijk. |

Het `trace_id` laat een downstream surface (burgerportaal, toezichtstool) terugnavigeren naar de executie-trace die de beschikking heeft geproduceerd. De trace blijft in de cel; alleen de referentie reist mee met het record.

#### Rechtsmiddel-route afgeleid uit de procedure

De cel leest geen `bezwaarbaar`-veld uit `produces`. In plaats daarvan bevraagt ze, op pull-tijdstip, het procedure-state van het decretogram. Welk rechtsmiddel geldt, hangt af van de `procedure_id` (zie RFC-022 §3.3 voor de families):

- **Gewone Awb-beschikking** (Awb titel 4.4 *geldschulden*, meeste bestuurlijke boetes): een `bezwaar_route`.
- **Lex-specialis eigen-regime** zoals de **Wahv**: een `beroep_route` (administratief beroep bij de OvJ, Wahv art. 6, daarna kantonrechter, Wahv art. 9). Géén Awb-bezwaar.
- **UOV / concretiserend BAS**: een `beroep_route` (direct beroep).
- **STRAFBESCHIKKING**: een `verzet_route` (Sv 257e), uit het strafprocesrechtelijke model, niet uit RFC-008.

Veldafleiding voor een `bezwaar_route` (Awb-beschikking):

| `bezwaar_route`-veld | Afleiding |
|---|---|
| `intake` | de bezwaar-intake-URL van de cel voor de `procedure_id` van de regel (cel-config) |
| `termijn_grondslag` | het Awb-artikel (of lex-specialis-override) dat de termijn bepaalde, bv. `"Awb 6:7"` |
| `termijn_einddatum` | `bezwaartermijn_einddatum`-output van de BEKENDMAKING-stage-hooks (Awb 6:8 + Termijnenwet) |
| `direct_beroep_mogelijk` | true wanneer Awb 7:1a van toepassing is; anders afwezig |

Veldafleiding voor een `beroep_route` (Wahv-regime):

| `beroep_route`-veld | Afleiding |
|---|---|
| `intake` | de administratief-beroep-intake van de cel (voor de Wahv: OM/CVOM) |
| `termijn_grondslag` | het lex-specialis-artikel dat de termijn bepaalde, bv. `"Wahv art. 6"` |
| `termijn_einddatum` | de beroeptermijn-einddatum, berekend op de notificatie-equivalente stage van de Wahv-procedure |
| `vervolgrechtsmiddel` | de instantie van het vervolg-beroep, bv. `"kantonrechter (Wahv art. 9)"` |

Als de procedure geen enkel rechtsmiddel kent (AVV-zonder-direct-beroep, beleidsregel), is geen route aanwezig maar wel een `geen_rechtsbescherming_reden`.

#### Eén vordering, helemaal uitgewerkt

Om de mapping concreet te maken: één Wahv-sanctie van wetsartikel tot MBO-scherm. De waarden zijn fictief maar realistisch.

**Casus.** Een Mulder-feit (fout parkeren) onder zaakkenmerk `WM-2026-0001234`. De engine voert het Wahv-lexogram uit met `procedure_id: wahv_sanctie`, produceert een `BESCHIKKING` met `decision_type: BETALINGSVERPLICHTING` van €99,00, bekendgemaakt op 2026-03-01. De Wahv-beroeptermijn (6 weken, Wahv art. 6) wordt op de bekendmaking-equivalente stage berekend op 2026-04-12 (inclusief Termijnenwet-correctie).

**FCID-record** dat de cel op een burger-pull serialiseert (velden volgens de afleidingstabel hierboven; `beroep_route` omdat de Wahv lex specialis is):

```json
{
  "event_type": "BetalingsverplichtingOpgelegd",
  "category": "ALGEMEEN",
  "juridische_grondslag_omschrijving": "Wahv art. 2 + bijbehorende beleidsregel",
  "juridische_grondslag_bron": "https://wetten.overheid.nl/BWBR0004581",
  "zaakkenmerk": "WM-2026-0001234",
  "gebeurtenis_kenmerk": "0190a3f2-7c1e-7b44-9a02-1f3c8d5e6a7b",
  "bedrag": 9900,
  "beroep_route": {
    "intake": "https://cjib.example/wahv/beroep",
    "termijn_grondslag": "Wahv art. 6",
    "termijn_einddatum": "2026-04-12",
    "vervolgrechtsmiddel": "kantonrechter (Wahv art. 9)"
  },
  "signature": "eyJ...",
  "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736"
}
```

**MBO-scherm**, na on-device aggregatie in de burger-client: één regel "Verkeersboete CJIB, €99,00", met een directe link naar Wahv art. 2 op wetten.overheid.nl, een knop "Beroep instellen" die naar de OvJ/CVOM-intake wijst met de zichtbare einddatum "uiterlijk 12 april 2026", en een "bekijk onderbouwing"-link die via `trace_id` naar de executie-trace terugnavigeert. Betaalt de burger op 2026-03-20, dan verschijnt onder hetzelfde `zaakkenmerk` een tweede record (`BetalingVerwerkt`, `bedrag: 9900`, `gebeurtenis_datetime: 2026-03-20`) in dezelfde tijdlijn.

(De exacte FCID-veldnamen volgen uit vraag 3 aan CJIB; een afwijkende naam werkt hier door, maar de afleidingen blijven gelijk.)

### Producer-zijde: chronicle-stream-events

Een chronicle-stream-entry die in de FCID-response zichtbaar moet zijn, declareert het op dezelfde manier:

```yaml
# chronicles/cjib_payments.yaml
$id: cjib_payments
competent_authority: cjib
chronicle: cjib_main_chronicle
events:
  - name: payment_received
    intake: incasso_system_intake
    fields:
      case_reference: $external.zaakkenmerk
      amount_cents: $external.bedrag_centen
      received_at: $external.received_at
    extensions:
      blauwe_knop:
        payload: fcid
        event_type: BetalingVerwerkt
        category: ALGEMEEN
```

Een chronicle-stream-event zonder `extensions.blauwe_knop`-blok wordt nog steeds opgenomen in de kroniek van de cel; het verschijnt alleen niet in de FCID-response. Het executogram is het event zelf; FCID is alleen de wire-format wanneer de cel als Blauwe-Knop-source antwoordt.

Veld-afleiding voor executogram-records:

| FCID-veld | Afleiding |
|---|---|
| `event_type` | uit `extensions.blauwe_knop.event_type` |
| `category` | uit `extensions.blauwe_knop.category` |
| `zaakkenmerk` | hetzelfde `zaakkenmerk` als het oorspronkelijke decretogram, dat de betaling koppelt aan de verplichting |
| `gebeurtenis_kenmerk` | UUID v7, gegenereerd op pull-tijdstip |
| `bedrag` | uit het `amount_cents`-veld van het event |
| `gebeurtenis_datetime` | uit het `received_at`-veld van het event |
| `signature` | per response (zie boven), niet per record |

#### Rechtsbescherming op executogram-records

De meeste executogram-records dragen geen `bezwaar_route`. Een ontvangen betaling is een feit, geen besluit; bezwaar maken tegen een feit is niet waar bezwaar voor is.

Een kleine klasse executogram-records draagt er wel een: events die *impliciet een nested besluit referencen*. Een `kwijtschelding_verleend`-event is de buitenkant van een kwijtscheldings-decretogram (met eigen AWB-lifecycle). De chronicle-stream declareert de link:

```yaml
- name: kwijtschelding_verleend
  references_decision: $external.kwijtschelding_decision_id
  fields:
    case_reference: $external.zaakkenmerk
    reden: $external.reden
  extensions:
    blauwe_knop:
      payload: fcid
      event_type: BetalingsverplichtingIngetrokken
      category: ALGEMEEN
```

Wanneer `references_decision` aanwezig is, kijkt de cel het RFC-008-procedure-state van die beslissing op en leidt de `bezwaar_route` daaruit af. De BEKENDMAKING-guard geldt ook hier: als de gerefereerde beslissing nog niet bekendgemaakt is, verschijnt het kwijtscheldings-event niet in de FCID-response.

### Consumer-zijde: hoe een regeling vorderingen opvraagt

Een regeling die de openstaande vorderingen van een burger nodig heeft, gebruikt het normale `source`-blok (RFC-007) en benoemt de bron-cel in plaats van een regeling. De cel ontsluit `openstaande_vorderingen` als één van haar lexostatussen, gedefinieerd in `cells/cjib/capabilities.yaml` (zie eerder):

```yaml
input:
  - name: openstaande_vorderingen
    source:
      regulation: cjib                      # cel-id in de FSC-service-registry
      output: openstaande_vorderingen       # de lexostatus die de cel ontsluit
      parameters:
        bsn: $bsn
```

De semantiek is uniform: de regeling vraagt `openstaande_vorderingen` op bij cel `cjib`. Het transport waarover die vraag loopt, kiest de engine-runtime op grond van de cel-context (Blauwe Knop in een burger-client, FSC op een bevoegde-instantie-server); de regeling-YAML is in beide gevallen identiek. De mechaniek staat in RFC-022 §4; cel-resolutie in `source.regulation` is een uitbreiding van RFC-007 die na de werksessie geamendeerd wordt.

Wat dit voor de pilot betekent, zijn twee soorten consumenten:

- **Burger-context (Blauwe Knop).** De engine draait in een burger-client (RegelRecht-WASM, een mobiele app) en pullt zelf de Blauwe-Knop-source van CJIB. Voorbeelden: een tool die uitrekent of iemand in aanmerking komt voor een minnelijke schuldregeling, een Wsnp-rekentool, een proactieve melding over een naderende deurwaardingsstap. Geen serverside aggregatie, niets verlaat het apparaat.
- **Bevoegde-instantie-context (FSC).** De engine draait op een server met een wettelijke grondslag of een gemodelleerde burger-machtiging, en roept CJIB via FSC. Voorbeelden: een gemeente die in een Wsnp-procedure de schuldsom moet kennen, een beschermingsbewindvoerder met rechtbank-mandaat, een schuldhulpverlener met machtiging. CJIB toetst de grondslag aan de serverkant en geeft een (gefilterde) `openstaande_vorderingen` terug.

Beide bestaan náást elkaar; CJIB kan als bron beide endpoints draaien. Dezelfde Wsnp-regeling-YAML werkt zo ongewijzigd in een burger-app én in een gemeente-systeem, omdat de transport-keuze in de cel-config zit, niet in de regeling.

### Vertrouwen en signing

Vertrouwen wordt overgenomen uit [RFC-009 §5](/rfcs/rfc-009). De cel tekent FCID-responses (Blauwe Knop) en FSC-responses; de ontvanger verifieert tegen de relevante Trust Anchor (App Manager voor Blauwe Knop, FSC Directory voor FSC). De handtekening is per response, niet per individueel record; dat volgt de Blauwe-Knop-specificatie.

Er zijn hier twee aparte vragen die niet door elkaar moeten lopen. De eerste is een echte beveiligingsvraag: mag de FSC-mTLS-sleutel uit RFC-009 óók als JWS-signing-sleutel voor Blauwe Knop dienen? Dat raakt key-usage/EKU-constraints en functiescheiding, en RFC-009 (dat transport-auth en payload-signing in aparte lagen houdt) moet hierop geamendeerd worden; het antwoord kan zijn dat aparte sleutelparen vereist zijn. **Om de pilot niet aan die nog-open vraag op te hangen, kiezen we voor de pilot bewust de veilige default: een eigen, dedicated JWS-signing-sleutel voor de RegelRecht-aangedreven source, los van de FSC-mTLS-sleutel.** De pilot is daarmee niet geblokkeerd, ongeacht wat de canonieke RFC-009-amendement uiteindelijk besluit; sleutelhergebruik blijft een latere optimalisatie, niet iets op het kritieke pad.

De tweede vraag is operationeel en kan de werksessie wél beantwoorden: kan de RegelRecht-cel die naast de bestaande source draait, CJIB's *bestaande* Blauwe-Knop-signing-sleutel (die al een JWS-payload-sleutel is) hergebruiken, of krijgt de tweede source een eigen sleutel? Beide werken; dit is een afstemming met CJIB (vraag 4 aan CJIB), geen beveiligingsbeslissing.

### Buiten scope

- **Burger-authenticatie zelf.** Wordt door DigiD en App Manager via Blauwe Knop Connect gedaan; de cel valideert alleen de tokens die uit die flow voortkomen.
- **Betaalverwerking** (iDEAL, automatische incasso, reconciliatie) ligt upstream van deze integratie.
- **De Financial Claim Request API en Session API** rondom FCID zijn nog niet geïntegreerd; de pilot beperkt zich tot het FCID-response-formaat.
- **Burger-machtigingen via DigiD Machtigen** in het Blauwe-Knop-pad. De huidige Blauwe Knop Connect-spec beschrijft alleen de directe burger-flow; gemachtigdentoegang is een open vraag (zie ook open vraag 10 in bijlage A).
- **De juridische grondslag** voor elke specifieke FSC-aanroep in context 2 is per geval en per relevante wettelijke bepaling. RegelRecht modelleert dat niet; de aanroepende cel is daarvoor verantwoordelijk.
- **De AWB-lifecycle-interne werking.** Alle bezwaar-mechaniek leeft in RFC-008. Deze bijlage leest RFC-008's outputs.
- **Productie-deployment-werk** (NEN 7513-logging, SLA, load-testing, key-rotation-schedule). Deze pilot draait in een sandbox; productie-werk komt na de drie maanden vergelijking, in een separaat traject.
