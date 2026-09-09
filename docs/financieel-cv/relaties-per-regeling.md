# Relaties per regeling — welk RFC-mechanisme, welk veldtype, en waarom

Inventarisatie van de **relatiemechanismen** die het Financieel CV gebruikt:
welke RFC eraan ten grondslag ligt, met welke veldtypen het in de YAML staat,
en waarom juist dat mechanisme voor die regeling nodig was.

**Peildatum:** 2026-07-01 · **Schema:** v0.5.4 · **Branch:**
`traject/financieel-cv-validatie-df48ddd1` · **Opgemaakt:** 8 september 2026

> Let op: de Wtl staat op `2026-01-01` (er is geen latere versie) en het
> Reïntegratiebesluit op `2026-05-02`. De overige zes wetten op `2026-07-01`.

---

## 1. De mechanismen

Regelrecht kent zeven manieren waarop een artikel zich tot iets buiten zichzelf
verhoudt. Niet elk mechanisme is een "relatie tussen wetten" — de eerste twee
zijn dat niet, maar ze bepalen wel wat er van buiten moet komen.

| # | Mechanisme | Veld in de YAML | RFC | Waar het voor is |
|---|---|---|---|---|
| 1 | **Parameter** | `execution.parameters[]` | RFC-001 | Een gegeven dat de engine niet kan afleiden en dat van buiten komt: UWV, gemeente, werkgever, burger |
| 2 | **Output** | `execution.output[]` | RFC-001 | Wat het artikel oplevert. Elke output is aanroepbaar van buiten |
| 3 | **Cross-law input** | `execution.input[].source.regulation` | RFC-007 *(deels geïmplementeerd)* | Artikel A vraagt een uitkomst op bij wet B en geeft daarbij parameters mee. Voor als een voorwaarde in A juridisch elders is gedefinieerd |
| 4 | **Delegatie (IoC)** | `open_terms[]` ↔ `implements[]` | RFC-003 | "Bij AMvB / bij ministeriële regeling". De hogere regeling declareert een open term; de lagere meldt zich aan als invuller |
| 5 | **Rechtskarakter** | `execution.produces.legal_character` | RFC-001 | Wat het artikel juridisch voortbrengt: BESCHIKKING, TOETS, BESLUIT_VAN_ALGEMENE_STREKKING. Tevens het filter waarop Awb-hooks vuren (RFC-007, RFC-008) |
| 6 | **Besluittype** | `execution.produces.decision_type` | RFC-001 | De uitkomst-soort naast het karakter: TOEKENNING, GEEN_BESLUIT, ALGEMEEN_VERBINDEND_VOORSCHRIFT |
| 7 | **Markering** | `untranslatables[]` | RFC-012 | Wettekst die niet getrouw te berekenen is. `accepted: true` = engine rekent door, mens beoordeelt |

Daarnaast draagt elke actie een `legal_basis` voor herleidbaarheid (RFC-013) en
elk artikel `references` naar de vindplaatsen in de wettekst.

### Twee dingen die het dossier níét gebruikt

- **Bevoegdheid (RFC-002).** `competent_authority` komt in geen van de zeven
  wetten voor. De RFC is Accepted en Implemented, maar het veld is in dit
  dossier nooit ingevuld — wie het artikel uitvoert (UWV, college, minister)
  staat dus nergens machineleesbaar, terwijl dat juist bij de gemeentelijke
  route het onderscheidende punt is. Openstaand punt.
- **RFC-031 (markeringen en open normen)** is **Draft / Not implemented**. Dat
  nummer is gereserveerd voor een toekomstige samenvoeging van vier velden tot
  twee. De velden die wij gebruiken zijn nog die van RFC-012. Reken er dus niet
  op dat de huidige structuur blijft.

### Wat het verschil is tussen cross-law en delegatie

Beide koppelen twee regelingen, maar in tegengestelde richting en om een
andere reden.

- **Cross-law** is een *vraag omhoog of opzij*, op uitvoeringsniveau: de
  Ziektewet wil weten of iemand in het doelgroepregister staat, dus roept hij
  Wfsv 38b aan. De aanroeper kent de aangeroepene bij naam.
- **Delegatie** is een *aanbod van onderaf*: de wet zegt "bij AMvB worden
  nadere regels gesteld" en kent die AMvB níét bij naam. De AMvB meldt zichzelf
  aan met `implements`. Zo blijft de wet leesbaar zonder dat hij hoeft te weten
  welk besluit hem invult.

