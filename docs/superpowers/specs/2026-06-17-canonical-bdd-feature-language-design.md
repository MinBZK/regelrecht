# Canonieke, engine-onafhankelijke BDD feature-taal — design

**Datum:** 2026-06-17
**Component:** `bdd/` (nieuw, repo-root), `packages/engine/tests/bdd/`, `frontend/src/gherkin/`
**Branch:** `feat/bdd-canonical-grammar`
**Aanleiding:** codebase-audit 2026-06-11, advies 14 ("Gherkin-stappen uit één bron")

## Doel

Eén **canonieke, wet-agnostische BDD feature-taal** voor regelrecht, gedefinieerd
door een machine-leesbare grammar die de bron van waarheid is. Uit die grammar
worden de step-bindings voor **elke** engine gegenereerd (codegen), zodat
hetzelfde `.feature`-bestand verbatim draait in de editor, onder `just bdd`, en
op elke toekomstige engine. Drift tussen engines is daarmee onmogelijk
by-construction.

## Context (huidige situatie)

Er bestaan vandaag **twee gescheiden Gherkin-dialecten** die stil uit elkaar lopen:

- **Rust** (`packages/engine/tests/bdd/steps/{given,when,then,notes}.rs`, ~40
  stappen): rijk en **domeinspecifiek** — `the bijstandsaanvraag is executed for
  participatiewet article 43`, `the uitkering_bedrag is "X" eurocent`, `the
  citizen has the right to bijstand`. Elke nieuwe wet vraagt nieuwe stap-zinnen
  en dus engine/test-code. Wordt gedraaid door `just bdd` tegen
  `features/*.feature`, met alle wetten geladen uit de **levende**
  `corpus/regulation/nl/` (via `helpers/regulation_loader.rs`).
- **JavaScript** (`frontend/src/gherkin/steps.js` + `formMapper.js`, ~20 stappen):
  generiek — `I evaluate "x" of "y"`, `output "x" equals N`, `parameter "x" is
  "y"`, `the following "personal_data" data with key "bsn":`. Voedt de
  scenario-builder in de editor. `steps.js` voert uit tegen de WASM-engine,
  `formMapper.js` mapt AST ↔ formulier-state (en is een derde, deels overlappende
  spiegel).

**De drift, concreet:**
`corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature`
staat volledig in het **editor-dialect** en draait daardoor **niet** onder `just
bdd` — de Rust-engine kent zijn stappen niet. De waardevolle, met de hand
geschreven wet-validatiescenario's (inclusief zorgvuldige `# NB:`-annotaties die
"gewenste uitkomst vs huidige engine" documenteren) worden dus alleen door de
editor gedragen, niet in CI tegen de levende wet.

**Belangrijke observaties uit de repo:**
- Het patroon "scenario's naast de wet" bestaat al:
  `corpus/regulation/.../scenarios/*.feature` (de corpus-backend leest/schrijft
  deze al via `list_files(scenarios, feature)`).
- Er zijn al doel-test-wetten in de repo (`corpus/regulation/nl/wet/test_untranslatables/`,
  `tests/fixtures/federation/*`, `frontend/e2e/fixtures/*`) — bruikbaar voor
  engine-conformance zonder nieuwe bevroren kopieën.
- `cucumber = "0.23"` (attribute-macro stijl: `#[given(expr=…)]`).

## Beslissingen (uit brainstorm)

1. **Puur wet-agnostisch.** Eén kleine vaste vocabulaire die voor elke wet werkt;
   geen per-wet zinnen. Domeinspecifieke Rust-stappen worden weg-gegeneraliseerd.
   Bestaande `features/*.feature` mogen aangepast worden.
2. **Bron van waarheid = machine-leesbare grammar + draagbare conformance-suite.**
   De grammar is normatief; de suite is uitvoerbaar bewijs.
3. **Codegen meteen** (niet gefaseerd): de step-bindings voor Rust én JS worden
   gegenereerd uit de grammar, zodat drift by-construction onmogelijk is.
4. **Canoniek = het editor-dialect** (geformaliseerd), **Rust migreert ernaartoe.**
   De editor-vocabulaire is al generiek en de waardevolle corpus-scenario's staan
   er al in; Rust is de kant die domeinstappen kwijtraakt.
