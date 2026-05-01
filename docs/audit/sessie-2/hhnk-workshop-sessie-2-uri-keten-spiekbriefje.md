# Sessie 2 — URI-keten-audit — facilitator-spiekbriefje

*Voor de vervolgsessie na 2026-04-23 (HHNK art 26). Focus: art 11 URI 1990 + cascade naar art 12/13/15/16 via execution-trace in de editor. 1 A4 print, of als losse Obsidian-tab naast het jargon-briefje.*

**Doel sessie**: deelnemers leren een execution-trace lezen, en de keten URI 1990 → URI 12/13 → URI 15/16 koppelen aan de officiële wettekst — zodat ze in vervolgsessies zelfstandig discussies kunnen voeren over waar de Leidraad-laag haakjes zou moeten vinden.

## Hoe we dit document gebruiken

Dit is **tegelijk facilitator-spiekbriefje en gedeeld werkdocument**. Tijdens de sessie staat het op het scherm; deelnemers kijken mee en typen antwoorden in de inbreng-zones (kantlijn van een vraag, of de "Opbrengst sessie 2"-sectie helemaal onderaan). Cursieve quotes (*"…"*) zijn zinnen die de facilitator hardop uitspreekt — voor deelnemers fungeren ze als ondertiteling van wat ze net hoorden.

Lees dit document nooit van begin tot eind voor; gebruik het als kaart waar je samen op staat.

---

## De editor-view in 3 panes

| Pane | Rol in sessie 2 |
|---|---|
| **Links — Tekst** | *Legitimatie-bron*. Hier wijs je de wet-zin aan vóór je naar rechts kijkt. Niet voortdurend — alleen op overgangsmomenten. |
| **Midden — Scenario's / Invoer** | *Aanpas-paneel*. Tijdens trace-walk vrijwel onaangeraakt. Pas op het einde gebruiken voor wat-als (één parameter, trace ziet 't direct). |
| **Rechts — Resultaat / Execution trace** | *Hoofdpodium.* 80% van de aandacht. |

**Pre-flight (5 min vóór sessie):**
- Editor open op `editor/uitvoeringsregeling_invorderingswet_1990/11`
- Hero-scenario "Gedeeltelijke kwijtschelding" auto-uitgevoerd, trace zichtbaar
- Resultaat-pane breder gesleept zodat trace niet rechts afkapt
- Tweede browser-tab klaar voor twee-scenario-diff (zie afsluiter)
- Decoder-tabel (hieronder) op A4 gedrukt voor elke deelnemer

---

## Visueel anker — lege formule op flap

Teken vóór de sessie deze formule groot op een flap of whiteboard. Tijdens de walk vul je 'm in drie momenten in. Dit is het sterkste mentale anker dat je kunt geven — deelnemers houden de hele cascade vast in één regel.

```
hoogte = MAX(0, aanslag − ___ − 0.8 × ___)
                          ↑          ↑
                         art 12     art 13 (← art 15 + art 16)
```

**Fill-moment 1 (begin walk):** zet de twee vraagtekens en de pijlen. Zeg: *"art 11 heeft twee gaten — vermogen en betalingscapaciteit. Die gaan we vullen."*

**Fill-moment 2 (na art 12-bezoek):** vul links het vermogen-getal in (uit de trace). *"Eerste gat dicht: vermogen = 0."*

**Fill-moment 3 (na art 13-cascade):** vul rechts het BC-getal in. *"Tweede gat dicht: BC = €463,80. De rest is rekenwerk."*

Als laatste rekent de groep zelf de uitkomst uit. Sluit af door 'm op de flap te omcirkelen en te vergelijken met de GESLAAGD-balkjes in de Resultaat-pane.

**Waarom dit werkt:** de hele wet-cascade (4 artikelen, ~20 trace-frames) collapst tot één regel waarvan je live ziet hoe de gaten gevuld worden. Mensen die de trace-detail niet konden volgen, kunnen alsnog de uitkomst-logica reconstrueren. De flap blijft de hele sessie hangen als referentie.

### Tweede regel op flap — de bovenste laag (zichtbaar maken wat sessie 2 niet doet)

Onder de URI-formule schrijf je een tweede regel, die expliciet maakt dat sessie 2 alleen de **hoogte-vraag** uitlegt en niet de **mag-vraag**:

