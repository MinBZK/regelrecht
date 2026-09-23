# Vier-weg-classificatie van de 64 untranslatables

Elk van de 64 untranslatables van de zeven wetten krijgt precies een van de vier
labels uit
`.claude/skills/regelrecht-stelselanalyse/references/classification.md`. Het label
bepaalt waar de bevinding landt en welke actie volgt. De untranslatables die het
label *acceptabele untranslatable* dragen, krijgen daarnaast het subtype
**factual** of **judgment** uit
`.claude/skills/regelrecht-audit-products/references/method-glossary.md`. De
judgment-set wordt de beslispunten van de sessie; dat is de handoff die
`regelrecht-dossier/references/routing.md` voorschrijft.

Gemeten op 22 september 2026 tegen `traject/financieel-cv-validatie-df48ddd1`.

## De verdeling

| Label | Aantal | Waar het landt | Actie |
|---|---|---|---|
| Modellering-fout | 4 | `modellering-fixes-plan` | Wij fixen de YAML |
| Wetgevings-fout (kandidaat) | 4 | `wetgevingsfouten-analyse` | Label eerst bevestigen in de sessie |
| Engine-limitatie (kandidaat) | 17 | `engine-limitaties` | Bewijs-poort openen, zie hieronder |
| Acceptabele untranslatable | 39 | Gemarkeerd in de YAML | Geen actie |
| **Totaal** | **64** | | |

Van de 39 acceptabele untranslatables zijn er **22 factual**
(feitelijk vaststelbaar, kan alsnog een caller-parameter of gate worden) en
**17 judgment** (oordeel of prognose, blijft untranslatable zonder
waardeverlies). De judgment-set levert samen met de vier
wetgevings-fout-kandidaten vijftien beslispunten op.

## Twee poorten die nog niet open zijn

**De bewijs-poort op de engine-limitaties.** Het sjabloon `engine-limitaties`
eist dat een limitatie pas wordt opgenomen na een reproduceerbare engine-run die
het falen aantoont, met scenario en foutuitkomst in het veld Bewijs. Alle 17
items hieronder leunen op de `reason`-tekst, die beweert dat de operatieset het
niet aankan. Geen daarvan is met een run aangetoond. Tot dat gebeurt zijn het
open vragen en geen limitaties; het sjabloon noemt de omgekeerde aanname
uitdrukkelijk als veelgemaakte fout, en item #11 laat zien dat zij ook hier
voorkomt.

**Het gate-criterium van de validerende modus.** `routing.md` stelt dat de
resterende open punten *judgment* moeten zijn en niet *factual*. De vier
modellering-fouten moeten er dus voor donderdag uit.

## Modellering-fout

| # | Wet | Artikel | Constructie | Waarom dit label |
|---|---|---|---|---|
| 11 | Pwet | 10c | afronding van het evenredig verminderde subsidiebedrag | RFC-023 en RFC-024 over bedragen en afronding zijn geimplementeerd. De engine kan dit; onze YAML gebruikt het niet. |
| 33 | Wfsv | 38b | datum_opname_doelgroepregister als invoer (geen berekening) | De omschrijving noemt een LKV-periode van drie jaar. Wtl 2.12 kent die termijn niet meer. |
| 34 | Wfsv | 38b | chapeau-uitsluiting beschut werk (Pwet 10b lid 1) | De reason noemt zelf dat een `source`-aanroep naar Pwet 10b cleaner zou zijn. RFC-007 is aanvaard en Pwet levert de output al. |
| 64 | ZW | 29b | doelgroepverklaring-procedure — geen lagere regelgeving | Bevestigd dat er geen lagere regelgeving is. Een bevestigde leegte is geen untranslatable. |

## Wetgevings-fout (kandidaat)

`classification.md` noemt het subtielste onderscheid: dezelfde clausule is een
acceptabele untranslatable wanneer er een **kenbare beslisser** en een **toetsbaar
kader** zijn, en een wetgevings-fout wanneer beslisser, kader of grond ontbreekt.
Bij deze vier ontbreekt er een. Het label is een kandidaat: de sessie bevestigt
of weerlegt het, en pas daarna gaat het naar `wetgevingsfouten-analyse`.

