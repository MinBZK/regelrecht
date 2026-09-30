# Financieel CV

Dossier bij het modelleren van de regelingen voor werkgevers en werknemers met
een arbeidsbeperking, zoals de regelhulp Financieel CV ze bundelt: no-riskpolis,
loonkostenvoordeel, loonkostensubsidie, loondispensatie, jobcoaching en
werkplekaanpassing, proefplaatsing en beschut werk.

Het doel van de huidige fase is vast te stellen welke normen onder de bedragen
een kenbare vindplaats hebben, zodat de bedragen berekenbaar worden. Het model
bepaalt wie recht heeft; de hoogte hangt nog op een aantal open normen.

## Stand op 30 september 2026

- De wetten staan in **regelrecht-corpus** op de trajectbranch
  `traject/financieel-cv-validatie-df48ddd1`, op schema v0.7.0. Dit dossier bevat
  de analyse en het sessiemateriaal, niet de wetten zelf.
- De letter-fideliteitsaudit van 23 september vond ruim zestig afwijkingen
  tussen model en wettekst. Ongeveer 35 zijn gerepareerd; zie
  [`fideliteitsreparaties.md`](fideliteitsreparaties.md).
- Juristfeedback ronde 4 (24 september) is vastgelegd maar nog niet in het model
  verwerkt. Alle 23 open normen zijn behandeld; zie
  [`szw/2026-09-24-juristfeedback-ronde4.md`](szw/2026-09-24-juristfeedback-ronde4.md).
- Wat er openstaat, met eigenaar en vindplaats, staat in
  [`szw/actieregister.md`](szw/actieregister.md).

## Indeling

| Map of bestand | Wat |
|---|---|
| [`szw/`](szw/) | Juristfeedback: letterlijk ([`ruwe-feedback.md`](szw/ruwe-feedback.md)), uitgewerkt per ronde, en het actieregister |
| [`classificatie-untranslatables.md`](classificatie-untranslatables.md) | De 64 untranslatables vier-weg geclassificeerd |
| [`schema-migratie.md`](schema-migratie.md) | Van v0.5.4 naar v0.7.0: markings, open termen, en wat uit de YAML ging |
| [`fideliteitsaudit.md`](fideliteitsaudit.md), [`fideliteitsreparaties.md`](fideliteitsreparaties.md) | De audit tegen de letter van de wet, en wat daarvan is hersteld |
| [`modellering-fixes-plan.md`](modellering-fixes-plan.md), [`wetgevingsfouten-analyse.md`](wetgevingsfouten-analyse.md) | Modelleerfouten met fix, en kandidaten voor een wetgevingsfout |
| [`gegevensherkomst.md`](gegevensherkomst.md) | Alle invoergegevens naar herkomst |
| [`doelgroepregister-categorieen.md`](doelgroepregister-categorieen.md), [`beslisboom-doelgroepregister.md`](beslisboom-doelgroepregister.md) | De gronden van Wfsv 38b, de rechten per grond en de hoogte |
| [`relaties-per-regeling.md`](relaties-per-regeling.md) | Welk relatiemechanisme elke regeling gebruikt |
| [`diepte-van-een-variabele.md`](diepte-van-een-variabele.md), [`wat-diepte-vraagt.md`](wat-diepte-vraagt.md) | Eén open norm acht niveaus diep uitgetrokken, en wat nodig is om zo'n keten uit te voeren |
| [`stelsel-ontbrekende-regelingen.mmd`](stelsel-ontbrekende-regelingen.mmd) | Welke regelingen nog nodig zijn om de keten te sluiten |
| [`walkthrough-juristsessie.html`](walkthrough-juristsessie.html) | Het sessie-instrument: Koen en Sadee in acht haltes, met plakvlak en trace per halte |
| [`inwinlijst.html`](inwinlijst.html) | Welke lagere regelingen erbij moeten, en in welke volgorde |
| [`doorloop/`](doorloop/) | Generator voor het doorloop-artefact uit de traces |
| [`mvt-referenties.md`](mvt-referenties.md), [`scope-bepaling.md`](scope-bepaling.md), [`dataminimalisatie.md`](dataminimalisatie.md) | Achtergrond: memorie van toelichting, scope, gegevensminimalisatie |
| [`pyyaml-valkuil.md`](pyyaml-valkuil.md), [`ankers-naar-wetten-overheid.md`](ankers-naar-wetten-overheid.md) | Bevindingen over het gereedschap |
| [`drift-project-brief.md`](drift-project-brief.md), [`deployed-self-serve-rollout.md`](deployed-self-serve-rollout.md) | Plannen: herbouw op de 2026-teksten, en de editor voor jurist en beleidsmedewerkers |
| [`archief/`](archief/) | Materiaal van de kick-off (april) en de sessies van juli en 24 september, waaronder het oorspronkelijke projectoverzicht |

## Werkwijze

1. Modelleren in regelrecht-corpus, op de trajectbranch.
2. Controleren met drie poorten: `script/validate.sh`, `script/cross-law-integriteit.py`
   en de BDD-suite met de scenario's van Koen en Sadee.
3. Traces genereren (`TRACE=1`, `REGULATION_PATH` naar de trajectcorpus) en in de
   walk-through of het doorloop-artefact tonen.
4. Voorleggen aan de jurist, per halte, met stickies in drie kleuren: geel voor een
   vindplaats, roze voor een oordeel per geval, blauw voor twijfel.
5. Feedback eerst letterlijk vastleggen in `szw/ruwe-feedback.md`, daarna uitwerken
   en als actie opnemen in het actieregister.

Werkafspraak: geen modelleerjargon in een terugkoppeling aan de jurist, en geen
namen van deelnemers in deze openbare repo.
