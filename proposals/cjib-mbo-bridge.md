# Voorstel: RegelRecht in de chronolexosfeer

*RegelRecht, CJIB en Mijn Betaaloverzicht als één keten van lexogrammen, decretogrammen en executogrammen*

*Auteur: Anne Schuth · Datum: 2026-05-26 · Status: voorstel ter bespreking*

## Aanleiding

Drie publicaties verschenen in 2025 die allemaal hetzelfde willen oplossen, maar nog niet aan elkaar gekoppeld zijn.

In juli 2025 ging Vorderingenoverzicht Rijk verder als Mijn Betaaloverzicht (MBO). De achterliggende standaard, het Financial Claims Information Document (FCID), staat op vorijk.nl en wordt door minimaal acht CRI-rijksorganisaties gebruikt of voorbereid. CJIB int de bijbehorende vorderingen, namens een groeiende kring opdrachtgevers.

In december 2025 publiceerde de Denktank Achterkant van de Overheid het ontwerpdocument [Nieuwland](https://achterkantvandeoverheid.nl/) en, parallel, het [Chronolexografie-position paper](https://chronolexografie.nl/). Daarin staat een coherent begrippenkader voor "adequaat digitaal vastleggen van de rechtstoestand": chronolexocellen, kronieken, en drie typen vastlegging (lexogram, decretogram, executogram). Eén van de redacteuren (Timen Olthof) werkt aan VORIJK/MBO; één van de geïnterviewden voor het document (Eelco Hotting, BZK) is degene met wie ik deze week sprak over CJIB. De conceptuele grond is dus al gedeeld.

Wat ontbreekt is de invulling waarmee CJIB en de andere bronorganisaties dit denkkader concreet kunnen maken. RegelRecht heeft sinds 2024 een corpus van 23 wetten in machine-leesbare vorm. RFC-009 (Multi-Organisation Execution) regelt al de federatie tussen bronorganisaties via FSC. Wat ontbreekt aan RegelRecht-kant is het incasso-domein: geen enkele wet in het corpus produceert vandaag een `BETALINGSVERPLICHTING` of een `STRAFBESCHIKKING`.

Dit voorstel beschrijft hoe deze drie werelden één keten worden, geformuleerd in de Chronolexografie-vocabulaire. Het is geschreven om CJIB en het VORIJK/MBO-team een eerste concrete stap te bieden, geen open vraag.

## Het denkkader: drie soorten vastlegging

Chronolexografie onderscheidt drie typen vastlegging die in de rechtsstaat alle drie nodig zijn. De drieslag is functioneel, niet hiërarchisch.

- **Lexogram** is de vastlegging van een (mogelijke) wijziging in wet- of regelgeving. Voorbeeld: de Wahv zoals die geldt sinds 1 januari 2025.
- **Decretogram** is de vastlegging van een concreet besluit bij toepassing van wetgeving. Voorbeeld: de Wahv-sanctie van €X die op datum Y aan kentekenhouder Z wordt opgelegd.
- **Executogram** is de vastlegging van feitelijke afhandeling. Voorbeeld: een betaling van €X die op datum Z bij CJIB binnenkomt onder zaakkenmerk Y.

In de huidige situatie wonen deze drie in gescheiden systemen, met telkens een verlies aan context op de overgangen. De citizen ziet wel het bedrag in MBO (executogram-laag), maar niet de beschikking (decretogram) of het artikel (lexogram). De gevolgen daarvan zijn breed beschreven in Nieuwland en in eerdere publicaties van Kafkabrigade.

RegelRecht en MBO kunnen samen alle drie de typen bedienen, mits we ze in het schema en in de uitwisseling expliciet onderscheiden. Dat is wat dit voorstel uitwerkt.

## De drie werelden in één tabel

De kern van het werk zit in een correcte mapping tussen RegelRecht's huidige schema, FCID's huidige event-types, de Chronolexografie-driedeling, en de feitelijke CJIB-praktijk. Hieronder de mapping zoals wij die nu zien.

### Wet- en besluit-laag (lexogrammen en decretogrammen)

| RegelRecht (schema)                                                | Chronolex-type            | FCID v4 (event_type)              | CJIB-werkelijkheid                                  |
|--------------------------------------------------------------------|---------------------------|-----------------------------------|-----------------------------------------------------|
| YAML in `corpus/regulation/`                                       | lexogram                  | n.v.t. (FCID is event-laag)       | Wahv, Sv 257a, Sr 36e/36f, sectorale wetten         |
| `BESCHIKKING` + `decision_type: STRAFBESCHIKKING`                  | decretogram               | `FinancieleVerplichtingOpgelegd`  | OM-strafbeschikking onder Sv 257a                   |
| `BESCHIKKING` + `decision_type: BETALINGSVERPLICHTING`             | decretogram               | `BetalingsverplichtingOpgelegd`   | Wahv-sanctie, inclusief verhoging                   |
| `BESCHIKKING` + `decision_type: BESTUURLIJKE_BOETE`                | decretogram               | `BetalingsverplichtingOpgelegd`   | NVWA, NEa, RDI, ATKM                                |
| `BESCHIKKING` + `decision_type: INCASSO_BESCHIKKING`               | decretogram               | `BetalingsverplichtingOpgelegd`   | expliciet incassobesluit na non-betaling            |
| `BESCHIKKING` + `decision_type: INTREKKING_BESCHIKKING`            | decretogram (intrekking)  | `BetalingsverplichtingIngetrokken`| herzieningsbeschikking, kwijtschelding              |

### Feitelijke-handelingen-laag (executogrammen)

Executogrammen zijn geen regelinguitvoer. Ze zijn geen besluit. Ze leggen vast wat er feitelijk gebeurt: een betaling komt binnen, een kwijtschelding wordt verleend, een deurwaardertraject start. In het voorstel krijgt deze laag een aparte plek in het corpus (`corpus/executogram/`) met een eigen schema, naast de regelinguitvoer in `corpus/regulation/`.

| Executogram-stream (corpus/executogram/)         | Chronolex-type | FCID v4 (event_type)         | CJIB-werkelijkheid                            |
|---------------------------------------------------|----------------|------------------------------|-----------------------------------------------|
| `betaling_ontvangen`                              | executogram    | `BetalingVerwerkt`           | giraal of via incasso ontvangen               |
| `kwijtschelding_verleend`                         | executogram    | `BetalingsverplichtingIngetrokken` | uitvoeringsbesluit met grond in beleidsregels |
| `deurwaardertraject_gestart`                      | executogram    | optioneel `BetalingsverplichtingOpgelegd` voor kosten | incassoroute na non-betaling |

### Velden

| RegelRecht                                       | FCID-veld                          | Bron in de praktijk                         |
|--------------------------------------------------|------------------------------------|---------------------------------------------|
| `article.url` (wetten.overheid.nl-permalink)     | `juridische_grondslag_bron`        | Direct uit het corpus (lexogram-laag)       |
| Eerste zin van `article.text` of `article.title` | `juridische_grondslag_omschrijving`| Direct uit het corpus                       |
| Hash(engine `organisation_id` + beschikking-id)  | `zaakkenmerk`                      | Vandaag: CJIB-zaaknummer                    |
| UUID v7 per event                                | `gebeurtenis_kenmerk`              | Nieuw, generatie aan engine-zijde           |
| Currency-output × 100                            | `bedrag` (centen, integer)         | Opgelegd of openstaand bedrag               |
| RFC-009 FSC-signature                            | event-signature                    | Bronorganisatie-key uit FSC Directory       |

### Vier categorieën

FCID kent vier categorieën: Algemeen, Administratiekosten, Verhoging, Rente. Ze zijn loodrecht op de chronolex-driedeling. Voorstel: een nieuw optioneel veld `fcid_category` op `produces` (voor decretogrammen) of in de executogram-stream-definitie (voor executogrammen).

### Wat er aan RegelRecht-kant nu nog niet is

Eelco's diagnose klopt: dit vraagt data en velden die nu nog ontbreken. Concreet:

1. Vijf nieuwe `decision_type`-waarden op `produces` (BETALINGSVERPLICHTING, STRAFBESCHIKKING, BESTUURLIJKE_BOETE, INCASSO_BESCHIKKING, INTREKKING_BESCHIKKING). Allemaal decretogram-types.
2. Twee optionele velden op `produces`: `fcid_emit` en `fcid_category`.
3. Een nieuwe top-level corpusmap `corpus/executogram/` met eigen schema voor executogram-stromen. Dit is conceptueel het belangrijkste: registratie van feiten hoort niet in regelinguitvoer.

Dit is een schema-bump van v0.5.2 naar v0.6.0. Het bestaande corpus hoeft niet aangepast te worden, want alle uitbreidingen zijn additief.

## Twee bewegingsrichtingen voor een chronolexocel

Een RegelRecht-engine bij een bronorganisatie ís een chronolexocel in de zin van het position paper. Hij produceert chronolexogrammen (alle drie de types), bewaart ze in zijn kronieken, en wisselt ze uit met andere cellen via FSC. De cel kan twee rollen vervullen.

### RegelRecht als producer voor MBO

Een CJIB-engine die de Wahv uitvoert produceert een decretogram (de beschikking). Met `fcid_emit: true` op het juiste artikel emitteert dezelfde engine direct een FCID-event naar het MBO-endpoint. De handtekening op het event is dezelfde FSC-key die het besluit tekent. Eén signing-mechanisme, twee output-formaten. CJIB hoeft geen aparte FCID-emitter te bouwen naast het uitvoeringssysteem.

```
                Wahv-artikel (lexogram in corpus YAML)
                          │
                          ▼
                  RegelRecht-engine = chronolexocel
                  ┌───────────────┐
                  │   uitvoeren   │  (chronolexoreductie)
                  └───────┬───────┘
                          │
              ┌───────────┴───────────┐
              ▼                       ▼
        decretogram              FCID-event
        (BESCHIKKING,            (BetalingsverplichtingOpgelegd,
         gesigneerd)              gesigneerd)
              │                       │
              ▼                       ▼
        burger / dossier          MBO-endpoint
```

Voor de burger betekent dit: de vordering in MBO bevat een directe link naar het wetsartikel (lexogram), naar de berekening waarmee de beschikking tot stand kwam (decretogram-trace), en later naar de betaling die er tegenover komt te staan (executogram). Alle drie de lagen zichtbaar onder hetzelfde zaakkenmerk en trace_id.

De executogram-laag werkt parallel. Een betaling die bij CJIB binnenkomt wordt vastgelegd via een entry in `corpus/executogram/cjib_wahv_betalingen.yaml`, die de engine gebruikt om een FCID `BetalingVerwerkt` te emitteren. Hetzelfde patroon voor kwijtscheldingen en andere executograms.

### RegelRecht als consumer van MBO

De andere richting is even bruikbaar. Een toekomstige Wsnp-engine, of de Betalingsregeling-Rijk-procedure bij CJIB zelf, heeft de openstaande vorderingen van een burger nodig als input. Het voorstel is een wrapper-regeling `procedureregeling_vorderingenoverzicht_rijk` in het corpus, met CJIB als `competent_authority` en `openstaande_vorderingen[]` als output. Een andere regeling die deze data nodig heeft verwijst via een gewone `source.regulation`. De engine ziet `competent_authority: CJIB` en gebruikt het RFC-009 ACCEPT-pad: één FSC-call naar de CJIB-cel, één gesigneerd antwoord terug.

```
       Wsnp-regeling (lexogram, toekomstig)
                  │
                  │ source.regulation:
                  │   procedureregeling_vorderingenoverzicht_rijk
                  ▼
       RegelRecht-engine ───────► CJIB chronolexocel
                  ▲                       │
                  │                       ▼
                  └──── lexostatus over openstaande vorderingen (gesigneerd)
```

In Chronolexografie-termen: de consumer-cel vraagt de CJIB-cel om een lexostatus over de openstaande vorderingen van burger X op moment Y. De CJIB-cel voert een reductie uit over zijn eigen kronieken en levert het antwoord. Geen bulk-data, geen kopie van een register, alleen wat voor deze vraag relevant is. Dit is precies de "respectvol onderscheid tussen registreren en interpreteren" uit Nieuwland §5.2: de CJIB-cel houdt de feiten; de consument interpreteert ze in zijn eigen wettelijke context.

## Wat dit CJIB oplevert

Eén bron voor de lexogram-, decretogram- en executogram-laag. De FCID-emitter zit in dezelfde engine die de Wahv uitvoert. Wijzigt de wet, dan beweegt het FCID-event mee zonder aparte release in een tweede systeem.

"Samen zien" voor de burger, in de zin van Nieuwland §5.4: dezelfde tijdlijn van lexogrammen, decretogrammen en executogrammen is gelijktijdig en gelijkwaardig toegankelijk voor de burger en voor CJIB. Dat sluit aan op de vergewisplicht uit [Awb 3:9](https://wetten.overheid.nl/BWBR0005537) én op het MBO-principe dat data bij de bron blijft.

Een directe invulling van de Chronolexografie-architectuur, met behoud van organisatie-autonomie. Elke bronorganisatie is een eigen cel met eigen kronieken. CJIB is een cel. NVWA is een cel. DUO is een cel. De cellen wisselen lexostatussen uit volgens FSC-contracten, niet bulk-data. Dit past op de denktank-uitwerking en op CJIB's huidige juridische rol.

Voorspelbare schaalbaarheid voor nieuwe opdrachtgevers. NVWA, NEa, RDI en de andere sectorale toezichthouders die instromen in de Betalingsregeling Rijk kunnen dezelfde mapping gebruiken: hun bestuurlijke-boete-besluiten krijgen `decision_type: BESTUURLIJKE_BOETE` en `fcid_emit: true`, en zijn klaar voor MBO.

Een eerste praktische stap richting wat Nieuwland §7.3.2 een Wet gegevensboekhouding noemt. Die wet moet nog tot stand komen. De architectuur die hier wordt voorgesteld is technisch al uitvoerbaar onder de huidige rechtsbasis, en zou onder een Wet gegevensboekhouding zonder wijziging de statutaire onderbouwing krijgen die er nu impliciet is in Awb 4.4 plus sectorale regelingen.

Een bijdrage aan rechtsbescherming op het juiste moment. Nieuwland §7.2.1 stelt het scherp: "wat uitwerkt als straf, verdient rechtsbescherming als straf". Voor een Wahv-sanctie die door automatische ophoging financieel disproportioneel uitpakt voor iemand met laag inkomen, is "samen zien" tijdens de oplegging, niet pas bij bezwaar, het verschil tussen tijdig handelen en te laat.

## Wat we van CJIB nodig hebben

Vijf dingen, geen open einde.

1. **Validatie van het uitvoeringslandschap.** Het bijgevoegde overzicht [CJIB-uitvoeringslandschap](https://docs.regelrecht.rijks.app/concepts/cjib-uitvoeringslandschap) is opgebouwd uit publieke bronnen. Welke regelingen ontbreken of zijn fout toegewezen?
2. **Keuze van een pilot.** De Wahv ligt voor de hand: groot volume, helder juridisch kader, één opdrachtgever. Liever iets anders? OM-strafbeschikking voor één feitcode is ook een optie.
3. **FCID-versie en endpoint-status.** FCID v4.2.0 is volgens vorijk.nl de huidige standaard. Welke versie draait nu in jullie pilot of productie, en op welke endpoints?
4. **Knelpunten in de mapping.** Voor `zaakkenmerk` stel ik een deterministische hash voor, maar CJIB's zaaknummer-systematiek is leidend. Voor de signature ga ik uit van de RFC-009 FSC-key. Botsen deze keuzes met de CJIB-praktijk?
5. **Cel-topologie.** Draait CJIB straks één chronolexocel, één per opdrachtgever, of één per regelinggebied? RFC-009 ondersteunt elke optie, maar de keuze heeft gevolgen voor kroniek-ordening en sleutelbeheer. Hier zou ik graag samen met Timen Olthof, vanuit zijn VORIJK-positie, naar kijken.

## Volgende stap

Een werksessie van een dagdeel met CJIB, het VORIJK/MBO-team, Eelco en mij. Agenda: bovenstaande mapping-tabellen rij voor rij doorlopen, de pilot kiezen, de cel-topologie schetsen, knelpunten benoemen. Daarna kan RFC-019 in de RegelRecht-repo van Proposed naar Accepted, en kunnen we de eerste wet (Wahv of alternatief) als machine-leesbare regeling in het corpus opnemen, samen met de eerste executogram-stream. Pilot levert dan een werkende producer-pad én een fixture die laat zien hoe een burger zijn vordering kan natrekken tot op het wetsartikel.

## Bijlagen

- [CJIB-uitvoeringslandschap](https://docs.regelrecht.rijks.app/concepts/cjib-uitvoeringslandschap): tabel van alle CJIB-regelingen met grondslag, chronolex-rol, RegelRecht-mapping en FCID-event
- [RFC-019: RegelRecht in de chronolexosfeer](https://docs.regelrecht.rijks.app/rfcs/rfc-019): technische uitwerking van dit voorstel
- [RFC-009: Multi-Organisation Execution](https://docs.regelrecht.rijks.app/rfcs/rfc-009): federatie-architectuur waar dit voorstel op leunt
- [Chronolexografie-position paper](https://chronolexografie.nl/position-paper/) van Timen Olthof en Marc van Andel, december 2025
- [Nieuwland, een ontwerp voor een digitale rechtsstaat](https://achterkantvandeoverheid.nl/) van Denktank Achterkant van de Overheid, 15 december 2025
- [FCID-spec op vorijk.nl](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/)
