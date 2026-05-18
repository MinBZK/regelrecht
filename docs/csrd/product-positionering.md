# CSRD-regelhulp — product-positionering


## Het voorstel in één zin

Een regelhulp die EU-ondernemingen helpt te bepalen of zij onder de
**CSRD-rapportageplicht** vallen, **met uitleg waarom**, gebaseerd op
de actuele tekst van Richtlijn 2013/34/EU + 2022/2464/EU (CSRD) +
Omnibus 2026.

## Scope — hard afgebakend

| Wel | Niet |
|-----|------|
| `valt_onder_csrd` — boolean uitkomst | ESRS-datapunten invullen (~1000 velden) |
| Drempelwaarden-toets (>1000 fte, >€450M omzet) | Materialiteit-toets (double materiality assessment) |
| Rapportage-type bepalen (individueel / geconsolideerd / exempt via groep) | XBRL-tagging of digital-reporting-format |
| Ingangsdatum (eerste boekjaar) | Accountantskantoor-assurance |
| Uitleg + verwijzing naar relevante artikelen | Inhoudelijke duurzaamheidsadviezen |
| Doorklik naar RVO / AFM / EUR-Lex | Toezicht of handhaving |

## Wat onderscheidt deze regelhulp van bestaande tools?


| Voordeel | Wat het is | Wat bestaande tools NIET kunnen | Voor wie waardevol |
|----------|------------|-----------------------------------|---------------------|
| **Provenance** | Per uitkomst zichtbaar maken welke artikelen en condities zijn getoetst (box-drawing trace, terug naar artikel-niveau) | MVO Wetchecker geeft alleen ja/nee zonder uitleg | Bedrijven die juridische zekerheid willen; accountants die het besluit moeten verifiëren |
| **Actualiteit** | Corpus + harvester garandeert dat wetswijzigingen (zoals Omnibus 2026) direct doorwerken in de tool | MVO Wetchecker en statische RVO-pagina's lopen achter bij wetswijzigingen | Iedere ondernemer; vooral relevant rond ingangsmomenten (2027) |
| **Machine-readable bron** | Dezelfde engine kan via API aan andere tooling worden gekoppeld (BI, accountantssoftware, KVK) | Bestaande tools zijn standalone HTML/wizards | Accountantskantoren, BI-platforms, ketenpartijen |


## Bestaande tools in het landschap

**Belangrijke nuance** (toegevoegd 2026-05-13): MVO Wetchecker blijkt
**eigen RVO-tool** te zijn, niet een externe concurrent. Gehost via
regelhulpenvoorbedrijven.nl/mvo-wetchecker/ (zelfde portal als
Financieel CV-regelhulp). De juristen-mail noemde een verouderde URL
(mvonederland.nl/wetchecker). Dit verandert het positioneringsvraagstuk:
het gaat niet om competitie maar om **product-architectuur binnen RVO's
regelhulpen-portfolio**.

| Tool | Eigenaar | Wat het doet | Relatie tot ons werk |
|------|----------|--------------|----------------------|
| **MVO Wetchecker** | **RVO (zelfde eigenaar)** | Vragenlijst die 8 duurzaamheidswetten doorloopt — CSRD, CSDDD, EUDR, e.a. Geeft per wet kort de verplichtingen + verwijzingen | **Te positioneren** — onze CSRD-regelhulp wordt een module binnen MVO Wetchecker, een aparte regelhulp, of vervangt de CSRD-tak van MVO Wetchecker. Zie nieuwe vraag 5 hieronder. |
| RVO CSRD-pagina | RVO | Uitlegtekst over CSRD | Statisch — onze tool is interactief en geeft persoonlijk antwoord |
| SER over Omnibus | SER | Beleidsachtergrond Omnibus | Geen tool, alleen artikelen |
| Accountants-software (Workiva, Tagetik, etc.) | Privaat | Volledige rapportage-platforms (ESRS-detail + XBRL) | Andere doelgroep (grote bedrijven met budget); andere prijsklasse — niet onze concurrent |