Dat onderscheid is de reden dat je ze niet door elkaar kunt gebruiken: een
open term met een vaste verwijzing zou de wet laten breken zodra de AMvB
wordt vervangen.

---

## 2. Per regeling

### NRP — no-riskpolis · Ziektewet art. 29b

| | |
|---|---|
| Outputs | 13 |
| Parameters | 59 |
| Cross-law inputs | **8**, naar 4 wetten |
| Open terms | 0 |
| Markeringen | 6 |
| Rechtskarakter | BESCHIKKING |

**Mechanisme: cross-law (RFC-007), en verreweg het zwaarst van alle zeven.**

| Vraagt op bij | Output |
|---|---|
| Wet WIA | `heeft_recht_op_iva_uitkering` |
| Wet WIA | `heeft_recht_op_wga_uitkering` |
| Wet WIA | `arbeidsongeschiktheidspercentage` |
| Wet WIA | `wachttijd_einddatum` |
| Wajong | `heeft_recht_op_arbeidsondersteuning` |
| Wfsv | `behoort_tot_doelgroepregister_banenafspraak` |
| Participatiewet | `heeft_recht_op_lks` |
| Participatiewet | `verricht_arbeid_in_beschut_werk` |

**Waarom cross-law en niet gewoon parameters.** Artikel 29b somt in lid 1 en 2
doelgroepen op die het zelf niet definieert — het verwijst naar de WIA, de
Wajong en de Participatiewet. Zou je die als boolean-parameter aannemen, dan
zou de vraag "is deze persoon WIA-gerechtigd?" tweemaal in het stelsel worden
beantwoord, met kans op uiteenlopende uitkomsten. Nu is er één plaats waar dat
antwoord vandaan komt, en verandert er iets in de WIA, dan werkt dat vanzelf
door in de no-riskpolis.

Dat is ook het risico ervan: één verkeerd gelezen zin bij de bron werkt door in
alles wat eraan hangt. Zie de fout in Wfsv 38b uit
[ronde 3](szw/2026-09-08-juristfeedback-ronde3.md).

De 59 parameters zijn grotendeels de doorgeefparameters voor die acht
aanroepen: elke aangeroepen wet wil zijn eigen invoer.

---

### LKV — loonkostenvoordeel · Wtl art. 2.1, 2.6, 2.14

| Artikel | Outputs | Params | Cross-law | Intra-law | Markeringen |
|---|---|---|---|---|---|
| 2.1 | 6 | 28 | **1** | 2 | 3 |
| 2.6 | 4 | 8 | 0 | 0 | 2 |
| 2.14 | 3 | 8 | 0 | 0 | 1 |

Rechtskarakter: BESCHIKKING · Eenheid: `eurocent` (RFC-023)

**Mechanisme: cross-law (1×) plus intra-law (2×).**

De enige cross-law is dezelfde kapstok als bij de NRP: Wfsv
`behoort_tot_doelgroepregister_banenafspraak`. Dat is bewust — LKV-categorie
banenafspraak en de no-riskpolis hangen aan dezelfde doelgroepstatus, en die
hoort uit één bron te komen.

De twee intra-law inputs van artikel 2.1 zijn `is_arbeidsgehandicapte_werknemer`
en `is_herplaatsen_arbeidsgehandicapte`. Let op de vorm: de `source` noemt wél de
regeling maar **geen artikel**, dus de engine resolvet die outputs op wetniveau.
Artikel 2.1 kiest de categorie terwijl de voorwaarden per categorie elders in de
Wtl staan; verwijzen in plaats van kopiëren houdt die voorwaarden op één plek.

**Waarom `eurocent` en geen euro's.** RFC-023: bedragen in de kleinste eenheid,
zodat er geen drijvende komma aan te pas komt. Bij een tegemoetkoming per
verloond uur zou afronding in euro's over een jaar zichtbaar gaan schelen.

---

### LKS — loonkostensubsidie · Participatiewet art. 10c (met 10d)

| | |
|---|---|
| Outputs | 5 |
| Parameters | 9 |
| Cross-law | 0 |
| Open terms | **1** |
| Markeringen | 6 — samen met Ziektewet 29b het hoogste per artikel |
| Rechtskarakter | BESCHIKKING · eenheid `eurocent` |

