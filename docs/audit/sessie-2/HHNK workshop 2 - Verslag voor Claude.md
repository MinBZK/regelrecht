# HHNK workshop 2 — verslag voor Claude

> *Bron: `HHNK workshop 2 - Bron voor verslag.md` (ruwe notities Daan, direct na sessie). Dit document is een herziene versie van die ruwe notities, opgesteld in dialoog met Claude op 1 mei 2026, bedoeld als invoer voor een toekomstige Claude-sessie die de logica van art 11–16 URI 1990 verbetert. De ruwe bron is bewust onaangeroerd gelaten als historisch artefact.*

In workshop 2 zijn we verder aan de slag gegaan met kwijtschelding. Daan had voorbereiding klaar (slidedeck, workshopdoc, audit-doc) maar het werk verliep langs een andere lijn dan voorbereid.

**Aanwezig**:
- Project initiator met kennis invordering
- Dossier controleur intern HHNK (andere dan vorige kwijtscheldingsexpert)

---

## Hoe dit verslag tot stand kwam — methode

Twee instrumenten zijn na de workshop ingezet om de ruwe notities te verfijnen voordat ze als input naar een volgende Claude-sessie gaan:

**1. Execution-trace als grondslag**
De trace van het hero-scenario ('Volledige kwijtschelding voor iemand zonder inkomen' aangepast naar `is_pensioengerechtigd=true`, NBI=1659000) is regel voor regel doorgenomen. Daaruit kwamen drie soorten observaties:

- *Concreet rekenpad* — welke parameters welke outputs voeden, en in welke volgorde de actions runnen (bv. `betalingscapaciteit = MAX(0, 12 × (NBI − extra_uitgaven − kostennorm))`)
- *Expliciete delegations* — `Delegation: Open term 'X' using default value: Y`-regels die laten zien op welke punten in URI een lagere regeling (HHNK Leidraad) zou kunnen inhaken via `implements:`. Deze run heeft er twee.
- *Anomalieën* — outputs die wel berekend worden maar niet doorlopen (`uitgaven_totaal_maand` in deze run; staat als open vraag onder bevinding 2b).

**2. Dialoog Daan ↔ Claude over onhandige formuleringen**
Een aantal observaties uit de ruwe notities lieten meerdere lezingen toe. Claude heeft daarop verduidelijkingsvragen gesteld; Daan heeft die beantwoord en/of de oorspronkelijke woorden gecorrigeerd. Belangrijkste momenten:

- *Eenheid-issue*: ruwe notitie zei "inkomen ophogen naar 1659" — trace toont `1659000`. Bleek: invoerveld is in eurocent, dus 1659000 cent = €16.590/maand (geen AOW maar hoog inkomen). UX-issue, niet modellerings-issue.
- *Percentage-verwarring*: ruwe notitie noemde 0.9, 0.8 en 0.4 in één punt alsof dat over hetzelfde getal ging. Claude heeft de drie uit elkaar getrokken (kostennorm-percentage in art 16 vs. aanwendings-percentage in art 11 vs. HHNK-praktijk). Daan bevestigde achteraf dat tijdens de sessie 0.9 (art 16) en 0.8 (art 11) door elkaar waren gehaald — die verwarring zat in de leesfout, niet in de YAML.
- *"Kostennorm op verkeerde plek"*: ruwe notitie suggereerde een architectuur-issue. Na percentage-ontwarring blijft daarvan over: één concrete trace-bevinding (`uitgaven_totaal_maand` ongebruikt in BC) die nog onderzocht moet worden — klassificatie open.
- *HHNK BC-formule `(0.8(I−U))/2`*: ruwe notitie liet onduidelijk of `/2` "halfjaarlijks" of "0.8 gehalveerd naar 0.4" betekende. Daan bevestigde: de tweede lezing — directe vervanging van het aanwendings­percentage van 0.8 → 0.4.

