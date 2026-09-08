# POC feature conversion: notes

Log of converting the POC Gherkin feature files (Dutch step dialect) to the
canonical regelrecht BDD grammar (`bdd/grammar.yaml`) with
`corpus/demo/tools/convert_features.mjs`, and of running them with
`just bdd-demo`. Every judgement call lives in the converter or in one of the
two law-rewrite tools next to it; the generated `.feature` files under
`corpus/demo/regulation/nl/**/scenarios/` are never edited by hand.

Regenerate and run:

    node corpus/demo/tools/convert_features.mjs <poc>/features corpus/demo/regulation/nl
    uv run corpus/demo/tools/rewrite_array_property_access.py corpus/demo/regulation/nl
    uv run corpus/demo/tools/round_amount_outputs.py corpus/demo/regulation/nl
    just bdd-demo

The two `uv run` tools are idempotent; running them again reports 0 rewrites.

## Counts (last run)

| | |
|---|---|
| POC feature files | 46 |
| skipped as non-engine (`integratie/`, `web/`, `synthesize.feature`) | 5 |
| converted feature files | 41 |
| scenarios converted | 331 |
| `@wip` (skipped by the runner) | 1 |
| run by `just bdd-demo` | 330 |
| passing | 330 |
| failing | 0 |

## Rules applied

### Structure

- One canonical `Background:` with the calculation date and the parameters of the
  POC Background. Register data goes into each scenario, because it differs per
  scenario. A POC Background table is inherited by every scenario.
- Every POC `When de <law> wordt uitgevoerd door <service>` becomes
  `When I evaluate outputs "<asserted outputs>" of "<law $id>"`; the output list is
  exactly what the scenario's Then steps assert (deduplicated, in order). A
  scenario with several Whens keeps them as phases (archiefwet, omgevingswet,
  kinderopvang, huurtoeslag); parameters a later phase adds (a `met` table, a
  claims table) are emitted right before its When.
- The law is looked up by `$id` (the POC law name); when several versions exist
  the latest `valid_from` not after the calculation date decides which outputs
  exist.
- The generated file lives in the directory of the first evaluated law:
  `corpus/demo/regulation/nl/<law dir>/scenarios/<poc file name>`.

### Given steps

| POC | canonical |
|---|---|
| `de datum is "D"` | `the calculation date is "D"` |
| `een persoon met BSN "X"` | `parameter "bsn" is "X"` |
| `een organisatie met KVK-nummer "X"` / `een onderneming met KVK nummer "X"` | `parameter "kvk_nummer" is "X"` |
| `een werkgever met loonheffingennummer "X"` | `parameter "loonheffingennummer" is "X"` |
| `een werknemer met bruto jaarloon "X" euro` | `parameter "bruto_loon" is X` |
| `de aanvraag betreft een "X"` | `parameter "aanvraag_type" is "X"` |
| `een <entity> met ID "X"` | parameter per steps.py (`ICT-project`/`project` -> `project_id`, `organisatie` -> `organisatie_id`, else `<entity>_id`) |
| `een archiefstuk met de volgende eigenschappen` + table | `the following parameters:` (name/value rows) |
| `er is geen Bibob-advies uitgebracht voor deze onderneming` | comment (no bibob_adviezen row applies) |
| `de volgende <SERVICE> <table> gegevens` + table | register rows for the materialiser (below) |
| `When ... met` + table, `When de burger deze gegevens indient:` (claims) | `the following parameters:` before the When; a claim overrides the input of the same name, which is what a parameter does in the engine |

Parameters the evaluated law declares but the scenario never supplies are passed
as `parameter "x" is "null"`: they were None in the POC, and the engine has no
default for an absent parameter (the schema has none either).

A POC phase that only established that required data was still missing
(`ontbreken er verplichte gegevens`) is dropped with a comment; it is an
application-level check the engine has no equivalent for. The scenario continues
with the values the citizen then supplied.

### Then steps

All mappings follow `features/steps/steps.py`; the notable ones:

- `is voldaan aan de voorwaarden` -> `output "voldoet_aan_voorwaarden" is true`;
  when the law has no such output -> `the execution succeeds` (steps.py:
  `requirements_met = bool(outputs)`). The negative form without the output would
  become a comment and tag the scenario `@wip`; it did not occur.
- Euro amounts (`is het toeslagbedrag "X" euro` etc.) -> eurocent integer
  (`Math.round(X * 100)`); the output name follows steps.py (`hoogte_toeslag`
  else `jaarbedrag`, `pensioen_uitkering_maandelijks` else `pensioenbedrag`,
  `uitkeringsbedrag`, `woonkostentoeslag`, `startkapitaal`, `bedrijfskapitaal_max`,
  `subsidiebedrag`). `is het <veld> "N" eurocent` uses steps.py's field aliases.
- `heeft de output "f" waarde "v"` / `is het veld "f" gelijk aan "v"`: `v` is read
  as the POC did (JSON, then int, then string) and emitted as `is true/false`,
  `is null`, `equals N` or `equals "s"`.
- `is de output "f" leeg` / `is het veld "f" een lege lijst` ->
  `output "f" equals "[]"`. This leans on the JSON-cell extension below: a quoted
  value starting with `[` or `{` is parsed as JSON.
- `bevat de output "f" waarde "v"` -> `output "f" contains "v"` only when the law
  types `f` as a string. Array membership (177 POC assertions, nearly all in the
  delegation-provider laws) is not expressible in the canonical grammar and
  becomes a comment; negative membership likewise. Those scenarios keep their
  other assertions (`heeft_delegaties`, `voldoet_aan_voorwaarden`, ...).
- Approximate POC assertions (WW 1%, kindgebonden budget 2%) are emitted exactly;
  where the engine's value differs it is adopted through the `ADOPTED` table in
  the converter with the POC's text as a comment. One case: kindgebonden budget
  392541 where the POC said "ongeveer €3.925,00".
- The qualitative kindgebonden-budget steps (`lager door hoog inkomen`, ...) would
  be emitted as exact assertions on the observed value via `ADOPTED`; they only
  occur in the skipped `integratie/` files.
- Case-management steps and the whole `integratie/`, `web/` and
  `synthesize.feature` files are skipped: they test the POC application, not the
  engine.

### Register data

The POC tables are fed through the demo materialiser
(`frontend-demo/src/data/materialize.js`, the code the demo app uses) with
`corpus/demo/bindings.yaml`. Cells are parsed the way steps.py did: the
`STRING_FIELDS` columns stay strings (`null`/empty -> null), everything else JSON,
then int, then string. A table given again (in the scenario after the Background,
or twice in a scenario) replaces the earlier rows, as `set_source_dataframe` did.

For every law reachable from the evaluated law(s) through `source.regulation`
one step is emitted per (service, key field):

    Given the following "<service>" data with key "<key>" for law "<law>":

with every materialised input of that law as a column (null included), so the
runner sees the same picture as the demo app. Arrays and objects are written as
JSON in the cell. Key values are every distinct value of a key column (`bsn`,
`kvk_nummer`, `organisatie_id`, ...) across the scenario's tables plus the
parameters; for `bsn`, every `*bsn*` column counts (partners, children). An
object-valued key (`adres` in the LAA law) is written as JSON; the engine keys
every complex value as `"complex"`, which works because a scenario has one.

Four POC readings of a materialised value that the materialiser does not share
are applied in the converter (`materialiseScenario`, `applyPocValueSemantics`):

