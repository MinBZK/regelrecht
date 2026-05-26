# Voorstel: RegelRecht × CJIB × Mijn Betaaloverzicht

*Auteur: Anne Schuth · Datum: 2026-05-26 · Status: voorstel ter bespreking*

## Aanleiding

Het CJIB int financiële verplichtingen voor een groeiend deel van de Rijksoverheid. Sinds 10 juli 2025 toont Mijn Betaaloverzicht (MBO) deze verplichtingen aan de burger in één overzicht. De standaard achter MBO, het Financial Claims Information Document (FCID), is op vorijk.nl gepubliceerd en wordt door minimaal acht CRI-rijksorganisaties gebruikt of voorbereid. Wat ontbreekt is de stap van het besluit zelf naar het FCID-event: een burger ziet wel het bedrag, maar niet de artikel-link, de berekening of de redenering die er onder zit.

Dat is precies het probleem dat [RegelRecht](https://regelrecht.rijks.app) oplost voor de fiscale en sociale wetten in zijn corpus. Een RegelRecht-engine voert het wetsartikel uit en levert een beschikking met volledige executie-trace, juridische bron en handtekening. [RFC-009 (Multi-Organisation Execution)](https://docs.regelrecht.rijks.app/rfcs/rfc-009) regelt al hoe meerdere bronorganisaties via FSC samen één keten vormen. Wat ontbreekt aan RegelRecht-kant is het incasso-domein: geen enkele wet in het corpus produceert vandaag een `BETALINGSVERPLICHTING` of een `STRAFBESCHIKKING`.

Dit voorstel beschrijft hoe RegelRecht en MBO één keten kunnen vormen. Het is geschreven om CJIB en het VORIJK-team een concrete eerste stap te bieden, geen open vraag.

## De mapping-puzzel

De kern van het werk zit in een goede mapping tussen drie werelden: de RegelRecht-schematermen, de FCID-velden, en de feitelijke CJIB-praktijk. Hieronder de mapping zoals wij die nu zien. De CJIB-kolom is de plek waar wij domeinkennis nodig hebben.

### Outputs en events

| RegelRecht (schema)                                                | FCID v4 (event_type)              | CJIB-werkelijkheid                                  |
|--------------------------------------------------------------------|-----------------------------------|-----------------------------------------------------|
| `BESCHIKKING` + `decision_type: STRAFBESCHIKKING`                  | `FinancieleVerplichtingOpgelegd`  | OM-strafbeschikking onder Sv 257a                   |
| `BESCHIKKING` + `decision_type: BETALINGSVERPLICHTING`             | `BetalingsverplichtingOpgelegd`   | Wahv-sanctie, inclusief verhoging                   |
| `BESCHIKKING` + `decision_type: BESTUURLIJKE_BOETE`                | `BetalingsverplichtingOpgelegd`   | NVWA-, NEa-, RDI-boete                              |
| Override via [RFC-007](https://docs.regelrecht.rijks.app/rfcs/rfc-007) | `BetalingsverplichtingIngetrokken` | Herzieningsbeschikking, kwijtschelding              |
| Externe input "betaling ontvangen"                                 | `BetalingVerwerkt`                | Giraal binnen, incassosysteem, BR Rijk              |

### Velden

| RegelRecht                                       | FCID-veld                          | Bron in de praktijk                         |
|--------------------------------------------------|------------------------------------|---------------------------------------------|
| `article.url` (wetten.overheid.nl-permalink)     | `juridische_grondslag_bron`        | Direct uit het corpus                       |
| Eerste zin van `article.text` of `article.title` | `juridische_grondslag_omschrijving`| Direct uit het corpus                       |
| Hash(engine `organisation_id` + beschikking-id)  | `zaakkenmerk`                      | Vandaag: CJIB-zaaknummer                    |
| UUID v7 per event                                | `gebeurtenis_kenmerk`              | Nieuw, generatie aan engine-zijde           |
| Currency-output × 100                            | `bedrag` (centen, integer)         | Opgelegd of openstaand bedrag               |
| RFC-009 FSC-signature                            | event-signature                    | Bronorganisatie-key uit FSC Directory       |

### Vier categorieën

FCID kent vier categorieën: Algemeen, Administratiekosten, Verhoging, Rente. Voorstel: een nieuw optioneel veld `produces.fcid_category` op artikel-niveau, met diezelfde enum. Een Wahv-sanctie-artikel krijgt dan `fcid_category: ALGEMEEN`, het administratiekosten-artikel `ADMINISTRATIEKOSTEN`. De engine emitteert per artikel één FCID-regel met de juiste categorie. Geen impliciete logica, geen verborgen mapping.

### Wat er aan RegelRecht-kant nu nog niet is

Eelco's diagnose klopt: dit vraagt data en velden die nu nog ontbreken. Concreet zes nieuwe `decision_type`-waarden (BETALINGSVERPLICHTING, STRAFBESCHIKKING, BESTUURLIJKE_BOETE, INCASSO_OPGELEGD, INCASSO_INGETROKKEN, BETALING_VERWERKT) en twee optionele velden (`fcid_emit`, `fcid_category`) op `produces`. Dat is een schema-bump naar v0.6.0. Het bestaande corpus hoeft niet aangepast te worden, want alle uitbreidingen zijn additief.

## Twee bewegingsrichtingen

### RegelRecht als producer voor MBO

Een CJIB-engine die de Wahv uitvoert produceert een beschikking. Met `fcid_emit: true` op het juiste artikel emitteert dezelfde engine direct een FCID-event naar het MBO-endpoint. De handtekening op het event is dezelfde FSC-key die het bronbesluit tekent. Eén signing-mechanisme, twee output-formaten. CJIB hoeft geen aparte FCID-emitter te bouwen naast het uitvoeringssysteem.

```
                Wahv-artikel (corpus YAML)
                          │
                          ▼
                  RegelRecht-engine
                  ┌───────────────┐
                  │   uitvoeren   │
                  └───────┬───────┘
                          │
              ┌───────────┴───────────┐
              ▼                       ▼
        BESCHIKKING              FCID-event
        (gesigneerd)             (gesigneerd)
              │                       │
              ▼                       ▼
        burger / dossier          MBO-endpoint
```

Voor de burger betekent dit: de vordering in MBO bevat een directe link naar het wetsartikel. Wie wil weten waarom een bedrag is opgelegd, kan de hele executie-trace opvragen bij CJIB (RFC-009 §8, conform [Logboek Dataverwerkingen](https://logius-standaarden.github.io/logboek-dataverwerkingen/)).

### RegelRecht als consumer van MBO

De andere richting is even bruikbaar. Een toekomstige Wsnp-engine, of de Betalingsregeling-Rijk-procedure bij CJIB zelf, heeft de openstaande vorderingen van een burger nodig als input. Het voorstel is een dunne wrapper-regeling `procedureregeling_vorderingenoverzicht_rijk` in het corpus, met CJIB als `competent_authority` en `openstaande_vorderingen[]` als output. Een andere regeling die deze data nodig heeft verwijst via een gewone `source.regulation`. De engine ziet `competent_authority: CJIB` en gebruikt de RFC-009 ACCEPT-pad: één FSC-call naar de CJIB MBO-endpoint, één gesigneerd antwoord terug.

```
       Wsnp-regeling (toekomstig)
                  │
                  │ source.regulation:
                  │   procedureregeling_vorderingenoverzicht_rijk
                  ▼
       RegelRecht-engine ───────► CJIB MBO-endpoint
                  ▲                       │
                  │                       ▼
                  └─────  openstaande_vorderingen[]  (gesigneerd)
```

Geen schema-uitbreiding nodig. De wrapper-regeling is één YAML-bestand, plus een entry in de FSC service-registry.

## Wat dit CJIB oplevert

Eén bron voor "wat is de wet" en "wat staat er open". De FCID-emitter zit in dezelfde engine die de Wahv uitvoert. Wijzigt de wet, dan beweegt het FCID-event mee zonder aparte release in een tweede systeem.

Transparantie zonder extra werk. Burger ziet niet alleen het bedrag maar ook de juridische grondslag, op verzoek inclusief executie-trace. Dat sluit aan op de [vergewisplicht uit Awb 3:9](https://wetten.overheid.nl/BWBR0005537) en op het MBO-principe dat data bij de bron blijft.

Voorspelbare schaalbaarheid voor nieuwe opdrachtgevers. NVWA, NEa, RDI en de andere sectorale toezichthouders die instromen in de Betalingsregeling Rijk kunnen dezelfde mapping gebruiken: hun bestuurlijke-boete-besluiten krijgen `decision_type: BESTUURLIJKE_BOETE` en `fcid_emit: true`, en zijn klaar voor MBO.

Een eerste stap richting machine-leesbare Betalingsregeling Rijk. De regeling zelf consumeert dan de eigen FCID-stream van CJIB en past beleidsregels toe, in plaats van handmatige interpretatie.

## Wat we van CJIB nodig hebben

Vier dingen, geen open einde.

1. **Validatie van het uitvoeringslandschap.** Het bijgevoegde overzicht [CJIB-uitvoeringslandschap](https://docs.regelrecht.rijks.app/concepts/cjib-uitvoeringslandschap) is opgebouwd uit publieke bronnen. Welke regelingen ontbreken of zijn fout toegewezen? Welke opdrachtgevers staan er die er feitelijk niet meer zijn?
2. **Keuze van een pilot.** De Wahv ligt voor de hand: groot volume, helder juridisch kader, één opdrachtgever. Liever iets anders? OM-strafbeschikking voor één feitcode is ook een optie. Wat past in jullie roadmap?
3. **FCID-versie en endpoint-status.** FCID v4.2.0 is volgens vorijk.nl de huidige standaard. Welke versie draait nu in jullie pilot of productie, en op welke endpoints?
4. **Knelpunten in de mapping.** Voor `zaakkenmerk` stel ik een deterministische hash voor, maar jullie zaaknummer-systematiek is leidend. Voor de signature ga ik uit van de RFC-009 FSC-key. Zijn er punten waarop deze keuzes botsen met CJIB-praktijk?

## Volgende stap

Een werksessie van een dagdeel met CJIB, het VORIJK-team, Eelco en mij. Agenda: bovenstaande tabel rij voor rij doorlopen, de pilot kiezen, knelpunten benoemen. Daarna kan RFC-022 in de RegelRecht-repo van Proposed naar Accepted, en kunnen we de eerste wet (Wahv of alternatief) als machine-leesbare regeling in het corpus opnemen. Pilot levert dan een werkende producer-pad én een fixture die laat zien hoe een burger zijn vordering kan natrekken tot op het wetsartikel.

## Bijlagen

- [CJIB-uitvoeringslandschap](https://docs.regelrecht.rijks.app/concepts/cjib-uitvoeringslandschap): tabel van alle CJIB-regelingen met grondslag en mapping naar RegelRecht-schema en FCID
- [RFC-022: RegelRecht × MBO](https://docs.regelrecht.rijks.app/rfcs/rfc-022): technische uitwerking van dit voorstel
- [RFC-009: Multi-Organisation Execution](https://docs.regelrecht.rijks.app/rfcs/rfc-009): federatie-architectuur waar dit voorstel op leunt
- [FCID-spec op vorijk.nl](https://vorijk.nl/docs/financiele-verplichtingen/document_types/financial_claims_information_document/)