**Status van de bevindingen na deze ronde**:
- Bevestigde issues: eenheid-UX, AOW-floor (punt 1), oude normbedragen (punt 3), HHNK-40%-aanwendingsplek inclusief ontbrekende `open_term` op art 11 (punt 4).
- Open vraag (uit te zoeken): `uitgaven_totaal_maand` ongebruikt in BC-formule (punt 2b).
- Toelichting in plaats van actie: percentage-verwarring (punt 2a — geen YAML-aanpassing nodig, wel uitleg-blokje voor volgende sessie).

---

## 1. Introductie kennismaking en recap scope-bepaling

1. Uitgelegd aan de hand van briefjes dat logica meer in art 11 URI 1990 terecht is gekomen.
2. Voorbeelden van punten waar we in de eerste sessie aan hebben geraakt — met name rondom art 26 Leidraad HHNK (betalingscapaciteit, autowaarde etc.).

## 2. Procesflow opgesteld

**Doel**: begrip Daan waar rol HHNK nu in zit.

![[Scherm­afbeelding 2026-05-01 om 08.51.19.png]]

**Geleerd**: trace met enkele losse aanpassingen is een nuttige manier om naar HHNK-werkwijze te kijken. HHNK krijgt afwijzersadvies en toetst vooral op dát punt. Het bouwen van traces rondom die enkele afwijzingen sluit aan bij hun proces.

## 3. Editor: scenario doorlopen

We zijn aan de slag gegaan met het scenario 'Volledige kwijtschelding voor iemand zonder inkomen'. Daan heeft deze lokaal gewijzigd naar `is_pensioengerechtigd = true`, default verder vrijwel alles 0 (inkomen, vermogen-componenten, uitgaven). Tussendoor hebben we ook gespeeld met `netto_besteedbaar_inkomen_maand = 1659000` om een AOW-achtig inkomen te simuleren.

**Eenheid-noot**: het invoer-veld `netto_besteedbaar_inkomen_maand` verwacht eurocent. `1659000` cent = €16.590 per maand — dat is geen AOW maar een hoog inkomen. Bedoeld was waarschijnlijk €1659/maand (≈ AOW alleenstaand 2026); dan had `165900` ingevuld moeten worden. De BC die de trace toont (€182.252/jaar) is consistent met de feitelijk ingevoerde €16.590/maand, niet met €1659. UX-issue: editor maakt eenheid niet zichtbaar.

### Wat opviel — bevindingen

**1. NBI=0 wordt geaccepteerd voor een pensioengerechtigde**

- Niet logisch dat iemand kwijtschelding krijgt als NBI=0 is. Een burger heeft altijd ergens inkomen.
- Bij `is_pensioengerechtigd=true` zou AOW als impliciete ondergrens van NBI moeten gelden. Een 67-jarige met NBI=0 zou de vraag oproepen hoe iemand in zijn levensonderhoud voorziet (hoewel niet juridisch gespecificeerd).
- Onderscheid: BC=0 kan, NBI=0 kan niet.
- Complicatie: AOW is geen standaard normbedrag — vakantiegeld telt mee, en het bedrag kan minder zijn afhankelijk van opbouwjaren in NL.
- Trace-observatie: AOW komt al wel in de keten voor, maar **als kostennorm-input** (`netto_ouderdomspensioen_alleenstaand = 155815` wordt gebruikt als bijstandsnorm-vervanger in art 16 bij pensioengerechtigden). Aan de **inkomstenkant** (NBI) zit geen automatische AOW-injectie. Dat is wat in de workshop bedoeld werd met "AOW staat wel in trace maar niet in inkomenshoek".

**2a. Verwarring tijdens sessie tussen twee percentages — toelichting, geen YAML-issue**

Tijdens de walk hebben we de volgende twee percentages door elkaar gehaald:

- **0.9 in URI art 16** — kostennorm = 90% van bijstandsnorm. Dit bepaalt **hoeveel iemand mag houden om in zijn levensonderhoud te voorzien** (drempel-bepaling).
- **0.8 in URI art 11 sub-2°** — "ten minste 80%" van de BC moet aangewend worden. Dit bepaalt **hoeveel van het meerdere inkomen verplicht naar de aanslag gaat** (aanwendingsgraad).

