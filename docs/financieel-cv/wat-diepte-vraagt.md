# Wat er nodig is om een variabele werkelijk uit te trekken

**Datum:** 24 september 2026 · **Aanleiding:**
[`diepte-van-een-variabele.md`](diepte-van-een-variabele.md), de acht niveaus
onder `is_uitgesloten_beschut_werk_pwet_10b`

Die ladder is beschreven, niet gemodelleerd. Dit document zet op een rij wat
ervoor nodig is om zo'n keten wél uit te voeren, en scheidt daarbij wat er al
staat van wat blokkeert. Alles hieronder is vastgesteld op 23 en 24 september,
in het dossier zelf of in de code.

## Samenvatting

| | Onderwerp | Stand |
|---|---|---|
| A1 | Vinden vóór concluderen | werkwijze, geen bouwwerk |
| A2 | Een regeling ophalen | **bestaat**, één commando |
| A3 | Verwijzen naar een oude redactie | **blokkeert** — RFC-020 is concept |
| A4 | Een aangekondigde wijziging vastleggen | geen plaats in het formaat |
| B1 | Diepte van de keten | **geen blokkade**, budget is 20 |
| B2 | Een open term die een waarde draagt | werkt, mits `implements` of `default` |
| B3 | De parameterlast per niveau | **grootste rem** |
| B4 | Afwezig tegenover onbekend | **valstrik**, kost stille fouten |
| B5 | Harde limieten van de motor | bijt nu al |
| C1 | Ankering op het juiste artikel | geen controle |
| C2 | Een poort die niets meet | opgelost, blijft een risico |
| C3 | De YAML 1.1-val | bekend, drie keer misgegaan |
| C4 | Ankers naar wetten.overheid.nl | corpusbrede bevinding |
| D1 | De goedkope voorselectie | klaar om te draaien |
| D2 | Waar het oordeel wordt vastgelegd | **bestaat** sinds v0.7.0 |

---

## A. De regeling het corpus in krijgen

### A1. Vinden vóór concluderen

"Niet in het corpus" is iets anders dan "bestaat niet". Drie keer in twee dagen
bleek iets bereikbaar dat als afwezig genoteerd stond: Besluit SUWI 3.5, de Wet
minimumloon in `corpus-poc`, en het Besluit advisering beschut werk zelf. Dat
laatste is één keer misgegaan doordat een BWB-nummer werd gegokt; de 404 die
daarop volgde werd gelezen als bewijs van afwezigheid.

Nodig is geen bouwwerk maar een volgorde: zoeken via de SRU-dienst van
`repository.overheid.nl` of via de BWB-nummerreeks van het jaar van
inwerkingtreding, en een nummer nooit afleiden uit een vermoeden.

### A2. Een regeling ophalen — dit bestaat al

De harvester neemt een BWB-nummer en levert YAML:

```
regelrecht-harvester download BWBR0035947
```

Voor niveau 3 van de ladder is dat de hele inwinstap. Wat daarna resteert is
modelleerwerk, geen infrastructuur.

### A3. Verwijzen naar een oude redactie — dit blokkeert

Artikel 3 lid 3 en 4 van het Besluit advisering beschut werk verwijzen naar het
Besluit uitvoering sociale werkvoorziening *"zoals die artikelen luidden vóór de
inwerkingtreding van artikel II van de Invoeringswet Participatiewet"*. Een
statische verwijzing in de zin van Aanwijzing 3.47.

Het corpus héeft die redactie staan, als `2012-07-01.yaml`. Wat ontbreekt is de
mogelijkheid om er vanuit een ander artikel naar te verwijzen: de
versieselectie gaat op één globale peildatum. **RFC-020 (`as_of`) beschrijft
precies dit; de status is Draft en niet geïmplementeerd.** Zolang dat zo is, is
niveau 4 van deze ladder niet te modelleren zonder de oude tekst te kopiëren, en
kopiëren maakt de herkomst juist onzichtbaar.

Dit is de enige harde blokkade in de hele keten.

### A4. Een aangekondigde wijziging vastleggen

Bij artikel 3 meldt wetten.overheid.nl een *"toekomstige wijziging voorzien met
ingang van 1 juli 2028"*, en op 24 februari 2026 is een ontwerpbesluit tot
aanpassing voorgehangen bij de Tweede Kamer (Kamerstuk 34352, nr. 351). Wie dit
modelleert, modelleert een bepaling die beweegt.

Het formaat kent geen veld om dat vast te leggen zonder de geldende tekst te
vervuilen. Een `marking` is het niet: er is geen taalgat. Voor nu hoort het in
de doc-producten; of het een eigen vorm verdient is een RFC-vraag.

---

## B. Wat het formaat en de motor moeten kunnen

### B1. De diepte van de keten is geen probleem

