# Letter-fideliteitsaudit van de zeven wetten

**Datum:** 23 september 2026 · **Methode:**
`.claude/skills/law-letter-fidelity-audit` · **Omvang:** zeven parallelle
audits, één per regeling, elk read-only

Elke audit heeft eerst de `text:`-velden woordelijk vergeleken met de officiële
BWB-toestand en daarna per lid gevraagd of de formule dekt wat de letter zegt.
Verdicts volgen de skill: **modelfout** (het model wijkt af van de letter),
**wettekst-gevolg** (het model volgt de letter getrouw, de letter zelf geeft een
raar resultaat) en **letter-vs-toelichting** (een jurist moet kiezen).

## Eerst het goede nieuws

**Geen enkele verbatim-drift.** Alle gecontroleerde artikelteksten zijn
woordelijk gelijk aan de geldende toestand. De harvester haalt de tekst correct
binnen; het verschil zit in de vertaling ernaartoe.

**Bijna geen letter-vs-toelichting.** Van de ruim zestig bevindingen zijn er
drie een juristkeuze en twee een wettekst-gevolg. De rest is modelfout: te
repareren zonder dat iemand hoeft te kiezen tussen letter en bedoeling.

**Twee artikelen zijn exemplarisch.** Wet WIA 43 draagt alle negen
uitsluitingsgronden, met onderdeel a gesplitst in 1° en 2°, en markeert de
tenzij-clausule van onderdeel b in plaats van die weg te laten. Wajong 2:24
vertaalt een kan-bepaling eerlijk als `mag_proefplaatsing_aangaan` en registreert
de ziekte-onderbreking als marking met een correct citaat.

## Zes patronen

De bevindingen zijn niet los van elkaar. Zes vormen verklaren vrijwel alles.

### 1. De dragende rechtsgevolgtrekking heeft geen output

Het model beschrijft de feiten *rond* een artikel in plaats van het rechtsgevolg
dát het artikel maakt. Daardoor heeft een uitsluiting of beperking niets om op
aan te grijpen.

| Wet | Wat ontbreekt |
|---|---|
| Ziektewet 29b lid 8 | "Dit artikel is niet van toepassing indien de werknemer werkzaam is in een dienstbetrekking in de zin van artikel 2 Wsw" — geen enkele afsluitende uitsluiting in het model |
| Participatiewet 10b lid 1 en 6 | De aanbiedingsplicht van het college is geen output, dus lid 6 (plicht vervalt zodra het quotum vol is) kan nergens ingrijpen |
| Participatiewet 10c lid 2 | "Een aanvraag kan slechts eenmaal per twaalf maanden worden ingediend" staat nergens |
| Wtl 2.6 lid 3 c | "niet langer van toepassing indien de periode van artikel 2.8 is verstreken" — de drie-jaarsuitputting ontbreekt |

Bij de Wtl is de asymmetrie het bewijs: het spiegelartikel 2.14 lid 2 c ís
gemodelleerd. Het gaat om een omissie, niet om een keuze.

### 2. "Kan" wordt "heeft recht op"

| Wet | Letter | Model |
|---|---|---|
| Wet WIA 35 lid 1 | "Het UWV **kan** ... op aanvraag voorzieningen toekennen" | `heeft_recht_op_jobcoaching`, `decision_type: TOEKENNING` |
| Wajong 2:22 lid 1 | idem | `heeft_recht_op_werkplekaanpassing` |
| WW 76a lid 1 | "Het UWV **kan** toestemming verlenen" | gebonden uitkomst, en de outputnaam legt de bevoegdheid bij de werknemer |

Het corpus weet hoe het moet: Wajong 2:24 doet dezelfde constructie wél goed, en
Wajong 2:20 is het omgekeerde geval — daar staat "vermindert", gebonden, en het
model is dan terecht een recht.

### 3. Open termen zijn inert

De WIA-audit is in de engine gaan kijken: `packages/engine/src/typecheck.rs:1451`
declareert een open term alleen als symbool in scope. Niets injecteert hem in een
formule. **Een open term die geen actie aanroept, doet niets aan de uitkomst.**

Dat raakt: de twee materiële criteria van WIA 35 lid 2 c en d, dezelfde twee in
Wajong 2:22, het causaliteitsvereiste in WIA 4, de
werkgeverslastenvergoeding in Pwet 10d lid 4 — en de 23 open termen die bij de
migratie van 22 september zijn aangemaakt.

Lid 2 van WIA 35 opent met "worden **uitsluitend** verstaan": een limitatieve,
inhoudelijke afbakening. Het enige lid-2-element in de formule is dát er een
aanvraag is ingediend.

### 4. Temporele begrenzing ontbreekt

Duur-outputs zijn constanten die nooit in een vergelijking worden gebruikt.