```
mag_kwijtschelding = EN( hoogte > 0, NIET uitgesloten, scope_OK, ondernemer_OK )
                            ↑              ↑              ↑          ↑
                       URI art 11     HHNK 26.1.9    verord 1   verord 5
                       (sessie 2)     (sessie 1)     (sessie 3)  (sessie 3)
```

Eén zin bij het tekenen: *"sessie 2 zit aan de linkerkant van die EN. Sessie 1 hebben jullie de 7 + 4 uitsluitingsgronden gevalideerd — dat is de tweede pijl. De URI-trace die we straks lopen zegt niets over die andere pijlen, dus mag-de-burger-überhaupt blijft buiten beeld vandaag."*

Dit voorkomt dat deelnemers denken dat de URI-keten "het hele kwijtschelding-besluit" maakt. Hij maakt alleen de hoogte; HHNK Leidraad maakt het besluit.

— Inbreng / vragen vanuit de groep:
-

---

## Decoder: trace-output → URI artikel

De trace toont **output-namen, geen artikelnummers**. De engine routeert op output-naam (uniek per wet); voor jullie zijn artikelen het mentale anker. Vandaar deze decoder.

| Trace-naam | Komt uit |
|---|---|
| `hoogte_kwijtschelding` | **art 11** |
| `aanwendbare_betalingscapaciteit` | **art 11** |
| `kan_kwijtschelding_worden_verleend` | **art 11** |
| `vermogen_bedrag` | **art 12** |
| `betalingscapaciteit` | **art 13** |
| `gemiddelde_uitgaven_b_c_g_maand` | **art 15** |
| `kostennorm_bedrag` / `kostennorm_basis` | **art 16** |
| (overige a/d/e/f/h kosten-componenten) | art 15 |

**Facilitator-discipline**: noem **altijd hardop het artikelnummer** zodra een `Resolving #X`-regel langskomt. *"`#BETALINGSCAPACITEIT` — dat is artikel 13."* Na 3–4 keer leggen deelnemers de mapping zelf.

---

## Trace-markers — wat zegt elke regel

| Wat je ziet | Zeg in de zaal |
|---|---|
| `Evaluating rules for X` | "engine stapt nu in [artikel volgens decoder]" |
| `Resolving $X#Y` | "engine vraagt **Y** op uit **X** — sprong naar [artikel]" |
| `Resolving from RESOLVED_INPUT: N` | "die waarde **N** komt van het sourced artikel" |
| `Resolving from PARAMETERS: $X = N` | "die waarde **N** is meegegeven door de caller" |
| `Delegation: Open term 'X' using default value: Y` | "**hier had een lagere regeling iets kunnen invullen — leeg, dus federale default**" |
| `Compute OP(...) = N` | "berekening, resultaat **N**" |
| `IF(took default) / CASE 0: False / DEFAULT: ...` | "vertakking, deze case past niet, fallback gebruikt" |
| `Untranslatable: "..."` | "engine geeft toe: dit stuk modelleren we bewust niet" |

---

## Drie ankerpunten in een typische URI 1990-trace

**Anker 1 — `Untranslatable: "Onverminderd het bepaalde in artikel 8 en artikel 18"`**
Open hier hard. Wijs naar de Tekst-pane (slotzin van art 11). *"Engine leest de wet net zo letterlijk als jullie. Hier zegt-ie: dit kan ik niet rekenen. Die juridische open ruimte blijft van jullie."* Sterke binnenkomer voor juristen.

*Brug naar sessie 1.* Direct na de openingszin, terugkoppelen: *"weet je nog, gisteren hebben we de 7 + 4 uitsluitingsgronden van HHNK 26.1.9 gevalideerd. URI 1990 heeft op deze positie alleen 'art 8 en art 18' — verder niets. HHNK heeft de positie ingevuld; dat is exact de bovenliggende laag waar we gisteren mee bezig waren."* Een minuut, daarna door. Niet-deelnemers van sessie 1 begrijpen 't ook: er is een wet-laag boven URI die uitsluitingen regelt.

— Inbreng / vragen vanuit de groep:
-

**Anker 2 — `Delegation: Open term 'kostennorm_percentage' using default value: 0.9`**
**Goud-moment voor Leidraad-discussie.** Pauzeer, zeg langzaam: *"op exact deze regel zou de HHNK Leidraad iets moeten doen. Hij doet niets. Engine valt terug op federale 0.9. Wie van jullie kan vertellen welke regel uit de Leidraad hier zou moeten landen?"* — daarna 30 sec stilte. Dit moment koppelt de driehoek (URI / medeoverheden / Leidraad) aan een zichtbare trace-regel.