| # | Wet | Artikel | Constructie | Wat ontbreekt | Beslispunt |
|---|---|---|---|---|---|
| 2 | Pwet | 8a | "evenwichtig over deze personen worden verdeeld" (lid 2 a) | Meetbare maatstaf en individueel rechtsmiddel | B12 |
| 10 | Pwet | 10c | "de in de sector gebruikelijke volledige dienstbetrekking" als plafond op de arbeidsduur (lid 4 zin 3) | Kenbare bron voor de norm | B13 |
| 15 | Pwet | 10da | "begeleiding op de werkplek" is niet gedefinieerd | Definitie en doorverwijzing, dus geen kader | B14 |
| 54 | WIA | 37 | oordeelvorming UWV of eigenrisicodrager ('reeel uitzicht') | Criterium wie van twee partijen oordeelt | B15 |

## Engine-limitatie (kandidaat)

| # | Wet | Artikel | Constructie |
|---|---|---|---|
| 5 | Pwet | 10 | tweejaarstermijn in de WIA-uitstroomcategorie (lid 1) |
| 9 | Pwet | 10c | 50%-regeling eerste zes maanden zonder loonwaardevaststelling (lid 5) |
| 12 | Pwet | 10c | jaarlijkse herziening loonkostensubsidie (lid 7) |
| 17 | WW | 76a | onderbreking wegens ziekte |
| 28 | Wajong | 2:24 | onderbreking wegens ziekte |
| 38 | Wfsv | 38f | beslissingstermijn als kalenderjaar t-1 ipv concrete dag-termijn |
| 40 | Wtl | 2.1 | aanvang dienstbetrekking-toets — 12-maanden uitsluiting |
| 42 | Wtl | 2.6 | zes-maandentoets van lid 1 onderdeel b |
| 43 | Wtl | 2.6 | vijfjaarsvenster en elf-wekenvoorwaarde van lid 2 |
| 44 | Wtl | 2.14 | onderbrekingen binnen de periode van artikel 2.16 |
| 46 | WIA | 23 | samentelling van perioden (lid 3, 4 en 5) |
| 50 | WIA | 35 | lid 4 onderdeel b — Pwet 7.1.a college-ondersteuning t/m 2 jaar minimumloon zonder LKS |
| 53 | WIA | 37 | onderbreking wegens ziekte |
| 55 | WIA | 43 | uitzondering "andere dienstbetrekking" in onderdeel b |
| 59 | ZW | 29b | "onmiddellijk voorafgaand aan de dienstbetrekking" (lid 1 onderdeel a) en "voorafgaand aan" (lid 2 onderdeel a) |
| 61 | ZW | 29b | ontstaansmoment van het recht op ziekengeld (lid 2, slot) |
| 62 | ZW | 29b | lid 2-duur als 'onbeperkt zolang dienstbetrekking voortduurt' |

Zeven ervan zijn dezelfde constructie in een andere wet: onderbreking wegens
ziekte staat in WW 76a, Wajong 2:24 en WIA 37; de tweejaarstermijn staat in Pwet
10 en WIA 35; de herhalingstoets over zes maanden staat in Wtl 2.1 en 2.6. Een
bewijs-run per constructie volstaat, niet per item.

## Acceptabele untranslatable — judgment

Deze zeventien zijn de beslispunten. `routing.md`: de judgment-set untranslatables
wordt de beslispunten van de workshop.

