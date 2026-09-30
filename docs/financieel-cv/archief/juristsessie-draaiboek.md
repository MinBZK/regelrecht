# Draaiboek: juristsessie wetten valideren (2 uur, donderdag)

Opzet voor de validatiesessie met één jurist over de zeven wetten achter het
Financieel CV. Gebouwd op de skill `regelrecht-audit-products` (productcatalogus
en facilitatie-patronen), met de stand van de validatie-branch
`traject/financieel-cv-validatie-df48ddd1` als inhoud.

De feitelijke onderbouwing staat in
[`juristsessie-voorbereiding.md`](juristsessie-voorbereiding.md): de
tekstcontrole tegen wetten.overheid.nl, de regelgeving die nog ontbreekt, en wat
de huidige RFC's toelaten. Lees dat eerst; het verschuift twee agendapunten.

Naslag die hierbij hoort: `host-briefing-wetten.md` (spiekkaarten per regeling,
jargon, klikpad), `README.md` (scope, open vragen), `mvt-referenties.md`
(juridische context per regeling).

## Waar we staan

Zeven wetten, op schema v0.5.4:

| Wet | machine_readable | untranslatables | waarvan met juristspoor |
|---|---|---|---|
| Ziektewet 29b (NRP) | 1 | 6 | 2 |
| Participatiewet 10c/10d (LKS) | 6 | 16 | 1 |
| Wajong 2:20 (LDP) | 4 | 11 | 1 |
| Wtl 2.1 (LKV/LIV) | 3 | 6 | 3 |
| Wet WIA 35 (JC/WPA) | 8 | 14 | 0 |
| WW 76a (PP) | 1 | 2 | 0 |
| Wfsv 38b (doelgroepregister) | 2 | 9 | 0 |
| **totaal** | **25** | **64** | **7** |

Alle 64 staan op `accepted: true`. Dat veld zegt alleen dat de modelleur het
heeft afgevinkt; het zegt niet wie dat deed of dat een jurist ernaar heeft
gekeken. Zeven stuks dragen in de `reason`-tekst een spoor van de
juristvalidatie van juni 2026, de overige 57 niet. Dat onderscheid is de
aanleiding voor deze sessie, en het is ook de eerste ontwerpkeuze die hieronder
terugkomt.

## Wat deze sessie is, en wat niet

De skill onderscheidt twee soorten sessies. **Verkennend** is vroeg en zonder
poort: domeinkennis ontginnen op een ruwe analyse. **Validerend** heeft een
poort: schema valide, tests groen, modelleerfouten al gerepareerd, en wat
overblijft is oordeel. Deze sessie is validerend, en dat betekent dat feitelijke
defecten er niet in thuishoren. Vind je er vooraf nog, dan repareer je die aan
de desk; anders gaat het uur op aan werk dat geen jurist nodig heeft.

Twee uur is genoeg voor ongeveer vijftien beslispunten van vijf tot acht
minuten, plus kader, pauze en afronding. 64 untranslatables passen daar niet in,
dus het meeste werk zit in de triage vooraf.

## Vooraf: de poort (desk, niet in de sessie)

```bash
just validate      # schema-validatie van alle YAMLs
just bdd           # beide buckets; BDD_BUCKET=corpus beperkt tot de wetscenario's
just bdd-trace     # traces, handig om een uitkomst te kunnen tonen
```

Is er iets rood, dan hoort dat vóór donderdag gerepareerd. Een rode suite tijdens
de sessie leidt de aandacht naar de machinerie in plaats van naar de wet.

Daarnaast staan er vier desk-punten uit de voorbereiding open, waarvan er één
de sessie zelf raakt: de vier Wajong-artikelen dragen posities als nummer
(`135`, `140`, `142`, `144` voor 2:15, 2:20, 2:22 en 2:24) en hun `url` wijst
naar een anker dat niet bestaat. Wie donderdag een Wajong-nummer natrekt, komt
op een lege pagina uit.

Verder klaarzetten:

1. De editor op het traject, kolommen **Tekst · Machine · Scenario's**. Dat is het
   validatieritueel visueel gemaakt: wettekst links, formule in het midden,
   uitkomst rechts.
2. Een tweede scherm of tweede venster met dit draaiboek en een notitiedocument.
   Niet op het deelscherm.
