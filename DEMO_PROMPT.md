Implementeer een nieuwe Nederlandse wet in de regelrecht engine.

Volg deze stappen:

1. **Zoek de wet op** via wetten.overheid.nl. Vind de volledige wettekst, het BWB-ID, de publicatiedatum en de URL. Bepaal welk(e) artikel(en) zich het beste lenen voor machine-leesbare implementatie: artikelen met concrete drempels, voorwaarden, categorieën of berekeningen. Vermijd artikelen die puur procedureel of definitieartikelen zijn.

2. **Zoek de Memorie van Toelichting (MvT) op** via zoek.officielebekendmakingen.nl of via de Kamerstukken. Vind de toelichting bij de gekozen artikelen. Zoek specifiek naar:
   - Rekenvoorbeelden die de wetgever geeft
   - Concrete scenario's en casussen
   - Randgevallen die expliciet worden besproken
   - Bedoelde uitkomsten bij specifieke situaties

3. **Bepaal de regelgevingslaag** op basis van het type (WET, KONINKLIJK_BESLUIT, MINISTERIELE_REGELING, etc.) en zorg dat de bijbehorende directory wordt gescand in `engine/rule_resolver.py` (regel 46). Als die er al in staat, sla deze stap over.

4. **Maak het YAML-wetbestand** aan op `regulation/nl/<laag>/<wet_slug>/<datum>.yaml`. Het bestand moet voldoen aan het JSON-schema (valideer met `uv run python script/validate.py <yaml_file>`). Inclusief:
   - De letterlijke Nederlandse wettekst in het `text`-veld
   - `machine_readable` met `definitions` (benoemde constanten uit de wet), `execution.parameters`, `execution.output` en `execution.actions`
   - Kies de operaties die het beste passen bij de logica van de wet (vergelijkingen, rekenkundige operaties, logische operaties, etc.)
   - Elk getal, elke drempel en elke voorwaarde moet herleidbaar zijn tot een specifiek onderdeel van het artikel

5. **Maak een Gherkin feature-bestand** aan op `features/<wet_slug>.feature` met:
   - Scenario's die **direct gebaseerd zijn op de voorbeelden en casussen uit de MvT**
   - Voeg bij elk scenario een commentaar toe met de bron (bijv. `# MvT Kamerstukken II, 2023-2024, 36XXX, nr. 3, p. 12`)
   - Vul aan met scenario's voor randgevallen en onderdelen die de MvT niet expliciet als voorbeeld noemt, zodat ALLE onderdelen van het artikel gedekt zijn
   - Gebruik de bestaande `Given a query with the following data:` stap voor invoerparameters
   - Gebruik de bestaande `Given the calculation date is "<datum>"` stap

6. **Voeg stapdefinities toe** in `features/steps/steps.py` volgens het erfgrensbeplanting/rijbewijs patroon:
   - Een `@when`-stap die `service.evaluate_law_output(law_id=..., output_name=..., ...)` aanroept
   - `@then`-stappen voor elke uitvoerwaarde die je wilt controleren

7. **Itereer tot alle tests groen zijn**: draai `just test-all` en los eventuele problemen op in het YAML- of feature-bestand.

8. **Verifieer correctheid**: controleer elk scenario tegen zowel de wettekst als de MvT. Zorg dat elk onderdeel van het artikel gedekt is en dat de MvT-voorbeelden exact kloppen.

Doe dit op een nieuwe branch `feature/<wet_slug>`. Commit en push als alle tests slagen.

De wet is: **[NAAM WET]**





















## Suggesties voor wetten

### Herkenbaar voor breed publiek

| Wet | Interessante artikelen | Waarom goed |
|-----|----------------------|-------------|
| Drank- en Horecawet | Art. 20 (leeftijdsgrenzen) | Iedereen kent de 18+ regel, simpel en helder |
| Leerplichtwet 1969 | Art. 3-4a (leerplicht + kwalificatieplicht) | Leeftijdsbereiken, uitzonderingen, breed herkenbaar |
| Rijkswet op het Nederlanderschap | Art. 6 (optie), Art. 8 (naturalisatie) | Voorwaarden voor Nederlanderschap, actueel onderwerp |
| Paspoortwet | Art. 9 (geldigheidsduur) | Leeftijdsafhankelijke geldigheidsduur, simpel patroon |

### Financieel / toeslagen

| Wet | Interessante artikelen | Waarom goed |
|-----|----------------------|-------------|
| Wet op de huurtoeslag | Art. 7-13 (recht + berekening) | Inkomen + huur + huishoudtype, vergelijkbaar met zorgtoeslag |
| Wet kinderopvang | Art. 1.6-1.7 (recht op kinderopvangtoeslag) | Voorwaarden + berekening, relevant voor veel gezinnen |
| Wet minimumloon en minimumvakantiebijslag | Art. 8-12 (hoogte minimumloon) | Leeftijdsafhankelijke bedragen, concrete getallen |
| AOW (Algemene Ouderdomswet) | Art. 7-7a (recht op ouderdomspensioen) | Opbouw per jaar, leeftijdsgrens, breed bekend |

### Arbeid en sociaal

| Wet | Interessante artikelen | Waarom goed |
|-----|----------------------|-------------|
| Arbeidstijdenwet | Art. 5:7-5:8 (max werktijd, rusttijden) | Numerieke drempels, dag/week/periode berekeningen |
| Wet arbeid en zorg | Art. 4:2-4:3 (zwangerschapsverlof) | Duur en berekening, herkenbaar |
| Werkloosheidswet | Art. 16-17 (recht op WW-uitkering) | Wekeneis + jareneis, concrete tellingen |

### Ruimtelijk / wonen

| Wet | Interessante artikelen | Waarom goed |
|-----|----------------------|-------------|
| Omgevingswet | Art. 5.1 (vergunningplicht) | Categorieen activiteiten, nieuw en actueel |
| Woningwet | Art. 1b (bouwvoorschriften) | Vergunningcategorieen per type bouwwerk |

### Verkeer

| Wet | Interessante artikelen | Waarom goed |
|-----|----------------------|-------------|
| Wegenverkeerswet 1994 | Art. 110-111 (snelheidslimieten) | Categorieeen wegen + voertuigen, herkenbaar |
| Wet rijonderricht motorrijtuigen 1993 | Art. 9 (bevoegdheidseisen) | Voorwaarden per categorie, aansluitend bij rijbewijs |