5. **Geen bevroren/aparte wetten voor wet-validatiescenario's.** Die draaien tegen
   de **levende** wet — breken door een wetswijziging is het gewenste signaal
   (mens beslist: wetsfout of scenario bijwerken). Engine-conformance gebruikt de
   bestaande doel-test-wetten.
6. **Opgeslagen scenario's worden eenmalig gemigreerd** naar canoniek; geen
   permanente backward-compat-parser.

## Architectuur

### Twee buckets, één taal

Beide buckets gebruiken dezelfde grammar en gegenereerde bindings; ze verschillen
alleen in **waar de wetten vandaan komen** en **wat een failure betekent**.

**Bucket A — wet-validatiescenario's**
`corpus/regulation/.../scenarios/*.feature`. Geschreven in de editor, draaien
tegen de **levende wet**. Een failure betekent: de wet is gewijzigd of het
scenario klopt niet meer → een mens kijkt. `# NB:`-annotaties blijven behouden.
Gedraaid door: editor (WASM), `just bdd`, toekomstige engines. Tier `core`.

**Bucket B — engine-conformance-suite**
`bdd/conformance/*.feature`. Bewijst dat een engine de **hele taal** correct
spreekt, inclusief de capability-tiers (`notes`, `untranslatable`, `provenance`).
Gebruikt de bestaande doel-test-wetten (deterministisch — dit test de engine,
niet een echte wet). Gedraaid door elke engine voor zijn ondersteunde tiers.

### Layout

```
bdd/
  grammar.yaml              # normatieve vocabulaire (bron van waarheid)
  conformance/
    *.feature               # bucket B — engine-conformance, asserts inline
  codegen/
    gen-rust.*              # grammar -> Rust cucumber step-bindings
    gen-js.*                # grammar -> JS matchers + formMapper-patterns
corpus/regulation/.../scenarios/*.feature   # bucket A — wet-validatie (bestaat al)
```

### Grammar-format

`grammar.yaml` is een platte lijst canonieke stappen:

```yaml
- id: assert_output_equals
  keyword: then                       # categorie/documentatie; Gherkin And/But blijft werken
  tier: core                          # core | notes | untranslatable | provenance
  text: 'output "{output}" equals {value}'
  args:
    - { name: output, type: string }
    - { name: value,  type: value }   # value = bool/int/float/string-inferentie
  action: assert_equals               # semantische actie
```

- **Placeholder-syntax engine-neutraal**: `{naam}` + getypeerde `args`-lijst. De
  codegen vertaalt `type` → matcher (`string`, `int`, `float`, `value`, plus een
  `datatable`-vlag voor stappen met een trailing tabel).
- **Action-id**: koppelt de stap aan een semantische actie. De action-set is klein
  en vast:
  - setup: `set_calculation_date`, `load_law`, `set_parameter`, `set_data_source`
  - uitvoeren: `evaluate`
  - asserts (core): `assert_succeeds`, `assert_fails`, `assert_equals`,
    `assert_boolean`, `assert_null`, `assert_contains`
  - tier `provenance`: `assert_provenance`
  - tier `untranslatable`: `set_untranslatable_mode`, `assert_tainted`
  - tier `notes`: `set_law_articles`, `add_note`, `resolve_note`,
    `assert_note_resolves`, `assert_note_match`
- **Nieuwe verwoording = alleen grammar wijzigen.** Een nieuwe *capability* = nieuwe
  action (zeldzaam) + grammar-regel + implementatie in elke engine die de tier
  ondersteunt.

### Codegen

- **Rust** (`build.rs` in de BDD-testcrate): leest `grammar.yaml`, emit
  `OUT_DIR/generated_steps.rs` met de `#[given/when/then(expr=…)]`-functies. Elke
  functie parst zijn args volgens de grammar en roept een handgeschreven
  `Actions`-implementatie op de BDD-`World` aan. Geïncludeerd via
  `include!(concat!(env!("OUT_DIR"), "/generated_steps.rs"))`.
- **JS** (Node build-script, draait in `npm run build`/pre-build): leest de
  grammar, emit `frontend/src/gherkin/steps.generated.js` (matcher-array →
  action-dispatch) en `formMapper.generated.js` (pattern + extract). De
  handgeschreven action-implementaties (tegen WASM) blijven in een apart module.

De step-*zinnen* en arg-parsing bestaan daardoor maar één keer (de grammar);
beide kanten zijn gegenereerd.

### Engine-runner-contract