**Anker 3 — IF/CASE-cascade onder `kostennorm_basis`**
De `CASE 0: False / CASE 1: False / DEFAULT: 140150` ladder is waar **huishoudtype** zich vertaalt in een normbedrag. Wijs naar de Invoer-pane: *"huishoudtype = alleenstaand triggert deze hele ladder. CASE 0 = ander geval? Past niet. Default = standaard-norm 140150."* Maakt zichtbaar dat juridische kwalificaties → cijfers worden via expliciete regels.

— Inbreng / vragen vanuit de groep:
-

---

## Mini-oefening — WSNP-burger en de twee lagen

Pak een gevalideerde grond uit sessie 1 (bv. **g₇ — WSNP**, gemapt vanuit 26.2.17). Stel de groep deze vraag:

> *"Als g₇ = true voor deze burger (zit in WSNP), op welk moment in de URI 1990-trace wordt dat zichtbaar?"*

**Antwoord (dit is de leereffect-clou):** nergens. URI 1990 weet niets van WSNP. De gate landt pas op de **bovenliggende HHNK-output**. De URI-trace zou gewoon doorrekenen tot een hoogte > 0; de HHNK-laag overrulet die uitkomst met `uitgesloten_van_kwijtschelding = true`.

**Inzicht voor deelnemers:** de URI-keten kan een succesvolle uitkomst geven die HHNK alsnog weigert. Twee lagen, twee logicas. Sessie 1 heeft de bovenste laag al gevalideerd; sessie 2 ontleedt de onderste.

Voor wie wil doordenken: hetzelfde geldt voor de 4 untranslatables uit 26.1.9 die menselijk oordeel vergen — die hebben in URI 1990 een **eigen broer** in de Untranslatable-regel van art 11 (Anker 1). Het mechanisme "wet erkent eigen grenzen" leeft op beide lagen, alleen verschilt de uitwerking.

— Inbreng / vragen vanuit de groep:
-

---

## Sprint-indeling van de trace-walk

Splits in drie sprintjes, elk afsluiten met één pauze-vraag.

**Sprint A — Entry tot eerste compute**
Van "Evaluating rules for uitvoeringsregeling_invorderingswet_1990" tot `Compute MULTIPLY(...) = 126135`.
Vraag: *"die 126135 — parameter, source, of berekening? Wie wijst aan in welke regel het antwoord zit?"*

**Sprint B — IF/CASE-cascade huishoudtype**
Van `IF(took default) = 140150` doorlopen tot CASEs uitgepakt zijn.
Vraag: *"welk parameter-woord uit de midden-pane stuurt dit hele blok?"*

**Sprint C — Terug naar art 11 actions**
Trace leidt terug naar `aanwendbare_betalingscapaciteit`, `hoogte_kwijtschelding`, `kan_kwijtschelding_worden_verleend`. Wijs naar de drie GESLAAGD-balkjes.
Vraag: *"hoogte = 10000. Welke wet-zin links rechtvaardigt precies 10000 en niet minder?"* (Antwoord: onderdeel **a** — geen vermogen, geen BC, dus aanslag blijft staan.)

---

## Hulpstrategieën

**1. Voorspel-vóór-klik**
Vóór elke resolve: *"wat gaat de engine nu nodig hebben, en uit welk artikel?"* Pas dán klik je het frame open. Maakt fout-voorspellingen tot leerzame momenten.

**2. Twee-vingers-regel**
Eén vinger op de Tekst-pane, één op de trace. Nooit tegelijk verplaatsen. Hardop: *"in de wet staat hier 'X' — dat is dit frame in de trace."* Dit is wat juristen vasthoudt.

**3. Hero-scenario kiezen**
Niet alle drie achter elkaar lopen. Pak alleen **"Gedeeltelijke kwijtschelding"** als hoofd-walk — daar gebeurt rekenwerk dat niet triviaal naar 0 collapst. Bewaar "Volledige" en "Geen" voor de twee-scenario-diff afsluiter.

**4. Diepte-bypass uit de mouw**
Mensen verliezen de weg na 4 niveaus. Heb een vooraf-besloten zin klaar: *"hier boomt het nog 2 niveaus door — voor straks. Voor nu nemen we dit getal als gegeven."* Bewust scope-houden.