- Wtl: de bedragen worden over álle verloonde uren van het kalenderjaar gerekend, terwijl de wet alleen uren telt binnen de periode waarin aan de voorwaarden is voldaan. De termijnen van 2.8, 2.12 en 2.16 worden niet gerekend.
- WW 76a: `max_duur_proefplaatsing_maanden` is een losse constante. Er is geen termijn, dus ook niets om met lid 4 op te schorten.
- Wet WIA 23 lid 6: de verkorte wachttijd van dertien tot achtenzeventig weken wordt overgeslagen, terwijl het een uitdrukkelijke derogatie is ("in afwijking van het eerste lid").
- Wajong 2:16: het recht vervalt in het model per direct zodra een voorwaarde omslaat, zonder de twee maanden van lid 1 a en zonder de uitzonderingen van lid 2 en 3.

### 5. Verankering aan het verkeerde artikel

| Waar | Wat |
|---|---|
| Participatiewet 10c | Het hele `machine_readable`-blok is het model van 10d: parameters, open termen en markings verwijzen alle naar 10d. Artikel 10d zelf heeft geen model |
| Wtl 2.1 | Draagt de bedragen van 2.9, 2.13 en 2.17 en de duur van 2.8, 2.12 en 2.16. Die artikelen tellen in elke corpustelling als niet gemodelleerd |
| Reïntegratiebesluit 1a | `implements` hangt aan een artikel dat uitsluitend grondslagen opsomt. De echte nadere regels staan in de artikelen 2, 4, 5, 6 en 7, die geen model hebben |
| Wfsv 38b | De output heet `behoort_tot_doelgroepregister_banenafspraak`, maar 38b definieert alleen het begrip arbeidsbeperkte; de registratie is artikel 38d |

### 6. Toegevoegde voorwaarden die de letter niet stelt

De omgekeerde richting van patroon 1, en even schadelijk.

- **Wfsv 38b**: de chapeau-uitsluiting beschut werk staat in lid 1 en wordt in lid 6 woordelijk herhaald, maar **lid 2 laat haar weg**. Het model wrapt de NOT boven de hele OR, dus ook boven lid 2.
- **Wajong 2:22 lid 1**: de letter richt zich tot "de jonggehandicapte"; het model eist daarbovenop recht op arbeidsondersteuning.
- **Participatiewet 10 lid 5**: de aanvraag is in de letter de weg om gevolg te geven aan een aanspraak die lid 1 onvoorwaardelijk toekent. Het model maakt er een voorwaarde vóór het bestaan van de aanspraak van.
- **Wtl**: de twee zelfstandige routes van 10d lid 1 en lid 2 zijn tot één cumulatieve voorwaardenketen samengevoegd, waardoor de hoofdroute structureel onwaar oplevert.

## Wat de uitkomst raakt, per wet

| Wet | Zwaarste bevinding |
|---|---|
| Ziektewet | Lid 8 ontbreekt; lid 2 e mist "op of na 1 januari 2015"; lid 1 b reduceert vijf alternatieve tijdvakken tot de WIA-wachttijd; de 70% van het dagloon (lid 5 t/m 7) is niet gemodelleerd en niet gedeclareerd |
| Participatiewet | Het 10c-blok is integraal het model van 10d; de werkgeverslastenvergoeding staat buiten de 70%-clausule, heeft `default: 0` en wordt nergens gebruikt, dus de subsidie is bij constructie te laag |
| Wfsv | Onderdeel g van lid 1 ontbreekt; de chapeau wordt ten onrechte ook op lid 2 toegepast; `grond_opname_doelgroepregister` kent geen case voor lid 6, zodat twee outputs van hetzelfde artikel elkaar tegenspreken |
| Wtl | Geen enkele temporele begrenzing in het bedrag; artikel 2.10 is niet gemodelleerd, dus de Wsw-uitsluiting en de beëindigingsgrond worden niet getoetst |
| Wet WIA | Artikel 37 laat een IVA-gerechtigde toe tot de proefplaatsing, terwijl de letter zich uitsluitend tot de gedeeltelijk arbeidsgeschikte richt; artikel 35 kent toe op een aanvraag zonder inhoudelijke toets |
| Wajong | De WIA-definitie van volledig en duurzaam arbeidsongeschikt past niet op de Wajong; 2:15 lid 2 mist het lid-1-moment, waardoor een ingangsdatum vóór het achttiende jaar kan ontstaan |
| WW en Reïntegratiebesluit | Lid 2 verliest "onverminderd artikel 20 lid 1 b", zodat de uitkering in het model doorloopt na afloop van de uitkeringsduur; de `implements` van het besluit is leeg |

## Drie bevindingen corrigeren het werk van 22 en 23 september

**De open termen uit de migratie zijn dood gewicht.** Onder v0.5.4 waren het
`untranslatables`: die zeiden eerlijk "dit vangen we niet". Als open term zien ze
eruit als dekking en zijn ze inert. Dat verklaart ook waarom `required: false`
de BDD-suite groen maakte zonder één uitkomst te veranderen.

