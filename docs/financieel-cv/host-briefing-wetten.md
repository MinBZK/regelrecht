# Host-briefing — de wetten achter het Financieel CV

Voor de facilitator, niet voor de deelnemers. Wat je **moet** kunnen dragen
tijdens de juristvalidatie: het stelsel, de acht regelingen, de vier
struikelpunten, en waar het model zelf toegeeft dat het de wet niet vangt.

> De jurist-versie is `pre-read-juristvalidatie.md`. Dit is jouw spiekbrief.

---

## 1. De kern in vijf zinnen

1. Het Financieel CV bundelt **instrumenten voor werkgevers/werknemers met een
   afstand tot de arbeidsmarkt** — verspreid over zes wetten plus de AWB.
2. Die instrumenten worden uitgevoerd door **drie verschillende organisaties**
   (UWV, Belastingdienst, gemeente), elk met eigen loket en termijn.
3. Precies dáárom bestaat de RVO-regelhulp: anders moet een werkgever bij elke
   organisatie apart aankloppen.
4. Wij hebben die regelingen **machine-leesbaar** gemaakt: per uitkomst een
   formule die terug te voeren is op een wetsartikel.
5. Donderdag toetst de jurist één ding: **klopt de formule met de wettekst?**

## 2. Het stelsel — wie doet wat

| Laag | Wat | Voorbeeld |
|---|---|---|
| **Wet** (formeel) | de inhoudelijke aanspraak | Ziektewet 29b, Pwet 10c/10d |
| **Lagere regelgeving** | AMvB / min. regeling / beleidsregel, opgeroepen via "bij ministeriële regeling…" | Reïntegratiebesluit, UWV-beleidsregels loondispensatie |
| **Uitvoering** | past toe op de casus, produceert een **beschikking** | UWV, Belastingdienst, gemeente |
| **Procesrecht** | AWB — motiveringsplicht + bezwaartermijn op **elke** beschikking | AWB 3:46, 6:7 |

**Wie voert uit (moet je uit je hoofd kennen):**
- **UWV** → no-riskpolis, jobcoaching, werkplekaanpassing, loondispensatie, proefplaatsing
- **Belastingdienst** → LKV en (voorheen) LIV, via de loonaangifte
- **Gemeente** (college B&W) → loonkostensubsidie, inclusief de loonwaardevaststelling

## 3. De acht regelingen — spiekkaarten

### NRP — No-riskpolis · Ziektewet 29b · UWV
Wordt de werknemer ziek, dan betaalt **UWV het ziekengeld** in plaats van de
werkgever. Neemt het financiële risico van ziekte weg — dé drempelverlager bij
het aannemen van iemand met een arbeidsbeperking.
- **Gate:** behoort de werknemer tot een doelgroep (lid 1: WIA/Wajong-routes; lid 2: banenafspraak; lid 4: voortgezet dienstverband na WIA-vaststelling).
- **Duur:** lid 1 = **5 jaar**; lid 2 (banenafspraak) = **onbeperkt** — in het model als `-1` (sentinel).
- **Valkuil:** bij overlap wint de langste termijn, dus lid 2 wint van lid 1.

### LKS — Loonkostensubsidie · Pwet 10c + 10d · Gemeente
De gemeente betaalt de werkgever **het verschil** tussen het minimumloon en wat
de werknemer feitelijk waard is. De werknemer krijgt gewoon **vol WML-loon**.
- **Formule:** (WML+vakantiebijslag) − (loonwaarde+VB), met een **maximum van 70%** van WML+VB.
- **Rekenvoorbeeld Koen:** 215.500 − 129.300 = 86.200 eurocent = **€862/maand** (max zou 150.850 zijn).
- **Gate:** Pwet-doelgroep én "kan het minimumloon niet verdienen".