**Mechanisme: delegatie (RFC-003), en veel markeringen.**

De open term is de vergoeding voor werkgeverslasten, waarnaar art. 10d lid 4
doorverwijst met "bij ministeriële regeling vastgesteld". Wij weten nog niet
wélke regeling dat is — die vraag staat uit bij UWV (actie 1.5). Juist daarom
is een open term het goede mechanisme: de wet kan volledig gemodelleerd blijven
terwijl de invuller nog ontbreekt, en zodra de regeling wordt geharvest meldt
die zich met `implements` aan zonder dat de Pwet hoeft te veranderen.

Zes markeringen is veel, en dat is niet toevallig: de LKS rekent met loonwaarde,
deeltijdfactor en een 70%-maximum, waarbij de wet op meerdere plaatsen naar
gemeentelijke of ministeriële invulling wijst.

---

### Gemeentelijke route · Participatiewet art. 8a, 10, 10b, 10da, 10e

Toegevoegd in ronde 2. Deze vijf artikelen zijn samen het interessantste geval
in het dossier, omdat ze **elk rechtskarakter en elk delegatieniveau** laten
zien dat het stelsel kent.

| Artikel | Open terms | Rechtskarakter | Bijzonderheid |
|---|---|---|---|
| 8a — verordeningsplicht + proefplaatsing | 3 | BESCHIKKING | eenheid `months` |
| 10 — aanspraak ondersteuning | 2 | BESCHIKKING | verordeningsvoorbehoud |
| 10b — beschut werk | 0 | BESCHIKKING | |
| 10da — begeleiding op de werkplek | 0 | **TOETS** | harde aanspraak |
| 10e — nadere regels | **4** | *geen* `execution` | alleen open terms |

**Waarom 10da TOETS is en 10 BESCHIKKING.** Artikel 10da stelt vast *dat* een
aanspraak bestaat, zonder dat er een besluit aan te pas komt: één zin, geen
voorbehoud, geen delegatie. Dat is een toets, geen beschikking. Artikel 10 lid 1
geeft de aanspraak "overeenkomstig de verordening" — daar volgt een besluit van
het college op. Dat verschil in `legal_character` is precies het onderscheid dat
de presentatielaag moet tonen (actie 2.6).

**Waarom 10e géén `execution` heeft.** Vier AMvB-grondslagen, alle
kan-bepalingen. Zolang de AMvB er niet is, verandert 10e niets aan de aanspraak
van art. 10 lid 1. Een `execution` toevoegen zou suggereren dat er iets te
berekenen valt. Vier open terms zonder execution zegt precies wat er staat: hier
kán iets ingevuld worden, en dat is nog niet gebeurd.

---

### LDP — loondispensatie · Wajong art. 2:20

| | |
|---|---|
| Open terms | 1 |
| Cross-law | 0 |
| Rechtskarakter | BESCHIKKING |

> **Let op bij het lezen van dit corpus met Python.** De Wajong en de Awb
> nummeren hun artikelen met een dubbele punt (`2:20`, `3:46`). In YAML 1.1 —
> wat PyYAML gebruikt — is `2:20` een getal in grondtal 60 en wordt het stil
> `140`. De bestanden zijn correct en `serde_yaml` (YAML 1.2) leest ze goed,
> dus de engine ook. Wie met PyYAML analyseert krijgt nummers die niet bestaan,
> en botsingen: `220` zou dan zowel 3:40 als een echt artikel aanwijzen. Haal
> `number` uit het `#ArtikelNNN`-anker of gebruik een YAML 1.2-lezer.

---

### JC / WPA — jobcoaching en werkplekaanpassing · Wet WIA art. 35

| | |
|---|---|
| Outputs | 4 |
| Parameters | 8 |
| Open terms | 1 (`nadere_regels_voorzieningen_artikel_35`) |
| Markeringen | 4 |
| Rechtskarakter | BESCHIKKING |

**Mechanisme: delegatie (RFC-003), en dit is de enige plek in het dossier waar
de IoC-koppeling daadwerkelijk rondloopt.** Het Reïntegratiebesluit
(`amvb/reintegratiebesluit`, art. `1a`) declareert `implements` op
`(wet_werk_en_inkomen_naar_arbeidsvermogen, 35, nadere_regels_voorzieningen_artikel_35)`
en vult die term dus in.