`MAX_CROSS_LAW_DEPTH` staat op 20 en geldt voor kruisverwijzingen en interne
artikelverwijzingen samen. De ladder is acht niveaus. Er is dus ruimte, en dat is
het vermelden waard omdat het de vanzelfsprekende zorg wegneemt.

### B2. Een open term die werkelijk een waarde draagt

De IoC-constructie van RFC-003 (Accepted, Implemented) werkt: een hogere wet
verklaart een `open_term`, een lagere regeling verklaart `implements` en levert
de waarde.

Daarbij hoort een correctie op de fideliteitsaudit. Die stelt dat een open term
inert is, omdat `typecheck.rs` hem alleen als symbool in scope declareert. Dat
geldt voor een open term **zonder** `default`. Een open term **met** een
`default`-blok dat een actie draagt, levert wél een waarde: de
werkgeverslastenvergoeding van Participatiewet 10d wordt sinds 24 september in
een formule gebruikt en de typecheck accepteert dat.

Voor een keten betekent dat: elk niveau moet óf een `implements` uit een lagere
regeling ontvangen, óf zelf een default dragen. Gedeclareerd zonder een van
beide is de slechtste toestand — het ziet eruit als dekking en doet niets.

### B3. De parameterlast per niveau — de grootste rem

De motor verlangt **elke** parameter van de aangeroepen wet, ongeacht welke
uitkomst wordt opgevraagd. De aanroep van Wfsv 38b naar Participatiewet 10b
vraagt om `is_uitsluitend_aangewezen_op_beschut_werk`, een pass-through van één
collegevaststelling, en moet daarvoor toch de hele parameterlijst van 10b
meeleveren — inclusief het quotum van lid 6, dat met de chapeau niets te maken
heeft.

Dat is geen slordigheid in de aanroep. Het is de reden dat dieper modelleren de
invoerlast laat stijgen in plaats van dalen. Elk extra niveau duwt zijn volledige
parameterlijst omhoog naar de aanroeper.

Nodig is parameterafbakening per uitkomst: een aanroep die alleen de feiten
verlangt waarvan de gevraagde uitkomst afhangt. Zonder dat wordt elke ladder van
acht niveaus een invoerformulier.

### B4. Afwezig tegenover onbekend — een stille valstrik

Een parameter met `required: false` die de aanroeper weglaat, levert volgens
RFC-036 (Proposed, Implemented) geen `null` maar een **onbekende waarde die de
parameter noemt**. Een afwezigheidstoets geschreven als `EQUALS … null` vangt dat
niet, en het onbekende resultaat plant zich voort tot in het bedrag:

```
hoogte_lks_voltijd_eurocent_per_maand = Unknown([
  MissingFact { law: "participatiewet", name: "vaststelling_loonwaarde_blijft_achterwege" },
  MissingFact { law: "participatiewet", name: "datum_aanvang_dienstbetrekking" }
])
```

Dit is op 24 september bij de reparatie van Participatiewet 10d gevonden. De
uitweg was de betrokken parameters verplicht te maken, met `nullable: true` waar
de waarde werkelijk kan ontbreken. Dat werkt, maar het schuift de last naar de
aanroeper en verergert B3.

Nodig is een toets in het formaat die afwezigheid onderscheidt van een
ontbrekende waarde, zonder de parameter verplicht te maken.

### B5. Harde limieten van de motor

| Limiet | Waarde | Gevolg |
|---|---|---|
| `MAX_LOADED_LAWS` | 100 | Een diepe keten over zeven regelingen plus hun lagere regelgeving loopt hiertegenaan. Op 24 september was een mini-corpus van acht bestanden nodig om de BDD-suite te draaien; het volledige corpus telt 22.468 bestanden |
| `MAX_ARRAY_SIZE` | 1000 | Bijt nu al: `participatiewet/2026-01-01.yaml` en `2026-02-04.yaml` laden niet, met "Too many articles (1018, max 1000)" |
| `MAX_YAML_SIZE` | 1 MB | Nog niet geraakt |

De eerste twee zijn beveiligingsgrenzen tegen geheugenuitputting, geen
inhoudelijke keuzes. Ze zijn geschreven voor een stelsel van tien tot twintig
regelingen. Een diepteoefening over zeven regelingen met hun lagere regelgeving
zit tegen die aanname aan.

---

## C. Wat gecontroleerd moet worden

### C1. Ankering op het juiste artikel

Een model hoort te staan bij het artikel dat de regel stelt. Op 23 september was
dat drie keer niet zo:

- het hele model van de loonkostensubsidie stond onder Participatiewet 10c, dat
  alleen over de vaststelling van de doelgroep gaat; artikel 10d had geen model;
- Wtl 2.1 droeg de bedragen van 2.9, 2.13 en 2.17 en de duur van 2.8, 2.12 en
  2.16, zodat die artikelen in elke corpustelling als niet-gemodelleerd tellen;
- het `implements` van het Reïntegratiebesluit hing aan artikel 1a, dat
  uitsluitend de grondslagen opsomt.