### LDP — Loondispensatie · Wajong 2:20 · UWV
**Let op: dit is het spiegelbeeld van LKS.** Hier mag de werkgever *minder dan
het minimumloon* betalen; de werknemer houdt zijn Wajong-aanvulling.
- **Gate:** arbeidsprestatie "duidelijk minder dan" minimumloon-equivalent, ter beoordeling van UWV.
- **Lid 2:** een beding tot lagere beloning is **nietig** — tenzij UWV dispensatie verleent.
- ⚠️ **Dit verschil LKS↔LDP is het makkelijkst te verhaspelen.** Onthoud: *subsidie = gemeente vult aan; dispensatie = werkgever mag minder betalen.*

### LKV — Loonkostenvoordeel · Wtl 2.1 · Belastingdienst
Vast bedrag per verloond uur, automatisch via de loonaangifte. Vier categorieën:
| | Categorie | Bedrag | Max/jaar |
|---|---|---|---|
| a | oudere werknemer | €3,05/uur | €6.000 |
| b | arbeidsgehandicapte werknemer | €3,05/uur | €6.000 |
| c | **banenafspraak** | €1,01/uur | €2.000 |
| d | herplaatsen arbeidsgehandicapte | €3,05/uur | €6.000 |
- **Rekenvoorbeeld:** Koen (c) = 101 × 1664 uur = **€1.680,64**; Sadee (b) = 305 × 1664 = **€5.075,20**.
- **Art. 4.1 lid 3:** bij meerdere categorieën wint **het hoogste bedrag** — niet de volgorde.

### LIV — Lage-inkomensvoordeel · Wtl hfdst. 3 · **AFGESCHAFT**
Generieke subsidie op lage lonen (tot 120% WML), €0,49/uur, max €960.
**Per 1 januari 2025 afgeschaft** (Wet 36458, wegens beperkte effectiviteit).
- In de demo geeft de engine bewust een **"Output not found"-fout** — dat is het bewijs dat de regeling niet meer bestaat, geen storing.

### JC + WPA — Jobcoaching & werkplekaanpassing · Wet WIA 35 · UWV
Voorzieningen om werken mogelijk te maken: persoonlijke ondersteuning
(jobcoach, lid 2.d) en aanpassing van de werkplek (lid 2.c).
- **Gate lid 1:** structurele functionele beperking + arbeidsverhouding.
- **⚠️ Lid 4 = de uitsluitingen.** 4.a sluit **Wajong-gerechtigden** uit (die gaan via Wajong-eigen voorzieningen, art. 2:22 e.v.); 4.b sluit mensen uit voor wie het **college** al zorg draagt (Pwet 7.1.a).
- Niet-meeneembare werkplekaanpassingen voor de wérkgever lopen via art. 36 — **niet gemodelleerd**.

### PP — Proefplaatsing · WW 76a · UWV
Werken met behoud van uitkering om te kijken of het klikt. **Maximaal 6 maanden.**
- **Gate:** je moet een **WW-uitkering** hebben. Voor Pwet- en Wajong-mensen loopt re-integratie via een andere route.

### Doelgroepregister banenafspraak · Wfsv 38b · UWV
Geen "instrument" maar de **gedeelde grondslag**: zit iemand in het register?
- **Opname-gronden 38b.1 a–f:** Pwet-LKS-toeleiding, WSW-indicatie, Wajong-arbeidsondersteuning, AMvB-indicatie, eigen-verzoek-WML-route.
- Wordt **cross-law aangeroepen** door de Ziektewet (NRP lid 2.e) en de Wtl (LKV categorie c).
- **Uitsluiting:** beschut werk (Pwet 10b) hoort er *niet* in.

## 4. De twee casussen — je rode draad

Schema per persoon: `persona-koen.mmd` en `persona-sadee.mmd` (status → grondslag
→ uitkomst). Beide zijn **fictief**; de bedragen komen uit de draaiende engine.

### Koen — de Pwet/banenafspraak-route
42 jaar · Pwet-doelgroep met LKS · in het doelgroepregister banenafspraak ·
loonwaarde **60%** van WML · 1664 verloonde uren · logistiek MKB via de gemeente.

