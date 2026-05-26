# Voorstel: een eerste pilot met CJIB op de RegelRecht/MBO-keten

*Auteur: Anne Schuth · Datum: 2026-05-26 · Status: voorstel ter bespreking*

## Aanleiding

Drie publicaties verschenen in 2025 die hetzelfde willen oplossen.

In juli 2025 ging Vorderingenoverzicht Rijk verder als Mijn Betaaloverzicht (MBO). De achterliggende standaard, het Financial Claims Information Document (FCID, v4.2.0 mei 2026), staat op vorijk.nl en wordt door minimaal acht CRI-rijksorganisaties gebruikt of voorbereid. CJIB int de bijbehorende vorderingen, namens een groeiende kring opdrachtgevers.

In december 2025 publiceerde de Denktank Achterkant van de Overheid het ontwerp [Nieuwland](https://achterkantvandeoverheid.nl/) en het [Chronolexografie-position paper](https://chronolexografie.nl/). Daarin staat een coherent begrippenkader voor "adequaat digitaal vastleggen van de rechtstoestand": chronolexocellen, kronieken, en drie typen vastlegging (lexogram, decretogram, executogram). Eén van de redacteuren (Timen Olthof) werkt aan VORIJK/MBO; één van de geïnterviewden (Eelco Hotting, BZK) is degene met wie ik deze week sprak.

RegelRecht heeft sinds 2024 een corpus van 23 wetten in machine-leesbare vorm, plus federatie tussen bronorganisaties via FSC (RFC-009). Wat ontbrak aan RegelRecht-kant was het incasso-domein en een expliciete plek voor executogrammen. Dat wordt deze week opgelost met twee documenten: een generieke architectuur-RFC en een concrete integratie-spec.

Dit voorstel gaat over de derde stap: een pilot waarmee CJIB als eerste bronorganisatie de gecombineerde RegelRecht/MBO-keten in productie kan brengen.

## Wat er nu klaar ligt

In de RegelRecht-codebase zijn drie nieuwe documenten beschikbaar:

**[RFC-019: Chronolexogram types in the schema and corpus](https://docs.regelrecht.rijks.app/rfcs/rfc-019).** Generiek. Voegt drie eerste-klas concepten toe aan RegelRecht: lexogram (regeling), decretogram (engine-output met BESCHIKKING), executogram (nieuw, in een aparte corpus-map). Breidt `decision_type` uit met vijf nieuwe waarden voor financiële verplichtingen. Geen verwijzing naar FCID, MBO of CJIB; de RFC blijft generiek zodat hij ook over twee jaar nog klopt.

**[Integratie-document: Mijn Betaaloverzicht (FCID)](https://docs.regelrecht.rijks.app/integrations/mbo-fcid).** Concreet. Beschrijft hoe een RegelRecht-engine FCID-events emitteert, hoe velden worden afgeleid, hoe de consumer-wrapper voor het ophalen van openstaande vorderingen werkt, en hoe trust en signing zijn ingericht. Target FCID v4.x, beweegt mee met de standaard.

**[CJIB-uitvoeringslandschap](https://docs.regelrecht.rijks.app/concepts/cjib-uitvoeringslandschap).** Inventarisatie. Welke regelingen CJIB uitvoert (zelfstandig en namens andere bronorganisaties), met grondslagen, BWB-IDs en de chronolex-rol per opdrachtgever. Opgebouwd uit publieke bronnen; nog te valideren met domeinkennis.

Samen zijn dit de bouwstenen. De architectuur is RegelRecht-eigen en blijft bestaan ook als FCID verandert of als MBO ophoudt te bestaan; de integratie-laag is herzienbaar zonder dat het RFC-proces opnieuw hoeft.

## Het denkkader: drie soorten vastlegging

Chronolexografie onderscheidt drie typen vastlegging die in de rechtsstaat alle drie nodig zijn.

- **Lexogram**: vastlegging van een (mogelijke) wijziging in wet- of regelgeving. Voorbeeld: de Wahv zoals die geldt sinds 1 januari 2025.
- **Decretogram**: vastlegging van een concreet besluit. Voorbeeld: een Wahv-sanctie van €X die op datum Y aan kentekenhouder Z wordt opgelegd.
- **Executogram**: vastlegging van feitelijke afhandeling. Voorbeeld: een betaling van €X die op datum Z bij CJIB binnenkomt onder zaakkenmerk Y.

In de huidige situatie wonen deze drie in gescheiden systemen, met telkens een verlies aan context op de overgangen. De citizen ziet wel het bedrag in MBO, maar niet de beschikking of het artikel. De gevolgen daarvan zijn beschreven in Nieuwland en in eerdere publicaties van Kafkabrigade.

De pilot die hieronder volgt sluit deze keten voor één wet (Wahv) bij één organisatie (CJIB).

## Wat de pilot inhoudt

Voor één pilotwet (voorkeur: Wahv) leveren we drie samenhangende artefacten op.

**Een lexogram.** Een YAML-bestand `corpus/regulation/nl/wet/wet_administratiefrechtelijke_handhaving_verkeersvoorschriften/<valid_from>.yaml`. Dit is de Wahv in machine-leesbare vorm conform het RegelRecht-schema. Eén artikel produceert een `BESCHIKKING` met `decision_type: BETALINGSVERPLICHTING` en `outbound_emit: true`.

**Een executogram-stream.** Een YAML-bestand `corpus/executogram/cjib_wahv_betalingen.yaml` met minstens drie events: `betaling_ontvangen`, `kwijtschelding_verleend`, `intrekking_verwerkt`. Elk event mapt naar het juiste FCID `event_type`.

**Een werkende emit-pad.** Een RegelRecht-engine draait bij CJIB (in eerste instantie ontwikkel-/pilot-omgeving) met de Wahv-lexogram en de executogram-stream geladen. Op een Wahv-beschikking emit de engine een FCID-event naar het MBO-pilot-endpoint, getekend met de CJIB-FSC-key. Op een betaling die binnenkomt vanuit het surrounding incasso-systeem doet hij hetzelfde voor `BetalingVerwerkt`.

Aan citizen-zijde is het resultaat: een Wahv-vordering in MBO bevat een directe link naar het Wahv-artikel, een referentie naar de executie-trace die het bedrag bepaalde, en (na betaling) een gekoppeld BetalingVerwerkt-event onder hetzelfde zaakkenmerk.

## Wat de pilot CJIB oplevert

Eén bron voor de lexogram-, decretogram- en executogram-laag. De FCID-emitter zit in dezelfde engine die de Wahv uitvoert. Wijzigt de wet, dan beweegt het FCID-event mee zonder aparte release.

"Samen zien" voor de burger in de zin van Nieuwland §5.4: dezelfde tijdlijn van vastleggingen is gelijktijdig en gelijkwaardig toegankelijk voor burger en bronorganisatie. Dat sluit aan op de vergewisplicht uit [Awb 3:9](https://wetten.overheid.nl/BWBR0005537) en op het MBO-principe dat data bij de bron blijft.

Een directe invulling van de Chronolexografie-architectuur, met behoud van organisatie-autonomie. CJIB is een chronolexocel met eigen kronieken en eigen sleutels. NVWA, NEa, DUO, CAK kunnen straks elk hun eigen cel zijn, met dezelfde mappingsregels. Geen centraal systeem.

Voorspelbare schaalbaarheid voor nieuwe opdrachtgevers. Sectorale toezichthouders die instromen in de Betalingsregeling Rijk krijgen `decision_type: BESTUURLIJKE_BOETE` en `outbound_emit: true`, en zijn klaar voor MBO. Geen schemawijziging per opdrachtgever.

Een eerste praktische stap richting wat Nieuwland §7.3.2 een Wet gegevensboekhouding noemt. Die wet moet nog tot stand komen. De architectuur die hier wordt voorgesteld is technisch al uitvoerbaar onder de huidige rechtsbasis, en zou onder een Wet gegevensboekhouding zonder wijziging de statutaire onderbouwing krijgen die er nu impliciet is in Awb 4.4 plus sectorale regelingen.

## Wat we van CJIB nodig hebben

Vijf dingen, geen open einde.

1. **Validatie van het uitvoeringslandschap.** Het [bijgevoegde overzicht](https://docs.regelrecht.rijks.app/concepts/cjib-uitvoeringslandschap) is opgebouwd uit publieke bronnen. Welke regelingen ontbreken of zijn fout toegewezen?
2. **Bevestiging of bijstelling van de pilotwet.** Wahv ligt voor de hand vanwege volume en helder juridisch kader. Liever iets anders? OM-strafbeschikking voor één feitcode is ook een optie. NVWA-bestuurlijke boetes zou de schaalbaarheidskant scherper testen omdat het sectoraal is.
3. **FCID-versie en endpoint-status.** Welke versie draait nu in jullie pilot of productie, en op welke endpoints? Het [integratie-document](https://docs.regelrecht.rijks.app/integrations/mbo-fcid) target FCID v4.x, maar als jullie op v3 zitten wijken we daarvan af.
4. **Knelpunten in de mapping.** Voor `zaakkenmerk` stel ik een deterministische hash voor, maar CJIB's zaaknummer-systematiek is leidend. Voor de signature ga ik uit van de RFC-009 FSC-key. Botsen deze keuzes met de CJIB-praktijk?
5. **Cel-topologie.** Draait CJIB straks één chronolexocel, één per opdrachtgever, of één per regelinggebied? RFC-009 en RFC-019 ondersteunen elke optie, maar de keuze heeft gevolgen voor kroniek-ordening en sleutelbeheer. Hier zou ik graag samen met Timen Olthof, vanuit zijn VORIJK-positie, naar kijken.

## Volgende stap

Een werksessie van een dagdeel met CJIB, het VORIJK/MBO-team, Eelco en mij. Agenda: het uitvoeringslandschap valideren, de pilotwet vastpinnen, de cel-topologie schetsen, knelpunten benoemen. Daarna kan RFC-019 in de RegelRecht-repo van Proposed naar Accepted, kan de schema-bump landen, en kunnen we beginnen met de Wahv-lexogram en de eerste executogram-stream.

Doel: binnen één maand na de werksessie een werkende emit-pad in een pilot-omgeving, met één Wahv-beschikking die als FCID-event in MBO-pilot belandt en daarvandaan teruggetraceerd kan worden naar het wetsartikel.

## Bijlagen

- [RFC-019: Chronolexogram types in the schema and corpus](https://docs.regelrecht.rijks.app/rfcs/rfc-019): generieke architectuur
- [Integratie-document: Mijn Betaaloverzicht (FCID)](https://docs.regelrecht.rijks.app/integrations/mbo-fcid): concrete uitwerking voor FCID v4.x
- [CJIB-uitvoeringslandschap](https://docs.regelrecht.rijks.app/concepts/cjib-uitvoeringslandschap): inventarisatie
- [RFC-009: Multi-Organisation Execution](https://docs.regelrecht.rijks.app/rfcs/rfc-009): federatie-architectuur waar dit op leunt
- [Chronolexografie-position paper](https://chronolexografie.nl/position-paper/) van Olthof en Van Andel, december 2025
- [Nieuwland, een ontwerp voor een digitale rechtsstaat](https://achterkantvandeoverheid.nl/) van Denktank Achterkant van de Overheid, 15 december 2025
- [FCID-spec op vorijk.nl](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/)
