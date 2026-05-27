# Voorstel: een eerste pilot met CJIB op de RegelRecht/MBO-keten

*Auteur: Anne Schuth · Datum: 2026-05-27 · Status: voorstel ter bespreking*

## Aanleiding

In juli 2025 ging Vorderingenoverzicht Rijk verder als Mijn Betaaloverzicht (MBO). De achterliggende standaard, het Financial Claims Information Document (FCID), staat op vorijk.nl. De huidige stabiele versie is v3.0.0 (mei 2023); v4.x is experimenteel. MBO wordt gebruikt door de acht oorspronkelijke CRI-rijksorganisaties (Belastingdienst, Dienst Toeslagen, CJIB, DUO, SVB, CAK, UWV, RVO), inmiddels uitgebreid tot circa vijftien aansluitende organisaties via de Betalingsregeling Rijk. CJIB int de bijbehorende vorderingen, namens een groeiende kring opdrachtgevers.

In december 2025 publiceerde de Denktank Achterkant van de Overheid het ontwerp [Nieuwland](https://achterkantvandeoverheid.nl/) en het [Chronolexografie-position paper](https://chronolexografie.nl/). Daarin staat een coherent begrippenkader voor "adequaat digitaal vastleggen van de rechtstoestand": chronolexocellen, kronieken, en drie typen vastlegging (lexogram, decretogram, executogram). Eén van de redacteuren (Timen Olthof) werkt aan VORIJK/MBO; één van de geïnterviewden (Eelco Hotting, BZK) is degene met wie ik deze week sprak.

RegelRecht heeft sinds 2024 een conceptueel raamwerk opgebouwd dat schaalt naar duizenden regelingen: `legal_character` en `decision_type` voor besluiten, AWB-lifecycle als first-class construct, cross-law executie, federatie tussen bronorganisaties via FSC, Inversion of Control voor gedelegeerde regelgeving, en chronicle-achtige executie-provenance. De wetten die nu in machine-leesbare vorm in het corpus staan dienen vooral als bewijs dat het raamwerk werkt; nieuwe wetten worden opgenomen op het moment dat een cel ze nodig heeft.

Wat aan de RegelRecht-kant nog ontbrak waren drie dingen: het incasso-domein als categorie van besluiten, een expliciete plek voor executogrammen, en een schoon onderscheid tussen norm en registratie. Dat wordt deze week opgelost met de [voorgestelde RFC-019](https://docs.regelrecht.rijks.app/rfcs/rfc-019). Dit voorstel gaat over de derde stap: een pilot waarmee CJIB als eerste cel de gecombineerde RegelRecht/MBO-keten in een pilot-omgeving in productie kan brengen, zonder dat CJIB's huidige uitvoering daarvoor wijzigt.

## Wat staat er niet in dit voorstel

Voor de zekerheid, omdat dit punt makkelijk verkeerd valt: dit is geen voorstel om CJIB's huidige Wahv-uitvoering te vervangen. Het lexogram dat we bouwen staat **naast** wat CJIB nu draait. Het doel van de pilot is dat de twee voor dezelfde casuïstiek tot hetzelfde bedrag, dezelfde termijn en dezelfde bezwaarroute komen. Pas als die matching klopt, is een vervolggesprek over rolverdeling op zijn plek. Eerder niet.

## Woordenlijst

Een korte vertaling van de Chronolexografie-begrippen die in dit voorstel terugkomen, in CJIB-taal:

- **Lexogram**: een wet of regeling in machine-leesbare vorm. Vergelijkbaar met "de regelingstekst zoals jullie compliance-team die interpreteert", maar dan in YAML die een engine direct kan uitvoeren.
- **Decretogram**: een concreet besluit. Bij CJIB: een Wahv-sanctie, een OM-strafbeschikking, een schadevergoedingsmaatregel.
- **Executogram**: een feit dat de afhandeling registreert. Bij CJIB: een binnengekomen betaling, een verleende kwijtschelding, een gestart deurwaardertraject.
- **Cel (chronolexocell)**: een organisatie die kronieken bijhoudt, sleutels beheert en bevoegd gezag draagt. CJIB is een cel. NVWA, OM, DUO, CAK ook. Dit is niet nieuw maar dezelfde notie als wat RegelRecht eerder al `competent_authority` noemde.
- **Engine**: één van de componenten die in een cel kan draaien. Een cel kan één engine bevatten, meerdere, of een engine plus een legacy-systeem.
- **FSC (Federatieve Service Connectivity)**: het standaard mechanisme waarmee cellen elkaar versleuteld bereiken. Wordt onder andere gebruikt voor handtekeningen en cross-cel queries.

## Wat er nu klaar ligt

De [voorgestelde RFC-019](https://docs.regelrecht.rijks.app/rfcs/rfc-019) doet het architecturele werk. Generiek, niet CJIB-specifiek: een andere uitvoeringsorganisatie die morgen aansluit hoeft RFC-019 niet open te trekken.

Korte samenvatting van wat de RFC voorstelt:

- Lexogrammen (regelingen) blijven in `corpus/regulation/`. Decretogrammen zijn engine-output met `BESCHIKKING`. Executogrammen krijgen een eigen top-level directory `chronicles/`, naast het corpus, omdat een registratie-specificatie geen wet is.
- De cel is geen nieuw concept maar hetzelfde als wat RFC-002 al `competent_authority` noemt en wat RFC-009 als `EngineIdentity` aan de engine-kant beschrijft.
- `decision_type` wordt voorgesteld om uitgebreid te worden met drie financiële-domein waarden (BETALINGSVERPLICHTING, STRAFBESCHIKKING, BESTUURLIJKE_BOETE).
- Intrekkingen zijn een nested besluit in de zin van RFC-008 (eigen AWB-lifecycle); `modality.is_intrekking_van` is alleen een backlink-veld naar het origineel besluit, geen nieuwe semantiek.
- Integraties hangen in een namespaced `extensions`-blok, en hun activatie gebeurt in de cel-configuratie, niet in de wet zelf.
- Cross-cell queries hergebruiken het bestaande `source`-blok uit RFC-007; de resolver bepaalt aan de hand van de FSC-service-registry of de naam in `source.regulation` een cel of een regeling is.
- Rechtsbescherming wordt niet als nieuw veld geïntroduceerd: een uitgaande integratie leidt de `bezwaar_route` af uit de RFC-008-procedure-stage van het decretogram, op het juiste moment (BEKENDMAKING), zodat de werkelijke einddatum meereist en niet een statische hint.

Voor de werksessie hieronder is het niet nodig RFC-019 helemaal door te lezen. Dit voorstel is zelfstandig leesbaar. De RFC is er voor de IT-lead die wil zien hoe de mapping er onder de motorkap uitziet.

## Het denkkader

Chronolexografie onderscheidt drie typen vastlegging die in de rechtsstaat alle drie nodig zijn.

- **Lexogram**: vastlegging van een (mogelijke) wijziging in wet- of regelgeving. Voorbeeld: de Wahv zoals die geldt sinds 1 januari 2025.
- **Decretogram**: vastlegging van een concreet besluit. Voorbeeld: een Wahv-sanctie van €X die op datum Y aan kentekenhouder Z wordt opgelegd.
- **Executogram**: vastlegging van feitelijke afhandeling. Voorbeeld: een betaling van €X die op datum Z bij CJIB binnenkomt onder zaakkenmerk Y.

In de huidige situatie wonen deze drie typen in gescheiden systemen, met telkens een verlies aan context op de overgangen. De burger ziet wel het bedrag in MBO, maar niet de beschikking of het artikel. De gevolgen daarvan zijn beschreven in Nieuwland en in eerdere publicaties van Kafkabrigade. De pilot die hieronder volgt sluit deze keten voor één wet bij één cel.

## Wat de pilot inhoudt

Voor één pilotwet (voorkeur: Wahv) leveren we drie samenhangende artefacten op.

**Een lexogram.** Een YAML-bestand `corpus/regulation/nl/wet/wet_administratiefrechtelijke_handhaving_verkeersvoorschriften/<valid_from>.yaml`. Dit is de Wahv in machine-leesbare vorm conform het RegelRecht-schema. Eén artikel produceert een `BESCHIKKING` met `decision_type: BETALINGSVERPLICHTING`, het juiste `procedure_id` per RFC-008 (default `beschikking`), en een `extensions.mbo_fcid.category: ALGEMEEN`-hint. De bezwaarweg zit niet in de regeling, want die wordt door RFC-008 afgeleid uit de AWB-procedure.

**Een chronicle-stream.** Een YAML-bestand `chronicles/cjib_wahv_betalingen.yaml` met minstens drie events: `payment_received`, `kwijtschelding_verleend`, `deurwaardertraject_gestart`. Per event de juiste FCID-mapping in `extensions.mbo_fcid`. `kwijtschelding_verleend` declareert `references_decision: <kwijtschelding-besluit-id>` zodat de integratie de bezwaarweg via dat besluit kan afleiden. `payment_received` en `deurwaardertraject_gestart` zijn feiten zonder bezwaar.

**Een werkende emit-pad.** Een RegelRecht-engine draait binnen een afgeschermde CJIB-pilot-omgeving met de Wahv-lexogram en de chronicle-stream geladen. De cel-configuratie activeert `mbo_fcid`. Wanneer een Wahv-beschikking door de AWB-lifecycle (RFC-008) bij de BEKENDMAKING-stage aankomt, emit de cel een FCID-event naar het MBO-pilot-endpoint, getekend met de CJIB-FSC-key, inclusief `bezwaar_route` die door AWB-6:7/6:8-hooks op dat moment is berekend (inclusief feitelijke einddatum). Op een betaling die binnenkomt vanuit het surrounding incasso-systeem doet de cel hetzelfde voor `BetalingVerwerkt`. Aan burger-zijde: een Wahv-vordering in MBO bevat een directe link naar het artikel, een referentie naar de executie-trace, een bezwaarknop met de juiste route en de werkelijke einddatum, en, na betaling, een gekoppeld BetalingVerwerkt-event onder hetzelfde zaakkenmerk.

Het Wahv-artikel ziet er in YAML ongeveer zo uit:

```yaml
# Primaire sanctie
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: BETALINGSVERPLICHTING
    procedure_id: beschikking            # RFC-008 default
    extensions:
      mbo_fcid:
        category: ALGEMEEN
        emit_at_stage: BEKENDMAKING

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
      mbo_fcid:
        category: ALGEMEEN
        emit_at_stage: BEKENDMAKING
```

De integratie ziet de `modality.is_intrekking_van` en stuurt voor de intrekking-instance een `BetalingsverplichtingIngetrokken`-event in plaats van een `BetalingsverplichtingOpgelegd`. Beide events delen het `zaakkenmerk` met de oorspronkelijke beschikking, zodat MBO ze in één tijdlijn presenteert.

CJIB hoeft deze YAML niet zelf te schrijven; ons team doet de eerste versie. Wat we van CJIB nodig hebben is dat een domeinexpert verifieert dat het klopt: dat de bedragen, termijnen en de bezwaarweg overeenkomen met wat het bestaande Wahv-systeem doet voor dezelfde casuïstiek.

## Wat de pilot CJIB oplevert

**Eén bron voor norm, besluit en feit.** Het lexogram zit in het corpus; het besluit komt uit de engine; het feit komt uit de chronicle-stream. Wijzigt de wet, dan beweegt het FCID-event mee zonder aparte release in een tweede systeem.

**"Samen zien" voor de burger in de zin van Nieuwland §5.4.** Dezelfde tijdlijn van vastleggingen is gelijktijdig en gelijkwaardig toegankelijk voor burger en cel. Dat sluit aan op de motiveringsplicht uit [Awb 3:46](https://wetten.overheid.nl/BWBR0005537) en op het MBO-principe dat data bij de bron blijft.

**Rechtsbescherming als ontwerp, niet als marketing.** De AWB-lifecycle uit RFC-008 levert de bezwaartermijn op het juiste moment (BEKENDMAKING, niet BESLUIT). De integratie pakt die termijn op en stuurt 'm mee als `bezwaar_route` in elk FCID-event. Een Wahv-sanctie met automatische ophoging die voor iemand met laag inkomen disproportioneel uitwerkt, krijgt op het moment van bekendmaking een zichtbare bezwaarknop in de MBO-surface met de werkelijke einddatum, niet pas na een aanmaning. Dit is de operationalisering van Nieuwland §7.2.1.

**Een directe invulling van de Chronolexografie-architectuur, met behoud van organisatie-autonomie.** CJIB is een cel met eigen kronieken en eigen sleutels. NVWA, NEa, DUO, CAK kunnen straks elk hun eigen cel zijn, met dezelfde mappingsregels. Geen centraal systeem, geen vendor lock-in.

**Voorspelbare schaalbaarheid voor nieuwe opdrachtgevers.** Sectorale toezichthouders die instromen in de Betalingsregeling Rijk krijgen `decision_type: BESTUURLIJKE_BOETE`, het juiste `procedure_id` per RFC-008, en een `extensions.mbo_fcid.category`. Geen schemawijziging per opdrachtgever, geen forks van regelingen alleen voor verschillen in MBO-aansluiting.

## Wat wij van onze kant inbrengen

Concrete tegenprestatie, geen open einde aan de CJIB-kant:

- **Het lexogram en de chronicle-stream** worden door ons geschreven, op basis van CJIB's bestaande Wahv-uitvoering en de wetstekst. Eerste versie binnen twee weken na vaststelling van de pilotwet.
- **Een pilot-sandbox** waarin de engine met de Wahv-lexogram draait, gevoed door dezelfde input als CJIB's huidige systeem zou krijgen. CJIB hoeft geen productie-IT te raken voor de pilot.
- **Een koppelteam** van twee mensen: ik (Anne) als architect en één engineer voor de implementatie van de chronicle-stream en de cel-config. Aan CJIB-kant is één domeinexpert en eventueel één IT-contact genoeg om in een wekelijks ritme te valideren.
- **Volledige documentatie** van de mapping per artikel, controleerbaar tegen het bestaande Wahv-systeem. Mismatches zijn data voor het volgende gesprek, niet een falen.
- **Geen leveringsverplichting** als de pilot niet werkt. Na drie maanden Wahv-vergelijking is een no-go besluit een legitieme uitkomst, geen mislukking.

## Wat we van CJIB nodig hebben

Vijf dingen, geen open einde.

1. **Validatie van het uitvoeringslandschap** (zie bijlage A). Het overzicht is opgebouwd uit publieke bronnen. Welke regelingen ontbreken of zijn fout toegewezen?
2. **Bevestiging of bijstelling van de pilotwet.** Wahv is een goede pilot omdat het juridisch kader helder is (enkelvoudig artikel, weinig nesting), CJIB de wet al uitvoert (mismatches zijn meetbaar tegen een bestaande baseline), en de foutmarge bij een afwijking laag is (parkeerboete, geen bijstand). Liever iets anders? OM-strafbeschikking voor één feitcode is een optie. NVWA-bestuurlijke boetes zouden de schaalbaarheid scherper testen omdat het sectoraal is.
3. **FCID-versie en endpoint-status.** Welke versie draait nu in jullie pilot of productie, en op welke endpoints? De integratie-specificatie kan starten op v3.0.0 (de stabiele versie) en meeschalen naar v4.x zodra die productie-rijp is.
4. **Knelpunten in de mapping.** Voor `zaakkenmerk` geldt CJIB's eigen zaaknummer-systematiek als leidend. Voor signing gaan we uit van de RFC-009 FSC-key. Botst dit met de CJIB-praktijk?
5. **Cel-topologie en bezwaar-routing.** Hoeveel cellen zou CJIB draaien (één centraal, één per opdrachtgever, één per regelinggebied)? En per type vordering: waar landt het bezwaar formeel, en waar landt het in de praktijk? Een CAK-eigen-bijdrage-vordering staat onder CJIB-zaaknummer maar het bezwaar gaat juridisch naar CAK; in de operationele realiteit belt de burger naar CJIB. De `bezwaar_route` in het FCID-event moet kloppen met beide werelden. Hier wil ik graag samen met Timen Olthof naar kijken.

## Volgende stap

Een werksessie van een dagdeel met CJIB, het VORIJK/MBO-team, Eelco en mij. Agenda: het uitvoeringslandschap valideren, de pilotwet vastpinnen, de cel-topologie schetsen, de bezwaar-routing per type vordering uitwerken, knelpunten benoemen. Daarna kan de voorgestelde RFC-019 verder, kan de bijbehorende schema-bump (v0.5.2 → een nieuwe v0.6.0) worden voorbereid, en kunnen we beginnen met het Wahv-lexogram en de eerste chronicle-stream.

Doel: binnen één maand na de werksessie een werkende emit-pad in een pilot-omgeving, met één Wahv-beschikking die als FCID-event in MBO-pilot belandt, een bezwaarknop bevat met de juiste route en termijn, en die teruggetraceerd kan worden naar het wetsartikel. Daarna drie maanden Wahv-vergelijking tegen de bestaande CJIB-uitvoering. Pas daarna een gesprek over wat volgt. Geen jaartallen op het pad daarna; eerst dit laten kloppen.

## Bijlagen

- [Bijlage A: CJIB-uitvoeringslandschap](#bijlage-a-cjib-uitvoeringslandschap)
- [Bijlage B: FCID-mapping in detail](#bijlage-b-fcid-mapping-in-detail)
- [RFC-019: Chronolexogram types in the schema and the cell model](https://docs.regelrecht.rijks.app/rfcs/rfc-019)
- [RFC-009: Multi-Organisation Execution](https://docs.regelrecht.rijks.app/rfcs/rfc-009)
- [Chronolexografie-position paper](https://chronolexografie.nl/position-paper/) van Olthof en Van Andel, december 2025
- [Nieuwland, een ontwerp voor een digitale rechtsstaat](https://achterkantvandeoverheid.nl/) van Denktank Achterkant van de Overheid, 15 december 2025
- [FCID-spec op vorijk.nl](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/)

---

## Bijlage A: CJIB-uitvoeringslandschap

Deze bijlage inventariseert wat CJIB feitelijk doet: welke regelingen het zelf uitvoert, welke het namens andere organisaties uitvoert, en het beleidskader daaromheen. Niet normatief; achtergrondmateriaal voor de werksessie. Onzekerheden zijn met `[onzeker]` gemarkeerd.

### Waarom CJIB centraal staat

CJIB is een *zelfstandig bestuursorgaan* (ZBO) onder het ministerie van Justitie en Veiligheid. Het is het centrale financiële-handhavingsknooppunt van de Nederlandse staat: bijna elke administratiefrechtelijke en strafrechtelijke financiële verplichting komt hier uiteindelijk terecht wanneer een burger niet vrijwillig betaalt. Per 2026 voert CJIB uit voor minstens vijftien opdrachtgevers, van OM tot een sectorale inspectie als NEa.

### Mapping op de drie chronolexogram-typen

CJIB's dagelijkse werk raakt alle drie de vastleggingstypen:

- CJIB *voert lexogrammen uit*: de wetten en beleidsregels onder welke het werkt.
- CJIB *produceert decretogrammen*: Wahv-sancties, OM-strafbeschikkingen die het uitvoert.
- CJIB *registreert executogrammen*: binnengekomen betalingen, verleende kwijtscheldingen, gestarte deurwaardertrajecten.

RFC-019 plaatst elk van deze in zijn juiste plek in de repository-layout: lexogrammen in `corpus/regulation/`, chronicle-stream-definities (die declareren welke executogrammen een cel registreert) in `chronicles/`.

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

In Chronolexografie-termen: de cel van de *opdrachtgever* produceert de primaire decretogram (de inhoudelijke beschikking); CJIB's cel registreert de executogrammen (betaling, kwijtschelding) namens die opdrachtgever. Of CJIB ook een eigen vervolg-decretogram produceert (bijvoorbeeld een dwangbevel onder Awb 4:114) hangt af van de regeling en het convenant.

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
- *Fiscale aanslagen* (inkomstenbelasting, BTW, etc.). De Belastingdienst voert zijn eigen invordering uit op grond van de Invorderingswet 1990.
- Gemeentelijke leges en lokale heffingen, zelfde reden als parkeerboetes.
- *Civielrechtelijke vorderingen* tussen particulieren. Die lopen via gerechtsdeurwaarders.
- Deurwaardersbeslag in privaatrechtelijke geschillen.

De lijn is grofweg: CJIB doet door de staat opgelegde financiële verplichtingen onder publiek recht (strafrechtelijk, bestuursrechtelijk, of specifieke civielrechtelijke slachtoffermaatregelen), specifiek wanneer de inning centraal op rijksniveau is belegd.

### Beleidskader

| Instrument | Jaar | Bron |
|---|---|---|
| Beleidsregels tenuitvoerlegging strafrechtelijke en administratiefrechtelijke beslissingen (USB 2021) | 2021 | [Stcrt 2021, 33851](https://zoek.officielebekendmakingen.nl/stcrt-2021-33851.html) |
| Wet USB + Invoeringswet USB | Stb 2017, 82; Stb 2019, 504; in werking 2020-01-01 | Boek 6 Sv |
| Aanwijzing OM-strafbeschikking | 2022A003 | [OM publicatie](https://www.om.nl/onderwerpen/beleidsregels/aanwijzingen/executie/aanwijzing-om-strafbeschikking-2022a003) |
| Algemene wet bestuursrecht titel 4.4 (Bestuursrechtelijke geldschulden) | In werking 2009-07-01 | [BWBR0005537 art. 4:85–4:125](https://wetten.overheid.nl/BWBR0005537) |
| Evaluatiewet bestuursrechtelijke geldschuldenregeling Awb (35.477) | In behandeling/aangenomen | [Eerste Kamer dossier](https://www.eerstekamer.nl/wetsvoorstel/35477_evaluatiewet) |
| CRI-programma (Clustering Rijksincasso) | Lopend | [eenoverheidsincasso.nl](https://www.eenoverheidsincasso.nl/) |

### Voorgestelde uitbreiding op het RegelRecht-schema

Het huidige `produces.decision_type` enum heeft negen waarden (TOEKENNING, AFWIJZING, GOEDKEURING, GEEN_BESLUIT, ALGEMEEN_VERBINDEND_VOORSCHRIFT, BELEIDSREGEL, VOORBEREIDINGSBESLUIT, ANDERE_HANDELING, AANSLAG). Geen daarvan beschrijft het financiële handhavingsdomein.

De voorgestelde RFC-019 voegt drie waarden toe, elk een afzonderlijk type besluit:

- `BETALINGSVERPLICHTING`: generieke financiële verplichting opgelegd door een bestuursorgaan
- `STRAFBESCHIKKING`: strafrechtelijke afdoening onder Sv 257a
- `BESTUURLIJKE_BOETE`: sectorale bestuurlijke boete

*Intrekkingen* van een eerdere beschikking zijn geen apart type: ze zijn dezelfde `decision_type` met `modality.is_intrekking_van: <id>`. Dit volgt de Awb-praktijk: een intrekking is een handeling op een bestaand besluit, niet een nieuw type besluit. `legal_character: BESCHIKKING` dekt al deze gevallen.

### Open vragen en data-gaten

De volgende items konden niet uit publieke bronnen worden geverifieerd en hebben input van CJIB of zijn opdrachtgevers nodig:

1. **Volledige CJIB-portfolio.** Interne USB-lijsten bestaan maar zijn niet publiek geïndexeerd.
2. **CJIB-zijdige FCID-adoptiestatus.** Welke regelingen emitteren al FCID (in pilot of productie), en welke versie?
3. **DFEI-scope.** CRI noemt "Dienst Financiële en Economische Integriteit", maar de exacte overdracht naar CJIB is onduidelijk.
4. **Cel-topologie bij CJIB.** Draait CJIB één cel per opdrachtgever, één per regelingstype, of één centraal? RFC-009 en RFC-019 ondersteunen elke keuze; de keuze beïnvloedt chronicle-ordering en signing-sleutels.
5. **Bilaterale convenanten** die niet in Staatscourant gepubliceerd worden: er kunnen extra opdrachtgevers zijn die niet via publieke bronnen zichtbaar zijn.
6. **DNA-V kostenverhaalgrondslag.** BWBR0017212 koppelt kostenverhaal niet rechtstreeks aan CJIB; dit loopt waarschijnlijk via beleidsregels die nog geverifieerd moeten worden.
7. **Per-opdrachtgever BWB-IDs** met `[onzeker]` in de tabel.
8. **Bezwaar-routing.** Elk door CJIB geëmitteerd FCID-event draagt een `bezwaar_route` die op het emissie-moment uit de RFC-008-procedure-stage van het decretogram is afgeleid. Voor decretogrammen die CJIB zelf produceert (Wahv-sanctie) wijst de route naar CJIB's eigen bezwaar-intake. Voor decretogrammen die CJIB namens een andere cel draagt (een CAK-besluit, een OM-strafbeschikking) wijst de route formeel naar die andere cel, terwijl de burger in de praktijk vaak CJIB belt. De wettelijke routing per regeling moet per geval gevalideerd worden.
9. **Wet gegevensboekhouding-interactie.** Nieuwland §7.3.2 schetst een Wet gegevensboekhouding die de executogram-zijdige registratie een wettelijke basis zou geven. De huidige grondslag van CJIB is impliciet in Awb 4.4 + sectorale wetten; een expliciete wet zou het beeld wijzigen.

---

## Bijlage B: FCID-mapping in detail

Deze bijlage specificeert hoe een cel die een RegelRecht-engine draait integreert met MBO via FCID. Inhoud is technisch; bedoeld voor de IT-lead die de pilot begeleidt.

### Doelversie

Baseline: **FCID v3.0.0** (mei 2023, huidige stabiele versie volgens vorijk.nl). De architectuur is voorbereid op v4.x zodra die productie-rijp is. v4.2.0 is per mei 2026 nog experimenteel; we volgen wat CJIB feitelijk draait. De integratie-spec is herzienbaar zonder dat RFC-019 opnieuw opengaat.

### Wat de integratie doet

Een cel die een RegelRecht-engine draait en de `mbo_fcid`-integratie activeert, emit twee stromen events naar MBO-endpoints:

- **Decretogram-afgeleide FCID-events**, wanneer een regeling een `BESCHIKKING` produceert met een financiële `decision_type` en haar `extensions.mbo_fcid`-blok de FCID-categorie declareert. Emissie gebeurt op een specifieke AWB-lifecycle-stage (RFC-008), in de regel `BEKENDMAKING`.
- **Executogram-afgeleide FCID-events**, wanneer een event in een chronicle-stream-bestand (onder `chronicles/`) een `extensions.mbo_fcid`-blok declareert en de bijbehorende intake afgaat.

In de consumer-richting kan een regeling de openstaande vorderingen van een burger bij MBO opvragen door de CJIB-cel te noemen in een normaal `source`-blok. De query bereikt de CJIB-cel via het RFC-009 ACCEPT-pad; geen wrapper-regeling, geen nieuwe schema-velden.

### Activatie

Een cel beslist zelf of ze meedoet. Activatie zit in de cel-configuratie, niet in een regeling:

```yaml
# cel-config (schets; volledig cel-config formaat in latere RFC)
integrations:
  mbo_fcid:
    enabled: true
    endpoint: https://mbo.example.gov.nl/intake
    fcid_version: 3.0.0
```

De vorm hierboven is voorlopig en kan evolueren. Wat vaststaat: activatie is een cel-beslissing, niet onderdeel van een regeling. Een gemeente die het Wahv-lexogram draait maar niet aansluit op MBO laat het `mbo_fcid`-blok in haar cel-config gewoon weg. Dezelfde regeling-YAML werkt in beide cellen.

### FCID-event-typen

FCID definieert vier event-typen. Elk mapt op precies één chronolexogram-type.

| FCID `event_type` | Chronolexogram-type | Bron in RegelRecht |
|---|---|---|
| `FinancieleVerplichtingOpgelegd` | decretogram | engine-output, `decision_type: STRAFBESCHIKKING` (totaalbedrag) |
| `BetalingsverplichtingOpgelegd` | decretogram | engine-output, `decision_type: BETALINGSVERPLICHTING` / `BESTUURLIJKE_BOETE` |
| `BetalingsverplichtingIngetrokken` | decretogram (intrekking-modaliteit) | engine-output, zelfde `decision_type` als origineel, met `produces.modality.is_intrekking_van` gezet |
| `BetalingVerwerkt` | executogram | chronicle-stream-event, getriggerd door intake vanuit incasso-systeem |

Een intrekking is zelf een nieuwe BESCHIKKING met haar eigen AWB-lifecycle (per RFC-008 resolved Open Question 5). De integratie herkent het als intrekking via `produces.modality.is_intrekking_van: <oorspronkelijke-id>` en mapt naar `BetalingsverplichtingIngetrokken`. Intrekking en origineel delen een `zaakkenmerk` zodat een downstream consument ze aan elkaar kan koppelen.

### Producer-zijde: decretogram-afgeleide FCID

Een regel die FCID moet emitten bij het produceren van een decretogram, declareert die intentie in het `extensions.mbo_fcid`-blok:

```yaml
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: BETALINGSVERPLICHTING
    procedure_id: beschikking         # RFC-008 procedure-selectie
    extensions:
      mbo_fcid:
        category: ALGEMEEN
        emit_at_stage: BEKENDMAKING   # default; overrijdbaar
```

`category` is een van `ALGEMEEN`, `ADMINISTRATIEKOSTEN`, `VERHOGING`, `RENTE`. Een regeling die meerdere FCID-regels uit één beschikking produceert (hoofdsom + administratiekosten + verhoging) declareert die als aparte artikelen of aparte `produces`-blokken, elk met zijn eigen `extensions.mbo_fcid.category`.

`emit_at_stage` selecteert de RFC-008-lifecycle-stage waarop emissie afgaat. Default is `BEKENDMAKING`: een verplichting die niet bekendgemaakt is heeft geen juridische bestaansgrond om in MBO te tonen.

#### Veld-afleiding

Wanneer de cel een FCID-event emit vanuit een decretogram op de geconfigureerde stage:

| FCID-veld | Afleiding |
|---|---|
| `event_type` | uit `decision_type` plus `modality.is_intrekking_van` per de tabel hierboven |
| `category` | uit `extensions.mbo_fcid.category` |
| `juridische_grondslag_omschrijving` | eerste zin van `article.text`, of `article.title` als korter |
| `juridische_grondslag_bron` | `article.url` (canonieke wetten.overheid.nl-link) |
| `zaakkenmerk` | de cel's bestaande zaaknummer-systematiek; anders deterministische hash van `(cell.id, beschikking_id)` |
| `gebeurtenis_kenmerk` | UUID v7, gegenereerd op emissie-tijdstip |
| `bedrag` | currency-getypeerde output × 100 (FCID vereist centen als integer) |
| `bezwaar_route` | afgeleid uit het RFC-008-procedure-state van het decretogram op de emissie-stage; zie hieronder |
| `signature` | de cel's FSC-signing key (RFC-009 §5) |
| `trace_id` | W3C Trace Context `trace_id` uit de executie-trace van het decretogram |

Het `trace_id` laat een downstream surface (burgerportaal, toezichtstool, andere cel) terugnavigeren naar de executie-trace die de beschikking heeft geproduceerd. De trace blijft in de cel; alleen de referentie reist mee met het event.

#### `bezwaar_route` afgeleid uit RFC-008

De integratie leest geen `bezwaarbaar`-veld uit `produces`. In plaats daarvan bevraagt ze, op emissie-tijdstip, het RFC-008-procedure-state van het decretogram voor de bezwaar-stage-outputs:

| `bezwaar_route`-veld | Afleiding |
|---|---|
| `intake` | de bezwaar-intake-URL van de cel voor de `procedure_id` van de regel (cel-config) |
| `termijn_grondslag` | het AWB-artikel (of lex-specialis-override) dat de termijn bepaalde, bv. `"Awb 6:7"` of `"Vw 2000 art. 69"` |
| `termijn_einddatum` | `bezwaartermijn_einddatum`-output van de BEKENDMAKING-stage-hooks (AWB 6:8 + Termijnenwet) |
| `direct_beroep_mogelijk` | true wanneer AWB 7:1a van toepassing is; anders afwezig |

Als de procedure geen bezwaar-stage heeft (UOV, AVV-zonder-direct-beroep), is `bezwaar_route` afwezig en is hetzij `beroep_route` (UOV, concretiserend BAS) hetzij `geen_rechtsbescherming_reden` (AVV, beleidsregel) aanwezig.

### Producer-zijde: executogram-afgeleide FCID

Een chronicle-stream-entry die FCID moet emitten declareert het op dezelfde manier:

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
      mbo_fcid:
        event_type: BetalingVerwerkt
        category: ALGEMEEN
```

Een chronicle-stream-event zonder `extensions.mbo_fcid`-blok wordt nog steeds opgenomen in de kroniek van de cel; het verschijnt alleen niet in MBO.

Veld-afleiding voor executogrammen:

| FCID-veld | Afleiding |
|---|---|
| `event_type` | uit `extensions.mbo_fcid.event_type` |
| `category` | uit `extensions.mbo_fcid.category` |
| `zaakkenmerk` | hetzelfde `zaakkenmerk` als het oorspronkelijke decretogram, dat de betaling koppelt aan de verplichting |
| `gebeurtenis_kenmerk` | UUID v7, gegenereerd op emissie-tijdstip |
| `bedrag` | uit het `amount_cents`-veld van het event |
| `gebeurtenis_datetime` | uit het `received_at`-veld van het event |
| `signature` | de cel's FSC-signing key |

#### Rechtsbescherming op executogrammen

De meeste executogrammen dragen geen `bezwaar_route`. Een ontvangen betaling is een feit, geen besluit; bezwaar maken tegen een feit is niet waar bezwaar voor is.

Een kleine klasse executogrammen draagt er wel een: events die *impliciet een nested besluit referencen*. Een `kwijtschelding_verleend`-event is de buitenkant van een kwijtscheldings-decretogram (met eigen AWB-lifecycle). De chronicle-stream declareert de link:

```yaml
- name: kwijtschelding_verleend
  references_decision: $external.kwijtschelding_decision_id
  fields:
    case_reference: $external.zaakkenmerk
    reden: $external.reden
  extensions:
    mbo_fcid:
      event_type: BetalingsverplichtingIngetrokken
      category: ALGEMEEN
```

Wanneer `references_decision` aanwezig is, kijkt de integratie het RFC-008-procedure-state van die beslissing op en leidt de `bezwaar_route` daaruit af.

### Consumer-zijde: bevragen van MBO

Een regeling die de openstaande vorderingen van een burger nodig heeft, gebruikt het normale `source`-blok (RFC-007) en benoemt de CJIB-cel in plaats van een regeling. De CJIB-cel ontsluit `openstaande_vorderingen` als één van zijn lexostatus-outputs:

```yaml
input:
  - name: openstaande_vorderingen
    source:
      regulation: cjib                      # cel-id in de FSC-service-registry
      output: openstaande_vorderingen       # de lexostatus die de cel ontsluit
      parameters:
        bsn: $bsn
```

De resolver van de engine zoekt `cjib` op in de FSC-service-registry. Omdat het resolvt naar een cel (niet naar een regeling in het geladen corpus), volgt de resolver het RFC-009 ACCEPT-pad: een federatieve query naar de CJIB-cel. Een CJIB-engine zelf antwoordt lokaal uit de eigen kroniek. Hoe dan ook ziet de consument een lijst vordering-records die als downstream-input bruikbaar is.

### Vertrouwen en signing

Vertrouwen wordt overgenomen uit [RFC-009 §5](https://docs.regelrecht.rijks.app/rfcs/rfc-009) zonder wijziging. De cel tekent zowel decretogram-afgeleide als executogram-afgeleide events met haar FSC-key. De ontvanger verifieert tegen de Trust Anchor in de FSC Directory. Het `event_type` onderscheidt de twee in het verkeer; de signing-key niet.

### Buiten scope

- **Burger-authenticatie** voor portaal-zijdige toegang tot MBO-data ligt op het API-gateway-niveau.
- **Betaalverwerking** (iDEAL, automatische incasso, reconciliatie) ligt upstream van deze integratie.
- **De Financial Claim Request API en Session API** rondom FCID zijn nog niet geïntegreerd.
- **De juridische grondslag** voor elke specifieke uitwisseling (welke cel mag welk event naar welke ontvanger sturen) is per geval en per relevante wettelijke bepaling.
- **De AWB-lifecycle-interne werking.** Alle bezwaar-mechaniek leeft in RFC-008. Deze integratie leest RFC-008's outputs.