Beide getallen staan op hun juridisch juiste plek in de YAML. Het workshop-gevoel "kostennorm staat niet op de goede plek" kwam voornamelijk uit deze verwarring. Voor de volgende sessie: vóór de walk-through expliciet uit elkaar trekken — twee percentages, twee plekken in de wet, twee functies.

**2b. Open vraag — `uitgaven_totaal_maand` lijkt ongebruikt in BC-berekening**

Bevinding uit de trace die nog uitgezocht moet worden:

- De trace berekent `uitgaven_totaal_maand` (= som van betalingen_belastingschulden, netto_woonlasten, premies_ziektekosten, alimentatie, aflossingen_belastingschulden, kostgangerskosten, kindgebonden_budget_tekort, overige_noodzakelijke_uitgaven). In onze run: `60000` cent = €600/maand (uit `betalingen_belastingschulden_maand`).
- Vervolgens: `betalingscapaciteit = MAX(0, 12 × (NBI − extra_uitgaven_maand − kostennorm_bedrag))`. `uitgaven_totaal_maand` zit hier níet in.

URI art 13 wettekst zegt dat BC het positieve verschil is tussen NBI en kosten van bestaan + bepaalde uitgaven (uit art 15). De "+ bepaalde uitgaven" lijkt in de huidige modellering te ontbreken in de BC-formule. **Hier moet naar gekeken worden** — is dit een modelleringsbug, of is `uitgaven_totaal_maand` doelbewust een losse output die elders gebruikt wordt? Concrete trace-regels om mee te beginnen:

```
║   ║   └──Computing betalingscapaciteit
║   ║       ├──Compute MAX(...) = 18225192
║   ║       │   └──Compute MULTIPLY(...) = 18225192    ← × 12
║   ║       │       └──Compute SUBTRACT(...) = 1518766
║   ║       │           ├──Compute SUBTRACT(...) = 1659000
│   │   │   │   │       │   └──$EXTRA_UITGAVEN_MAAND = 0
│   │   │   │   │       └──$KOSTENNORM_BEDRAG = 140234
```

vs.

```
║   ║   │   └──Computing uitgaven_totaal_maand
║   ║   │       ├──Compute ADD(...) = 60000
║   ║   │       │   ├──$BETALINGEN_BELASTINGSCHULDEN_MAAND = 60000
║   ║   │       │   └──[verdere ADD-keten van overige uitgaven, allemaal 0]
║   ║   │       └──Result: uitgaven_totaal_maand = 60000
```

**3. Normbedragen lijken op meerdere plekken oud**

Lijken oud ingevuld waar ze naar nieuwe normbedragen zouden moeten verwijzen.

- Woonlasten, huurnorm, kostgangersbedrag goed nakijken.
- Bedragen in art 15 URI horen via cross-law `source:` te verwijzen naar `wet_op_de_huurtoeslag`. Die koppeling is nu een shortcut: literal definitions in de YAML. Concreet zien we in de trace: `WOONLASTEN_DREMPEL_MAAND = 25815` en `WOONLASTEN_MAXIMUM_MAAND = 87966`. Voor consistente actualisering: cross-law-source aanleggen i.p.v. hardcoded.

**4. HHNK-aanwendingspercentage in praktijk: 40% i.p.v. 80%**

HHNK rekent in praktijk met aanwendingsgraad 40% (vervangt de 80% uit URI art 11 sub-2°). Aanwendbare BC = 0.4 × BC i.p.v. 0.8 × BC. Voor de burger gunstiger (kleiner deel van BC verplicht ingezet → grotere kwijtschelding) — *lex pro cive*, want HHNK doet minder dan het URI-minimum.

Eerder genoteerd als `BC = (0.8(inkomsten−uitgaven))/2 in HHNK geval`. Dit was **niet** "halfjaarlijks" of "0.8 gedeeld door 2", maar bedoeld als directe vervanging van de aanwendings­percentage in art 11 sub-2°. De `/2` was kort­schrift voor het halveren van 0.8 naar 0.4.