Een engine die de taal "spreekt" levert:
1. Een implementatie van de action-set (de tiers die hij ondersteunt).
2. Een runner die `.feature`-bestanden inleest, wetten laadt (bucket A: uit de
   gegeven bron/corpus; bucket B: uit de doel-test-wetten), en de gegenereerde
   bindings koppelt aan zijn action-implementatie.
3. Een gedeclareerde set ondersteunde tiers. De runner draait alleen features
   waarvan álle gebruikte tiers ondersteund worden (tier af te leiden uit
   `@tier`-tags op de feature/scenario, ongetagd = `core`).

## Migratieplan (elke stap houdt beide engines groen)

1. **Grammar + codegen opzetten.** `bdd/grammar.yaml` formaliseert de
   editor-vocabulaire (core) plus de bestaande Rust-only capabilities als
   getierde stappen. Beide generatoren bouwen; gegenereerde bindings worden
   geproduceerd náást de bestaande, nog niet aangesloten.
2. **Editor omschakelen** op `steps.generated.js` / `formMapper.generated.js`.
   Omdat canoniek ≈ het huidige editor-dialect is dit laag-risico en grotendeels
   identiek. `formStateToGherkin` emit canonieke zinnen. Eenmalige migratie van
   opgeslagen scenario's met afwijkende verwoording.
3. **Rust BDD-`World` generiek maken.** Generieke acties: `load_law` (laad
   willekeurige wet i.p.v. preloaded set + hardcoded agency-tabellen),
   `set_data_source`, `set_parameter`. Gegenereerde Rust-bindings aanzetten;
   handgeschreven domein-steps verwijderen.
4. **Features herschrijven en verplaatsen.** Wet-validatiescenario's uit
   `features/*.feature` → `corpus/regulation/.../scenarios/` (bucket A, canoniek).
   Capability-scenario's (notes/untranslatable/provenance, multi-output) →
   `bdd/conformance/` (bucket B). `just bdd` draait voortaan beide buckets.
5. **CI-borging.** Een job die de codegen draait en faalt als de gegenereerde
   files niet in sync zijn met de grammar (`git diff --exit-code`); plus beide
   engines draaien hun toepasselijke buckets. Wet-validatie (bucket A) draait
   tegen de levende corpus, zodat een wetswijziging die een scenario breekt
   zichtbaar wordt in CI.

## Wat dit oplevert t.o.v. nu

| Nu | Na |
|---|---|
| Editor-scenario draait niet onder `just bdd` | Eén scenario draait verbatim in editor, `just bdd`, en elke toekomstige engine |
| Nieuwe wet = nieuwe stap-zinnen + engine-code | Nieuwe wet = alleen scenario's schrijven, nul engine-code |
| Stapdefinities 3× handmatig, stille drift | Eén `grammar.yaml`, bindings gegenereerd → drift onmogelijk |
| Wetswijziging breekt scenario stil (alleen in editor zichtbaar) | `just bdd` draait bucket A tegen de levende wet in CI → breekt zichtbaar, mens beslist |
| Editor is de enige tool die deze features kan draaien | Engine-portabiliteit bewijsbaar via de conformance-suite |

## Niet in scope (non-goals)

- Geen wijziging aan wet-uitvoeringssemantiek of het corpus/law-formaat.
- Geen nieuwe engine bouwen — alleen de taal + de twee bestaande engines erop
  aansluiten.
- De editor ontsluit `notes`/`untranslatable`/`provenance` niet in deze stap; die
  blijven Rust-tier (apart traject; de capability-tiers maken die uitbreiding
  later mogelijk zonder de taal te versplinteren).
- Geen automatische scenario-generatie.

## Open punten voor het implementatieplan

- Exacte canonieke verwoording per stap (waar Rust en editor nu verschillen, bv.
  `the following RVIG "personal_data" data:` vs `the following "personal_data"
  data with key "bsn":` → kies de editor-vorm met expliciete key).
- Of de Rust `build.rs`-codegen attribute-macro-functies emit of overstapt op
  cucumber's programmatische step-registratie (`Cucumber::new().given(...)`) —
  te bepalen op basis van wat het schoonst genereert met cucumber 0.23.
- Vorm van de eenmalige migratie van opgeslagen editor-scenario's (script vs
  hand).
- Hoe `just bdd` bucket A ontdekt (glob over `corpus/regulation/**/scenarios/`).