### Actuele staat MVO Wetchecker (mei 2026)

Eigen vermelding op de tool: *"the outcome of the MVO Wetchecker may
not yet fully reflect the most recent legislation. Once changes are
processed, the MVO Wetchecker will be updated."*

Concreet betekent dit: de Omnibus 2026-drempelwaarden (1000 fte,
€450M omzet) zijn op moment van schrijven (2026-05-13) **nog niet
verwerkt** in MVO Wetchecker. Daarmee is onze "actualiteit"-
differentiator letterlijk waar gemaakt: een corpus-gedreven regelhulp
kan binnen dagen na een wetswijziging worden bijgewerkt, een statisch
onderhouden Wetchecker loopt achter.

## Wat dit vraagt om te beslissen — vóór de bouw

### Vraag 1 — Wie is de eigenaar van de regelhulp? ✅ BESLOTEN

**RVO is eigenaar van de regelhulp** (bevestigd 2026-05-13). Zelfde
pad als de Financieel CV-regelhulp: RVO host + support; regelrecht is
"data + engine provider".

Wat dat betekent voor het werk hier:

- **regelrecht-repo levert**: corpus (YAMLs), engine-execution,
  BDD-validatie, API
- **RVO bouwt**: de eindgebruiker-wizard, UX, branding, hosting,
  support
- **Productieaansluiting**: vergelijkbaar met huidige Financieel CV —
  RVO consumeert de regelrecht-API of geharveste outputs
- **Geen eigen frontend nodig in regelrecht-repo** (geen
  `frontend-regelhulp-csrd/`); de showcase-discussie vervalt

Daarmee komt **provenance** in een ander licht: regelrecht moet de
provenance-trace via API exposeren zodat RVO hem in de UI kan tonen.
Dat is een concreter API-contract dan ik eerder schetste.

### Vraag 2 — Tijdshorizon van het product

- CSRD-ingangsdatum is **1 januari 2027**. Verkennings-doelgroep
  (>1000 fte, >€450M) is overzichtelijk maar tijdkritisch.
- Voor wanneer moet de regelhulp werkend zijn? Q4 2026? Q1 2027?

### Vraag 3 — Onderhoud van EU-corpus

- Geen harvester voor EUR-Lex op dit moment — corpus wordt handmatig
  bijgehouden bij EU-wijzigingen.
- Wie houdt 2013/34 + 2022/2464 + ESRS-verordening actueel als de
  volgende Omnibus (of EU-aanpassing) komt?
- **Alternatief**: investeren in een EUR-Lex-harvester (apart RFC/sprint,
  schatting ~1 sprint werk). Dan vergelijkbaar geautomatiseerd als de
  BWB-harvester voor NL-recht.

### Vraag 4 — Wat doen we met NL-omzetting (Boek 2 BW)?

- CSRD wordt in NL omgezet via Wet implementatie CSRD die Boek 2 BW
  wijzigt.
- Voor de **eerste slice**: niet nodig — EU-richtlijn is voldoende.
- Voor **productie**: ondernemers zullen verwachten dat de regelhulp
  verwijst naar NL-recht, niet alleen EU.
- Op welke termijn vullen we Boek 2 BW aan?

### Vraag 5 — Verhouding tot bestaande MVO Wetchecker (RVO-eigen tool)

Beide tools zijn van RVO. Drie mogelijke positioneringen:

- **(a) Module binnen MVO Wetchecker** — onze CSRD-regelhulp vervangt
  de CSRD-tak van MVO Wetchecker. Voordeel: één regelhulp-portal voor
  ondernemer, gedeelde branding. Nadeel: vraagt afstemming met
  MVO-Steunpunt over UX-integratie.