Dezelfde regeling heeft een Wajong-tegenhanger (art. 2:22) met een eigen open
term, en die koppeling werkt óók — zie bevinding 1 voor de eerder gemelde fout
die geen fout bleek.

---

### PP — proefplaatsing · vier wetten

Het enige onderwerp dat in vier wetten tegelijk is gemodelleerd, met per wet
een eigen kader.

| Wet | Artikel | Open terms | Eenheid | Duur |
|---|---|---|---|---|
| WW | 76a | 1 | `months` | 6 |
| Wet WIA | 37 | 1 | `months` | 6 |
| Wajong | 2:24 | 1 | `months` | 6 |
| Participatiewet | 8a lid 2 d | (binnen de 3 van 8a) | `months` | 2 + max. 4 |

**Waarom vier keer apart en niet één gedeelde definitie via cross-law.** Dat was
een reële optie: de voorwaarden van WW 76a lid 3, WIA 37 lid 2 en Wajong 2:24
lid 3 zijn woordelijk gelijk. Toch is er bewust niet één wet aangewezen als
bron. Cross-law drukt een juridische afhankelijkheid uit, en die is er hier
niet: de wetgever heeft de bepaling in elke wet zelfstandig opgenomen. Zou de
WW zijn artikel wijzigen, dan verandert er niets aan de Wajong. Een cross-law
verwijzing zou een afhankelijkheid *verzinnen* die de wet niet kent — en zou de
Pwet-variant, die op elk punt afwijkt, alsnog buiten de deur zetten.

De duur staat als constante in `months` (RFC-023) zodat het verschil toetsbaar
is in plaats van alleen beschreven.

---

### DGR — doelgroepregister banenafspraak · Wfsv art. 38b en 38f

| Artikel | Outputs | Params | Markeringen | Rechtskarakter |
|---|---|---|---|---|
| 38b | 13 | 15 | 5 | **TOETS** |
| 38f | — | — | — | **BESLUIT_VAN_ALGEMENE_STREKKING** |

**Dit artikel is de kapstok van het hele dossier.** Het heeft zelf geen enkele
cross-law input, maar wordt door twee wetten aangeroepen (Ziektewet 29b en Wtl
2.1). Dat is precies de bedoeling: één plaats waar wordt vastgesteld of iemand
arbeidsbeperkte is, en twee regelingen die dat overnemen.

**Waarom TOETS en geen BESCHIKKING.** 38b stelt vast of iemand tot de doelgroep
behoort. Dat is een kwalificatie, geen besluit met rechtsgevolg voor de burger —
het besluit valt pas in de regeling die de status gebruikt. Vandaar TOETS.

38f is een `BESLUIT_VAN_ALGEMENE_STREKKING`: een besluit dat niet op één
persoon ziet. Het Reïntegratiebesluit draagt hetzelfde karakter; samen zijn dat
de enige twee in het dossier.

---

## 3. Wat de inventarisatie aan het licht bracht

Twee dingen die niet uit de scenario's blijken, omdat geen scenario ze afdekt.

### Bevinding 1 — ingetrokken

Hier stond dat het Reïntegratiebesluit `implements` declareert op Wajong-artikel
`2:22` terwijl dat artikel `142` zou heten, en dat de koppeling daardoor stil
doodloopt.

**Dat klopt niet.** Het artikel heet gewoon `2:22`. De `142` was een artefact van
de analyse: die gebruikte PyYAML, en YAML 1.1 leest `2:22` als het sexagesimale
getal 142. De engine gebruikt `serde_yaml` (YAML 1.2), ziet de string `2:22`, en
de sleutel `(law_id, article, open_term_id)` matcht dus wél.

Aangetoond met een kopie van het Wajong-bestand waarin één `number: 3:40` is
vervangen door de echte integer `340`: het origineel valideert, de kopie faalt
met `340 is not of type "string"`. Het bestand op schijf draagt dus een string.

Wat overblijft is een gereedschapsles, geen corpusfout: **analyseer dit corpus
niet met PyYAML.** Zie het kader bij de loondispensatie hierboven.

### Bevinding 2 — er is niets dat op BESCHIKKING vuurt

Zeventien van de 26 gemodelleerde FCV-artikelen declareren
`produces.legal_character: BESCHIKKING` (verder 2× TOETS, 2×
BESLUIT_VAN_ALGEMENE_STREKKING en 5 artikelen zonder karakter). Dat veld is volgens RFC-001 en RFC-008
het filter waarop Awb-hooks vuren: art. 3:46 motiveringsplicht, 6:7
bezwaartermijn.

