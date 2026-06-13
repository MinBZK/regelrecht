# Check-keten — waartegen wordt wets-zuiverheid & cross-law getoetst

Overzicht van álle checks op de correctheid van een machine-leesbare wet en op
cross-law-koppelingen, gerangschikt van puur-syntactisch (volledig geautomatiseerd)
naar juridisch-inhoudelijk (mens/agent-gedreven). Per check: **waartegen** wordt
afgemeten (het *orakel*), en of het een harde CI-gate is of methodologisch.

Een **orakel** is de bron van waarheid waaraan een check het verdict afleest. Een
CI-gate kan alleen op een *hermetisch* orakel draaien: stabiel, lokaal in de repo,
reproduceerbaar. Een orakel dat op afstand staat en kan veranderen (de live
wettekst) is niet gateable en hoort in de methodologische laag.

## De vijf lagen

```
SCHEMA ──► ENGINE ──► CROSS-LAW ──► METHODOLOGIE ──► EXTERNE BRON
└──── geautomatiseerd (CI) ────┴──── mens/agent-gedreven ────┘
```

De kern-spanning: **CI bewaakt vorm (formele correctheid), niet betekenis
(juridische getrouwheid).** Dat laatste leeft in de drift-check (Step 0) en de
expert-workshop.

## Orakel-catalogus

| Check | Toetst | Orakel | Type |
|---|---|---|---|
| Schema-validatie (`just validate`) | structuur/velden/types | `schema/vX.Y.Z/schema.json` (Draft-07) | CI-gate |
| Serde-deserialisatie | type-integriteit | Rust-structs (`ArticleBasedLaw`) | CI-gate |
| yamllint | YAML-stijl | `.yamllint` | CI-gate |
| `protect-schema` | onveranderlijkheid releasede schema's | git diff vs `origin/main` | CI-gate (PR) |
| `provenance-checks` (RFC-013) | schema-registratie, `$schema`-refs, symlink | repo-structuur | CI-gate (PR) |
| Engine-resolver (runtime) | bestaan wet/artikel/output, cycles, types | de geladen corpus zelf | alleen bij uitvoering (BDD) |
| **cross-law-integriteit** | MISPLACED/DANGLING/PLAIN-PARAM/IMPL-DANGLING/IMPL-NO-DATE | `regulation → outputs` + `open_terms`-index uit de corpus | **CI-gate** (`cross-law-integrity` job) |
| BDD-features | end-to-end reken-uitkomsten incl. IoC | verwachte waarden in `features/*.feature` | CI-gate — *let op meta-check* |
| RFC-013 execution receipt | reproduceerbaarheid | engine+schema+regulation-hash+scope | runtime |
| **drift-check** (Step 0) | `text:` ≡ geldende wettekst (structureel + tekstueel) | **wetten.overheid.nl/`<bwb>`/`<valid_from>`** (+ Staatsblad) | methodologisch (WebFetch + kalibratie) |
| 4-weg-classificatie | oorzaak/route van een bevinding | wet + jurisprudentie + schema/engine | methodologisch |
| defect-taxonomie | type/ernst wetgevings-fout | Staatsblad, nota van toelichting, ECLI | methodologisch |
| verificatie-cyclus | houdt een claim stand | externe bronnen via WebFetch | semi-automatisch |

## Cross-law specifiek

Twee mechanismen:
- **Directe sourcing** — `source: {regulation, output, parameters}` op een `input:`-veld;
  engine resolt via een `output_index`. Faalt pas bij uitvoering als de output ontbreekt.
- **IoC / delegatie** — `open_terms` (hogere wet) ↔ `implements` (lagere regeling), met
  temporele (`valid_from`) en scope-filtering (`gemeente_code`) en *lex superior > lex
  posterior*.

De statische gate is `cross-law-integriteit.py` (zie de skill-bestandenlijst). Sinds de
uitbreiding dekt hij ook de IoC-kant: `implements` moet naar een echt gedeclareerd
`open_term` wijzen (IMPL-DANGLING) en implementing-regelingen moeten `valid_from` dragen
(IMPL-NO-DATE; anders matcht de RFC-003-temporele filter elke datum). Draait nu als
CI-gate over `corpus/regulation`.

## De meta-check (scharnierpunt)

*"Maken de features en de YAML dezelfde fout?"* Zo ja, dan valideert de groene BDD-suite
de YAML tegen zichzelf, niet tegen de wet — groen bewijst dan niets over juridische
correctheid. Daarom bestaat de drift-check als gatekeeper vóór elke YAML-edit.

## De gaten — en hoe ze geadresseerd zijn

| Gap | Aard | Status |
|---|---|---|
| **1. cross-law niet in CI** | echte automatiseerbare gate | ✅ opgelost — `cross-law-integrity` CI-job |
| **2. `implements` niet gevalideerd** | echte automatiseerbare gate | ✅ opgelost — IMPL-DANGLING + IMPL-NO-DATE in het script |
| **3. tekst-getrouwheid niet in CI** | *orakel is de live wet → niet hermetisch* | ⏳ golden-text-subsysteem (aparte PR), zie hieronder |
| **4. `coverage_score` ≠ correctheid** | semantiek/duidelijkheid, geen gate | ✅ verhelderd — doc-comment op het veld + deze notitie |

### Waarom gap 3 anders is (en hoe het tóch kan)

De *vergelijking* `text:` vs referentietekst is triviaal deterministisch. Het
niet-deterministische zit in het **orakel**: de drift-check meet tegen de **live**
geldende tekst op wetten.overheid.nl — een remote, mutabele, te-interpreteren bron
(render-lag, rate-limiting, HTML-extractie). Daarom gebruikt de drift-check twee
onafhankelijke fetches die moeten overeenkomen, kalibratie ≥80% op ijkpunten, en
Staatsblad-fallback. Dat is statistisch, geen schone `==` — en dus geen CI-gate.

**Deterministisch kan wél via een gecommitte snapshot (golden-text):**
1. **Capture & bless** (methodologisch, drift-check, periodiek): haal per chunk de
   geldende tekst op, normaliseer volgens de verbatim-spiegelregel (zie
   `law-version-drift-check/reference.md` §2), en commit hash + tekst + herkomst.
2. **Gate** (deterministisch, CI, per-commit): herbereken de genormaliseerde hash van
   elk YAML-`text:`-chunk en vergelijk met de fixture. Mismatch → faal.

De eerlijke grens: de gate bewaakt *"YAML ≡ laatst geverifieerde tekst"*, niet *"YAML ≡
huidige live wet"*. Stille YAML-edits → CI faalt direct. Verandert de wet in de wereld →
alleen de periodieke drift-check (laag 1) vangt het. Dit is "approval-testing" + per-chunk
provenance (verwant aan RFC-013's `regulation_hash`, maar per lid i.p.v. per bestand).