Voor de modellering: dit hoort als HHNK-Leidraad-uitwerking via `implements:` op een `open_term` op URI art 11. Probleem: in de huidige URI art 11 is de 0.8 hard­gecodeerd in de action — er is **geen open_term** waaraan HHNK kan haken. Als we deze haakplek willen aanleggen, moet URI art 11 eerst een `open_term` krijgen voor het aanwendings­percentage.

Exacte plek in de Leidraad-tekst waar HHNK 40% beschrijft, nog niet gepin-pointed (te doen voor sessie 3).

**5. Verschillende typen belastingschulden — open vraag**

In de huidige modellering komen meerdere belastingschuld-velden voor:

- `betalingen_belastingschulden_maand` (art 15-uitgave a)
- `aflossingen_belastingschulden_maand` (art 15-uitgave e)
- `hoger_bevoorrechte_schulden` (art 12, vermogen-zijde)
- `aanslagbedrag` (de aanslag waarvoor kwijtschelding wordt aangevraagd)

Vraag voor vervolgsessie: zijn deze goed onderscheiden in alle relevante gevallen, of komen er praktijksituaties waarbij dezelfde schuld dubbel telt of mist? Concreet voorbeeld nog niet gedocumenteerd — ophalen bij HHNK-spreadsheet.

**6. Logica-aanpassingen voor scenario-uitbreiding**

Op meerdere plekken moet logica worden aangepast om voorbeelden verder uit te kunnen werken. Kandidaten die hierboven al zijn benoemd:

- AOW-floor bij `is_pensioengerechtigd=true` (punt 1)
- Uitgaven-architectuur in art 13 (punt 2b — afhankelijk van diagnose)
- Huurtoeslag cross-law-source in art 15 (punt 3)
- Open_term op art 11 voor aanwendings­percentage (punt 4)

Concrete vervolgstappen daar afhankelijk van wat de HHNK-spreadsheet aanlevert.

### HHNK-haakplekken zichtbaar in deze trace

In de execution-trace verschijnen expliciet **twee `Delegation:`-momenten** — dat zijn de open_terms waar HHNK Leidraad-logica kan inhaken via `implements:`:

1. `Delegation: Open term 'kostennorm_percentage' using default value: 0.9` — federale default uit art 16. HHNK Leidraad zou hier een afwijkend percentage kunnen invullen, *als* HHNK dat doet. Te bevestigen voor sessie 3.
2. `Delegation: Open term 'verhoging_financiele_middelen_vrijstelling' using default value: 0` — HHNK Leidraad heeft hier de €1.500/€1.800/€2.000-staffel (gevalideerd in sessie 1).

Een **derde haakplek** is er nu *niet* zichtbaar omdat hij ontbreekt: het aanwendings­percentage op URI art 11 is hardcoded 0.8. Voor de HHNK-40%-praktijk (punt 4) zou daar eerst een open_term aangelegd moeten worden in URI art 11 zelf.

## Vervolg

- Goed om volgende keer met de Leidraad-koppeling verder te gaan om te zien of we dichter bij kloppende logica kunnen komen.
- HHNK-collega's zorgen dat een spreadsheet met berekening bij ons komt: handig startpunt om langs onze eigen logica te houden in scenario's.
- **Open punten voor vervolg-Claude-sessie**:
  - **2b uitzoeken**: is `uitgaven_totaal_maand` ongebruikt in de BC-formule een bug of bewuste keuze? Zo bug: art 13's BC-formule corrigeren zodat art-15-uitgaven worden meegenomen.
  - **AOW-floor toevoegen** bij `is_pensioengerechtigd=true` (in art 13 of als caller-validatie).
  - **Huurtoeslag cross-law `source:`** aanleggen in art 15 voor woonlasten-drempel/maximum.
  - **Open_term op art 11** toevoegen voor het aanwendings­percentage zodat HHNK-40% via `implements:` kan hangen.
  - **Belastingschulden-typen verifiëren** met concreet HHNK-voorbeeld uit de spreadsheet.
  - **Eenheid-UX** in de editor (zichtbaar maken dat `netto_besteedbaar_inkomen_maand` in eurocent is).
  - **HHNK-40% pin-pointen** in de Leidraad-tekst (welke paragraaf, welke grondslag).