**F6 in het fixes-plan was fout.** Daar stond dat Wet WIA 37 de OR van de
artikelen 47 en 54 moet lezen. De letter verbiedt die OR: artikel 5 zegt "doch
die **niet** volledig en duurzaam arbeidsongeschikt is", artikel 47 lid 1 b zegt
"hij **is** volledig en duurzaam arbeidsongeschikt". Die twee sluiten elkaar uit.
De juiste binding is `is_gedeeltelijk_arbeidsgeschikt AND heeft_recht_op_wga_uitkering` —
smaller dan de OR.

**De marking op WW 76a lid 4 is verkeerd geclassificeerd.** De reden luidt dat
het formaat geen kalender van werk- tegenover ziektedagen kent. Maar `DATE_ADD`,
`DATE_DIFF` en `SUBTRACT` bestaan. Wat ontbreekt is een invoerfeit, en het schema
zegt daarover: "a value produced by another law is an input with a source", geen
marking.

**En de source-aanroep van F2 geeft twee parameters te veel mee.**
`behoort_tot_doelgroep_10b_lid_1` en `heeft_dienstbetrekking_beschut_werk` spelen
in de chapeau geen rol; die horen bij de andere output van Pwet 10b.

## Wat naar de sessie gaat

**B17 heeft een antwoord uit de letter en is geen keuze meer.** Wajong 2:4:
"volledig en duurzaam arbeidsongeschikt is de jonggehandicapte die **duurzaam
geen mogelijkheden tot arbeidsparticipatie heeft**". Wet WIA 4: "duurzaam slechts
in staat om met arbeid **ten hoogste 20%** te verdienen van het maatmaninkomen".
Wie vijftien procent kan verdienen is het onder de WIA wel en onder de Wajong
niet. De parameternaam is in beide wetten gelijk; de Ziektewet hernoemt netjes
naar `_wajong`, de Wajong zelf niet. Dat is een modelfout met een naamcollisie
als valstrik, geen juristvraag.

Wat wel juristvraag blijft:

1. Is het weglaten van de beschut-werkuitsluiting in Wfsv 38b lid 2 bedoeld, gegeven dat lid 6 haar woordelijk herhaalt?
2. Kent de doelgroepdefinitie een rangorde tussen de onderdelen, of is elke samenloop gelijkwaardig? De modelomschrijving claimt "volgorde volgt de wettekst"; lid 1 somt a tot en met g op met "of" vóór g, zonder rangorde.
3. Is Ziektewet 29b lid 1 onderdeel c en d bedoeld op de peildatum te toetsen of op de aanvang van de dienstbetrekking? De letter noemt geen peilmoment, en de grond verspringt nu op de achttiende verjaardag.
4. Mag een afgeleide grootheid als `arbeidsongeschiktheidspercentage` een `legal_basis` dragen van een artikel dat haar niet noemt?

## Twee wettekst-gevolgen, niet repareren

**Wet WIA 23 lid 6** verwijst naar "artikel 4, **tweede** lid" voor de definitie
van volledig en duurzaam arbeidsongeschikt, terwijl die in lid 1 staat; lid 2
definieert alleen "duurzaam". Het corpus geeft dat correct weer.

**Wajong 2:15 lid 5** verwijst naar "artikel 8:10, **vierde** lid". Dat lid is per
1 januari 2021 vervallen (Stb. 2020, 173/174); artikel 8:10 heeft nog twee leden.
De verwijzing in 2:15 is sindsdien dood. Het model laat die route terecht weg,
maar legt dat nergens vast — dat hoort een marking te worden, en de wettekst
verdient een wetgevingssignaal.

## Volgorde

1. **Wat de uitkomst verandert**: Ziektewet lid 8, Wtl 2.6 lid 3 c en de urentelling, Wet WIA 37, Wfsv chapeau over lid 2, Participatiewet 10d lid 1 tegenover lid 2.
2. **Wat de verankering herstelt**: het 10c-blok naar 10d, `legal_basis` in Wtl 2.6 en 2.14 onder `value:` inspringen, de `implements` van het Reïntegratiebesluit naar de artikelen die werkelijk regels stellen.
3. **Wat gedeclareerd hoort te worden in plaats van stilzwijgend weggelaten**: de 70% van het dagloon, de verkorte wachttijd, artikel 2:16, de formule van Wfsv 38f lid 2.
4. **De open termen**: kies per stuk of ze een actie gaan voeden of dat ze weg moeten. Gedeclareerd en ongebruikt is de slechtste van de drie toestanden.

## Wat deze audit niet heeft gedaan

De agenten hebben de letter met het model vergeleken. Zij hebben geen scenario's
gedraaid en geen uitkomsten getoetst — dat is bewust, want de skill schrijft die
volgorde voor: fideliteit eerst, anders ga je het model reverse-engineeren om een
test te laten slagen.

De BDD-suite staat op 76 van de 76 groen. Dat en dit rapport zijn niet met elkaar
in tegenspraak: de scenario's toetsen wat het model doet, niet of het model de
wet volgt.
