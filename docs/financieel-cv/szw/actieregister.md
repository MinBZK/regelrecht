# Financieel CV — actieregister

Doorlopend register van wat er is gesignaleerd, wanneer, en wat we ermee
hebben gedaan. Eén regel per actie. Nieuwe rondes komen onderaan erbij; de
losse sessienotities blijven daarnaast bestaan als bron.

De **onbewerkte** feedback staat in [ruwe-feedback.md](ruwe-feedback.md), met
per ronde de afwijkingen tussen wat er letterlijk gevraagd is en wat we
gebouwd hebben. Bij twijfel over de bedoeling wint dat bestand.

**Werkafspraak over branches.** Modelleerwerk landt op
`traject/financieel-cv-validatie-*` in de repo `regelrecht-corpus`. De branch
`traject/financieel-cv-0bc401e0` is de demo-omgeving van de RVO-mensen en
wordt niet zomaar onder ze gewijzigd: dat gaat via een aparte branch en een
PR, en demo-zichtbare gevolgen worden vooraf gemeld. Cherry-picken tussen de
twee werkt niet — ze hangen hun `machine_readable` op verschillende
wetsversies (validatie op 2026-07-01 / schema v0.5.4, RVO op 2025-01-01 en
2026-01-01 / schema v0.5.2).

**Statuswaarden.** `open` · `loopt` · `gedaan` · `belegd` (bij iemand buiten
het team) · `vervallen`.

---

## Ronde 1 — juristvalidatie 23 juli 2026

Bron: [2026-07-23-juristvalidatie-notities.md](2026-07-23-juristvalidatie-notities.md)

| # | Actie | Bij wie | Status | Wanneer gedaan | Waar |
|---|---|---|---|---|---|
| 1.1 | Navragen bij UWV hoe het zit met de openstaande onduidelijkheden | SZW-jurist | belegd | — | — |
| 1.2 | Wajong-voorzieningen (tegenhanger van WIA 35) modelleren | ons | gedaan | 2026-08-19 | `aa04ff3f170` — Wajong art. 2:22 gemodelleerd als tegenhanger van WIA 35 |
| 1.3 | Drie Wajong-tijdperken in de berekening verwerken | ons | open | — | De regimes staan alleen in de preambuletekst; er is geen modellering |
| 1.4 | LIV uit scope halen: scenario's, docs en diagrammen | ons | gedaan | vóór 2026-08-25 | Geen LIV-featurebestanden meer op de validatiebranch |
| 1.5 | Werkgeverslasten-vraag uitzoeken zodra de ministeriële regeling helder is | ons, na 1.1 | open | — | `werkgeverslastenvergoeding_eurocent` staat nog als niet-uitgewerkte `open_term` bij Pwet art. 10c |

Losse bevinding uit dezelfde ronde, buiten de actietabel: de LKS werd niet
naar rato van de arbeidsduur gekort. Opgelost op **2026-08-19** in
`fcc1738ebdd` (LKS naar rato bij deeltijd + doelgroep 10d.2.c). Koens bedrag
ging daarmee van het 36-uursbedrag naar €766,22 per maand bij 32 uur.

---

## Ronde 2 — juristfeedback op de doorloop Koen en Sadee, 2 september 2026

Bron: [2026-09-02-juristfeedback-doorloop.md](2026-09-02-juristfeedback-doorloop.md)