- **(b) Aparte regelhulp naast MVO Wetchecker** — eigen "CSRD-
  scope-regelhulp" op regelhulpenvoorbedrijven.nl. Voordeel: snelste
  pad zonder afstemming. Nadeel: ondernemer moet kiezen waar te
  beginnen; mogelijk inconsistentie tussen de twee tools.
- **(c) Backend-vervanging** — MVO Wetchecker behoudt zijn UX, maar
  trekt voor de CSRD-tak voortaan zijn data + logica uit de
  regelrecht-engine. Voordeel: één bron-of-waarheid, regelrecht is
  data-provider voor bestaande UI. Nadeel: vraagt MVO Wetchecker-
  refactor.

Mijn voorkeur: **(c) backend-vervanging**, eventueel met (a) als
groeipad. Concreet: regelrecht levert de API, MVO Wetchecker
consumeert. Bij Omnibus 2027 hoeft MVO Wetchecker niets te doen —
regelrecht-corpus wordt bijgewerkt en de tool reflecteert het
automatisch.

## Wat dit NIET wordt — managing expectations

- **Niet een ESRS-invultool.** De ~1000 datapunten van ESRS zijn
  rapportage-velden, geen wet-uitvoering. Andere tooling-categorie.
- **Niet een accountantskantoor-vervanger.** Assurance-werk blijft bij
  externe accountants.
- **Niet een materialiteit-bepalingstool.** Double materiality is
  expert judgment + stakeholder-proces, geen rule-engine output.
- **Niet beschikking-georiënteerd.** CSRD-plicht is geen besluit met
  bezwaartermijn; geen UWV/Belastingdienst-analoog.

## Wat het wél wordt

Een **kleine, scherp afgebakende wizard** met:

1. Een paar vragen (drempelwaarden + EU-vestiging + groepsstructuur)
2. Een **uitkomst-pagina**: val ik onder CSRD, wat is mijn type, wanneer
   begint mijn rapportageplicht
3. Een **uitleg-pagina**: welke artikelen leiden tot deze conclusie,
   met links naar de EU-richtlijn-tekst
4. Een **doorklik** naar RVO / accountantskantoor voor het feitelijke
   rapportage-werk

Geschatte gebruikstijd voor ondernemer: 3-5 minuten. Geschatte waarde:
één concreet antwoord op de meest gestelde CSRD-vraag in NL.

## Mogelijke meetlat voor succes

- **Adoptie**: aantal gebruikers / aantal uitgevoerde scope-toetsen
- **Doorklik**: % gebruikers dat na "val ik onder CSRD?" doorklikt naar
  RVO / accountantsadvies (mate van vertrouwen in uitkomst)
- **Onderhoudslast**: aantal corpus-wijzigingen per kwartaal
  (gauge voor of EUR-Lex-harvester nodig is)
- **Eventueel: API-aanvragen** als externe tooling het consumeert

## Voorgestelde besluitvolgorde

1. **Eigenaarschap-vraag** (vraag 1) — kies a/b/c. Bepaalt of regelrecht
   bouwt of alleen API levert.
2. **Provenance-pitch** versus alternatieven (sectie "differentiators")
   — kies hoofd-onderscheidend voordeel
3. **Scope-bekrachtiging** met jurist via
   [scope-bepaling.md](scope-bepaling.md) — bekrachtigt of de "Wel"-
   kolom hierboven juridisch klopt
4. **Pas dan** bouw beginnen (Sprint A uit het stappenplan): EU-corpus
   YAML + `valt_onder_csrd` machine_readable + BDD-personae

## Bijlagen

- [README.md](README.md) — projectoverzicht + voortgangsstatus
- [stelsel-overview.md](stelsel-overview.md) — stelsel-diagram + lagen
- [scope-bepaling.md](scope-bepaling.md) — wegkruis-tabel + open vragen
  voor jurist (bekrachtigt de "Wel"-kolom van dit document)
- [jurist-input-2026-05-13.md](jurist-input-2026-05-13.md) — bron