| # | Wet | Artikel | Constructie | Beslispunt |
|---|---|---|---|---|
| 4 | Pwet | 10 | "naar het oordeel van het college noodzakelijk geachte voorziening" | B9 |
| 14 | Pwet | 10c | samenloop met no-riskpolis en LKV | B1 |
| 18 | WW | 76a | oordeelvorming UWV ('reëel uitzicht') | B2 |
| 19 | Wajong | 2:15 | jonggehandicapte in de zin van artikel 2:3 | B6 |
| 21 | Wajong | 2:20 | duidelijk minder dan minimumloon-equivalent | B4 |
| 23 | Wajong | 2:20 | samenloop met no-riskpolis en LKV | B1 |
| 24 | Wajong | 2:22 | UWV "kan ... toekennen" — discretionaire bevoegdheid (lid 1) | B10 |
| 25 | Wajong | 2:22 | voorzieningen-criterium "in overwegende mate op het individu afgestemd" (lid 2.c) | B7 |
| 26 | Wajong | 2:22 | voorzieningen-criterium "noodzakelijke persoonlijke ondersteuning" (lid 2.d) | B8 |
| 29 | Wajong | 2:24 | oordeelvorming UWV ('reeel uitzicht') | B2 |
| 41 | Wtl | 2.1 | samenloop met no-riskpolis, LKS en loondispensatie | B1 |
| 45 | WIA | 4 | rechtstreeks en objectief medisch vast te stellen gevolg van ziekte of gebrek | B5 |
| 49 | WIA | 35 | "structurele functionele beperking" — UWV-discretie | B3 |
| 51 | WIA | 35 | voorzieningen-criterium "in overwegende mate op individu afgestemd" (lid 2.c) | B7 |
| 52 | WIA | 35 | noodzakelijkheid + compensatie voor beperkingen (lid 2.d) | B8 |
| 60 | ZW | 29b | vijfjaarstermijn bij onderbroken dienstverbanden | B11 |
| 63 | ZW | 29b | samenloop met LKV, LKS en loondispensatie (stelsel-eigenschap) | B1 |

## Acceptabele untranslatable — factual

Feitelijk vaststelbaar door een systeem, dus kan alsnog een caller-parameter of
gate worden. Geen actie voor de sessie, op de vijf scope-vragen na.

| # | Wet | Artikel | Constructie | Scope-vraag |
|---|---|---|---|---|
| 1 | Pwet | 8a | de verordening zelf | — |
| 3 | Pwet | 10 | "overeenkomstig de verordening, bedoeld in artikel 8a" | — |
| 6 | Pwet | 10 | onderzoeksplicht en besluitvorming (lid 4) | — |
| 7 | Pwet | 10b | UWV-advies en AMvB-criteria voor de vaststelling (lid 2 en 3) | — |
| 8 | Pwet | 10b | aantal te realiseren dienstbetrekkingen (lid 4 tot en met 6) | — |
| 13 | Pwet | 10c | EU-woonplaatsverplaatsing (lid 10) | — |
| 16 | Pwet | 10e | kan-bepaling zonder ingevulde AMvB | — |
| 20 | Wajong | 2:15 | ontstaan op grond van artikel 8:10 lid 4 (lid 5) | S5 |
| 22 | Wajong | 2:20 | vermindering naar evenredigheid | — |
| 27 | Wajong | 2:22 | onderdelen a en b van lid 2 (vervoer, intermediaire activiteiten) niet gemodelleerd | S4 |
| 30 | Wfsv | 38b | AMvB-indicatie (38b.1.d) — verwijst naar nog op te stellen indicatie | — |
| 31 | Wfsv | 38b | AMvB-beoordelingsregels jonggehandicapt (38b.3) | — |
| 32 | Wfsv | 38b | definitie verloonde uren (38b.4) — alleen begripsbepaling | — |
| 35 | Wfsv | 38f | berekeningsformule quotumpercentage (38f.2 — variabelen A t/m H) | S2 |
| 36 | Wfsv | 38f | AMvB-delegatie variabelen A-E en H (38f.3) | — |
| 37 | Wfsv | 38f | AMvB-delegatie variabelen F en G met voorhang (38f.4) | — |
| 39 | Wtl | 2.1 | doelgroepverklaring-vereiste binnen 3 maanden (art. 2.3, 2.6) | — |
| 47 | WIA | 23 | verkorte wachttijd op aanvraag (lid 6) | S3 |
| 48 | WIA | 23 | verlenging van de wachttijd (artikel 24, 25 en 26) | S3 |
| 56 | WIA | 47 | ingangsdatum van het recht (lid 2) | — |
| 57 | WIA | 54 | ingangsdatum van het recht (lid 2) | — |
| 58 | WIA | 54 | samenstelling van de WGA-uitkering (lid 3 en 4) | S1 |

## Beslispunten B1 tot en met B15

Werkvorm 1-2-4-all, maximaal drie minuten per punt, minderheidsstandpunt noteren
(`facilitation-patterns.md`). B12 tot en met B15 vragen om bevestiging van een
label en niet om een interpretatie.

