# Classificatie — buckets, afwijkingsklassen, bindendheid, documentstatus

Elk statement krijgt precies één bucket. Het label bepaalt de actie, en een verkeerd label
kost een ronde: een jurist-vraag die als "fix" in een fixes-plan belandt, wordt uitgevoerd
zonder dat iemand hem heeft beslist.

## De buckets

Overgenomen uit `law-letter-fidelity-audit`, zodat een bevinding uit een beleidsdocument en
een bevinding uit een wetsartikel in dezelfde taal landen.

| Bucket | Betekenis | Actie |
|---|---|---|
| **MODELFOUT** | het model wijkt af van de letter van de norm | fix richting de letter |
| **WETTEKST-GEVOLG** | het model volgt de letter getrouw, maar de letter geeft een vreemde of onbedoelde uitkomst | rapporteren, **niet** fixen |
| **LETTER-vs-TOELICHTING** | de secundaire tekst zegt iets dat de norm niet zegt | jurist beslist wat leidend is + wetgevings-signaal |
| **letter-getrouw** ✅ | statement en model dekken de norm | geen wijziging |
| **scope** | het statement gaat over iets dat bewust niet gemodelleerd is | beslisvraag, geen fix |

**De scherpste grens is MODELFOUT ↔ LETTER-vs-TOELICHTING.** Beide zien eruit als "het model
klopt niet met wat hier staat". Het onderscheid is de bron van de eis:

> Staat de eis in de **norm** en niet in het model → MODELFOUT.
> Staat de eis in de **toelichting** en niet in de norm → LETTER-vs-TOELICHTING, en het
> model is *goed zoals het is*.

Die tweede is contra-intuïtief en wordt daarom stelselmatig verkeerd geklasseerd: het lijkt
alsof je een gat dicht, terwijl je de toelichting boven de wet zet. De verankeringszoektocht
(`verankering.md`) bestaat om precies deze twee uit elkaar te houden — zonder die zoektocht
kún je het onderscheid niet maken, en dan wordt alles een "fix".

## Afwijkingsklassen

De klasse beschrijft *hoe* het statement en de norm uiteenlopen; de bucket beschrijft *wat
je ermee doet*.

| Klasse | Symptoom |
|---|---|
| `toelichting-bleed` | een criterium dat alleen in de secundaire tekst staat; de norm stelt het niet of alleen als open norm |
| `ontbrekend-bestanddeel` | de norm bevat een kwalificatie ("uitsluitend", "tijdelijk", een "tenzij") die het statement of het model laat vallen |
| `verkeerde-verankering` | het statement is opgehangen aan een kapstok-artikel terwijl de operatieve norm elders staat |
| `herformulering` | het statement zegt hetzelfde als de norm in andere woorden (spiegelzijde, samenvatting) |
| `bindendheid-vervlakking` | een `soft-default` uit de tekst is als harde regel overgenomen (of omgekeerd) |
| `buitenwettelijk` | de secundaire tekst is strenger of ruimer dan de norm toestaat |
| `geen` | geen afwijking |

`buitenwettelijk` is de klasse die je nooit stil mag laten passeren. Beleid dat de wet
inperkt of oprekt is geen modelleerdetail maar een bevinding voor het stelsel; hij landt in
`regelrecht-stelselanalyse` als wetgevings-/beleidssignaal.

## Bindendheid

De as die wetteksten niet nodig hebben. Uitvoeringsbeleid mengt harde regels met vuistregels
in dezelfde alinea, en het verschil verdwijnt zodra je het modelleert.

| Waarde | Herkenning | Gevolg voor modellering |
|---|---|---|
| `hard` | onvoorwaardelijk geformuleerd | mag als regel |
| `soft-default` | "in de regel", "in beginsel", "doorgaans", "als hoofdregel" | weerlegbare hoofdregel: modelleer mét de afwijkgrond, of markeer als untranslatable |
| `guidance` | "wij adviseren", "het verdient aanbeveling", werkadvies aan de behandelaar | geen regel |
| `informative` | uitleg, servicetekst, voorbeeld | geen regel |

*"In de regel ten hoogste 1.200 euro"* als hard maximum modelleren neemt de
afwijkbevoegdheid weg die de tekst juist geeft. Dat is een fideliteitsfout in het nadeel van
de burger, en hij is onzichtbaar in tests: de gemiddelde casus komt er niet aan.

## Documentstatus

Bepaal in fase 0, geldt voor het hele document, en overschrijft de bindendheid van elk
statement erin.

| Status | Juridisch | Praktisch |
|---|---|---|
| `beleidsregel` | Awb 4:81: bindt het bestuursorgaan; afwijken vergt motivering (4:84) | statements zijn norm-achtig |
| `verordening/regeling` | zelfstandige norm | dit is geen secundaire tekst — gebruik de gewone harvest-route |
| `toelichting` | geen norm | statements zijn uitleg |
| `werkinstructie` / `handboek` | formeel geen norm; feitelijk sturend, en via het vertrouwens- en gelijkheidsbeginsel alsnog relevant | statements zijn praktijk-bewijs, geen norm |
| `faq` / `voorlichting` | geen norm | statements zijn uitleg, vaak vereenvoudigd |

Zoek expliciet naar de disclaimerzin. Eén regel als *"Deze toelichting is informatief, u kunt
er geen rechten aan ontlenen"* bepaalt de status van elk statement in het document en hoort
als eerste statement in het register, ook al is hij zelf `informative`.

## Van register naar vervolg

- **MODELFOUT** → modellering-fixes-plan (`regelrecht-stelselanalyse` klasse 1).
- **LETTER-vs-TOELICHTING** → jurist-vraag én wetgevings-signaal: als het beleid bindend is,
  hoort het in de regeling, niet in de toelichting. Dat is de aanbeveling — niet een
  modelwijziging.
- **WETTEKST-GEVOLG** → wetgevings-fouten-analyse (klasse 2).
- **buitenwettelijk** → beleid-vs-wet-conflict; behandel als klasse 2 met de aantekening dat
  de conflictbron het beleid is, niet de wet.
- **scope** → beslisvraag voor de workshop (`regelrecht-audit-products`).

Een statement gaat nooit rechtstreeks naar `machine_readable`. Als een `niet-gevonden`
statement volgens de jurist bindend beleid is, is de uitkomst een wijziging van de
*regeling*, en pas daarna van het model.