1. A binding that selects on a cross-law input (`adres: $vestigingsadres` in the
   APV laws and precariobelasting, `bsn: $partner_bsn` in wet_inkomstenbelasting
   and wet_studiefinanciering, 31 bindings) is pre-resolved by following the
   reference to the law that produces the output; when that output is a plain
   projection of a table-bound input, that input is materialised for the same
   key (a record keyed on another field, such as `terras_locatie`, uses the
   scenario's single kvk_nummer). The materialiser itself resolves `$x` only
   against parameters and table-bound inputs, so the demo app does not get these
   values today.
2. A numeric input (`amount`/`number`) whose cell is null becomes 0: the POC's
   ADD skipped None operands, the engine raises on null. The materialiser already
   does this when no row matches at all.
3. An object input (a register row) carries every property the law reads on it,
   null when the row lacks the column: `row.get(kolom)` was None in the POC, the
   engine raises on a missing key. A property the law passes straight through to
   a number/amount output (the `*_gegevens` register laws) gets 0, as in rule 2.
4. An array that consists of nulls only (a column the table never had) becomes
   null: FOREACH treats null as an empty collection, which is what the POC's None
   amounted to.

Tables the bindings never reference are dropped silently. In the POC features
these are `BELASTINGDIENST inkomen`, the LAA `terugmelding`/`post_onbestelbaar`
tables, `DJI detentie`, `GEMEENTE_ROTTERDAM bibob_adviezen`, `IND vreemdelingenwet`,
`KVK bedrijfsgegevens`, `RECHTSPRAAK curatele`, `RvIG kinderen`, `UWV WIA` and
`UWV wet_structuur_uitvoeringsorganisatie_werk_en_inkomen`: mostly POC tables
named after a law, which the POC engine read as output overrides. The migrated
laws compute those outputs from the register tables the bindings do know. The
three LAA "meldt twijfel" scenarios pass because a missing register adres counts
as a deviation, not because their `terugmelding` rows reach the engine.

### Engine test-helper change

`packages/engine/tests/bdd/helpers/value_conversion.rs`: a table cell (and, through
the same function, a quoted capture) that starts with `[` or `{` and parses as JSON
becomes an Array/Object value. Needed for array- and object-typed register inputs
(`kinderen_leeftijden`, `adres`). A cell that does not parse stays a string. Unit
test `test_convert_json_array_and_object` in the same file (runs in the
`bdd_table` test target). Mirrored in `packages/frontend-shared/src/gherkin/actions.js`
(`parseValue`).

## Law and binding changes

All in `corpus/demo/`; each carries a YAML comment at the site. The demo laws
all validate against schema v0.5.7 after the changes (`script/validate.sh`).

### Made by tools (re-runnable, idempotent)

`corpus/demo/tools/rewrite_array_property_access.py`:

- POC implicit list mapping `$lijst.veld` on an array-valued variable -> `FOREACH`
  without `combine` (returns the body values as an array). 40 action values in
  12 laws (the delegation providers, handelsregisterwet, machtigingenwet, wgbo).
- POC `EXISTS $lijst` on an array (migrated as `NOT (EQUALS $x null)`, true for an
  empty list) -> `GREATER_THAN (FOREACH count) 0`, false for null and for the
  empty list, as EXISTS was. 27 sites in 13 laws. Scalars keep the null test.
- Reported, not rewritten: `handelsregisterwet` compares `$inschrijvingen.rechtsvorm`
  (a list) with `IN`; the POC semantics there have no single equivalent. No
  scenario reaches it with data.

`corpus/demo/tools/round_amount_outputs.py`: explicit rounding (RFC-024) on the
outputs whose POC scenarios assert whole eurocents:
`algemene_ouderdomswet.pensioenbedrag`, `pensioenwet.pensioen_uitkering_maandelijks`,
`werkloosheidswet.ww_uitkering_per_maand`, `wet_inkomstenbelasting.totale_belastingschuld`
and `.totale_heffingskortingen`, `wet_op_het_kindgebonden_budget.kindgebonden_budget_jaar`,
`zorgtoeslagwet.hoogte_toeslag` (both versions), `zvw/werkgeversbijdrage.zvw_werkgeversbijdrage`,
`participatiewet/bijstand/amsterdam.uitkeringsbedrag`, and
`wet_structuur_uitvoeringsorganisatie_werk_en_inkomen.verzekerde_jaren` (precision 2,
which makes the AOW opbouw 48/50 exactly as in the POC). ROUND (half-up) is used:
the POC expectations 126307.84 -> 126308, 209691.79 -> 209692, 210820.80 -> 210821
and 470595.96 -> 470596 rule out truncation. The one exception is
`werkloosheidswet.ww_duur_maanden`, where 12.5 is asserted as 12: FLOOR. The
rounding is null-guarded (`IF value null -> null`), because the engine's
arithmetic lets a null operand through but ROUND/FLOOR insist on a number; WW
evaluated for a Bbz applicant without dienstverbanden produced None in the POC
and would otherwise fail here.

### Made by hand

- Input order (the engine resolves inputs in declaration order; the POC resolved
  lazily): the register inputs that cross-law inputs pass as `$parameter` are
  declared first in `alcoholwet/vergunning/rotterdam` (bsn, vloeroppervlakte,
  type_bedrijf), `algemene_plaatselijke_verordening/exploitatievergunning`
  (bsn_eigenaar), `kieswet` (verkiezingsdatum) and
  `omgevingswet/energiebesparing/informatieplicht` (adres before bag_gebruiksdoel).
- `wet_brp`: `leeftijd` (AGE), `heeft_vast_adres`, `postadres` and `woonplaats`
  read a date or `$adres.*` that was None in the POC for a person without
  register data; the engine raises on a path on null or a null date, so the
  actions test for a value first (null / false otherwise).
- `burgerlijk_wetboek_handelingsonbekwaamheid.curator_bsn`: the one SWITCH-shaped
  site the tool reports; rewritten by hand to the same count + FOREACH form.
- `wet_structuur_uitvoeringsorganisatie_werk_en_inkomen`: a dienstverband without
  end date (empty or null) counted to the reference date in the POC; the engine's
  DATE_DIFF needs a date, so `to:` falls back to `$referencedate`.
- `wet_brp/terugmelding/belastingdienst` and `.../toeslagen`: the POC compared the
  BRP adres with the register adres as dicts that only held the register's
  columns (postcode, huisnummer). The materialised object also carries straat and
  woonplaats (null), so structural equality always failed; the comparison is now
  field-wise on postcode and huisnummer, with a missing register adres still
  counting as a deviation (as in the POC). Toeslagen also guards
  `$toeslagen_onderzoek.adres_geverifieerd` against a missing onderzoek.
- `alcoholwet/vergunning/rotterdam` passes the two optional parameters of
  `alcoholwet/vergunning` explicitly: `is_ingeschreven_svh_register` from its own
  `svh_registratie_geldig` input and `is_van_slecht_levensgedrag` from a new
  definition `geen_slecht_levensgedrag: false`. They were None in the POC when
  absent. The engine has no default for an absent parameter, and a null parameter
  makes it skip the referenced law altogether (RFC-007), so a constant is the
  only way to say "no such registration" here.
- `wet_kinderopvang.is_gerechtigd` was the constant `true`; the POC only computed
  outputs when the requirements held, so it read as false for someone who did not
  qualify. Now `$voldoet_aan_voorwaarden`. The other 25 constant-true boolean
  outputs in the corpus are left alone: for most of them the constant is the
  intended meaning (`heeft_delegaties: true`, "everyone has a SELF delegation"),
  and no scenario depends on the gating.
- By the coordinator, kept: `algemene_ouderdomswet/leeftijdsbepaling` declares an
  optional `bsn` parameter and its eight callers pass `bsn: $bsn`; the
  materialiser defaults a number/amount input with no register row to 0.

## `@wip`

- `zorgtoeslagwet_TOESLAGEN-2025-01-01 :: Persoon onder 18 heeft geen recht op zorgtoeslag`:
  POC data problem. Born 2007-01-01, calculation date 2025-02-01: the person is 18,
  the engine says voldoet_aan_voorwaarden true, the POC asserted "onder 18".

## Worth knowing

- The engine skips a referenced law when any parameter passed to it is null
  (`resolve_external_input_internal`, RFC-007) and errors when a declared
  parameter is absent. A law that wants "unknown" for an optional parameter can
  therefore only get it through a definition or a non-null register value.
- `$name.prop` on an array errors (`expected object, got array`); on null it
  errors as well; a missing key errors. The POC gave None in all three cases.
  Rules 2 to 4 above and the wet_brp guards cover the cases the scenarios hit;
  other persona data may hit more.
- The engine evaluates every action of an article for any requested output; the
  POC computed outputs only when its `requirements` held. Constant-true outputs
  therefore read differently in negative cases (see wet_kinderopvang).