Op deze branch bevat de Awb 565 artikelen en **nul** `machine_readable`. De
artikelen 3:46, 6:7 en 6:8 bestaan er wel degelijk — ze dragen alleen geen
executielogica. Er is dus niets dat luistert.

Dat is op zichzelf geen fout — de annotatie is correct en toekomstvast. Maar het
overzichtsdiagram in [README.md](README.md) toont de Awb-hooks als bestaande
verbindingen, en dat klopt op deze branch niet meer. Dat diagram stamt van de
demo-branch, waar de Awb wél gemodelleerd was.

---

## 4. Wat er nog nodig is om de keten compleet te maken

Zestien open terms in dit dossier wijzen naar een regeling die de uitkomst
invult, en één daarvan is aangesloten. Daarnaast staat een aantal wetten alleen
als parameter in het model. Hieronder wat er nodig is, op volgorde van gewicht
voor deze casus.

**De belangrijkste bevinding vooraf: het is geen harvest-probleem.** Alle wetten
hieronder staan al als tekst in de corpus. Wat ontbreekt is `machine_readable`.
Twee uitzonderingen: ministeriële regelingen en gemeentelijke verordeningen
bestaan als categorie helemaal niet — `regulation/nl/` kent alleen `wet`,
`amvb`, `beleidsregel` en één waterschapsverordening.

| # | Regeling | Status in de corpus | Waarom het knelt |
|---|---|---|---|
| 1 | **Wet sociale werkvoorziening** | 17 versies, **0 gemodelleerd** | Wordt in zes van de zeven wetten als parameter afgevangen — 17 parameters in totaal (`is_wsw_werknemer`, `is_wsw_geindiceerd_of_oude_indicatie`, `is_wsw_of_beschut_werk_dienstbetrekking`). Wie die invult bepaalt de uitkomst van NRP lid 2, LKS, LDP én JC/WPA, en niets controleert het |
| 2 | **Wet minimumloon en minimumvakantiebijslag** | 57 versies, **0 gemodelleerd** | De loonkostensubsidie rekent tegen het minimumloon: 41 verwijzingen in de Participatiewet alleen. Het bedrag komt nu als parameter binnen, dus de kern van de berekening leunt op een aangeleverd getal |
| 3 | **Algemene wet bestuursrecht** | 177 versies, **0 gemodelleerd** | Zeventien artikelen declareren `BESCHIKKING` als hook-trigger. Er luistert niets. Zonder de Awb ontbreekt de hele procedurele laag: motivering (3:46), bezwaartermijn (6:7), bekendmaking (6:8) |
| 4 | **Besluit loonkostensubsidie Participatiewet** | 3 versies, **0 gemodelleerd** | De open term `regels_doelgroep_lks_en_loonwaarde_amvb` bij Pwet 10e noemt dit besluit al bij naam in zijn default. Aansluiten via `implements` is klein werk met direct effect op de LKS |
| 5 | **Ministeriële regeling werkgeverslasten** | **categorie bestaat niet** | Pwet 10c delegeert `werkgeverslastenvergoeding_eurocent` naar de minister. Wij weten nog niet wélke regeling dat is — actie 1.5, uitgezet bij UWV. Zolang dat open staat kan het LKS-bedrag afwijken, zowel de subsidie als het 70%-maximum |
| 6 | **Gemeentelijke verordeningen** | **categorie bestaat niet** | Vier open terms delegeren naar de gemeenteraad (Pwet 8a drie, Pwet 10 één). Zonder verordening blijft de gemeentelijke route "de route bestaat", nooit een bedrag. Dat is scopevraag 2.7, geen modelleervraag |
| 7 | **UWV-beleidsregel dispensatiepercentage** | `beleidsregel/` bestaat (35 stuks), deze niet | Wajong 2:20 delegeert het percentage van de loondispensatie naar UWV. Zonder die regel zegt het model dát er dispensatie is, niet hoeveel |
| 8 | **Ministeriële regelingen proefplaatsing** | **categorie bestaat niet** | Drie open terms, één per wet (WW 76a, WIA 37, Wajong 2:24), over de uitvoering. Raakt de duur niet — die staat in de wet — dus lager in de lijst |
| 9 | **AMvB persoonlijke ondersteuning** | Pwet 10e, nog niet vastgesteld | Drie van de vier open terms bij 10e wachten op een AMvB die er niet is. Zolang die er niet is verandert 10e niets aan de aanspraak van art. 10 lid 1 |
| 10 | **Wet SUWI** | 60 versies, **0 gemodelleerd** | Alleen genoemd in Wfsv 38b lid 1 onderdeel g, voor een experimentbepaling. Raakt onze twee persona's niet |

