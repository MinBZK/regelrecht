---
name: law-statement-register
description: >
  Mines statements verbatim from secondary legal texts - toelichting, beleidsregel,
  werkinstructie, handboek, FAQ, circulaire - and turns them into an auditable
  statement register: per statement a verbatim quote, a recorded search for its
  anchoring in the norm corpus (verankerd / geparafraseerd / niet-gevonden, with the
  search terms), a deviation class, and a bucket (MODELFOUT / WETTEKST-GEVOLG /
  LETTER-vs-TOELICHTING / letter-getrouw / scope). Unlike a law, these documents have
  no article structure, no version history and no separation between norm and
  explanation, so the skill constructs all three: a canonical text with a source hash,
  a 100% tiling of that text, and content-derived statement identity that survives a
  reissued document. Four runnable gates (verbatim / coverage / anchor / signaalnet)
  make "nothing was silently skipped" a checkable claim instead of a promise. Has a
  second mode that diffs two versions of the same document to show what changed in
  policy without any norm changing. Use when the user mentions uitvoeringsbeleid,
  beleidsregels, werkinstructies, a toelichting or handbook, "statements ontginnen",
  a statement register, or wants to know what a new version of a policy document
  changed.
allowed-tools: Read, Write, Edit, Bash, Grep, Glob, WebFetch, AskUserQuestion
user-invocable: true
---

# Law Statement Register — statements ontginnen uit secundaire teksten

Uitvoeringsbeleid en toelichtingen bepalen in de praktijk hoe een wet uitpakt, maar ze
missen alles wat een wettekst gratis meelevert: geen artikelstructuur, geen versiebeheer,
geen citeerbaar URL-fragment, en geen scheiding tussen norm en uitleg. Elke bestaande
pipeline-skill (`law-download`, `law-generate`, `law-letter-fidelity-audit`,
`law-version-drift-check`) gaat uit van een gestructureerde, geversioneerde bron. Deze
skill maakt die structuur zélf, en bewijst daarna mechanisch dat er niets is weggevallen.

Het product is een **statement-register**: per statement een verbatim citaat, een
vastgelegde verankeringszoektocht, een afwijkingsklasse, een bucket, en een actie of
jurist-vraag.

## Kernprincipes

> **De norm is leidend, de secundaire tekst is uitleg.** Een toelichting legt de bedoeling
> uit maar is geen norm. Modelleer de open norm zoals de regeling die stelt; gebruik de
> toelichting als duiding, nooit als verkapt criterium.

Vier regels die de rest van de methode dragen:

1. **Een statement wordt nooit op eigen gezag een modelwijziging.** Een statement kan
   hooguit leiden tot een fix *richting de letter*, een rapportage, of een jurist-vraag.
   Nooit tot `machine_readable` dat de toelichting boven de norm zet. Zie
   `references/classificatie.md`.
2. **Uitgestelde relevantie.** Eerst betegelen, dán classificeren. Een lezer die de opdracht
   "haal de regels eruit" krijgt, laat stil weg; een lezer die "betegel dit document" krijgt,
   kan dat niet.
3. **Overslaan mag, stil overslaan niet.** Elk niet-normatief fragment krijgt een expliciete
   `disposition` mét reden. Elke negatieve verankeringsbevinding krijgt zijn zoektermen.
4. **Blind her-ontginnen vóór diffen.** Bij een nieuwe versie gaat het vorige register pas
   ná de nieuwe extractie open — anders erf je elke omissie van de vorige ronde.

## Werkstroom — 7 fasen

```
0  BEVRIES      bron + sha256 + retrieved_at + documentstatus
1  CANONISEER   scripts/canonicalize.sh  →  canonical.md + pages.tsv + manifest.yaml
2  BETEGEL      scripts/tile.py  →  concept-ledger, 100% dekkend    ⟹ gate: coverage
3  SWEEP        modaliteit-passes: proza · lijst · tabel · voetnoot · kader ·
                voorbeeld · bijlage/formulier
4  ANKER        statement = verbatim quote + {exact, prefix, suffix}  ⟹ gates: verbatim, anchor
5  VERANKER     zoektocht in het norm-corpus, mét zoektermen        (references/verankering.md)
6  CLASSIFICEER afwijkingsklasse → bucket → actie of jurist-vraag   (references/classificatie.md)
7  TEGENLEES    signaalnet-audit + drop-pass                        ⟹ gate: signaalnet
```