| # | Punt | Achtergrond | Items |
|---|---|---|---|
| B1 | Klopt "hoogste bedrag wint" (Wtl 4.1 lid 3) als voorrangsregel, en cumuleren LKV, LKS, loondispensatie en no-riskpolis zoals in juni 2026 bevestigd? | Actieregister 4.4. De MvT zwijgt erover. | #14, #23, #41, #63 |
| B2 | Welke beleidsregel of welk protocol stuurt "reeel uitzicht op een dienstbetrekking"? | Gelijkluidend in WW 76a en Wajong 2:24. Zie ook B15. | #18, #29 |
| B3 | Welke vindplaats stuurt "structurele functionele beperking"? | De reason noemt het Schattingsbesluit; staat dat vast? | #49 |
| B4 | Welke vindplaats stuurt "duidelijk minder dan het minimumloon"? | Raakt de hoogte van de loondispensatie. | #21 |
| B5 | Welke vindplaats stuurt "rechtstreeks en objectief medisch vast te stellen gevolg"? | Verzekeringsgeneeskundig; bepaalt de ingang van de hele WIA-keten. | #45 |
| B6 | Hoe wordt "jonggehandicapte in de zin van artikel 2:3" in de praktijk vastgesteld? | Beoordeling over een tijdvak van 52 weken rond het zeventiende jaar. | #19 |
| B7 | Welke vindplaats stuurt "in overwegende mate op het individu afgestemd"? | Gelijkluidend in WIA 35 lid 2 c en Wajong 2:22 lid 2 c. Eén antwoord dekt beide. | #25, #51 |
| B8 | Welke vindplaats stuurt "noodzakelijk" en "compensatie voor de beperkingen"? | Gelijkluidend in WIA 35 lid 2 d en Wajong 2:22 lid 2 d. | #26, #52 |
| B9 | Is "naar het oordeel van het college noodzakelijk geachte voorziening" toetsbaar, en waaraan? | Pwet 10 lid 4 schrijft onderzoek voor zonder drempel. | #4 |
| B10 | Mag de uitkomst van een kan-bepaling als "komt in aanmerking" worden getoond, of moet het "is toegekend" zijn? | Raakt actieregister 2.6, twee sterktes van aanspraak in de presentatielaag. | #24 |
| B11 | Begint de vijfjaarstermijn van ZW 29b opnieuw bij een opvolgend dienstverband, loopt hij door, of telt hij op? | De reason zegt zelf: vereist juridische interpretatie. | #60 |
| B12 | Bevestig het label: is "evenwichtig over deze personen worden verdeeld" een wetgevings-fout? | Geen meetbare maatstaf en geen individueel toetsbaar gevolg. Toets: wie beslist, wanneer, op welke grond, met welk rechtsmiddel. | #2 |
| B13 | Bevestig het label: is "de in de sector gebruikelijke volledige dienstbetrekking" een wetgevings-fout? | De norm staat in geen enkele kenbare bron; de aanleveraar levert de reeds gecapte arbeidsduur. | #10 |
| B14 | Bevestig het label: is "begeleiding op de werkplek" zonder definitie en zonder doorverwijzing een wetgevings-fout? | Pwet 10da. De verhouding tot Pwet 10 lid 1 volgt uit de praktijk, niet uit de tekst. | #15 |
| B15 | Bevestig het label: is "naar het oordeel van het UWV of de eigenrisicodrager" zonder criterium wie oordeelt een wetgevings-fout? | WIA 37 lid 2 d. Beslisser niet kenbaar, dus geen toetsbaar kader. | #54 |

## Scope-beslispunten S1 tot en met S5

Uit de scope-analyse, dot-voting of korte ja/nee (`workshop-draaiboek`, deel 2).