### Wat al wél is aangesloten

Het **Reïntegratiebesluit** (`amvb/reintegratiebesluit`, art. 1a) vult met
`implements` twee open terms: `nadere_regels_voorzieningen_artikel_35` bij de
Wet WIA en `nadere_regels_voorzieningen_artikel_2_22` bij de Wajong. Dat is de
enige werkende IoC-koppeling in het dossier, en meteen het model voor de rest.

### Wat dit betekent voor de volgorde

Nummer 1 tot en met 3 zijn geen invulling van een open term maar een gat in de
keten: de wet wordt aangeroepen als feit terwijl er een regeling achter zit die
het feit zou moeten bepalen. Dat is een ander soort werk dan 4 tot en met 9,
waar de aanhechting al klaarligt en alleen de invuller ontbreekt.

Het **Dagloonbesluit werknemersverzekeringen** staat er ook (4 versies, niet
gemodelleerd). Het valt buiten deze lijst omdat de no-riskpolis alleen het
*recht* modelleert en niet de hoogte van het ziekengeld; zodra die hoogte in
scope komt, schuift het besluit naar boven.

---

## 5. Samenvattend

Markeringen zijn hier per artikel geteld en opgeteld over de artikelen die de
regeling beslaat.

| Regeling | Artikelen | Cross-law | Delegatie | Markeringen | Karakter |
|---|---|---|---|---|---|
| NRP | ZW 29b | **8** | — | 6 | BESCHIKKING |
| LKV | Wtl 2.1, 2.6, 2.14 | 1 (+2 intra) | — | 6 | BESCHIKKING |
| LKS | Pwet 10c | — | 1 | 6 | BESCHIKKING |
| Gemeentelijke route | Pwet 8a, 10, 10b, 10da, 10e | — | **9** ⚠ | 10 | BESCHIKKING + TOETS |
| LDP | Wajong 2:20 | — | 1 | 3 | BESCHIKKING |
| JC / WPA | WIA 35 | — | 1 ✅ gekoppeld | 4 | BESCHIKKING |
| PP (4 wetten) | WW 76a, WIA 37, Wajong 2:24, Pwet 8a | — | 3 (+1 in 8a) | 8 | BESCHIKKING |
| DGR | Wfsv 38b, 38f | — | — | 9 | TOETS + BvAS |
| Reïntegratiebesluit | art. 1a | — | 2 (1 werkt) | 0 | BESLUIT VAN ALGEMENE STREKKING |

De Pwet-artikelen komen in twee rijen terug: 10c hoort bij de LKS, 8a hoort bij
zowel de gemeentelijke route als de proefplaatsing. Bij elkaar opgeteld draagt
de Participatiewet 16 markeringen, de meeste van alle wetten in het dossier.

⚠ De negen bij de gemeentelijke route zijn **gedeclareerde** open terms, geen
werkende koppelingen: er is geen enkele AMvB die zich met `implements` op een van
de negen heeft aangemeld. Alleen bij JC/WPA loopt een IoC-koppeling echt rond.
Tel open terms dus niet als relaties — het zijn aanhechtpunten die nog leeg zijn.

Het patroon: **cross-law concentreert zich bij de no-riskpolis**, omdat dat de
enige regeling is die haar doelgroep volledig elders laat definiëren.
**Delegatie concentreert zich bij de gemeentelijke route**, omdat de wetgever
daar de invulling bij de gemeenteraad en de regering heeft gelaten. Beide zijn
geen modelleerkeuze maar een afspiegeling van hoe de wet is opgebouwd — en dat
is precies wat je van een getrouwe modellering mag verwachten.

---

*Gegenereerd uit de corpus op `traject/financieel-cv-validatie-df48ddd1`,
peildatum 2026-07-01. Visuele weergave als artifact; zie het actieregister voor
de openstaande punten.*