3. De drie waarschuwingen uit de host-briefing paraat: het LIV-scenario faalt met
   opzet, Sadee krijgt geen jobcoaching via de WIA (lid 4.a is een
   doorverwijzing), en alle bedragen staan in eurocent.

## De triage vooraf: vier bakjes

Loop de 64 untranslatables langs en geef elk één label. Alleen bakje 3 gaat mee
de sessie in.

| Bakje | Wat het is | Aantal (indicatie) | Waar het heen gaat |
|---|---|---|---|
| 1. Taalgat | Het formaat of de motor kan de constructie niet uitdrukken | klein | Desk, en later een `marking` onder RFC-031 |
| 2. Open term of delegatie | Lagere regelgeving vult de inhoud in (AMvB, ministeriële regeling, verordening) | groot, zeker de Wfsv-reeks 38f en de Pwet-verordening | Desk: `open_term`, of harvesten |
| 3. Oordeel | De wettekst beslist het niet en een jurist moet kiezen | doel: 12 tot 15 | **Deze sessie** |
| 4. Scope | Bewust buiten de regelhulp gelaten | middel | Sessie, maar als korte ja/nee aan het eind |

Dit is geen bureaucratie: RFC-031 schrapt `untranslatables` in schema v0.7.0 en
vervangt het door `markings`, en een marking is uitdrukkelijk een *taalgat*, niet
een openstaande rechtsvraag. Een norm die een andere regeling invult hoort daar
niet in maar is een open term. Wie nu sorteert, heeft die migratie al half
gedaan; wie het laat liggen, sorteert straks alsnog, dan zonder jurist erbij.

## Het uur, blok voor blok

| Tijd | Blok | Vorm |
|---|---|---|
| 0:00 tot 0:10 | Kader en spelregels | Vertellen |
| 0:10 tot 0:25 | Blauwdruk: Ziektewet 29b helemaal door | Protocol A/B/C |
| 0:25 tot 1:05 | Vier oordeelsclusters, elk tien minuten hard | Protocol A/B/C |
| 1:05 tot 1:15 | Pauze | |
| 1:15 tot 1:40 | Oordeelsclusters vijf en zes | Protocol A/B/C |
| 1:40 tot 1:52 | Scope-besluiten S1 tot S5 | Ja/nee, kort |
| tussendoor | Cluster 7 past in de ruimte van S3, zie hieronder | |
| 1:52 tot 2:00 | Afronding en toezeggingen | |

De pauze staat er omdat de facilitatie-patronen die voorschrijven bij sessies
boven de 75 minuten. Niet schrappen als je uitloopt; schrap dan een cluster.

### 0:00 tot 0:10 — kader

Drie dingen zeggen, en niet meer:

1. **Wat we hebben gedaan.** Zeven wetten vertaald naar formules die een motor
   uitvoert, met twee casussen als rode draad. Het model markeert zelf wat het
   niet vangt.
2. **Wat we vragen.** Niet of het model mooi is, maar of het de wet volgt. "Wij
   hebben een voorstel, jij schiet het stuk, dat is het doel."
3. **Wat er met het antwoord gebeurt.** Elk oordeel wordt vastgelegd in de YAML
   naast de bepaling waar het over gaat, met datum en naam. Twijfel wordt ook
   vastgelegd, als open punt met een eigenaar. Zo is de sessie terug te vinden
   in de code, niet alleen in een verslag.

### 0:10 tot 0:25 — de blauwdruk

Ziektewet 29b, de no-riskpolis. Eén artikel, zes untranslatables, en het is de
eenvoudigste van de zeven. Doel is niet het artikel, doel is dat de jurist het
ritme leert kennen voordat de moeilijke clusters komen.

Per output het protocol uit de skill:

- **A, vertellen** (ongeveer een minuut): wettekst-citaat, de formule, één zin
  context.
- **B, toetsen** (twee tot acht minuten): dekt de formule de wettekst, klopt de
  bronverwijzing, en ken je een casus die dit breekt?
- **C, fixeren** (een halve minuut): "Output X: bevestigd, één open casus. Door."

Die laatste stap wordt overgeslagen zodra het druk wordt, en dan eindig je met
notities zonder conclusies. Zeg de C-zin hardop.

### 0:25 tot 1:40 — de zes oordeelsclusters

Elk cluster tien minuten, timer aan. Bij verzanding na vijf minuten parkeren in
de open punten en door.