Alle drie zijn gerepareerd, maar niets houdt tegen dat het opnieuw gebeurt. Een
ladder is alleen herleidbaar als elk niveau op zijn eigen artikel staat. Nodig is
een controle die de `legal_basis` van een actie vergelijkt met het artikel waar
die actie staat, en die een `implements` weigert op een artikel dat geen regel
stelt.

### C2. Een poort die niets meet

`script/cross-law-integriteit.py` bestond al en controleert of elke `source`- en
`implements`-binding oplost. Op 23 september rapporteerde hij groen met
`clean=0`: hij had geen enkele binding gevonden om te controleren. Na reparatie
staat de teller op 22.

| Draai op de trajectcorpus | clean | overige tellers |
|---|---|---|
| Vóór de reparatie | **0** | alles 0 |
| Na de reparatie | **22** | alles 0 |

Elke poort die telt heeft een ondergrens nodig: nul gevonden gevallen is een
storing, geen groen licht.

### C3. De YAML 1.1-val

PyYAML leest `2:20` als 140, omdat YAML 1.1 sexagesimale getallen kent. Dat is in
twee dagen drie keer misgegaan: bij de triage, in een bestaand document, en in
`cross-law-integriteit.py` zelf. Vastgelegd in
[`pyyaml-valkuil.md`](pyyaml-valkuil.md). Elke tool die dit corpus leest, heeft
een lader nodig waarin de sexagesimale resolvers uitstaan — juist een dossier met
Wajong-artikelnummers als `2:20` loopt hier tegenaan.

### C4. Ankers naar wetten.overheid.nl

Elk gemodelleerd artikel draagt een `url` met een anker dat op de doelpagina niet
bestaat, omdat het structuurpad ontbreekt. Gemeten over vier wetten met drie
verschillende nummerstijlen: nul kale ankers gevonden. Het raakt niet één wet
maar elk anker in het corpus. Zie
[`ankers-naar-wetten-overheid.md`](ankers-naar-wetten-overheid.md).

Voor een diepteoefening is dat meer dan een ongemak: de hele waarde van een
ladder is dat een lezer elk niveau kan natrekken.

---

## D. Werkwijze

### D1. De goedkope voorselectie

Niet alle 120 gegevens verdienen deze oefening; een geboortedatum uit de
basisregistratie personen heeft geen ladder. Wat wel loont is één ronde langs
alle 120 met drie vragen, grotendeels te beantwoorden uit de bestaande indeling:

1. Wijst de wettekst een lagere regeling aan die deze waarde invult?
2. Staat die regeling in het corpus?
3. Is wat overblijft een oordeel of een regel?

Zeven kandidaten zijn nu al bekend, en zes daarvan liggen al in het corpus. Dat
is een afgebakende lijst, geen programma van 120.

### D2. Waar het oordeel wordt vastgelegd — dit bestaat

Van de acht niveaus is er precies één waar werkelijk menselijk oordeel zit: de
maatstaf *"niet binnen redelijke grenzen door een werkgever"* in artikel 3 lid 1
van het Besluit advisering beschut werk. Al het andere is regel.

Voor zo'n norm kent schema v0.7.0 `decided_per_case_by`: een open term met een
gezag dat per geval oordeelt, zonder dat de wet een regeling aanwijst. Zeventien
van de 23 open termen in dit dossier dragen dat veld al.

### D3. Wie doet wat

| Stap | Waar het hoort |
|---|---|
| Vinden en inwinnen van de lagere regeling | harvester |
| De artikelen modelleren | desk |
| De maatstaf invullen | jurist |
| De aangekondigde wijziging volgen | dossierhouder |

---

## De kortste weg naar één werkende ladder

Als één keten helemaal doorlopen moet worden, is dit de volgorde. Niveau 3 is
haalbaar zonder nieuwe techniek; niveau 4 niet.

1. `regelrecht-harvester download BWBR0035947` — het besluit binnenhalen.
2. Artikel 2 en 3 modelleren. Artikel 3 lid 1 en 2 zijn een limitatieve
   voorwaarde, lid 3 en 4 zijn twee kortsluitingen, lid 5 en 6 twee derogaties,
   lid 7 een hergebruikclausule. Alleen "redelijke grenzen" blijft open term.
3. `implements` leggen op Participatiewet 10b lid 2, en controleren dat het
   artikel waarop het hangt werkelijk een regel stelt (C1).
4. De termijn van acht weken uit artikel 2 opnemen — die komt in het model
   nergens voor.
5. **Stoppen bij niveau 4.** De verwijzing naar de oude redactie vraagt RFC-020.
   Tot die er is, hoort daar een `marking` met `resolution: model` en een citaat
   van de statische verwijzing, zodat de leegte zichtbaar blijft in plaats van te
   verdwijnen.

Dat levert vier van de acht niveaus uitvoerbaar op, met de grens expliciet
vastgelegd op de plek waar zij ligt.