**5. Eén pin-down vraag per frame**
Bij parameter-aankomst: *"waar komt deze waarde vandaan, scenario-input of hoger artikel?"* Bij `source:`-sprong: *"welke wet-zin rechtvaardigt deze sprong?"* Bij action: *"als dit getal anders was, welk eindresultaat zou veranderen?"* Eén vraag per frame houdt het tempo strak.

**6. Tab-switch tussen artikelen**
Houd een "Artikel 12" tab open naast de "Artikel 11" tab. Bij Anker 1 of een source-resolve: klik naar art 12, laat zien hoe die zelf source heeft naar art 15, klik terug. 10 sec uitstap, maakt de keten lichamelijk.

---

## Afsluiter: twee-scenario-diff

De **scherpste** uitleg van de keten. Twee browser-tabs naast elkaar:
- Tab A: "Volledige kwijtschelding" (NBI=0)
- Tab B: "Gedeeltelijke kwijtschelding" (NBI=1300)

Vraag aan de groep: *"welk frame is in beide identiek, en welk frame divergeert?"* Divergentie zit precies waar `betalingscapaciteit` berekend wordt — dat is het concrete moment waarop de wet "iets doet" voor deze burger.

Of single-tab variant: pak `netto_besteedbaar_inkomen_maand` in de midden-pane, zet 'em van 0 → 1300, wacht 300ms. Wijs in de trace aan dat `Resolving from RESOLVED_INPUT: 0` nu een ander getal terugkomt. Live-voorspellingsmoment vóór de wijziging: *"gaat hoogte stijgen, dalen, of nul worden?"*

---

## Anti-patterns

- ❌ Niet alle 20+ frames langs lopen — kies 5 hero-frames vooraf.
- ❌ Niet de YAML zij-aan-zij projecteren met de trace — splitst de zaal in wel/niet YAML-lezers. Houd YAML in pop-up voor specifieke vragen.
- ❌ Niet improviseren op `open_terms` / `implements:` resolution in deze sessie — dat is sessie 3. Anders kakelen mensen door elkaar over juridische delegatie en YAML-mechaniek tegelijk.

---

## Eén regel als vangnet

Als je tijdens de walk merkt dat je de zaal kwijt bent:

> *"De trace is de wet, in volgorde gelezen door een machine — elke inspring is dezelfde sprong die jullie als jurist ook maken, alleen expliciet."*

Herijkt het mentale model. Daarna terug naar het frame waar je was.

---

## Bekende gap (post-sessie issue)

Trace toont geen artikelnummers — vandaar de decoder-tabel hierboven. Klein engine-fix mogelijk: ~10 regels Rust in `packages/engine/src/service.rs:592` (Article-node-naam) + `packages/engine/src/trace.rs:325` (renderer). Logisch follow-up issue voor regelrecht-mvp na de sessie.

---

## Opbrengst sessie 2 — vul samen in vóór afronding

Vier velden om plenair te vullen. Dit is wat sessie 3 (Leidraad-koppeling via `implements:`) als input gebruikt.

**1. Decoder-validatie**
- Welke trace-regels werden meteen begrepen?
  -
- Welke regels gaven verwarring?
  -

**2. Leidraad-aanvliegplekken**
- Op welk(e) trace-moment(en) miste je de Leidraad-laag?
  -
- Welke open-term riep de meeste discussie op?
  -

**3. Twee-lagen-logica (WSNP-oefening)**
- Was de twee-lagen-uitleg (URI rekent hoogte / HHNK bepaalt mag) meteen helder, of zorgde de WSNP-oefening voor "aha"-momenten?
  -
- Voor welke uitsluitingsgronden uit sessie 1 is de URI-laag-blindheid problematisch?
  -

**4. Parking lot — kwesties voor sessie 3 of later**
-
-
-

---

## Voor de facilitator na afloop (~15 min thuis)

- Inbreng-zones overlopen, ruwe punten ordenen
- Lijstje "verwarrende trace-regels" doorgeven aan engine-team (input voor de bekende-gap-fix uit deze doc)
- Sessie 3-agenda concretiseren met de Leidraad-aanvliegplekken uit veld 2
- Iemand uit sessie 1 die er vandaag níet bij was: stuur ze de WSNP-vraag + antwoord apart, zodat ze voorbereid sessie 3 in komen
