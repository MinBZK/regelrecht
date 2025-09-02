---
date: 2023-12-22
categories:
  - Techniek
authors:
  - regelrecht
template: blog-post.html
---

# NRML vs DMN: Keuze voor Nederlandse Context

Waarom we kozen voor een Nederlandse regel-taal in plaats van internationale standaarden en wat dit betekent voor de toekomst.

<!-- more -->

Bij het ontwikkelen van machine-uitvoerbare wetgeving stonden we voor een belangrijke keuze: gebruiken we bestaande internationale standaarden zoals DMN (Decision Model and Notation), of ontwikkelen we iets nieuws? Na uitgebreid onderzoek kozen we voor NRML (Nederlandse Rule Markup Language). Hier leggen we uit waarom.

## Decision Model and Notation (DMN)

DMN is een internationale standaard van de Object Management Group (OMG) voor het modelleren van bedrijfsregels. Op papier lijkt het een logische keuze:

### Voordelen van DMN
- **Gestandaardiseerd**: Internationale standaard met brede adoptie
- **Tooling**: Bestaande tools en implementaties beschikbaar  
- **Expertise**: Ontwikkelaars en analisten kennen de standaard al
- **Interoperabiliteit**: Eenvoudige uitwisseling met andere systemen

### Beperkingen van DMN
- **XML-based**: Complexe syntax die moeilijk leesbaar is
- **Business-focused**: Ontworpen voor bedrijfsregels, niet juridische specificaties
- **Beperkte expressiviteit**: Sommige juridische constructies zijn moeilijk uit te drukken
- **Geen juridische semantiek**: Mist concepten die specifiek voor wetgeving nodig zijn

## Nederlandse Rule Markup Language (NRML)

Na experimenteren met DMN besloten we tot de ontwikkeling van NRML, specifiek ontworpen voor Nederlandse wetgeving.

### Waarom NRML?

#### JSON-first Benadering
NRML gebruikt JSON als basis syntax in plaats van XML. Dit maakt het:
- **Leesbaar**: Juristen kunnen de structuur begrijpen zonder technische achtergrond
- **Developervriendelijk**: Moderne ontwikkelaars werken dagelijks met JSON
- **Lightweight**: Minder overhead dan XML-based oplossingen

#### Juridische Semantiek
NRML bevat concepten die specifiek voor wetgeving zijn ontworpen:
- **Wetsartikelen en paragrafen**: Directe mapping naar juridische structuren
- **Temporele logica**: Ondersteuning voor datum-afhankelijke regels
- **Uitzonderingsbehandeling**: Expliciete support voor juridische uitzonderingen
- **Verwijzingen**: Kruisverwijzingen tussen verschillende wetten en artikelen

#### Nederlandse Context
NRML is ontworpen met de Nederlandse rechtssystematiek in gedachten:
- **Nederlandse terminologie**: Concepten die aansluiten bij Nederlandse juridische traditie
- **Overheidsspecifieke constructies**: Ondersteuning voor typisch Nederlandse regelgeving
- **Meertaligheid**: Ondersteuning voor het werken in meerdere talen (Nederlands/Engels)

## Praktijkvoorbeeld

Vergelijk deze eenvoudige regel in beide talen:

### DMN (XML)
```xml
<decision id="aow_leeftijd" name="AOW Leeftijd">
  <decisionTable>
    <input id="geboortejaar" label="Geboortejaar">
      <inputExpression typeRef="number">
        <text>geboortejaar</text>
      </inputExpression>
    </input>
    <output id="aow_leeftijd" label="AOW Leeftijd" typeRef="number"/>
    <rule id="regel_1">
      <inputEntry>
        <text>&lt;= 1954</text>
      </inputEntry>
      <outputEntry>
        <text>65</text>
      </outputEntry>
    </rule>
  </decisionTable>
</decision>
```

### NRML (JSON)
```json
{
  "regel": "aow_leeftijd",
  "artikel": "AOW artikel 7a",
  "condities": {
    "geboortejaar": {"<=": 1954}
  },
  "resultaat": {
    "aow_leeftijd": 65
  },
  "test_scenarios": [
    {
      "input": {"geboortejaar": 1950},
      "verwacht": {"aow_leeftijd": 65}
    }
  ]
}
```

## Toekomstbestendigheid

Door te kiezen voor NRML behouden we controle over de evolutie van de standaard. Dit stelt ons in staat om:

- **Snel te itereren**: Aanpassingen kunnen direct worden doorgevoerd
- **Nederlandse behoeften te prioriteren**: Features die specifiek nuttig zijn voor Nederlandse wetgeving
- **Experimenteren**: Nieuwe concepten uitproberen zonder internationale standaardisatieprocessen

## Interoperabiliteit

NRML is niet bedoeld als eiland. We ontwikkelen:
- **Export naar DMN**: Voor organisaties die DMN-tooling willen gebruiken
- **OpenAPI specificaties**: Voor eenvoudige integratie met bestaande systemen
- **Multiple execution engines**: NRML-interpreters in verschillende programmeertalen

## Conclusie

De keuze voor NRML was niet eenvoudig, maar wel noodzakelijk. Internationale standaarden zoals DMN zijn waardevol, maar missen de specificiteit die nodig is voor machine-uitvoerbare wetgeving in de Nederlandse context.

NRML biedt ons de flexibiliteit om snel te experimenteren en te leren, terwijl we tegelijkertijd interoperabiliteit waarborgen met bestaande systemen. Het is een investering in een toekomst waarin Nederlandse wetgeving volledig machine-uitvoerbaar is, zonder concessies te doen aan juridische precisie of technische elegantie.