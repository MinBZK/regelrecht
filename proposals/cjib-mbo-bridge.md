# Voorstel: een eerste pilot met CJIB op de RegelRecht/MBO-keten

*Auteur: Anne Schuth · Datum: 2026-05-26 · Status: voorstel ter bespreking*

## Aanleiding

Drie publicaties verschenen in 2025 die hetzelfde willen oplossen.

In juli 2025 ging Vorderingenoverzicht Rijk verder als Mijn Betaaloverzicht (MBO). De achterliggende standaard, het Financial Claims Information Document (FCID, v4.2.0 mei 2026), staat op vorijk.nl en wordt door minimaal acht CRI-rijksorganisaties gebruikt of voorbereid. CJIB int de bijbehorende vorderingen, namens een groeiende kring opdrachtgevers.

In december 2025 publiceerde de Denktank Achterkant van de Overheid het ontwerp [Nieuwland](https://achterkantvandeoverheid.nl/) en het [Chronolexografie-position paper](https://chronolexografie.nl/). Daarin staat een coherent begrippenkader voor "adequaat digitaal vastleggen van de rechtstoestand": chronolexocellen, kronieken, en drie typen vastlegging (lexogram, decretogram, executogram). Eén van de redacteuren (Timen Olthof) werkt aan VORIJK/MBO; één van de geïnterviewden (Eelco Hotting, BZK) is degene met wie ik deze week sprak.

RegelRecht heeft sinds 2024 een conceptueel raamwerk opgebouwd dat schaalt naar duizenden regelingen: `legal_character` en `decision_type` voor besluiten, AWB-lifecycle als first-class construct, cross-law executie, federatie tussen bronorganisaties via FSC, Inversion of Control voor gedelegeerde regelgeving, en chronicle-achtige executie-provenance. De wetten die nu in machine-leesbare vorm in het corpus staan dienen vooral als bewijs dat het raamwerk werkt; nieuwe wetten worden opgenomen op het moment dat een cel ze nodig heeft. Wat aan de RegelRecht-kant nog ontbrak waren drie dingen: het incasso-domein als categorie van besluiten, een expliciete plek voor executogrammen, en een schoon onderscheid tussen norm en registratie. Dat wordt deze week opgelost met twee documenten: een generieke architectuur-RFC en een concrete integratie-spec.

Dit voorstel gaat over de derde stap: een pilot waarmee CJIB als eerste cel de gecombineerde RegelRecht/MBO-keten in productie kan brengen.

## Wat er nu klaar ligt

In de RegelRecht-codebase zijn drie nieuwe documenten beschikbaar.

**[RFC-022: Chronolexogram types in the schema and the cell model](https://docs.regelrecht.rijks.app/rfcs/rfc-022).** Generiek. Voegt drie eerste-klas concepten toe aan RegelRecht. Lexogrammen (regelingen) blijven in `corpus/regulation/`. Decretogrammen zijn engine-output met `BESCHIKKING`. Executogrammen krijgen een eigen top-level directory `chronicles/`, naast het corpus, omdat een registratie-specificatie geen wet is. De engine is geen cel maar een component binnen een cel. `decision_type` wordt uitgebreid met drie financiële-domein waarden (BETALINGSVERPLICHTING, STRAFBESCHIKKING, BESTUURLIJKE_BOETE). Intrekkingen zijn een nested besluit in de zin van RFC-008 (eigen AWB-lifecycle), met `modality.is_intrekking_van` als verwijzing naar het origineel. Integraties hangen in een namespaced `extensions`-blok in plaats van anonieme outbound-velden, en hun activatie gebeurt in de cel-configuratie, niet in de wet zelf. Cross-cell queries lopen via `source.kind: lexostatus_query`, geen wrapper-regelingen meer. Rechtsbescherming wordt niet als nieuw veld geïntroduceerd: een uitgaande integratie leidt de `bezwaar_route` af uit de RFC-008-procedure-stage van het decretogram, op het juiste moment (BEKENDMAKING), zodat de werkelijke einddatum meereist en niet een statische hint.

**[Integratie-document: Mijn Betaaloverzicht (FCID)](https://docs.regelrecht.rijks.app/integrations/mbo-fcid).** Concreet. Beschrijft hoe het `mbo_fcid` extensie-blok wordt geïnterpreteerd, hoe FCID-events worden afgeleid, hoe het `bezwaar_route` veld in de FCID-emissie de bezwaarweg meedraagt naar de MBO-surface, en hoe de consumer-side lexostatus-query naar CJIB werkt. Target FCID v4.x, herzienbaar zonder RFC-proces.

**[CJIB-uitvoeringslandschap](https://docs.regelrecht.rijks.app/concepts/cjib-uitvoeringslandschap).** Inventarisatie. Welke regelingen CJIB uitvoert (zelfstandig en namens andere cellen), met grondslagen, BWB-IDs, en de chronolex-rol per opdrachtgever.

Samen zijn dit de bouwstenen. De architectuur is RegelRecht-eigen en blijft bestaan ook als FCID verandert; de integratie-laag is herzienbaar zonder dat het RFC-proces opnieuw hoeft.

## Het denkkader

Chronolexografie onderscheidt drie typen vastlegging die in de rechtsstaat alle drie nodig zijn.

- **Lexogram**: vastlegging van een (mogelijke) wijziging in wet- of regelgeving. Voorbeeld: de Wahv zoals die geldt sinds 1 januari 2025.
- **Decretogram**: vastlegging van een concreet besluit. Voorbeeld: een Wahv-sanctie van €X die op datum Y aan kentekenhouder Z wordt opgelegd.
- **Executogram**: vastlegging van feitelijke afhandeling. Voorbeeld: een betaling van €X die op datum Z bij CJIB binnenkomt onder zaakkenmerk Y.

Het vierde sleutelconcept is de **chronolexocel**: de juridische en organisatorische eenheid die kronieken bijhoudt, sleutels beheert en bevoegd gezag draagt. CJIB is een cel. NVWA is een cel. Een RegelRecht-engine is een component dat in zo'n cel kan draaien, naast andere componenten. Implementaties variëren: één engine, meerdere, of een engine plus een legacy-systeem dat ook chronolexogrammen produceert. De cel-definitie ligt op het organisatorische vlak, niet op het binaire.

In de huidige situatie wonen de drie typen in gescheiden systemen, met telkens een verlies aan context op de overgangen. De citizen ziet wel het bedrag in MBO, maar niet de beschikking of het artikel. De gevolgen daarvan zijn beschreven in Nieuwland en in eerdere publicaties van Kafkabrigade. De pilot die hieronder volgt sluit deze keten voor één wet bij één cel.

## Wat de pilot inhoudt

Voor één pilotwet (voorkeur: Wahv) leveren we drie samenhangende artefacten op.

**Een lexogram.** Een YAML-bestand `corpus/regulation/nl/wet/wet_administratiefrechtelijke_handhaving_verkeersvoorschriften/<valid_from>.yaml`. Dit is de Wahv in machine-leesbare vorm conform het RegelRecht-schema. Eén artikel produceert een `BESCHIKKING` met `decision_type: BETALINGSVERPLICHTING`, het juiste `procedure_id` per RFC-008 (default `beschikking`), en een `extensions.mbo_fcid.category: ALGEMEEN`-hint. De bezwaarweg zit niet in de regeling, want die wordt door RFC-008 afgeleid uit de AWB-procedure.

**Een chronicle-stream.** Een YAML-bestand `chronicles/cjib_wahv_betalingen.yaml` met minstens drie events: `payment_received`, `kwijtschelding_verleend`, `deurwaardertraject_gestart`. Per event de juiste FCID-mapping in `extensions.mbo_fcid`. `kwijtschelding_verleend` declareert `references_decision: <kwijtschelding-besluit-id>` zodat de integratie de bezwaarweg via dat besluit kan afleiden. `payment_received` en `deurwaardertraject_gestart` zijn feiten zonder bezwaar.

**Een werkende emit-pad.** Een RegelRecht-engine draait binnen de CJIB-cel (in eerste instantie ontwikkel-/pilot-omgeving) met de Wahv-lexogram en de chronicle-stream geladen. De cel-configuratie activeert `mbo_fcid`. Wanneer een Wahv-beschikking door de AWB-lifecycle (RFC-008) bij de BEKENDMAKING-stage aankomt, emit de cel een FCID-event naar het MBO-pilot-endpoint, getekend met de CJIB-FSC-key, inclusief `bezwaar_route` die door AWB-6:7/6:8-hooks op dat moment is berekend (inclusief feitelijke einddatum). Op een betaling die binnenkomt vanuit het surrounding incasso-systeem doet de cel hetzelfde voor `BetalingVerwerkt`. Aan citizen-zijde: een Wahv-vordering in MBO bevat een directe link naar het artikel, een referentie naar de executie-trace, een bezwaarknop met de juiste route en de werkelijke einddatum, en, na betaling, een gekoppeld BetalingVerwerkt-event onder hetzelfde zaakkenmerk.

## Wat de pilot CJIB oplevert

Eén bron voor norm, besluit en feit. Het lexogram zit in het corpus; het besluit komt uit de engine; het feit komt uit de chronicle-stream. Wijzigt de wet, dan beweegt het FCID-event mee zonder aparte release in een tweede systeem.

"Samen zien" voor de burger in de zin van Nieuwland §5.4: dezelfde tijdlijn van vastleggingen is gelijktijdig en gelijkwaardig toegankelijk voor burger en cel. Dat sluit aan op de vergewisplicht uit [Awb 3:9](https://wetten.overheid.nl/BWBR0005537) en op het MBO-principe dat data bij de bron blijft.

Rechtsbescherming als ontwerp, niet als marketing. De AWB-lifecycle uit RFC-008 levert de bezwaartermijn op het juiste moment (BEKENDMAKING, niet BESLUIT). De integratie pakt die termijn op en stuurt 'm mee als `bezwaar_route` in elk FCID-event. Een Wahv-sanctie met automatische ophoging die voor iemand met laag inkomen disproportioneel uitwerkt, krijgt op het moment van bekendmaking een zichtbare bezwaarknop in de MBO-surface met de werkelijke einddatum, niet pas na een aanmaning. Dit is de operationalisering van Nieuwland §7.2.1.

Een directe invulling van de Chronolexografie-architectuur, met behoud van organisatie-autonomie. CJIB is een cel met eigen kronieken en eigen sleutels. NVWA, NEa, DUO, CAK kunnen straks elk hun eigen cel zijn, met dezelfde mappingsregels. Geen centraal systeem.

Voorspelbare schaalbaarheid voor nieuwe opdrachtgevers. Sectorale toezichthouders die instromen in de Betalingsregeling Rijk krijgen `decision_type: BESTUURLIJKE_BOETE`, het juiste `procedure_id` per RFC-008, en een `extensions.mbo_fcid.category`. Geen schemawijziging per opdrachtgever, geen forks van regelingen alleen voor verschillen in MBO-aansluiting.

## Wat we van CJIB nodig hebben

Vijf dingen, geen open einde.

1. **Validatie van het uitvoeringslandschap.** Het [bijgevoegde overzicht](https://docs.regelrecht.rijks.app/concepts/cjib-uitvoeringslandschap) is opgebouwd uit publieke bronnen. Welke regelingen ontbreken of zijn fout toegewezen?
2. **Bevestiging of bijstelling van de pilotwet.** Wahv ligt voor de hand vanwege volume en helder juridisch kader. Liever iets anders? OM-strafbeschikking voor één feitcode is ook een optie. NVWA-bestuurlijke boetes zou de schaalbaarheidskant scherper testen omdat het sectoraal is.
3. **FCID-versie en endpoint-status.** Welke versie draait nu in jullie pilot of productie, en op welke endpoints?
4. **Knelpunten in de mapping.** Voor `zaakkenmerk` geldt CJIB's eigen zaaknummer-systematiek als leidend. Voor signing gaan we uit van de RFC-009 FSC-key. Botst dit met de CJIB-praktijk?
5. **Cel-topologie en bezwaar-routing.** Hoeveel cellen zou CJIB draaien (één centraal, één per opdrachtgever, één per regelinggebied)? En per type vordering: waar landt het bezwaar? Voor Wahv-sancties: bij CJIB zelf. Voor doorgereikte besluiten (CAK-eigen-bijdrage, OM-strafbeschikking): bij de opdrachtgever-cel. De `bezwaar_route` in elk FCID-event moet kloppen. Hier wil ik graag samen met Timen Olthof naar kijken.

## Volgende stap

Een werksessie van een dagdeel met CJIB, het VORIJK/MBO-team, Eelco en mij. Agenda: het uitvoeringslandschap valideren, de pilotwet vastpinnen, de cel-topologie schetsen, de bezwaar-routing per type vordering uitwerken, knelpunten benoemen. Daarna kan RFC-022 in de RegelRecht-repo van Proposed naar Accepted, kan de schema-bump landen, en kunnen we beginnen met de Wahv-lexogram en de eerste chronicle-stream.

Doel: binnen één maand na de werksessie een werkende emit-pad in een pilot-omgeving, met één Wahv-beschikking die als FCID-event in MBO-pilot belandt, een bezwaarknop bevat met de juiste route en termijn, en die teruggetraceerd kan worden naar het wetsartikel.

## Bijlagen

- [RFC-022: Chronolexogram types in the schema and the cell model](https://docs.regelrecht.rijks.app/rfcs/rfc-022)
- [Integratie-document: Mijn Betaaloverzicht (FCID)](https://docs.regelrecht.rijks.app/integrations/mbo-fcid)
- [CJIB-uitvoeringslandschap](https://docs.regelrecht.rijks.app/concepts/cjib-uitvoeringslandschap)
- [RFC-009: Multi-Organisation Execution](https://docs.regelrecht.rijks.app/rfcs/rfc-009)
- [Chronolexografie-position paper](https://chronolexografie.nl/position-paper/) van Olthof en Van Andel, december 2025
- [Nieuwland, een ontwerp voor een digitale rechtsstaat](https://achterkantvandeoverheid.nl/) van Denktank Achterkant van de Overheid, 15 december 2025
- [FCID-spec op vorijk.nl](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/)