| Regeling | Uitkomst | Grondslag |
|---|---|---|
| NRP (Ziektewet 29b) | **Recht** · duur onbeperkt | lid 2.e — banenafspraak + LKS |
| LKS (Pwet 10c+10d) | **Recht** · €862 / maand | loonwaarde 60% van WML+VB |
| LKV (Wtl 2.1) | **Recht** · €1.680,64 / jaar | categorie c — banenafspraak |
| JC + WPA (WIA 35) | **Recht** | geen uitsluiting lid 4 |
| LDP (Wajong 2:20) | Geen recht | geen Wajong-status |
| PP (WW 76a) | Geen recht | geen WW-uitkering |
| LIV (Wtl h.3) | Bestaat niet meer | afgeschaft per 2025 |

### Sadee — de Wajong/banenafspraak-route
28 jaar · Wajong-uitkering · in het doelgroepregister banenafspraak ·
loonwaarde **70%** van WML · 1664 verloonde uren.

| Regeling | Uitkomst | Grondslag |
|---|---|---|
| NRP (Ziektewet 29b) | **Recht** · duur onbeperkt | lid 2.a (Wajong) + lid 2.e |
| LDP (Wajong 2:20) | **Recht** · loonbeding nietig | arbeidsprestatie < WML |
| LKV (Wtl 2.1) | **Recht** · €5.075,20 / jaar | categorie b — arbeidsgehandicapt (wint, art. 4.1 lid 3) |
| JC + WPA (WIA 35) | Geen recht | lid 4.a sluit Wajong uit → Wajong-eigen route |
| LKS (Pwet) | Geen recht | geen Pwet-doelgroep |
| PP (WW 76a) | Geen recht | geen WW-uitkering |
| LIV (Wtl h.3) | Bestaat niet meer | afgeschaft per 2025 |

### De contrasten — hier zit je verhaal
De twee zijn zo gekozen dat ze bij dezelfde instrumenten **tegengesteld** uitkomen:

| | Koen | Sadee | Waarom |
|---|---|---|---|
| **JC + WPA** | wél | **niet** | lid 4.a sluit Wajong uit → doorverwijzing, geen weigering |
| **LKV-bedrag** | €1.680,64 (cat. c) | **€5.075,20** (cat. b) | hoogste categorie wint (art. 4.1 lid 3) |
| **LKS vs LDP** | LKS (gemeente vult aan) | LDP (mag minder betalen) | subsidie ↔ dispensatie |
| **NRP** | via lid 2.e | via lid 2.a + 2.e | zelfde uitkomst, ándere grondslag |

Beiden komen op "onbeperkt" uit bij de NRP — maar langs een ander lid. Dat is
precies het soort routeringsvraag waar je de jurist op wilt hebben.

## 5. De sessie draaien — klikpad door de editor

Traject: **editor.regelrecht.rijks.app/trajecten/financieel-cv-0bc401e0**

### Vooraf
1. Controleer dat de traject-selector rechtsboven op **Financieel CV** staat.
2. Zet je kolommen (elke kolom heeft een eigen dropdown):

| Kolom 1 | Kolom 2 | Kolom 3 |
|---|---|---|
| **Tekst** | **Machine** | **Scenario's** |
| de wettekst | de formule | de casus + uitkomst |

Zo wijs je van links naar rechts: *wettekst → formule → uitkomst*. Dat is het
validatie-ritueel, visueel gemaakt.

### Je loopt wet-vóór-wet, niet bestand-voor-bestand

**Belangrijk:** de editor voert een scenario altijd uit tegen de wet die je
ópen hebt. Daarom staat elke persona als **één bestand per wet**, telkens met
dezelfde naam. Je opent dus een wet, kiest daar `financieel_cv_koen` (of
`_sadee`), en ziet Koens scenario(’s) voor díé wet.

Loop deze zes wetten in volgorde langs — voor **beide** persona's identiek:

| # | Open deze wet | Artikel | Scenario | Koen | Sadee |
|---|---|---|---|---|---|
| 1 | **Ziektewet** | `29b.1` | no-riskpolis — de blauwdruk | recht · onbeperkt | recht · onbeperkt |
| 2 | **Participatiewet** | `10c.1` | loonkostensubsidie | **€862 / maand** | géén recht |
| 3 | **Wajong** | `2:20.1` | loondispensatie | géén recht | recht · beding nietig |
| 4 | **Wet WIA** | `35.1` | jobcoaching + WPA | **wél** recht | **géén** recht (lid 4.a) |
| 5 | **Wtl** | `2.1` | LKV **+** LIV (2 scenario's) | €1.680,64 · LIV ⚠️ faalt | €5.075,20 · LIV ⚠️ faalt |
| 6 | **WW** | `76a.1` | proefplaatsing | géén recht | géén recht |

Zo land je bij elke stap op de wettekst én de formule van precies die wet —
dat is sterker dan alles vanuit één bestand tonen.

**Tip voor de vergelijking:** doe per wet eerst Koen, dan Sadee. Bij stap 4 en
5 zie je de contrasten dan direct naast elkaar.

### Per scenario — vier stappen
1. **Lees de titel voor** — dat is de menselijke claim.
2. **Toon de parameters** — de casus-feiten.
3. **Toon de uitkomst** + welk lid de grondslag is.
4. **Open de trace** → vraag: *"klopt dit met de wettekst?"*

### Drie dingen die je vóóraf moet zeggen
- ⚠️ **Het LIV-scenario faalt met opzet** (`Output not found`). Leg dat uit
  vóórdat je het toont, anders leest het als een storing.
- ⚠️ **Sadee krijgt géén jobcoaching via de WIA** — lid 4.a is een
  *doorverwijzing* naar Wajong 2:22, geen weigering.
- ⚠️ **Bedragen staan in eurocent**: `86200` = €862.

### Valkuilen in de navigatie
- **De logica hangt op het umbrella-artikel.** Klik je op `29b.2`, dan is het
  Machine-paneel leeg — de NRP-logica zit op **`29b.1`**. Zie §9.
- **Dezelfde bestandsnaam in elke wet is opzet.** `financieel_cv_koen` bestaat
  zes keer; elk bevat alleen de scenario's die op díé wet draaien.
- **Klik nooit op "Maak machine versie aan"** — dat schrijft een lege
  `machine_readable` naar de traject-branch.

### Als er iets niet laadt
- **Lang op "Scenario's laden"?** Normaal — zes wetten worden opgehaald.
- **Machine-paneel leeg?** Check of je op het juiste artikel staat (zie boven).
- **`Law not found`?** Harde reload (⌘⇧R); de traject-index moet de laatste
  commit oppikken.

## 6. De vier dingen die zeker langskomen

1. **LIV bestaat niet meer** (per 2025). Leg de fout uit *vóór* je 'm toont.
2. **Sadee krijgt géén jobcoaching via de WIA** — lid 4.a sluit Wajong uit. Dat
   voelt onrechtvaardig, maar het is een *doorverwijzing*, geen weigering: het
   loopt via Wajong-eigen voorzieningen (die wij niet gemodelleerd hebben).
3. **LKV bij twee categorieën:** wij passen "hoogste bedrag wint" toe. De MvT
   zwijgt hier expliciet over — dit is een **interpretatiekeuze**, geen wetstekst.
4. **Samenloop is niet gemodelleerd.** LKS↔LKV (Pwet 10d lid 9) en LIV↔LKV
   (Wtl 4.1.3) rekenen wij per regeling onafhankelijk uit. Wees hier eerlijk over.

## 7. Waar de jurist gaat duwen — onze eigen untranslatables

Het model markeert zelf wat het niet kan vangen. Dit is je eerlijkheids-troef:
*"dit hebben we bewust niet gemodelleerd, daar hebben we jou voor nodig."*

| Wet | Niet gemodelleerd |
|---|---|
| **Ziektewet** | vijfjaarstermijn bij onderbroken dienstverbanden; lid 2-duur als "onbeperkt"; doelgroepverklaring-procedure |
| **Wtl** | doelgroepverklaring binnen 3 maanden; 12-maanden-uitsluiting bij aanvang dienstbetrekking |
| **Pwet** | 50%-regeling eerste zes maanden (lid 5); evenredigheid bij < 36 uur (lid 4); jaarlijkse herziening (lid 7); EU-woonplaatsverplaatsing (lid 10) |
| **Wajong** | "duidelijk minder dan minimumloon"; "vermindering naar evenredigheid" (= UWV-beleidsregel) |
| **Wet WIA** | "structurele functionele beperking" (UWV-discretie); lid 4.b 2-jaars/LKS-toets; "in overwegende mate op individu afgestemd" |
| **WW** | onderbreking wegens ziekte; UWV-oordeel "reëel uitzicht" |
| **Wfsv** | AMvB-indicatie 38b.1.d; beoordelingsregels jonggehandicapt (38b.3); quotumformule 38f |

Plus in **alle** wetten: *samenloop met de andere regelingen* — stelselbreed niet gemodelleerd.

## 8. Jargon dat je niet mag verhaspelen

- **Loonwaarde** — wat iemand feitelijk produceert, als % van WML. Vastgesteld door de gemeente (LKS) of UWV.
- **WML + VB** — minimumloon inclusief vakantiebijslag; de rekenbasis voor LKS.
- **Eurocent** — *alle bedragen in het model staan in eurocenten.* `86200` = €862,00. Verhaspel dit niet op scherm.
- **Banenafspraak / doelgroepregister** — de landelijke afspraak om banen te creëren; het register (Wfsv 38b) bepaalt wie meetelt.
- **Beschikking** — het formele besluit; hangt AWB-rechten aan (motivering, bezwaar).
- **Untranslatable** — onze term voor "de wet zegt hier iets dat wij bewust niet in een formule vangen".
- **Open term / delegatie** — "bij ministeriële regeling…"; de wet verwijst door naar lagere regelgeving.

## 9. "Waarom zijn de meeste artikelen leeg?"

Dit komt gegarandeerd langs zodra iemand rondklikt. De harvester haalt de
**volledige wettekst** binnen; wij zetten `machine_readable` alleen op de
artikelen die **in scope** zijn:

| Wet | Artikelen | Met machine_readable |
|---|---:|---:|
| Ziektewet | 736 | 2 |
| Participatiewet | 987 | 2 |
| Wajong | 757 | 2 |
| Wet WIA | 744 | 2 |
| WW | 640 | 1 |
| Wfsv | 518 | 2 |
| Wtl | 124 | 1 |
| **Totaal** | **~4.500** | **12** |

**Het antwoord:** een lege Machine-tab is geen gat en geen bug. De corpus is
**compleet in tekst, bewust selectief in logica** — we maken alleen uitvoerbaar
wat het Financieel CV nodig heeft.

Concreet voorbeeld: in de Wfsv dragen alleen **38b.1** (doelgroepregister) en
**38f.1** (quotum) logica. Klik je op **artikel 38.1**, dan zie je een lege
Machine-tab — dat artikel gaat over *categorie werkgevers* voor de
premiedifferentiatie, en heeft niets met de banenafspraak te maken.

## 10. Back-pockets — als je het niet weet

- *"Dat weet ik niet — dat is precies waarom jij hier zit. Zullen we 'm als open punt noteren?"*
- *"De engine zegt X. Of dat juridisch klopt is jouw oordeel, niet het onze."*
- *"Dit hebben we als untranslatable gemarkeerd — we wisten dat dit interpretatie vergt."*
- Bij tijdgebrek: **NRP is de blauwdruk.** Als je die goed doorloopt, snapt de zaal de methode.