**Cluster 1. Termijnen en onderbrekingen.** De vijfjaarstermijn van ZW 29b bij
onderbroken dienstverbanden: begint die opnieuw, loopt hij door, of telt hij op?
De wettekst zegt "binnen vijf jaar na die dag in dienstbetrekking werkzaamheden
gaat verrichten" en beslist het niet. Daarnaast het vijfjaarsvenster en de
elf-wekenvoorwaarde van Wtl 2.1 lid 2, en de onderbrekingen binnen de periode van
Wtl 2.16. Dit cluster raakt drie wetten met hetzelfde patroon, dus één antwoord
levert hier drie beslissingen op.

**Cluster 7. Hangende wijzigingen.** Acht van de 25 gemodelleerde artikelen
dragen een wijziging zonder datum van inwerkingtreding (ZW 29b, Pwet 8a en 10,
Wtl 2.1, 2.6 en 2.14, WIA 43, Wfsv 38b), en de twee Wtl-artikelen hebben er een
met datum: 1 januari 2027. Vraag: modelleren we de geldende versie, of leggen we
de komende versie ernaast. Dit cluster kost vijf minuten en past in de ruimte
die S3 vrijspeelt.

**Cluster 2. Samenloop en cumulatie.** In juni is bevestigd dat no-riskpolis en
LKV altijd samengaan, en dat LKS of loondispensatie er bovenop komen wanneer de
loonwaardemeting daartoe aanleiding geeft. Wat nog openstaat is de tijdelijke
samenloop binnen de WIA, die toen naar een volgende iteratie is geschoven. Vraag
smal: welke combinaties sluiten elkaar uit, en op welke grond.

Zeg erbij dat een bevestiging hier wordt vastgelegd maar niet uitgevoerd: er is
geen mechanisme dat een stelselregel boven de losse regelingen uitdrukt. Anders
bevestigt hij iets in de veronderstelling dat het daarna in de applicatie
zichtbaar wordt.

**Cluster 3. Open normen met UWV-discretie.** "Structurele functionele
beperking" (WIA 35), "reëel uitzicht" (WW 76a en WIA), "duidelijk minder dan het
minimumloon" en "vermindering naar evenredigheid" (Wajong 2:20). Het model laat
deze open. De vraag aan de jurist is niet of dat mag, maar wat een uitvoerder
feitelijk invult en of dat in beleidsregels staat die wij kunnen harvesten. Het
corpus bevat geen enkele UWV-beleidsregel, dus de vraag is letterlijk: welke
regel of welk protocol is leidend, en waar staat die. Een vindplaats is hier een
betere uitkomst dan een oordeel.

**Cluster 4. Interpretatiekeuzes met geldgevolg.** Bij LKV met twee categorieën
passen wij "hoogste bedrag wint" toe; de MvT zwijgt daarover. De 50%-regeling van
Pwet 10d lid 5 in de eerste zes maanden zonder loonwaardevaststelling. De
evenredige vermindering onder de 36 uur en de afronding daarvan. Dit cluster
verandert bedragen op het scherm van een burger, dus hier hoort het oordeel
letterlijk genotuleerd.

**Cluster 5. Voorzieningen: wat valt onder WPA.** WIA 35 dekt ook vervoer,
intermediaire activiteiten en overige voorzieningen; wij hebben alleen
jobcoaching en werkplekaanpassing gemodelleerd. Wajong 2:20 lid 2 onderdelen a en
b zijn niet gemodelleerd. Vraag: hoort dat erbij in een regelhulp Financieel CV,
en zo ja, welke.

**Cluster 6. Wajong oud en nieuw.** Voor loondispensatie geldt het oude regime.
Geldt dat ook voor de doelgroepbepaling van de no-riskpolis, of daar beide? Eén
vraag, tien minuten, en het antwoord raakt twee wetten.

### 1:40 tot 1:52 — scope-besluiten

Kort en beslissend, geen discussie van tien minuten per punt. Vijf stellingen,
elk met een verwacht weerwoord dat je paraat hebt:

- **S1.** LIV blijft buiten de regelhulp, want afgeschaft per 2025. Weerwoord:
  "en het overgangsjaar?"