| # | Vraag | Items |
|---|---|---|
| S1 | Hoort de hoogte van de uitkering bij de regelhulp? Geen van de zeven wetten levert vandaag een uitkeringsbedrag. | #58 |
| S2 | Hoort de quotumsystematiek van Wfsv 38f bij een regelhulp voor een burger? De formule werkt op sectorniveau. | #35 |
| S3 | Hoort de variatie in de wachttijd erbij: verkorting op aanvraag en verlenging via de artikelen 24, 25 en 26? | #47, #48 |
| S4 | Horen de overige Wajong-voorzieningen erbij: vervoer, intermediaire activiteiten en de leefomstandighedenvoorziening? | #27 |
| S5 | Hoort het Wajong-overgangsrecht erbij, de route via 8:10 lid 4? Raakt actieregister 1.3. | #20 |

## Een bevinding die vervalt

Bevinding 1 uit [`juristsessie-voorbereiding.md`](juristsessie-voorbereiding.md),
"de Wajong-artikelen dragen verkeerde nummers", houdt geen stand. De bestanden
dragen `number: 2:15`, `2:20`, `2:22` en `2:24`. De getallen 135, 140, 142 en 144
ontstaan bij het lezen met PyYAML, dat YAML 1.1 implementeert en `2:20` uitrekent
als 2 x 60 + 20. De engine leest met serde_yaml, dat YAML 1.2 implementeert, en
het schema eist `"type": "string"` voor `number`. Zie
[`pyyaml-valkuil.md`](pyyaml-valkuil.md), dat deze val op 9 september al
beschreef.

Het tweede deel, over de ankers, is een andere vraag. In het hele corpus draagt
geen enkel anker een dubbele punt: alle 472.245 ankers in de Wajong- en
Awb-bestanden zijn van de vorm `#Artikel220`. Dat is een corpusbrede conventie van
de harvester, geen Wajong-defect. Of wetten.overheid.nl die vorm accepteert is met
een netwerkcontrole vast te stellen; die kon hier niet worden gedaan.

## Volledige lijst

