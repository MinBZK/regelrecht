---
title: "Voorstel: RegelRecht als engine achter de Blauwe-Knop-source van CJIB"
lang: nl
---

*Auteur: Anne Schuth · Datum: 2026-05-27 · Status: concept*

## Aanleiding

Mijn Betaaloverzicht (MBO, voorheen Vorderingenoverzicht Rijk) draait op een patroon dat in Nederland nog jong is maar principieel klopt: data blijft bij de bron, aggregatie gebeurt on-device in de burger-client, geen enkele overheidsorganisatie ziet het totaalbeeld. De onderliggende standaard is [Blauwe Knop Connect](https://vorijk.nl/standaard/connect/draft-bk-connect-00.html). De burger logt in via DigiD, krijgt een korte sessie, en haalt zelf bij elke aangesloten bronorganisatie zijn vorderingen op in het FCID-formaat. Voor zover wij uit publieke bronnen kunnen opmaken draait CJIB deze rol sinds 2025, net als de Belastingdienst, en zijn eind 2025 naar verwachting acht rijksorganisaties als Blauwe-Knop-source actief. Welke vorderingen CJIB vandaag precies via die source ontsluit (alleen Wahv, of ook mandaat-vorderingen) is nog te bevestigen; zie bijlage A, open vraag 2, en vraag 3 aan CJIB.

Wat aan dat patroon ontbreekt is juridische provenance per vordering. Een FCID-response van CJIB zegt vandaag "u heeft een vordering van €X". Wat ze niet zegt: op grond van welk artikel, berekend uit welke invoer, met welke bezwaartermijn die op het moment van bekendmaking daadwerkelijk is uitgerekend. Dat is de motiveringsplicht uit Awb 3:46, en een onderdeel van wat Nieuwland §5.4 met "samen zien" bedoelt.

RegelRecht heeft sinds 2024 een raamwerk opgebouwd waarmee een wetsartikel een uitvoerbare specificatie wordt: `legal_character` en `decision_type` voor besluiten, AWB-lifecycle als first-class construct (RFC-008), executie-trace per beschikking (RFC-013), federatie tussen organisaties via FSC (RFC-009). De wetten die nu in machine-leesbare vorm in het corpus staan dienen vooral als bewijs dat het raamwerk werkt; nieuwe wetten worden opgenomen op het moment dat een cel ze nodig heeft.

In december 2025 publiceerde de Denktank Achterkant van de Overheid het ontwerp [Nieuwland](https://achterkantvandeoverheid.nl/) en het [Chronolexografie-position paper](https://chronolexografie.nl/). Daarin staat een coherent begrippenkader voor het digitaal vastleggen van de rechtstoestand: chronolexocellen, kronieken, en drie typen vastlegging (lexogram, decretogram, executogram). Het begrippenkader sluit aan op het werk aan VORIJK/MBO en op recente gesprekken met BZK.

Het idee van dit voorstel: **een RegelRecht-engine achter de Blauwe-Knop-source van CJIB zetten, te beginnen bij de Wahv**. CJIB blijft de cel die de source aanbiedt; RegelRecht is de engine die de inhoud van de FCID-response uitrekent. Niet als vervanger van wat CJIB nu draait, maar er naast. De source geeft op een burger-pull een FCID-response terug met dezelfde bedragen, termijnen en bezwaarroute als CJIB's eigen systeem, plus de juridische onderbouwing per vordering, omdat de engine die uit de wet afleidt. Het hele MBO-patroon (data-bij-de-bron, on-device aggregatie, geen centrale stapel) blijft volledig intact. We dragen ertoe bij, we breken er niets aan.

## Wat staat er niet in dit voorstel

Voor de zekerheid, omdat dit makkelijk verkeerd valt:

- **Geen vervanging.** Het lexogram dat we bouwen draait naast CJIB's huidige Wahv-uitvoering. Het doel van de pilot is dat de twee voor dezelfde casuïstiek tot hetzelfde bedrag, dezelfde termijn en dezelfde bezwaarroute komen. Pas als die matching klopt, is een vervolggesprek over rolverdeling op zijn plek. Eerder niet.
- **Geen centrale aggregatie.** MBO werkt by design on-device; de cel houdt haar eigen data en stelt die beschikbaar op pull-request van een burger-client, MBO aggregeert pas in die client. Niets in dit voorstel verandert dat. Het Blauwe-Knop-patroon is wat we ondersteunen, niet iets dat we hervormen.
- **Geen nieuw broker-mechanisme.** We hergebruiken Blauwe Knop Connect zoals het is. RegelRecht voegt onder de motorkap juridische provenance toe; het transport, de authenticatie en de aggregatie blijven Blauwe Knop.
- **Geen wederkerige samen-zien-implementatie.** Dit voorstel adresseert één kant van Nieuwland §5.4: de burger ziet meer dan vandaag. De wederkerige kant (de cel of een derde ziet, met machtiging, wat de burger via MBO geaggregeerd voor zichzelf ziet) valt buiten scope van deze pilot. Die kant raakt DigiD Machtigen, de gemachtigden-flow van Blauwe Knop Connect en de positie van schuldhulpverleners, en is eigen vraagstuk.

## Woordenlijst

Een korte vertaling van de Chronolexografie- en Blauwe-Knop-begrippen die in dit voorstel terugkomen, in CJIB-taal:

- **Lexogram**: een wet of regeling in machine-leesbare vorm. Vergelijkbaar met "de regelingstekst zoals jullie compliance-team die interpreteert", maar dan in YAML die een engine direct kan uitvoeren.
- **Decretogram**: een concreet besluit. Bij CJIB: een Wahv-sanctie, een OM-strafbeschikking, een schadevergoedingsmaatregel. In RegelRecht-termen: een engine-output met `legal_character: BESCHIKKING`. FCID is daar één serialisatie van; het decretogram zelf is het primaire artefact.
- **Executogram**: een feit dat de afhandeling registreert. Bij CJIB: een binnengekomen betaling, een verleende kwijtschelding, een gestart deurwaardertraject. Concreet: een entry in een chronicle-stream-bestand. FCID is daar weer een serialisatie van.
- **Chronicle / chronolexochronicle**: een tijdlijn van vastleggingen die een cel bijhoudt. Een cel kan meerdere chronicles bijhouden (bijvoorbeeld één per regelingsgebied), elk een geordende reeks van haar decretogrammen en/of executogrammen.
- **Chronolexoreductie**: de afleiding van een lexostatus uit één of meer chronicles. Een filter-en-aggregatie-bewerking: "alle Wahv-besluiten in onze chronicle, voor BSN X, in of voorbij BEKENDMAKING, minus intrekkingen". Dit is wat een Blauwe-Knop-source op pull-moment uitvoert.
- **Lexostatus**: het *resultaat* van een chronolexoreductie. Bij CJIB: `openstaande_vorderingen` als lijst van vordering-records. De cel declareert welke lexostatussen ze aanbiedt en hoe ze elk uit haar chronicles worden afgeleid.
- **Cel (chronolexocell)**: een organisatie die kronieken bijhoudt, sleutels beheert en bevoegd gezag draagt. CJIB is een cel. NVWA, OM, DUO, CAK ook. Dit is niet nieuw maar dezelfde notie als wat RegelRecht eerder al `competent_authority` noemde.
- **Engine**: één van de componenten die in een cel kan draaien. Een cel kan één engine bevatten, meerdere, of een engine plus een legacy-systeem.
- **Blauwe-Knop-source**: een endpoint dat een bronorganisatie aanbiedt waar een geauthenticeerde burger-client (via DigiD + App Manager) zijn eigen FCID-records ophaalt. Geen push, geen centrale opslag.
- **FCID (Financial Claims Information Document)**: het JSON-formaat waarin een Blauwe-Knop-source vorderingen, betalingen en intrekkingen teruggeeft. Per response ondertekend door de bron-cel.
- **FSC (Federatieve Service Connectivity)**: het standaardmechanisme voor server-naar-server federatie tussen overheidsorganisaties. Naast Blauwe Knop, niet in plaats daarvan: Blauwe Knop is burger-naar-bron, FSC is bron-naar-bron (bijvoorbeeld een bevoegd schuldhulpverlener met machtiging).

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
- Rechtsbescherming wordt niet als nieuw veld geïntroduceerd: de cel leidt de `bezwaar_route` af uit de RFC-008-procedure-stage van het decretogram, op het juiste moment (BEKENDMAKING), zodat de werkelijke einddatum meereist en niet een statische hint.

RFC-022 raakt twee bestaande RFCs op punten die separaat aandacht vragen en in follow-up amendementen worden opgelost: RFC-007 (cel-resolutie in `source.regulation` en transport-keuze per cel-context) en RFC-009 (sleutelhergebruik tussen FSC-signing en Blauwe-Knop-response-signing). Voor de pilot maken we gebruik van de voorgestelde uitbreiding; de canonieke updates van die RFCs volgen na de werksessie.

Voor de werksessie hieronder is het niet nodig RFC-022 helemaal door te lezen. Dit voorstel is zelfstandig leesbaar. De RFC is er voor de IT-lead die wil zien hoe de mapping er onder de motorkap uitziet.

## Het denkkader

Chronolexografie onderscheidt drie typen vastlegging die in de rechtsstaat alle drie nodig zijn.

- **Lexogram**: vastlegging van een (mogelijke) wijziging in wet- of regelgeving. Voorbeeld: de Wahv zoals die geldt sinds 1 januari 2025.
- **Decretogram**: vastlegging van een concreet besluit. Voorbeeld: een Wahv-sanctie van €X die op datum Y aan kentekenhouder Z wordt opgelegd.
- **Executogram**: vastlegging van feitelijke afhandeling. Voorbeeld: een betaling van €X die op datum Z bij CJIB binnenkomt onder zaakkenmerk Y.

In de huidige situatie wonen deze drie typen in gescheiden systemen, met telkens een verlies aan context op de overgangen. De burger ziet via Blauwe Knop wel het bedrag, maar niet de beschikking of het artikel. De gevolgen daarvan zijn beschreven in Nieuwland en in eerdere publicaties van Kafkabrigade. De pilot die hieronder volgt sluit deze keten voor één wet bij één cel, zonder het Blauwe-Knop-patroon zelf te veranderen.

## Wat de pilot inhoudt

Voor één pilotwet (voorkeur: Wahv) leveren we drie samenhangende artefacten op.

**Een lexogram.** Een YAML-bestand `corpus/regulation/nl/wet/wet_administratiefrechtelijke_handhaving_verkeersvoorschriften/<valid_from>.yaml`. Dit is de Wahv in machine-leesbare vorm conform het RegelRecht-schema. Eén artikel produceert een `BESCHIKKING` met `decision_type: BETALINGSVERPLICHTING`, het juiste `procedure_id` per RFC-008 (default `beschikking`), en een `extensions.blauwe_knop`-hint die zegt: deze vordering hoort in de FCID-response zichtbaar te zijn zodra de procedure de BEKENDMAKING-stage heeft bereikt. De bezwaarweg zit niet in de regeling, want die wordt door RFC-008 afgeleid uit de AWB-procedure.

**Een chronicle-stream.** Een YAML-bestand `chronicles/cjib_wahv_betalingen.yaml` met minstens drie events: `payment_received`, `kwijtschelding_verleend`, `deurwaardertraject_gestart`. Per event de juiste FCID-mapping in `extensions.blauwe_knop`. `kwijtschelding_verleend` declareert `references_decision: <kwijtschelding-besluit-id>` zodat de cel de bezwaarweg via dat besluit kan afleiden. `payment_received` en `deurwaardertraject_gestart` zijn feiten zonder bezwaar.

**Een werkende Blauwe-Knop-source.** Een RegelRecht-engine draait binnen een afgeschermde CJIB-pilot-omgeving met de Wahv-lexogram en de chronicle-stream geladen. De cel-configuratie activeert het `blauwe_knop_source`-blok en publiceert een Blauwe-Knop-source-endpoint. Wanneer een burger-client (MBO-app, of een test-client in de pilot) een geauthenticeerde pull doet voor deze burger, voert de cel een chronolexoreductie uit: ze loopt door de relevante chronicles, filtert op BSN en op `current_stage >= visible_from_stage`, voegt betalingen en kwijtscheldingen toe, en serialiseert het resultaat als FCID-response. De response is ondertekend met de FSC-key van CJIB (RFC-009 §5) en bevat per vordering een `bezwaar_route` waarvan de einddatum door de AWB-6:8-hook op de BEKENDMAKING-stage is berekend, met de werkelijke datum.

Aan burger-zijde, on-device: een Wahv-vordering verschijnt in de MBO-app met een directe link naar het artikel, een verifieerbare verwijzing naar de executie-trace, een bezwaarknop met de juiste route en de werkelijke einddatum, en, na betaling, een gekoppeld BetalingVerwerkt-event onder hetzelfde zaakkenmerk. De MBO-app voegt deze response samen met die van andere bronorganisaties in de burger-client. Er is geen centraal punt waar dat geheel bestaat.

### Drie casuïstiek-klassen in de pilot

De Wahv-baseline alleen test eigen-uitvoering door CJIB. Maar CJIB int ook namens andere bestuursorganen, en juist die mandaat-vorderingen zijn de moeilijke gevallen voor bezwaar-routing en cel-topologie. De pilot dekt daarom drie klassen:

1. **Wahv-sanctie** (CJIB is primair bestuursorgaan). Doel: baseline vergelijken tegen de bestaande Blauwe-Knop-source van CJIB.
2. **Eén CAK-eigen-bijdrage-zaak** (CAK is primair, CJIB voert uit onder mandaat). Doel: bezwaar-routing-mapping testen. De `bezwaar_route` wijst juridisch naar CAK, terwijl de burger operationeel CJIB belt. Vergelijken met wat de huidige Blauwe-Knop-source van CJIB hier doet, geeft de eerste echte test van de mandaat-keten.
3. **Optioneel: één OM-strafbeschikking** (Sv 257a). Doel: aantonen dat een niet-bestuursrechtelijk besluit als `decision_type: STRAFBESCHIKKING` in de FCID-response past zonder dat de Awb-bezwaar-derivatie erop wordt losgelaten. Een strafbeschikking kent geen Awb-bezwaar/beroep maar *verzet* bij de strafrechter (Sv 257e); de rechtsbescherming-route wordt dus uit het strafprocesrechtelijke model afgeleid, niet uit RFC-008. De UOV-uitzondering (Awb afdeling 3.4) hoort hier nadrukkelijk **niet** bij: UOV geldt nooit voor een strafbeschikking. Wie de UOV-`beroep_route`-afleiding wil testen, gebruikt daarvoor een echte UOV-Awb-beschikking, niet deze klasse.

Een fundamentele mismatch tussen RegelRecht en CJIB-huidig in de CAK-bezwaarroute (klasse 2) is showstopper voor week 2 van de werksessie. Daarvoor moet eerst worden uitgezocht hoe CJIB nu de juiste bezwaarroute bepaalt voor mandaat-vorderingen, en hoe RegelRecht dat moet modelleren (in de Wahv-lexogram zit dat niet; het hoort bij het mandaat-convenant of een aparte beleidsregel die mee-geladen wordt).

### Het Wahv-artikel in YAML

```yaml
# Primaire sanctie
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: BETALINGSVERPLICHTING
    procedure_id: beschikking            # RFC-008 default
    extensions:
      blauwe_knop:
        payload: fcid
        category: ALGEMEEN
        visible_from_stage: BEKENDMAKING

# Intrekking (bv. na succesvol bezwaar of administratieve correctie):
# een nieuwe BESCHIKKING met dezelfde decision_type, plus modality.
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: BETALINGSVERPLICHTING
    procedure_id: beschikking
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

**Een tweede Blauwe-Knop-source naast de bestaande**, voor dezelfde Wahv-vorderingen, met identieke output op bedrag- en termijn-niveau plus volledige juridische provenance per vordering. Dat is iets dat CJIB op dit moment niet kan leveren en wat Nieuwland §7.2.1 wel vraagt.

**Eén bron voor norm, besluit en feit.** Het lexogram zit in het corpus; het besluit komt uit de engine; het feit komt uit de chronicle-stream. Wijzigt de wet, dan beweegt de FCID-response mee zonder aparte release in een tweede systeem. Compliance-werk en uitvoering komen samen onder hetzelfde artefact.

**Rechtsbescherming als ontwerp, niet als marketing.** De AWB-lifecycle uit RFC-008 levert de bezwaartermijn op het juiste moment (BEKENDMAKING, niet BESLUIT). De cel pakt die termijn op en stuurt 'm mee als `bezwaar_route` in elke FCID-response. Een Wahv-sanctie met automatische ophoging die voor iemand met laag inkomen disproportioneel uitwerkt, krijgt vanaf het moment van bekendmaking een zichtbare bezwaarknop in MBO met de werkelijke einddatum, niet pas na een aanmaning. Dit is de operationalisering van Nieuwland §7.2.1, ingebed in het Blauwe-Knop-patroon dat MBO al draait.

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
3. **FCID-versie en status van jullie huidige Blauwe-Knop-source.** v3.0.0 is de stabiele versie volgens vorijk.nl; v4.x is experimenteel. Welke draait nu in jullie pilot of productie, op welke endpoints, en welke FCID-event-typen ondersteunt jullie source vandaag? Welke vorderingen zijn al ontsloten (alleen Wahv, of ook CAK/OM-vorderingen)?
4. **Knelpunten in de mapping.** Voor `zaakkenmerk` geldt CJIB's eigen zaaknummer-systematiek als leidend. Voor signing van de FCID-response gaan we uit van de FSC-key uit RFC-009. Botst dit met de sleutels die jullie nu al voor de bestaande Blauwe-Knop-source gebruiken, of kunnen we dezelfde sleutel hergebruiken? Plus: hoe verloopt de bedrag-afronding op centen in jullie bestaande uitvoering (een beleidsregel-detail dat we in het lexogram moeten codificeren)?
5. **Cel-topologie en bezwaar-routing.** Hoeveel cellen zou CJIB draaien (één centraal, één per opdrachtgever, één per regelinggebied), hoeveel chronicles per cel, en hoe verhoudt dat zich tot de bestaande Blauwe-Knop-source (één source voor alles, of meerdere)? En per type vordering: waar landt het bezwaar formeel, en waar landt het in de praktijk? Een CAK-eigen-bijdrage-vordering staat onder CJIB-zaaknummer maar het bezwaar gaat juridisch naar CAK; in de operationele realiteit belt de burger naar CJIB. De `bezwaar_route` in de FCID-response moet kloppen met beide werelden. Dit punt wil ik graag samen met het VORIJK/MBO-team uitwerken.
6. **Deduplicatie bij twee parallel-lopende sources.** Tijdens de pilot draait CJIB voor de Wahv twee Blauwe-Knop-sources naast elkaar: de bestaande en de RegelRecht-aangedreven. De MBO-app krijgt voor dezelfde vordering twee FCID-records. Hoe wil het MBO/VORIJK-team dat de app dedupliceert (op `zaakkenmerk`, op `trace_id`-origin, op een ander criterium)? Het antwoord hierop bepaalt of we naast elkaar kunnen draaien zonder dat de burger dubbele vorderingen ziet.

## Volgende stap

Een werksessie van een dagdeel met CJIB, het VORIJK/MBO-team en BZK. Agenda: het uitvoeringslandschap valideren, de pilotwet vastpinnen, de cel-topologie schetsen, de bezwaar-routing per type vordering uitwerken, deduplicatie-strategie met MBO-team afspreken, knelpunten benoemen, en bevestigen dat de nieuwe Blauwe-Knop-source naast de bestaande kan draaien. Daarna kan de voorgestelde RFC-022 verder, kan de bijbehorende schema-bump (v0.5.4 → een nieuwe v0.6.0) worden voorbereid, en kunnen we beginnen met het Wahv-lexogram en de eerste chronicle-stream.

Doel: binnen één maand na de werksessie een werkende Blauwe-Knop-source in een pilot-omgeving, met één Wahv-beschikking die op een geauthenticeerde burger-pull in het MBO-pilotportaal verschijnt, een bezwaarknop bevat met de juiste route en termijn, en teruggetraceerd kan worden naar het wetsartikel. Daarna drie maanden vergelijking tegen de bestaande CJIB-Blauwe-Knop-source over alle drie casuïstiek-klassen. Pas daarna een gesprek over wat volgt. Geen jaartallen op het pad daarna; eerst dit laten kloppen.

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

CJIB is een *zelfstandig bestuursorgaan* (ZBO) onder het ministerie van Justitie en Veiligheid. Het is het centrale financiële-handhavingsknooppunt van de Nederlandse staat: bijna elke administratiefrechtelijke en strafrechtelijke financiële verplichting komt hier uiteindelijk terecht wanneer een burger niet vrijwillig betaalt. Per 2026 voert CJIB uit voor minstens vijftien opdrachtgevers, van OM tot een sectorale inspectie als NEa.

CJIB is ook één van de eerste Blauwe-Knop-sources die in productie draait. Per april 2025 was CJIB samen met de Belastingdienst de meest gevorderde aansluiting; tegen eind 2025 zijn naar verwachting alle acht oorspronkelijke CRI-rijksorganisaties als Blauwe-Knop-source actief (Belastingdienst, Dienst Toeslagen, CJIB, DUO, SVB, CAK, UWV, RVO). De inmiddels uitgebreide kring (NVWA, RDI, RDW e-Tol, Inspectie JenV, NEa, DFEI, ATKM) staat in de pijplijn. CJIB heeft daarmee zowel de breedste portfolio als de meest concrete Blauwe-Knop-implementatie. Dat maakt CJIB de logische eerste cel om een RegelRecht-engine achter de Blauwe-Knop-source te zetten.

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

CJIB int namens minstens vijftien opdrachtgevers. De juridische grondslag verschilt per geval: sommige zijn sectorale wetten die de minister van JenV of CJIB direct aanwijzen; andere zijn mandaatconstructies onder de Algemene wet bestuursrecht. Het Clustering Rijksincasso (CRI) programma, geformaliseerd via [eenoverheidsincasso.nl](https://www.eenoverheidsincasso.nl/onze-partners), structureert deze samenwerking.

In Chronolexografie-termen: de cel van de *opdrachtgever* produceert de primaire decretogram (de inhoudelijke beschikking); CJIB's cel registreert de executogrammen (betaling, kwijtschelding) namens die opdrachtgever. Of CJIB ook een eigen vervolg-decretogram produceert (bijvoorbeeld een dwangbevel onder Awb 4:114) hangt af van de regeling en het convenant. Voor de Blauwe-Knop-presentatie betekent dit dat één FCID-record over een CAK-vordering meerdere bron-relaties heeft: CAK heeft het primaire besluit genomen, CJIB beheert de inning. De `bezwaar_route` moet die werkelijkheid weerspiegelen (zie ook punt 8 in Open vragen, en vraag 5 aan CJIB).

| Opdrachtgever | Type vordering | Grondslag (best available) | Decretogram-cel | Executogram-cel | Past in voorgestelde schema-uitbreiding? |
|---|---|---|---|---|---|
| OM | Strafbeschikking, schadevergoeding, ontneming | Sv 257a, Sr 36f, Sr 36e | OM | CJIB | Nieuw `decision_type: STRAFBESCHIKKING` |
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

- *Gemeentelijke parkeerboetes*. Dit zijn fiscale gemeentelijke sancties; gemeenten innen via Cocensus, belastingsamenwerkingen, of in-house.
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
8. **Bezwaar-routing in de FCID-response.** Elke FCID-vordering in de response van de cel-als-Blauwe-Knop-source draagt een `bezwaar_route` die op het moment van pull uit de RFC-008-procedure-stage van het decretogram wordt afgeleid. Voor decretogrammen die CJIB zelf produceert (Wahv-sanctie) wijst de route naar CJIB's eigen bezwaar-intake. Voor decretogrammen die CJIB namens een andere cel draagt (een CAK-besluit, een OM-strafbeschikking) wijst de route formeel naar die andere cel, terwijl de burger in de praktijk vaak CJIB belt. De wettelijke routing per regeling moet per geval gevalideerd worden. Hoe bepaalt CJIB's bestaande Blauwe-Knop-source vandaag de bezwaarroute voor een CAK-vordering (hardcoded, of dynamisch)? Het antwoord bepaalt of de RegelRecht-cel dit uit een mandaat-convenant-modellering of uit een aparte beleidsregel-laag moet afleiden.
9. **Wet gegevensboekhouding-interactie.** Nieuwland §7.3.2 schetst een Wet gegevensboekhouding die de executogram-zijdige registratie een wettelijke basis zou geven. De huidige grondslag van CJIB is impliciet in Awb 4.4 + sectorale wetten; een expliciete wet zou het beeld wijzigen. De chronicle-stream-architectuur is bewust ontworpen om met die toekomstige wet mee te kunnen bewegen: per cel verifieerbaar wat er feitelijk geregistreerd is, op grond van welke grondslag, op welk moment.
10. **Burger-machtigingen.** Het Blauwe-Knop-patroon is burger-geïnitieerd via DigiD. Voor de schuldhulpverleningspraktijk en voor sommige sectorale uitvoeringscontexten is er behoefte aan een derde die met expliciete machtiging op een burger toegang krijgt. Of dat via DigiD Machtigen via dezelfde Blauwe-Knop-flow loopt of via FSC met een aparte machtigingscontext is uit publieke bronnen niet ondubbelzinnig af te leiden.
11. **Deduplicatie bij parallel-lopende sources** (zie ook vraag 6 aan CJIB). Tijdens de pilot leveren de bestaande en de RegelRecht-aangedreven source beide FCID-responses voor dezelfde Wahv-vorderingen. Welke sleutel hanteert de MBO-app voor deduplicatie? Mogelijke kandidaten: `zaakkenmerk` (mits beide sources hetzelfde zaaknummer hanteren), of `(zaakkenmerk, decision_type, bedrag)`-triplet plus `trace_id`-origin (de RegelRecht-aangedreven source levert `trace_id`, de oude bron mogelijk niet). De keuze hoort thuis bij MBO-team.

---

## Bijlage B: een door RegelRecht aangedreven Blauwe-Knop-source in detail

Deze bijlage specificeert hoe een cel met een RegelRecht-engine een Blauwe-Knop-source aandrijft en hoe diezelfde cel-mechaniek gebruikt kan worden voor cross-cel queries in beide voorkomende contexten (burger-client en bevoegde-instantie). Inhoud is technisch; bedoeld voor de IT-lead die de pilot begeleidt.

### Doelversie en transport

FCID-baseline: **v3.0.0** (mei 2023, huidige stabiele versie volgens vorijk.nl). De architectuur is voorbereid op v4.x zodra die productie-rijp is; v4.2.0 is per mei 2026 nog experimenteel. De integratie-spec is herzienbaar zonder dat RFC-022 opnieuw opengaat. Welke versie de pilot daadwerkelijk gebruikt, wordt bepaald door wat CJIB's bestaande Blauwe-Knop-source draait (vraag 3 aan CJIB).

Transport en authenticatie volgen [Blauwe Knop Connect](https://vorijk.nl/standaard/connect/draft-bk-connect-00.html) ongewijzigd. De burger authenticeert via DigiD en de App Manager; de cel valideert de aangeboden authorization-token; de cel antwoordt met een FCID-response ondertekend met haar eigen sleutel (JWS). Niets in dit voorstel verandert het protocol; we leveren een tweede source-implementatie naast de bestaande.

### Wat de cel doet op een pull

Een cel die een RegelRecht-engine draait en het `blauwe_knop_source`-blok in haar cel-config activeert, gedraagt zich als een Blauwe-Knop-source. Op een geauthenticeerde pull-request voor een specifieke burger voert de cel een **chronolexoreductie** uit op haar eigen chronicles:

1. Filter alle decretogrammen in de relevante chronicles op `subject_bsn == <burger>`.
2. Hou alleen decretogrammen waarvoor `current_stage >= visible_from_stage` per regel-niveau (default `BEKENDMAKING`). Een decretogram dat dat niet is, valt af; geen lege FCID-records, geen halfslachtige zichtbaarheid.
3. Voeg de relevante executogram-events toe (betalingen, kwijtscheldingen) uit de chronicles waarin die voorkomen.
4. Serialiseer het resultaat als FCID-records, conform de veld-afleidingen verderop.
5. Onderteken de hele response met de cel-eigen signing-sleutel (één signature per response, niet per record).

De response is per cel ondertekend. Er is geen push, geen centraal endpoint, geen kopie buiten de cel. De burger-client (MBO-app) verifieert de handtekening en aggregeert lokaal met de responses van andere sources.

**FCID is een serialisatie, niet een tweede laag.** Een decretogram is het primaire artefact (engine-output met `BESCHIKKING`); het verschijnt in de FCID-response als één record. Hetzelfde voor een executogram: een entry in een chronicle-stream is het primaire artefact, FCID is de wire-format. Geen twee-staps-afleiding, geen aparte FCID-state in de cel.

### Cell-capabilities: welke lexostatussen biedt de cel aan

Naast haar Blauwe-Knop-source-endpoint declareert een cel welke lexostatussen ze aan derden ontsluit. Dat is geen hardcoded eigenschap van de engine; het is een cel-eigen capability-declaratie. Schets voor CJIB:

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
          select_where: "decision_type IN (STRAFBESCHIKKING) AND current_stage >= BEKENDMAKING"
        - chronicle: cjib_mandate_executions
          select_where: "decision_type IN (BETALINGSVERPLICHTING, BESTUURLIJKE_BOETE) AND current_stage >= BEKENDMAKING"
      exclude:
        - "modality.is_intrekking_van IS NOT NULL OR has_intrekking_in_chronicle = true"
      group_by: zaakkenmerk
      order_by: bekendmaking_datum DESC
```

Een lexostatus is dus expliciet **het resultaat van een chronolexoreductie**, niet een output van één enkele regel. De cel is eigenaar van de definitie: zij weet uit welke chronicles de reductie samenvalt, welke filters gelden, hoe duplicaten worden gegroepeerd. Een consument die `openstaande_vorderingen` opvraagt (via Blauwe Knop in een burger-context, of via FSC in een bevoegde-instantie-context, zie verderop) krijgt dezelfde reductie, geserialiseerd in het transport-eigen formaat (FCID voor Blauwe Knop, ACCEPT-payload voor FSC).

Hetzelfde geldt voor de Blauwe-Knop-source-response zelf: die is conceptueel een **standaard lexostatus** met de naam `blauwe_knop_fcid_response` die op haar beurt grotendeels overlapt met `openstaande_vorderingen`, plus enkele FCID-specifieke velden (`gebeurtenis_kenmerk`, `signature`-block). De cel declareert die ook in `capabilities.yaml`; ze is een eerste-klas lexostatus, geen impliciete eigenschap van de engine.

De exacte vorm van `capabilities.yaml` (cell-config formaat) is voorlopig en wordt in een vervolg-RFC vastgepind. Wat vaststaat: een cel is verantwoordelijk voor het declareren van haar eigen reductie-logica, niet de consument.

### Activatie van het `blauwe_knop_source`-blok

Een cel beslist zelf of ze als Blauwe-Knop-source draait. Activatie is geen "rol" die je in- en uitschakelt; Blauwe Knop kent geen rollen-concept. Het is een feature-block in de cel-config dat het endpoint, de FCID-versie, de signing-sleutel en de capabilities aan elkaar koppelt:

```yaml
# cel-config (schets; volledig cel-config formaat in latere RFC)
blauwe_knop_source:
  enabled: true
  endpoint: https://cjib.gov.nl/blauwe-knop/fcid    # cel-eigen, geen centrale endpoint
  fcid_version: 3.0.0
  signing_key_ref: cjib-blauwe-knop-2026            # mag dezelfde zijn als bestaande source
  serves_lexostatus: blauwe_knop_fcid_response      # verwijst naar capabilities.yaml
```

Een gemeente die het Wahv-lexogram draait maar geen Blauwe-Knop-source wil zijn, laat het `blauwe_knop_source`-blok in haar cel-config gewoon weg. Dezelfde regeling-YAML werkt in beide cellen.

### FCID-event-typen en chronolexogram-mapping

FCID definieert vier event-typen. Elk is de serialisatie van precies één chronolexogram-type.

| FCID `event_type` | Chronolexogram-type | Brongegeven in de cel |
|---|---|---|
| `FinancieleVerplichtingOpgelegd` | decretogram | engine-output met `decision_type: STRAFBESCHIKKING` (totaalbedrag) |
| `BetalingsverplichtingOpgelegd` | decretogram | engine-output met `decision_type: BETALINGSVERPLICHTING` / `BESTUURLIJKE_BOETE` |
| `BetalingsverplichtingIngetrokken` | decretogram (intrekking-modaliteit) | engine-output, zelfde `decision_type` als origineel, met `produces.modality.is_intrekking_van` gezet |
| `BetalingVerwerkt` | executogram | chronicle-stream-event, getriggerd door intake vanuit incasso-systeem |

Een intrekking is zelf een nieuwe BESCHIKKING met haar eigen AWB-lifecycle (per RFC-008 resolved Open Question 5). De cel herkent een intrekking via `produces.modality.is_intrekking_van: <oorspronkelijke-id>` en serialiseert haar als `BetalingsverplichtingIngetrokken`. Intrekking en origineel delen een `zaakkenmerk` zodat de burger-client ze als één tijdlijn presenteert.

### Producer-zijde: hoe een lexogram-regel zichtbaar wordt in de FCID-response

Een regel waarvan het decretogram in de FCID-response moet verschijnen, declareert dat in het `extensions.blauwe_knop`-blok:

```yaml
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: BETALINGSVERPLICHTING
    procedure_id: beschikking         # RFC-008 procedure-selectie
    extensions:
      blauwe_knop:
        payload: fcid                 # voor nu altijd fcid; future-proof
        category: ALGEMEEN
        visible_from_stage: BEKENDMAKING   # default; overrijdbaar
```

`category` is een van `ALGEMEEN`, `ADMINISTRATIEKOSTEN`, `VERHOGING`, `RENTE`. Een regeling die meerdere FCID-records uit één beschikking produceert (hoofdsom + administratiekosten + verhoging) declareert die als aparte artikelen of aparte `produces`-blokken, elk met zijn eigen `extensions.blauwe_knop.category`. Alle records delen hetzelfde `zaakkenmerk`; de burger-client gebruikt `category` om ze in de UI samen te groeperen.

`visible_from_stage` selecteert de RFC-008-lifecycle-stage vanaf wanneer de vordering in de FCID-response zichtbaar is. Default is `BEKENDMAKING`: een verplichting die niet bekendgemaakt is heeft geen juridische bestaansgrond om in MBO te tonen. De cel evalueert per pull, per kandidaat-decretogram, of `current_stage >= visible_from_stage`. Vorderingen die nog niet zo ver zijn, vallen weg; ze verschijnen niet als null, niet als placeholder, ze zijn er gewoon niet.

De keuze is `visible_from_stage`, niet `emit_at_stage`. Het Blauwe-Knop-patroon kent geen emit-moment; er is geen push naar een centraal punt. Er is een pull-moment waarop de cel beslist wat ze in de response zet. De AWB-stage-binding is daar relevant: een vordering verschijnt in MBO vanaf het moment dat ze juridisch bekend is gemaakt, niet eerder.

De `bezwaar_route`-velden zijn pas correct vanaf `BEKENDMAKING`: de AWB 6:8-hook schrijft `bezwaartermijn_einddatum` op die stage (de termijnduur uit AWB 6:7 is een eigenschap van het besluit, bepaald bij BESLUIT, maar de einddatum kan pas vanaf de bekendmaking worden uitgerekend). Daarom is `BEKENDMAKING` de default; verlagen naar BESLUIT zou een onvolledige `bezwaar_route` opleveren en is daarom niet toegestaan voor decretogrammen waarvoor een `bezwaar_route` vereist is.

#### Veld-afleiding per FCID-record

| FCID-veld | Afleiding |
|---|---|
| `event_type` | uit `decision_type` plus `modality.is_intrekking_van` per de tabel hierboven |
| `category` | uit `extensions.blauwe_knop.category` |
| `juridische_grondslag_omschrijving` | bondige verwijzing naar het artikel, bijvoorbeeld `"Wahv art. 2 + bijbehorende beleidsregel"`. Niet de eerste zin van `article.text`: dat parafraseert de inhoud en past zelden in FCID's tekstveld-limiet (v3.0.0 typisch 255 tekens). De cel-config bepaalt de exacte vorm per regel, default is `<lexogram-naam> art. <nr>`. |
| `juridische_grondslag_bron` | `article.url` (canonieke wetten.overheid.nl-link) |
| `zaakkenmerk` | de cel's bestaande zaaknummer-systematiek; anders deterministische hash van `(cell.id, beschikking_id)`. Voor één verplichting met meerdere FCID-records (hoofdsom + admin + verhoging): hetzelfde `zaakkenmerk`, onderscheiden door `category`. |
| `gebeurtenis_kenmerk` | UUID v7, gegenereerd op pull-tijdstip |
| `bedrag` | currency-getypeerde output × 100 (FCID vereist centen als integer). Afrondingsregels (per record afronden, of totaal afronden) volgen de bestaande CJIB-uitvoering en worden in het lexogram gecodificeerd; mismatch hierop is meetbaar in de drie maanden vergelijking. |
| `bezwaar_route` | afgeleid uit het RFC-008-procedure-state van het decretogram op pull-tijdstip; zie hieronder |
| `signature` | de cel's FSC-signing key, hergebruikt als Blauwe-Knop-response-signing key (RFC-009 §5). Per response één signature (BK-spec), niet per record. Hergebruik tussen FSC en BK is een open vraag aan RFC-009 die separaat geamendeerd wordt; in de pilot accepteren we hergebruik onder voorbehoud van werksessie-bevestiging (vraag 4 aan CJIB). |
| `trace_id` | W3C Trace Context `trace_id` uit de executie-trace van het decretogram. Identificeert deze record als afkomstig uit de RegelRecht-aangedreven source, en maakt deduplicatie tegen de bestaande Blauwe-Knop-source mogelijk. |

Het `trace_id` laat een downstream surface (burgerportaal, toezichtstool) terugnavigeren naar de executie-trace die de beschikking heeft geproduceerd. De trace blijft in de cel; alleen de referentie reist mee met het record.

#### `bezwaar_route` afgeleid uit RFC-008

De cel leest geen `bezwaarbaar`-veld uit `produces`. In plaats daarvan bevraagt ze, op pull-tijdstip, het RFC-008-procedure-state van het decretogram voor de bezwaar-stage-outputs:

| `bezwaar_route`-veld | Afleiding |
|---|---|
| `intake` | de bezwaar-intake-URL van de cel voor de `procedure_id` van de regel (cel-config) |
| `termijn_grondslag` | het AWB-artikel (of lex-specialis-override) dat de termijn bepaalde, bv. `"Awb 6:7"` of `"Vw 2000 art. 69"` |
| `termijn_einddatum` | `bezwaartermijn_einddatum`-output van de BEKENDMAKING-stage-hooks (AWB 6:8 + Termijnenwet) |
| `direct_beroep_mogelijk` | true wanneer AWB 7:1a van toepassing is; anders afwezig |

Als de procedure geen bezwaar-stage heeft (UOV, AVV-zonder-direct-beroep), is `bezwaar_route` afwezig en is hetzij `beroep_route` (UOV, concretiserend BAS) hetzij `geen_rechtsbescherming_reden` (AVV, beleidsregel) aanwezig.

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

De semantiek is uniform: de regeling vraagt `openstaande_vorderingen` op bij cel `cjib`, en krijgt het resultaat van de chronolexoreductie die de cel daarvoor heeft gedefinieerd. Het transport waarover die vraag loopt, kiest de **engine-runtime** op grond van de cel-context waarin ze draait. Twee contexten zijn ondersteund; beide gebruiken dezelfde regeling-YAML en krijgen dezelfde reductie geretourneerd, in een transport-eigen formaat.

Cel-resolutie in `source.regulation` is een uitbreiding van RFC-007's bestaande resolver-semantiek. De officiële RFC-007-amendering volgt na de werksessie; voor de pilot accepteren we de uitbreiding pragmatisch.

#### Context 1: burger-client (Blauwe Knop)

Wanneer de engine in een burger-client draait (RegelRecht-WASM in de browser, of in een mobiele app), wordt de query opgelost via Blauwe Knop Connect. De engine acteert als Blauwe-Knop-client: de burger heeft een DigiD-sessie, de engine doet een pull naar de Blauwe-Knop-source van CJIB, ontvangt een ondertekende FCID-response, en geeft die als `openstaande_vorderingen` terug aan de regeling. Geen serverside aggregatie, geen kopie buiten het apparaat.

Dit is het juiste pad voor scenario's waarin de burger zelf de regeling uitvoert. Voorbeelden: een burger-applicatie die uitrekent of iemand in aanmerking komt voor een minnelijke schuldregeling, een rekentool die de gevolgen van een Wsnp-traject toont, een proactieve melding aan een burger over een naderende deurwaardingsstap. In al deze gevallen draait de engine in de burger-context en is Blauwe Knop het juiste mechanisme.

#### Context 2: bevoegde-instantie (FSC)

Wanneer de engine op een server van een bevoegde instantie draait, met een specifieke wettelijke grondslag om vorderingen van een burger op te vragen, wordt de query opgelost via FSC. Voorbeelden: een gemeente die in een Wsnp-procedure formeel schuldsanering uitvoert en daarvoor de schuldsom moet kennen, een beschermingsbewindvoerder met een mandaat van de rechtbank, een schuldhulpverleningsorganisatie met een burger-machtiging die in het systeem is gemodelleerd. De engine roept CJIB via FSC, met de FSC-key van de roepende cel en de relevante machtigings- of grondslag-attestatie. CJIB beoordeelt de aanroep aan de hand van de wettelijke grondslag en geeft (een gefilterde) `openstaande_vorderingen` terug.

Dit is bewust geen wijziging in RegelRecht's verhouding tot Blauwe Knop: FSC en Blauwe Knop bestaan náást elkaar in het Common-Ground-landschap, met verschillende use-cases. Een uitvoeringsorganisatie die als bron beschikbaar is in beide werelden, draait beide endpoints (Blauwe-Knop-source voor burgers, FSC-endpoint voor bevoegde instanties), met expliciete grondslag-controle aan de serverkant van het FSC-pad.

#### Selectie tussen contexten

De keuze tussen Blauwe Knop en FSC zit niet in de regeling. De regeling kent alleen `source.regulation: cjib`. De cel waarin de engine draait, weet welke context van toepassing is op grond van haar eigen identiteit en runtime-configuratie:

- Burger-client cel-config heeft alleen het `blauwe_knop_client`-blok geactiveerd; de resolver gebruikt Blauwe Knop.
- Bevoegde-instantie cel-config heeft alleen het `fsc_client`-blok geactiveerd; de resolver gebruikt FSC.
- Hybride cel (zeldzaam): beide blokken aanwezig; de resolver kiest op grond van de aanwezige machtigingscontext.

Dezelfde Wsnp-regeling-YAML werkt zo in een burger-app (Blauwe Knop) én in een gemeente-systeem (FSC) zonder wijziging. De juridische context die het transport bepaalt, zit in de cel-config, waar ze ook hoort.

### Vertrouwen en signing

Vertrouwen wordt overgenomen uit [RFC-009 §5](/rfcs/rfc-009). De cel tekent FCID-responses (Blauwe Knop) en FSC-responses; de ontvanger verifieert tegen de relevante Trust Anchor (App Manager voor Blauwe Knop, FSC Directory voor FSC). De handtekening is per response, niet per individueel record; dat volgt de Blauwe-Knop-specificatie.

RFC-009 is geschreven met FSC-mTLS-certificaten in gedachten. Of dezelfde private key herbruikbaar is voor de JWS-signing die Blauwe Knop vereist, is operationeel meestal mogelijk (een RSA- of EC-sleutel kan voor beide gebruikt worden) maar formeel een open punt: RFC-009 moet hierop geamendeerd worden. Voor de pilot is de praktische vraag: kan de bestaande Blauwe-Knop-signing-sleutel van CJIB hergebruikt worden door de RegelRecht-cel die ernaast draait, of vereist een tweede source een tweede sleutel? Beide werken; één sleutel is operationeel eenvoudiger. Dit is een onderdeel van de werksessie-agenda (vraag 4 aan CJIB).

### Buiten scope

- **Burger-authenticatie zelf.** Wordt door DigiD en App Manager via Blauwe Knop Connect gedaan; de cel valideert alleen de tokens die uit die flow voortkomen.
- **Betaalverwerking** (iDEAL, automatische incasso, reconciliatie) ligt upstream van deze integratie.
- **De Financial Claim Request API en Session API** rondom FCID zijn nog niet geïntegreerd; de pilot beperkt zich tot het FCID-response-formaat.
- **Burger-machtigingen via DigiD Machtigen** in het Blauwe-Knop-pad. De huidige Blauwe Knop Connect-spec beschrijft alleen de directe burger-flow; gemachtigdentoegang is een open vraag (zie ook punt 10 in Bijlage A, Open vragen).
- **De juridische grondslag** voor elke specifieke FSC-aanroep in context 2 is per geval en per relevante wettelijke bepaling. RegelRecht modelleert dat niet; de aanroepende cel is daarvoor verantwoordelijk.
- **De AWB-lifecycle-interne werking.** Alle bezwaar-mechaniek leeft in RFC-008. Deze bijlage leest RFC-008's outputs.
- **Productie-deployment-werk** (NEN 7513-logging, SLA, load-testing, key-rotation-schedule). Deze pilot draait in een sandbox; productie-werk komt na de drie maanden vergelijking, in een separaat traject.