- **S2.** Wsw blijft buiten scope en wordt afgevangen met een parameter.
- **S3.** Vervallen als vraag. Het Reïntegratiebesluit (BWBR0019152) is
  ingewonnen en heeft een `implements`-blok naar WIA 35 en Wajong 2:22, met
  bijpassende `open_terms` aan beide kanten. Wat overblijft is een check van een
  halve minuut: is dat de juiste grondslag. Open vraag 9 in de README is
  verouderd.
- **S4.** De quotumformule Wfsv 38f blijft buiten scope; alleen 38b telt voor het
  doelgroepregister.
- **S5.** De doelgroepvaststelling banenafspraak wordt gelezen uit één bron, de
  kapstok Wfsv 38b, en niet per regeling opnieuw.

### 1:52 tot 2:00 — afronding

Drie dingen hardop, en opschrijven terwijl de jurist er nog zit:

1. Welke oordelen zijn vastgelegd, en waar ze in de YAML landen.
2. Welke punten open blijven, met per punt een eigenaar en wat er nodig is om
   hem te sluiten.
3. Wat de jurist zelf wil zien voordat hij zijn naam ergens onder zet. Die vraag
   levert meestal de scherpste eis van het hele uur op.

## Hoe een oordeel wordt vastgelegd

Er is nu geen veld dat zegt wie een untranslatable heeft geaccepteerd. Tot schema
v0.7.0 is de goedkoopste oplossing de conventie die in de Ziektewet al staat: een
regel in `reason`, met datum en bron.

```yaml
untranslatables:
  - construct: vijfjaarstermijn bij onderbroken dienstverbanden
    reason: |-
      [bestaande tekst]

      Juristvalidatie (18 september 2026, <naam>): de termijn loopt door vanaf
      de eerste dag van de eerste dienstbetrekking; een onderbreking start geen
      nieuwe termijn. Grondslag: <artikel of beleidsregel>.
    accepted: true
```

Drie uitkomsten, drie bestemmingen. **Bevestigd** gaat als bovenstaand in de
YAML. **Afgewezen** wordt een desk-taak: de formule klopt niet en wordt
aangepast, met een scenario dat de nieuwe lezing vastlegt. **Onbeslist** wordt een
open punt in `README.md` met eigenaar en de vraag die beantwoord moet worden, niet
als losse notitie in een verslag.

Na de sessie hoort bij elk bevestigd oordeel dat bedragen of rechten raakt een
scenario in `scenarios/*.feature`, zodat de lezing regressiebestendig is. Dat is
de kant van `regelrecht-scenario-traces`: assert de knopen op het kritieke pad,
niet alleen de eindwaarde, anders verdwijnt een ketenfout stil achter een correct
eindbedrag.

## Na de sessie

1. Oordelen verwerken in de YAML, per wet, met de conventie hierboven.
2. Afwijzingen als desk-taken oppakken, met scenario.
3. Intern verslag (eerlijk, voor het team) en extern verslag (hoogover, voor de
   opdrachtgever) apart houden; de skill is daar expliciet over.
4. De triage-labels uit bakje 1 en 2 doorzetten naar de RFC-031-migratie, zodat
   die niet opnieuw gedaan hoeft te worden.

## Risico's en back-pockets

| Signaal | Wat je zegt |
|---|---|
| De jurist voelt zich getoetst | "Wij hebben een voorstel, jij schiet het stuk. Dat is het doel." |
| Verzanding in één edge-case | "Na vijf minuten parkeren we dit als open punt en gaan we door." |
| Te abstract | Terug naar de casus: "Stel Koen, 24 uur, loonwaarde 60 procent. Wat gebeurt er dan?" |
| "Dit kan ik zo niet beoordelen" | "Wat zou je nodig hebben om het wel te kunnen? Dat is precies wat ik wil weten." |
| Het model blijkt ergens fout | Noteren, niet ter plekke repareren. Dat is deskwerk. |

Als het volledig stroef loopt: de wettekst zelf voorlezen en de casus dáár
doorheen vertellen, in plaats van door de YAML.

## Wat de sessie oplevert als het goed gaat

Twaalf tot vijftien vastgelegde oordelen met grondslag, vijf scope-besluiten, een
lijst open punten met eigenaren, en een sorteerslag over de 64 untranslatables
die de migratie naar `markings` voorbereidt. Wat het níét oplevert is een
gevalideerd corpus: 25 machine_readable-blokken over zeven wetten kosten meer dan
één sessie, en dat hoort ook in de verwachting naar de jurist toe.