| # | Wet | Artikel | Label | Type | Punt | Constructie |
|---|---|---|---|---|---|---|
| 1 | Pwet | 8a | Acceptabele untranslatable | factual | — | de verordening zelf |
| 2 | Pwet | 8a | Wetgevings-fout (kandidaat) | — | B12 | "evenwichtig over deze personen worden verdeeld" (lid 2 a) |
| 3 | Pwet | 10 | Acceptabele untranslatable | factual | — | "overeenkomstig de verordening, bedoeld in artikel 8a" |
| 4 | Pwet | 10 | Acceptabele untranslatable | judgment | B9 | "naar het oordeel van het college noodzakelijk geachte voorziening" |
| 5 | Pwet | 10 | Engine-limitatie (kandidaat) | — | — | tweejaarstermijn in de WIA-uitstroomcategorie (lid 1) |
| 6 | Pwet | 10 | Acceptabele untranslatable | factual | — | onderzoeksplicht en besluitvorming (lid 4) |
| 7 | Pwet | 10b | Acceptabele untranslatable | factual | — | UWV-advies en AMvB-criteria voor de vaststelling (lid 2 en 3) |
| 8 | Pwet | 10b | Acceptabele untranslatable | factual | — | aantal te realiseren dienstbetrekkingen (lid 4 tot en met 6) |
| 9 | Pwet | 10c | Engine-limitatie (kandidaat) | — | — | 50%-regeling eerste zes maanden zonder loonwaardevaststelling (lid 5) |
| 10 | Pwet | 10c | Wetgevings-fout (kandidaat) | — | B13 | "de in de sector gebruikelijke volledige dienstbetrekking" als plafond op de arbeidsduur (lid 4 zin 3) |
| 11 | Pwet | 10c | Modellering-fout | — | — | afronding van het evenredig verminderde subsidiebedrag |
| 12 | Pwet | 10c | Engine-limitatie (kandidaat) | — | — | jaarlijkse herziening loonkostensubsidie (lid 7) |
| 13 | Pwet | 10c | Acceptabele untranslatable | factual | — | EU-woonplaatsverplaatsing (lid 10) |
| 14 | Pwet | 10c | Acceptabele untranslatable | judgment | B1 | samenloop met no-riskpolis en LKV |
| 15 | Pwet | 10da | Wetgevings-fout (kandidaat) | — | B14 | "begeleiding op de werkplek" is niet gedefinieerd |
| 16 | Pwet | 10e | Acceptabele untranslatable | factual | — | kan-bepaling zonder ingevulde AMvB |
| 17 | WW | 76a | Engine-limitatie (kandidaat) | — | — | onderbreking wegens ziekte |
| 18 | WW | 76a | Acceptabele untranslatable | judgment | B2 | oordeelvorming UWV ('reëel uitzicht') |
| 19 | Wajong | 2:15 | Acceptabele untranslatable | judgment | B6 | jonggehandicapte in de zin van artikel 2:3 |
| 20 | Wajong | 2:15 | Acceptabele untranslatable | factual | S5 | ontstaan op grond van artikel 8:10 lid 4 (lid 5) |
| 21 | Wajong | 2:20 | Acceptabele untranslatable | judgment | B4 | duidelijk minder dan minimumloon-equivalent |
| 22 | Wajong | 2:20 | Acceptabele untranslatable | factual | — | vermindering naar evenredigheid |
| 23 | Wajong | 2:20 | Acceptabele untranslatable | judgment | B1 | samenloop met no-riskpolis en LKV |
| 24 | Wajong | 2:22 | Acceptabele untranslatable | judgment | B10 | UWV "kan ... toekennen" — discretionaire bevoegdheid (lid 1) |
| 25 | Wajong | 2:22 | Acceptabele untranslatable | judgment | B7 | voorzieningen-criterium "in overwegende mate op het individu afgestemd" (lid 2.c) |
| 26 | Wajong | 2:22 | Acceptabele untranslatable | judgment | B8 | voorzieningen-criterium "noodzakelijke persoonlijke ondersteuning" (lid 2.d) |
| 27 | Wajong | 2:22 | Acceptabele untranslatable | factual | S4 | onderdelen a en b van lid 2 (vervoer, intermediaire activiteiten) niet gemodelleerd |
| 28 | Wajong | 2:24 | Engine-limitatie (kandidaat) | — | — | onderbreking wegens ziekte |
| 29 | Wajong | 2:24 | Acceptabele untranslatable | judgment | B2 | oordeelvorming UWV ('reeel uitzicht') |
| 30 | Wfsv | 38b | Acceptabele untranslatable | factual | — | AMvB-indicatie (38b.1.d) — verwijst naar nog op te stellen indicatie |
| 31 | Wfsv | 38b | Acceptabele untranslatable | factual | — | AMvB-beoordelingsregels jonggehandicapt (38b.3) |
| 32 | Wfsv | 38b | Acceptabele untranslatable | factual | — | definitie verloonde uren (38b.4) — alleen begripsbepaling |
| 33 | Wfsv | 38b | Modellering-fout | — | — | datum_opname_doelgroepregister als invoer (geen berekening) |
| 34 | Wfsv | 38b | Modellering-fout | — | — | chapeau-uitsluiting beschut werk (Pwet 10b lid 1) |
| 35 | Wfsv | 38f | Acceptabele untranslatable | factual | S2 | berekeningsformule quotumpercentage (38f.2 — variabelen A t/m H) |
| 36 | Wfsv | 38f | Acceptabele untranslatable | factual | — | AMvB-delegatie variabelen A-E en H (38f.3) |
| 37 | Wfsv | 38f | Acceptabele untranslatable | factual | — | AMvB-delegatie variabelen F en G met voorhang (38f.4) |
| 38 | Wfsv | 38f | Engine-limitatie (kandidaat) | — | — | beslissingstermijn als kalenderjaar t-1 ipv concrete dag-termijn |
| 39 | Wtl | 2.1 | Acceptabele untranslatable | factual | — | doelgroepverklaring-vereiste binnen 3 maanden (art. 2.3, 2.6) |
| 40 | Wtl | 2.1 | Engine-limitatie (kandidaat) | — | — | aanvang dienstbetrekking-toets — 12-maanden uitsluiting |
| 41 | Wtl | 2.1 | Acceptabele untranslatable | judgment | B1 | samenloop met no-riskpolis, LKS en loondispensatie |
| 42 | Wtl | 2.6 | Engine-limitatie (kandidaat) | — | — | zes-maandentoets van lid 1 onderdeel b |
| 43 | Wtl | 2.6 | Engine-limitatie (kandidaat) | — | — | vijfjaarsvenster en elf-wekenvoorwaarde van lid 2 |
| 44 | Wtl | 2.14 | Engine-limitatie (kandidaat) | — | — | onderbrekingen binnen de periode van artikel 2.16 |
| 45 | WIA | 4 | Acceptabele untranslatable | judgment | B5 | rechtstreeks en objectief medisch vast te stellen gevolg van ziekte of gebrek |
| 46 | WIA | 23 | Engine-limitatie (kandidaat) | — | — | samentelling van perioden (lid 3, 4 en 5) |
| 47 | WIA | 23 | Acceptabele untranslatable | factual | S3 | verkorte wachttijd op aanvraag (lid 6) |
| 48 | WIA | 23 | Acceptabele untranslatable | factual | S3 | verlenging van de wachttijd (artikel 24, 25 en 26) |
| 49 | WIA | 35 | Acceptabele untranslatable | judgment | B3 | "structurele functionele beperking" — UWV-discretie |
| 50 | WIA | 35 | Engine-limitatie (kandidaat) | — | — | lid 4 onderdeel b — Pwet 7.1.a college-ondersteuning t/m 2 jaar minimumloon zonder LKS |
| 51 | WIA | 35 | Acceptabele untranslatable | judgment | B7 | voorzieningen-criterium "in overwegende mate op individu afgestemd" (lid 2.c) |
| 52 | WIA | 35 | Acceptabele untranslatable | judgment | B8 | noodzakelijkheid + compensatie voor beperkingen (lid 2.d) |
| 53 | WIA | 37 | Engine-limitatie (kandidaat) | — | — | onderbreking wegens ziekte |
| 54 | WIA | 37 | Wetgevings-fout (kandidaat) | — | B15 | oordeelvorming UWV of eigenrisicodrager ('reeel uitzicht') |
| 55 | WIA | 43 | Engine-limitatie (kandidaat) | — | — | uitzondering "andere dienstbetrekking" in onderdeel b |
| 56 | WIA | 47 | Acceptabele untranslatable | factual | — | ingangsdatum van het recht (lid 2) |
| 57 | WIA | 54 | Acceptabele untranslatable | factual | — | ingangsdatum van het recht (lid 2) |
| 58 | WIA | 54 | Acceptabele untranslatable | factual | S1 | samenstelling van de WGA-uitkering (lid 3 en 4) |
| 59 | ZW | 29b | Engine-limitatie (kandidaat) | — | — | "onmiddellijk voorafgaand aan de dienstbetrekking" (lid 1 onderdeel a) en "voorafgaand aan" (lid 2 onderdeel a) |
| 60 | ZW | 29b | Acceptabele untranslatable | judgment | B11 | vijfjaarstermijn bij onderbroken dienstverbanden |
| 61 | ZW | 29b | Engine-limitatie (kandidaat) | — | — | ontstaansmoment van het recht op ziekengeld (lid 2, slot) |
| 62 | ZW | 29b | Engine-limitatie (kandidaat) | — | — | lid 2-duur als 'onbeperkt zolang dienstbetrekking voortduurt' |
| 63 | ZW | 29b | Acceptabele untranslatable | judgment | B1 | samenloop met LKV, LKS en loondispensatie (stelsel-eigenschap) |
| 64 | ZW | 29b | Modellering-fout | — | — | doelgroepverklaring-procedure — geen lagere regelgeving |

## Werkwijze

De 64 komen uit de `untranslatables`-blokken van de zeven wetten. Ze zijn gelezen
met een PyYAML-loader waarin de sexagesimale int- en float-resolvers van YAML 1.1
zijn uitgezet, zodat `number: 2:20` de string blijft. Het aantal en de verdeling
per wet komen overeen met de tabel in het draaiboek.

Het label volgt de beslisboom van `classification.md`: wijkt de YAML af van de
wettekst, dan modellering-fout; klopt de modellering maar is de wet zelf
onuitvoerbaar, dan wetgevings-fout; kloppen wet en modellering maar faalt de
engine, dan engine-limitatie; en anders acceptabele untranslatable, met de toets
op kenbare beslisser en toetsbaar kader. Het subtype factual of judgment volgt de
method-glossary. Waar een item twee kanten heeft, staat de reden voor de keuze in
de kolom ernaast.