**Fase 0 — bevriezen.** Hash de bronbytes. Beleids-PDF's worden op dezelfde URL vervangen
zonder versiemarkering; de hash is het enige dat "dit register is uit dát document gelezen"
toetsbaar maakt. Bepaal ook meteen de **documentstatus** — zoek expliciet naar een
disclaimer ("aan deze toelichting kunnen geen rechten worden ontleend", "dit is een
werkinstructie"). Eén zin herdefinieert de status van elk statement in het document.

**Fase 1 — canoniseren.** `scripts/canonicalize.sh bron OUTDIR`. Deterministisch, met de
normalisatie eenmalig en zichtbaar toegepast. `canonical.md` bevat géén ingevoegde tekens —
geen pagina-markers — zodat een citaat dat over een paginagrens loopt nog steeds verbatim is.

Twee takken, één contract (zelfde bytes in → zelfde tekst uit, geen oordeel):

| Bron | Extractor | Furniture-regel | Zijsporen |
|---|---|---|---|
| `.pdf` | `pdftotext -layout` | regels die op de meeste pagina's boven-/onderaan terugkomen | `pages.tsv` |
| `.html` | `scripts/html_canonical.py` (stdlib) | `nav`/`header`/`footer`/`aside`/`script`, icoon- en schermlezerlabels (`sr-only`, `visually-hidden`, `icon-label`), en versmallen tot `<main>`/`<article>` | `links.tsv` |

Twee dingen die de HTML-tak expliciet goed doet omdat ze anders stil misgaan:

- **`colspan` telt mee.** Een cel die twee kolommen beslaat moet twee kolomposities
  opschuiven, anders landt elke waarde erna onder de verkeerde kop — een fout antwoord,
  geen opmaakdetail. Hetzelfde geldt voor een lege begincel: die blijft als tab staan.
- **Links gaan naar `links.tsv`, niet de tekst in.** Een hyperlink op *"artikel 8 van de
  Awir"* is het document dat zélf zegt welke norm het bedoelt — precies wat fase 5
  reconstrueert. Weggooien is bewijs weggooien; inline zetten sloopt het verbatim citeren.

Bij HTML wijst `--root '#content'` het inhoudsblok aan als de pagina geen `<main>` heeft, en
laat `--keep-nav` zien wát er anders wegvalt. De HTML-tak bewaart structuur die de tiler nodig
heeft: koppen op een eigen regel, lijstitems met `- `, tabelrijen tab-gescheiden.

Een webbron moet **lokaal opgeslagen** zijn. Een pagina die via een tussenlaag is opgehaald en
samengevat is geen bron: er is dan niets om te hashen en niets om te herhalen.

**Fase 2 — betegelen.** `python3 scripts/tile.py canonical.md > statements.yaml` knipt de
segmenten uit de tekst zelf, langs de eigen nummering van het document. Dekking van 100% is
dan een eigenschap van de constructie in plaats van handwerk — en daarmee geen klus die je
gaat afsnijden. Het script beslist niets: alles komt eruit als `normative` zonder statements.

Daarna is het leeswerk: niet-normatieve delen krijgen
`disposition: informative | navigational | duplicate | non-textual` met reden.
Inhoudsopgaven zijn `navigational`, colofons `informative`, een beslisboom-plaatje
`non-textual` (nooit stil OCR-en en dat verbatim noemen — laat transcriberen en markeer dat).

**Fase 3 — sweep.** Loop elk segment af per modaliteit. Eén lineaire lees-pass verliest
systematisch tabelcellen, voetnoten en "let op"-kaders — juist de plekken waar de
uitzondering staat. Bij een tabel: de tabel is één segment, elke zelfstandig normerende rij
is één statement met de kolomkop in de `prefix`.

**Fase 4 — ankeren.** Het citaat is verbatim; het anker is `{exact, prefix, suffix}` in de
vorm van RFC-005 (`docs/src/content/rfcs/rfc-005.md`). Kies de kortste span die zelfstandig
leesbaar is, en neem bij een opsomming de **aanhef** mee — een los onderdeel "b. ..." zonder
chapeau is betekenisloos en is een klassieke bron van verkeerde modellering.

**Fase 5 — verankeren.** Zoek per statement in het norm-corpus van het dossier en leg vast:
`verankerd` (mét norm-citaat en vindplaats), `geparafraseerd` (beide teksten naast elkaar),
of `niet-gevonden` (**mét de gebruikte zoektermen**). Die laatste eis is niet cosmetisch: de
`niet-gevonden`-statements worden de jurist-vragen, en een negatieve bevinding die niemand
kan overdoen is geen bevinding.

**Fase 6 — classificeren.** Bucket + twee assen die wetteksten niet nodig hebben:
**bindendheid** (`hard` / `soft-default` / `guidance` / `informative`) en **documentstatus**.
"In de regel drie maanden" als harde drie maanden modelleren is een fideliteitsfout, geen
detail — het neemt de afwijkbevoegdheid weg.

**Fase 7 — tegenlezen.** Draai het signaalnet en beantwoord één vraag: *wat is er
weggevallen?* Elke ongedekte treffer is óf een gemist statement óf een ontbrekende
`disposition`.

## De vier gates

```bash
python3 scripts/statement_gates.py all --canonical canonical.md --ledger statements.yaml
python3 scripts/statement_gates.py explain      # toont de enige toegestane normalisatie
```

| Gate | Toetst | Vangt |
|---|---|---|
| `ledger` | het vocabulaire: `disposition`, `anchoring.status`, `type`, `bindingness`, `bucket`, `deviation_class` | een waarde buiten de lijst — inclusief een typefout die een regel stil uitschakelt |
| `verbatim` | elk citaat is letterlijk een substring van `canonical.md` | parafrase, opgeschoonde weglating, overgetypt citaat, negatieve bevinding zonder zoektermen |
| `coverage` | de segmenten betegelen `canonical.md` volledig | ongedekt fragment (met de eerste 80 tekens), disposition zonder reden |
| `anchor` | elk `{prefix, exact, suffix}` resolvet uniek | ORPHANED (0 treffers) en AMBIGUOUS (>1) — beide onbruikbaar voor de diff |
| `signaalnet` | elke normzin in een `normative` segment is gedekt | stil overgeslagen norm |

**`ledger` draait bij élke aanroep**, ook bij een losse gate. De andere gates sleutelen op
exact deze strings — het is `status == "niet-gevonden"` dat de zoektermen eist — dus een
ledger die `nietgevonden` schrijft zet die regel uit en houdt drie groene gates over. Een
vocabulaire dat alleen door spelling wordt gehandhaafd, wordt niet gehandhaafd.

Exit-code is het contract: 0 = schoon, 1 = bevindingen. `canonicalize.sh` en `tile.py`
gebruiken daarnaast **3** voor "buiten het getoetste bereik": een PDF zonder tekstlaag, of
een betegeling die op één groot blok uitkomt. Die weigeren liever dan een leeg of
misleidend-groen resultaat af te leveren. `tests/run.sh` bewijst op een synthetische fixture
dat elke gate zijn eigen defect pakt.

Het signaalnet-lexicon staat in `references/signaalnet.md` en is te vervangen met
`--lexicon`. Het is bewust ruw: liever een inhoudsopgave-regel als treffer (die je
wegdisponeert) dan een gemiste uitzondering.

## Modus 2 — versie-diff

Bij een nieuwe versie van hetzelfde document is de diff het interessantste product: hij laat
zien wat er aan **beleid** veranderde zonder dat een norm veranderde. Zie
`references/diff.md`. Verkort:

1. Her-ontgin het nieuwe document **blind** (fasen 0–7, vorige register dicht).
2. Resolveer elk anker van v1 tegen `canonical(v2)`: exact → `unchanged` · fuzzy boven de
   drempel → `reworded` · geen match → `candidate-removed`.
3. Match de v2-statements op v1 via anker en slug; wat overblijft is `added`.
4. Elke `reworded` krijgt een strekking-oordeel: `same-strekking` (redactioneel) of
   `changed-strekking` (norm-relevant) — de tweede altijd met menselijke bevestiging.

Statement-identiteit is gelaagd: een inhoud-afgeleide **slug** (`tweede-vrijlating-50pct`) is
de sleutel, `S<n>` is alleen een weergavenummer, en het anker is het zoekmiddel. Nooit
paginanummer of volgnummer als identiteit — een herzette PDF verschuift alles.

## Bestanden

**references/** — `classificatie.md` (buckets, afwijkingsklassen, bindendheid,
documentstatus) · `signaalnet.md` (het lexicon + verantwoording) · `verankering.md` (de
zoektocht en hoe je hem vastlegt) · `diff.md` (modus 2).

**templates/** — `register.md`, het registersjabloon.

**scripts/** — `canonicalize.sh` (bron → canonical.md + pages.tsv + manifest.yaml; PDF- en
HTML-tak) · `html_canonical.py` (de HTML-lezer, stdlib) · `tile.py` (fase 2: concept-ledger,
100% dekkend by construction) · `statement_gates.py` (de vier gates).

**tests/** — synthetische fixture met vijf opzettelijke defecten + `run.sh`; zie
`tests/README.md`.

## Waar de output landt

- **Werkbestanden** (`canonical.md`, `pages.tsv`, `manifest.yaml`, `statements.yaml`,
  `coverage-report.md`) zijn scratch. Ze horen niet in een corpus-repo.
- **Het register** (`templates/register.md`) landt in het dossier, naast de andere
  audit-producten van de sessie.
- **Bronbestanden blijven waar het dossier ze bewaart.** Haal nooit een dossier-document,
  citaat of registerinhoud naar een publieke repo. De fixture in `tests/` is verzonnen en
  moet dat blijven.

## Belangrijke regels

- **Classificeer nooit vóór je betegeld hebt.** De volgorde is de garantie.
- **Een `niet-gevonden` zonder zoektermen is geen bevinding.** De gate dwingt dit af.
- **Toelichting-statements worden geen `machine_readable`.** Wie een FAQ-antwoord van
  executielogica voorziet, heeft de toelichting tot norm gepromoveerd. Als een statement
  volgens de jurist bindend beleid is, hoort het in de regeling — dát is de aanbeveling,
  niet een modelwijziging.
- **Niets committen zonder toestemming**, in geen enkele repo.
- Zusterskills: `law-letter-fidelity-audit` (dezelfde buckets, maar per wetsartikel in plaats
  van per beleidsdocument), `law-mvt-research` (rekenvoorbeelden → Gherkin),
  `regelrecht-stelselanalyse` (waar de bevindingen in de 4-weg-classificatie landen).