| # | Actie | Bij wie | Status | Wanneer gedaan | Waar |
|---|---|---|---|---|---|
| 2.1 | WIA art. 35 lid 4.b: Koen-scenario corrigeren — het college draagt zorg, dus geen JC/WPA via UWV | ons | gedaan | 2026-09-02 | `wet_werk_en_inkomen_naar_arbeidsvermogen/scenarios/financieel_cv_koen.feature`, plus spiegelscenario voor ná de tweejaarsgrens |
| 2.2 | Gemeentelijke route modelleren zodat het CV geen vals "geen recht" toont: Pwet art. 10 (aanspraak, verordeningsvoorbehoud) en art. 10da (harde aanspraak LKS-doelgroep) | ons | gedaan | 2026-09-02 | `participatiewet/2026-07-01.yaml`; scenario's in `voorzieningen_arbeidsinschakeling.feature` |
| 2.3 | Proefplaatsing modelleren in de drie ontbrekende wetten: Pwet art. 8a lid 2 d (2+4 mnd), Wajong art. 2:24 (6 mnd), Wet WIA art. 37 (6 mnd) | ons | gedaan | 2026-09-02 | Drie nieuwe `proefplaatsing.feature`-bestanden; onjuist commentaar in het WW-Koen-scenario vervangen door het vergelijkingsoverzicht |
| 2.4 | Doorloop-artifact "Koen en Sadee door het stelsel" opnieuw genereren — toont nu nog de oude uitkomst voor JC/WPA en proefplaatsing | ons | open | — | — |
| 2.5 | Beslissen of en hoe ronde 2 naar de RVO-demobranch gaat, inclusief her-hangen op schema v0.5.2 en de 2025/2026-01-01-versies | ons + RVO | open | — | Vraagt een aparte branch en een PR; demo-zichtbare gevolgen vooraf melden |
| 2.6 | Presentatielaag: onderscheid tonen tussen een harde aanspraak (10da) en een aanspraak onder verordeningsvoorbehoud (10 lid 1) | ons | open | — | Zonder dat onderscheid leest een gemeentelijke route als een UWV-beschikking |
| 2.7 | Scopevraag: gaan we gemeentelijke verordeningen laden? Zonder die verordeningen blijft de Pwet-route "de route bestaat", nooit een bedrag | ons | open | — | — |
| 2.8 | Aggregator: kunnen proefplaatsing en loonkostensubsidie samenlopen? Zelfde open vraag als LKS ↔ LKV (Pwet art. 10d lid 9) | ons | open | — | — |

---

## Ronde 3 — juristfeedback op de terugkoppeling, 8 september 2026

Bron: [2026-09-08-juristfeedback-ronde3.md](2026-09-08-juristfeedback-ronde3.md)

| # | Actie | Bij wie | Status | Wanneer gedaan | Waar |
|---|---|---|---|---|---|
| 3.1 | Wfsv 38b onderdeel c: slotzin is een voorwaardelijke insluiting, geen uitsluiting — wie duurzaam geen arbeidsvermogen heeft telt wel mee zodra hij werkt | ons | gedaan | 2026-09-08 | `wet_financiering_sociale_verzekeringen/2026-07-01.yaml`; nieuwe parameter `verricht_arbeid_in_dienstbetrekking`, doorgegeven vanuit Wtl en Ziektewet; drie scenario's in `doelgroepregister_banenafspraak.feature` |
| 3.2 | Terugkoppeling zonder modelleerjargon schrijven — "aangeleverd feit" was niet te volgen | ons | gedaan | 2026-09-08 | Antwoord + parameteroverzicht per nieuw artikel in de rondenotitie; werkafspraak in `ruwe-feedback.md` |
| 3.3 | Reden vastleggen waarom de doelgroepverklaring bij de WIA blijft en bij de banenafspraak verviel | ons | gedaan | 2026-09-08 | `wet_tegemoetkomingen_loondomein/2026-01-01.yaml` bij `heeft_geldige_doelgroepverklaring_2_15` en in de samenloop-untranslatable |
| 3.4 | De drie aanvaarde LKV-bepalingen niet als invoer modelleren maar als disclaimer tonen | ons | belegd | — | Uitspraak genoteerd bij de drie untranslatables; uitvoering hoort bij actie 2.6 (presentatielaag) |
| 3.5 | Onderdeel f van Wfsv 38b controleren op dezelfde constructie | ons | gedaan | 2026-09-08 | Gecontroleerd: f is wél een absolute uitsluiting ("niet langer … meer heeft", zonder dienstbetrekking-clausule). Ongewijzigd |

---

## Openstaand, samengevat

| # | Actie | Status |
|---|---|---|
| 1.1 | UWV-navraag | belegd bij de SZW-jurist |
| 1.3 | Drie Wajong-tijdperken | open |
| 1.5 | Werkgeverslasten in de LKS-grondslag | open, wacht op 1.1 |
| 2.4 | Doorloop-artifact hergenereren | open |
| 2.5 | Ronde 2 naar de RVO-demobranch | open |
| 2.6 | Twee sterktes van aanspraak in de presentatielaag | open |
| 2.7 | Verordeningen wel of niet laden | open, scopevraag |
| 2.8 | Samenloop proefplaatsing ↔ LKS | open |
| 3.4 | Drie aanvaarde LKV-bepalingen als disclaimer tonen | belegd bij de presentatielaag (2.6) |
