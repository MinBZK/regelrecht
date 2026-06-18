# Canonical, engine-agnostic BDD feature language — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One machine-readable `bdd/grammar.yaml` becomes the single source of truth for a small, law-agnostic Gherkin vocabulary; codegen produces the step-bindings for both the Rust cucumber engine and the editor's JS executor, so the same `.feature` file runs verbatim in the editor, under `just bdd`, and on any future engine — drift is impossible by construction.

**Architecture:** A flat grammar (list of canonical steps, each with a text template, typed args, a tier, and a semantic `action` id) is consumed by two generators. The Rust generator (`build.rs`) emits cucumber attribute-macro step functions that parse their captures and call a single hand-written `dispatch(action, args, table)` on a now-generic `World`. The JS generator emits `frontend/src/gherkin/grammar.generated.js` (patterns + emit-templates + action ids) consumed by a thin `steps.js` executor (against WASM) and by `formMapper.js` (form-state ↔ Gherkin). Two buckets share the language: **bucket A** = law-validation scenarios next to the live laws (`corpus/regulation/.../scenarios/*.feature`), **bucket B** = engine-conformance scenarios (`bdd/conformance/*.feature`) against existing synthetic test laws. Capability tiers (`core`, `notes`, `untranslatable`, `provenance`) gate which features an engine runs.

**Tech Stack:** Rust (cucumber-rs 0.23, attribute macros, `harness = false`, `build.rs` codegen, `serde_yaml`), JavaScript/ESM (`@cucumber/gherkin` 40, Vite/Vitest, Node build script), Vue 3 editor, WASM engine.

---

## Background facts (verified against the current tree)

These are the ground truths the tasks below depend on. Do not re-derive — but DO re-read the cited file before editing it.

### Rust side (`packages/engine/`)
- Entry: `packages/engine/tests/bdd/main.rs` discovers `<repo-root>/features/` and runs `RegelrechtWorld::cucumber().max_concurrent_scenarios(1).with_default_cli().run_and_exit(features_dir)`.
- `packages/engine/tests/bdd/world.rs`: `RegelrechtWorld` holds `service: LawExecutionService`, `calculation_date: String`, `parameters: BTreeMap<String, Value>`, `result: Option<ArticleResult>`, `error: Option<EngineError>`, `external_data: ExternalData` (typed per-agency maps), plus note fields (`note_articles`, `note_selector`, `note_result`). `#[derive(World)]`, `#[world(init = Self::new)]`. `Self::new()` calls `load_all_regulations(&mut service)`.
- Steps: `packages/engine/tests/bdd/steps/{given,when,then,notes}.rs` — 54 step defs total, ~40 domain-specific.
- Helpers: `helpers/regulation_loader.rs` (`load_all_regulations`, walks `corpus/regulation/nl` and `service.load_law(content)` each `.yaml`), `helpers/value_conversion.rs` (`convert_gherkin_value(&str)->Value`, `parse_table_to_params(&Table)->BTreeMap`, `values_equal_with_tolerance`, `parse_eurocent`, `parse_euro_to_eurocent`), `helpers/mod.rs`.
- `packages/engine/Cargo.toml`: `[[test]] name="bdd" harness=false`; `cucumber = "0.23"` in dev-deps. No `build.rs` yet.
- `Justfile` recipe `bdd:` → `cd packages/engine && {{ci_flags}} cargo test --test bdd -- --nocapture`.
- Data-source registration: `when.rs::execute_healthcare_allowance` registers each external source onto `service` (mirror its exact call — read it to get the method name/signature, e.g. `service.register_data_source(name, key, records)`).
- Provenance API: `then.rs` reads `OutputProvenance::{Direct,Reactive,Override}` off the result. Untranslatable: `service.set_untranslatable_mode(...)`, `value.is_untranslatable()`. Notes: `annotation::resolve(selector, articles) -> MatchResult`, `TextQuoteSelector`.

### JS side (`frontend/`)
- `frontend/src/gherkin/parser.js`: `parseFeature(text)` → `{feature, background: Step[]|null, scenarios: [{name, tags, steps}]}`; step = `{keyword, text, dataTable?: string[][], docString?}`. Uses `@cucumber/gherkin` 40.
- `frontend/src/gherkin/steps.js`: `createStepDefinitions({loadDependency})` → array of `{pattern: RegExp, execute: async (ctx, engine, match, step) => void}`. 18 patterns (listed in Task 1). Helpers `parseValue`, `tableToRecords`, `getOutput`, `assertOutput`, `primitiveEqual`. Engine calls: `engine.hasLaw`, `engine.execute(lawId, output, params, date)`, `engine.registerDataSource(name, key, records)`.
- `frontend/src/gherkin/formMapper.js`: `PATTERNS` (17 extraction patterns → form-state fragments), `mapFeatureToForm(parsed)`, `getEffectiveSetup`, `formStateToGherkin(formState)` (emits canonical phrasings), `syncEditedValues`. ~522 lines.
- `frontend/src/gherkin/context.js`: `ExecutionContext` (`calculationDate`, `parameters`, `result`, `error`, `executed`).
- Consumers: `frontend/src/components/ScenarioBuilder.vue` (imports `parseFeature`, `mapFeatureToForm`, `getEffectiveSetup`, `formStateToGherkin`, `syncEditedValues`), `frontend/src/components/ScenarioForm.vue` (imports `parseValue`; calls `engine.registerDataSource`, `engine.executeWithTrace`, `engine.clearDataSources`).
- Vitest config in `frontend/vite.config.js`: `pool: 'vmThreads'`, `server.deps.inline: [/@cucumber\//]`.

### Features & corpus
- `features/`: `bezwaartermijn, bijstand, date_operations, einddatum, erfgrensbeplanting, multi_output, negative_scenarios, notes, untranslatables, woo, zorgtoeslag`.
- **Bucket A (live-law validation)**: `bijstand`(→participatiewet), `zorgtoeslag`(→wet_op_de_zorgtoeslag), `woo`(→wet_open_overheid), `erfgrensbeplanting`(→burgerlijk_wetboek_boek_5), `bezwaartermijn`(→vreemdelingenwet_2000). All referenced laws exist in `corpus/regulation/`.
- **Bucket B (conformance)**: `notes`(no law), `untranslatables`(test_untranslatables), `multi_output`(test_untranslatables + vreemdelingenwet_2000), `date_operations`(test_date_operations), `einddatum`(test_einddatum + test_einddatum_afnemer). `negative_scenarios` splits: error-handling → B, law-boundary checks → A.
- Synthetic test laws present: `corpus/regulation/nl/wet/test_untranslatables/`, `test_date_operations/`, `test_einddatum/`, `test_einddatum_afnemer/`. (Notes need no law.)
- `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature` already exists in canonical editor dialect (the reference example).
- Scenario-file infra already shipped: `packages/editor-api/src/corpus_handlers.rs` (`list_scenarios`, `get_scenario`, `save_scenario`) + `packages/corpus/src/backend.rs` (`list_files(dir, Some("feature"))`).

### The canonical vocabulary (normative — built in Task 1)

Core (editor dialect, every engine supports):

| action | given/when/then | canonical text | args |
|---|---|---|---|
| `set_calculation_date` | given | `the calculation date is "{date}"` | date:string |
| `load_law` | given | `law "{law}" is loaded` | law:string |
| `set_parameter` | given | `parameter "{name}" is "{value}"` | name:string, value:string |
| `set_parameter` | given | `parameter "{name}" is {value}` | name:string, value:number |
| `set_parameters_table` | given | `the following parameters:` + table | — |
| `set_data_source` | given | `the following "{source}" data with key "{key}":` + table | source:string, key:string |
| `evaluate` | when | `I evaluate "{output}" of "{law}"` | output:string, law:string |
| `assert_succeeds` | then | `the execution succeeds` | — |
| `assert_fails` | then | `the execution fails` | — |
| `assert_fails_with` | then | `the execution fails with "{message}"` | message:string |
| `assert_boolean` | then | `output "{output}" is true` | output:string, (literal true) |
| `assert_boolean` | then | `output "{output}" is false` | output:string, (literal false) |
| `assert_equals` | then | `output "{output}" equals {value}` | output:string, value:number |
| `assert_equals` | then | `output "{output}" equals "{value}"` | output:string, value:string |
| `assert_null` | then | `output "{output}" is null` | output:string |
| `assert_contains` | then | `output "{output}" contains "{value}"` | output:string, value:string |

Tier `provenance` (multi-output / output-set / provenance — Rust-only for now):

| action | kw | text | args |
|---|---|---|---|
| `evaluate_outputs` | when | `I evaluate outputs "{outputs}" of "{law}"` | outputs:string, law:string |
| `assert_exact_outputs` | then | `the result contains exactly the outputs "{outputs}"` | outputs:string |
| `assert_provenance` | then | `output "{output}" has direct provenance` | output:string, (literal "direct") |
| `assert_provenance` | then | `output "{output}" has reactive provenance` | output:string, (literal "reactive") |
| `assert_provenance` | then | `output "{output}" has override provenance` | output:string, (literal "override") |

Tier `untranslatable`:

| action | kw | text | args |
|---|---|---|---|
| `set_untranslatable_mode` | given | `the untranslatable mode is "{mode}"` | mode:string |
| `assert_tainted` | then | `output "{output}" is tainted as untranslatable` | output:string |

Tier `notes` (RFC-005/018; generic already):

| action | kw | text | args |
|---|---|---|---|
| `set_note_articles` | given | `a law with the following articles:` + table | — |
| `set_note_selector_exact` | given | `a note selecting "{quote}"` | quote:string |
| `set_note_selector_context` | given | `a note selecting "{quote}" with prefix "{prefix}" and suffix "{suffix}"` | quote, prefix, suffix:string |
| `set_note_hint_article` | given | `the note hints article "{article}"` | article:string |
| `set_note_hint_position` | given | `the note hints article "{article}" at position {start} to {end}` | article:string, start:number, end:number |
| `resolve_note` | when | `the note is resolved` | — |
| `assert_note_resolves` | then | `the note resolves to article "{article}"` | article:string |
| `assert_note_exact_match` | then | `the note is an exact match` | — |
| `assert_note_fuzzy_match` | then | `the note is a fuzzy match` | — |
| `assert_note_orphaned` | then | `the note is orphaned` | — |
| `assert_note_ambiguous` | then | `the note is ambiguous` | — |

**Dispatch model (keeps codegen trivial):** every generated step parses its captures into an ordered `args` list (each `Str`/`Num`/`Bool`), appends any grammar `literals`, and calls one hand-written `dispatch(action_id, args, table)`. All semantics live in ONE Rust file (`dispatch.rs`) and ONE JS file (`actions.js`). Codegen needs zero per-action knowledge.

---

## Phase 0 — Scaffolding & grammar (source of truth)

### Task 0.1: Create the `bdd/` directory and grammar schema doc

**Files:**
- Create: `bdd/README.md`

- [ ] **Step 1: Write `bdd/README.md`**

```markdown
# bdd/ — canonical BDD feature language

`grammar.yaml` is the single source of truth for regelrecht's law-agnostic
Gherkin vocabulary. Step bindings for every engine are GENERATED from it:

- Rust: `packages/engine/build.rs` → `$OUT_DIR/bdd_generated_steps.rs`
- JS:   `bdd/codegen/gen-js.mjs` → `frontend/src/gherkin/grammar.generated.js`

Never hand-edit a generated file. Change `grammar.yaml`, then run
`just bdd-codegen` (which regenerates both and is checked in CI).

## Two buckets, one language
- **Bucket A — law validation**: `corpus/regulation/**/scenarios/*.feature`.
  Run against the LIVE laws. A failure means a law changed or the scenario is
  stale — a human decides. Tier `core` only.
- **Bucket B — engine conformance**: `bdd/conformance/*.feature`. Prove an
  engine speaks the whole language (incl. `notes`, `untranslatable`,
  `provenance` tiers) against synthetic `test_*` laws.

## Tiers
`core` (all engines), `notes`, `untranslatable`, `provenance`. A feature's
required tiers come from its `@tier:<name>` tags (untagged = `core`). A runner
only runs features whose tiers it supports.
```

- [ ] **Step 2: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add bdd/README.md
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "docs(bdd): scaffold bdd/ directory and language README"
```

### Task 0.2: Author `bdd/grammar.yaml` (the normative grammar)

**Files:**
- Create: `bdd/grammar.yaml`

- [ ] **Step 1: Write `bdd/grammar.yaml`** — transcribe the vocabulary tables above exactly. Schema per entry: `id` (unique), `action`, `keyword` (given|when|then), `tier` (core|notes|untranslatable|provenance), `text` (canonical phrasing with `"{name}"` for quoted-string args and `{name}` for numeric args), optional `args` (ordered list of `{name, type}` where type ∈ string|number), optional `datatable: true`, optional `literals` (ordered list of extra values appended to args after captures; YAML bool → `Bool`, string → `Str`, number → `Num`).

```yaml
version: 1

# Canonical, law-agnostic BDD vocabulary. Source of truth — codegen reads this.
# text: "{x}" = quoted string capture; {x} = bare numeric capture.
# Quoted-only steps emit Cucumber-expression bindings ({string}); any numeric
# arg forces a regex binding. See bdd/codegen/.

steps:
  # ---------- core: setup ----------
  - id: set_calculation_date
    action: set_calculation_date
    keyword: given
    tier: core
    text: 'the calculation date is "{date}"'
    args: [{ name: date, type: string }]

  - id: load_law
    action: load_law
    keyword: given
    tier: core
    text: 'law "{law}" is loaded'
    args: [{ name: law, type: string }]

  - id: set_parameter_string
    action: set_parameter
    keyword: given
    tier: core
    text: 'parameter "{name}" is "{value}"'
    args: [{ name: name, type: string }, { name: value, type: string }]

  - id: set_parameter_number
    action: set_parameter
    keyword: given
    tier: core
    text: 'parameter "{name}" is {value}'
    args: [{ name: name, type: string }, { name: value, type: number }]

  - id: set_parameters_table
    action: set_parameters_table
    keyword: given
    tier: core
    text: 'the following parameters:'
    datatable: true

  - id: set_data_source
    action: set_data_source
    keyword: given
    tier: core
    text: 'the following "{source}" data with key "{key}":'
    args: [{ name: source, type: string }, { name: key, type: string }]
    datatable: true

  # ---------- core: execute ----------
  - id: evaluate
    action: evaluate
    keyword: when
    tier: core
    text: 'I evaluate "{output}" of "{law}"'
    args: [{ name: output, type: string }, { name: law, type: string }]

  # ---------- core: asserts ----------
  - id: assert_succeeds
    action: assert_succeeds
    keyword: then
    tier: core
    text: 'the execution succeeds'

  - id: assert_fails
    action: assert_fails
    keyword: then
    tier: core
    text: 'the execution fails'

  - id: assert_fails_with
    action: assert_fails_with
    keyword: then
    tier: core
    text: 'the execution fails with "{message}"'
    args: [{ name: message, type: string }]

  - id: assert_boolean_true
    action: assert_boolean
    keyword: then
    tier: core
    text: 'output "{output}" is true'
    args: [{ name: output, type: string }]
    literals: [true]

  - id: assert_boolean_false
    action: assert_boolean
    keyword: then
    tier: core
    text: 'output "{output}" is false'
    args: [{ name: output, type: string }]
    literals: [false]

  - id: assert_equals_number
    action: assert_equals
    keyword: then
    tier: core
    text: 'output "{output}" equals {value}'
    args: [{ name: output, type: string }, { name: value, type: number }]

  - id: assert_equals_string
    action: assert_equals
    keyword: then
    tier: core
    text: 'output "{output}" equals "{value}"'
    args: [{ name: output, type: string }, { name: value, type: string }]

  - id: assert_null
    action: assert_null
    keyword: then
    tier: core
    text: 'output "{output}" is null'
    args: [{ name: output, type: string }]

  - id: assert_contains
    action: assert_contains
    keyword: then
    tier: core
    text: 'output "{output}" contains "{value}"'
    args: [{ name: output, type: string }, { name: value, type: string }]

  # ---------- tier: provenance / multi-output ----------
  - id: evaluate_outputs
    action: evaluate_outputs
    keyword: when
    tier: provenance
    text: 'I evaluate outputs "{outputs}" of "{law}"'
    args: [{ name: outputs, type: string }, { name: law, type: string }]

  - id: assert_exact_outputs
    action: assert_exact_outputs
    keyword: then
    tier: provenance
    text: 'the result contains exactly the outputs "{outputs}"'
    args: [{ name: outputs, type: string }]

  - id: assert_provenance_direct
    action: assert_provenance
    keyword: then
    tier: provenance
    text: 'output "{output}" has direct provenance'
    args: [{ name: output, type: string }]
    literals: ['direct']

  - id: assert_provenance_reactive
    action: assert_provenance
    keyword: then
    tier: provenance
    text: 'output "{output}" has reactive provenance'
    args: [{ name: output, type: string }]
    literals: ['reactive']

  - id: assert_provenance_override
    action: assert_provenance
    keyword: then
    tier: provenance
    text: 'output "{output}" has override provenance'
    args: [{ name: output, type: string }]
    literals: ['override']

  # ---------- tier: untranslatable ----------
  - id: set_untranslatable_mode
    action: set_untranslatable_mode
    keyword: given
    tier: untranslatable
    text: 'the untranslatable mode is "{mode}"'
    args: [{ name: mode, type: string }]

  - id: assert_tainted
    action: assert_tainted
    keyword: then
    tier: untranslatable
    text: 'output "{output}" is tainted as untranslatable'
    args: [{ name: output, type: string }]

  # ---------- tier: notes (RFC-005 / RFC-018) ----------
  - id: set_note_articles
    action: set_note_articles
    keyword: given
    tier: notes
    text: 'a law with the following articles:'
    datatable: true

  - id: set_note_selector_exact
    action: set_note_selector_exact
    keyword: given
    tier: notes
    text: 'a note selecting "{quote}"'
    args: [{ name: quote, type: string }]

  - id: set_note_selector_context
    action: set_note_selector_context
    keyword: given
    tier: notes
    text: 'a note selecting "{quote}" with prefix "{prefix}" and suffix "{suffix}"'
    args: [{ name: quote, type: string }, { name: prefix, type: string }, { name: suffix, type: string }]

  - id: set_note_hint_article
    action: set_note_hint_article
    keyword: given
    tier: notes
    text: 'the note hints article "{article}"'
    args: [{ name: article, type: string }]

  - id: set_note_hint_position
    action: set_note_hint_position
    keyword: given
    tier: notes
    text: 'the note hints article "{article}" at position {start} to {end}'
    args: [{ name: article, type: string }, { name: start, type: number }, { name: end, type: number }]

  - id: resolve_note
    action: resolve_note
    keyword: when
    tier: notes
    text: 'the note is resolved'

  - id: assert_note_resolves
    action: assert_note_resolves
    keyword: then
    tier: notes
    text: 'the note resolves to article "{article}"'
    args: [{ name: article, type: string }]

  - id: assert_note_exact_match
    action: assert_note_exact_match
    keyword: then
    tier: notes
    text: 'the note is an exact match'

  - id: assert_note_fuzzy_match
    action: assert_note_fuzzy_match
    keyword: then
    tier: notes
    text: 'the note is a fuzzy match'

  - id: assert_note_orphaned
    action: assert_note_orphaned
    keyword: then
    tier: notes
    text: 'the note is orphaned'

  - id: assert_note_ambiguous
    action: assert_note_ambiguous
    keyword: then
    tier: notes
    text: 'the note is ambiguous'
```

- [ ] **Step 2: Sanity-check the YAML parses and ids are unique**

Run:
```bash
cd /workspace/regelrecht/.worktrees/bdd-canonical-grammar
python3 -c "import yaml,collections; s=yaml.safe_load(open('bdd/grammar.yaml'))['steps']; ids=[x['id'] for x in s]; dup=[k for k,v in collections.Counter(ids).items() if v>1]; assert not dup, dup; print(len(s),'steps OK')"
```
Expected: `34 steps OK`

- [ ] **Step 3: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add bdd/grammar.yaml
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "feat(bdd): normative canonical grammar.yaml (source of truth)"
```

---

## Phase 1 — Rust codegen + generic World

Strategy to keep `just bdd` green throughout: build the generated bindings + generic dispatch as a NEW module compiled alongside the existing steps, rewrite the `features/` to canonical phrasing in Phase 3, and remove the old domain steps only after the generated ones cover everything (Task 1.6). Within Phase 1 we get codegen compiling and the generic World methods in place, with the old steps still active so the suite stays green.

### Task 1.1: Add the grammar-model crate-internal codegen library

The same parsing/translation logic is needed by `build.rs` AND by the CI sync check. Put it in a tiny standalone module compiled by `build.rs` via `include!` (build scripts can't depend on workspace crates easily), and ALSO usable from a small generator binary. Simplest robust approach: a single self-contained `bdd/codegen/gen_rust.rs` file `include!`d by `build.rs`.

**Files:**
- Create: `bdd/codegen/grammar_model.rs` (shared structs + YAML parse + text→binding translation)
- Modify: `packages/engine/Cargo.toml` (add `serde`, `serde_yaml` to `[build-dependencies]`)

- [ ] **Step 1: Write `bdd/codegen/grammar_model.rs`**

```rust
// Shared grammar model + translation. include!'d by build scripts and the
// codegen binary. No external deps beyond serde/serde_yaml.
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Grammar {
    pub steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
pub struct Step {
    pub id: String,
    pub action: String,
    pub keyword: String, // given|when|then
    #[serde(default)]
    pub tier: String,
    pub text: String,
    #[serde(default)]
    pub args: Vec<Arg>,
    #[serde(default)]
    pub datatable: bool,
    #[serde(default)]
    pub literals: Vec<serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
pub struct Arg {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String, // string|number
}

pub fn load_grammar(path: &std::path::Path) -> Grammar {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read grammar {}: {e}", path.display()));
    serde_yaml::from_str(&raw)
        .unwrap_or_else(|e| panic!("invalid grammar yaml {}: {e}", path.display()))
}

/// True when the step has at least one numeric arg (forces a regex binding).
pub fn needs_regex(step: &Step) -> bool {
    step.args.iter().any(|a| a.ty == "number")
}

/// Translate canonical `text` into a Cucumber Expression (quoted args -> {string}).
/// Only valid when !needs_regex(step).
pub fn to_cucumber_expr(step: &Step) -> String {
    // Replace each "{name}" (quoted) with {string}; bare {name} not allowed here.
    let mut out = step.text.clone();
    for a in &step.args {
        let quoted = format!("\"{{{}}}\"", a.name); // "{name}"
        out = out.replace(&quoted, "{string}");
    }
    out
}

/// Translate canonical `text` into an anchored regex with one capture per arg.
pub fn to_regex(step: &Step) -> String {
    let mut out = String::from("^");
    let mut rest = step.text.as_str();
    let mut body = String::new();
    // Walk args in order, replacing their placeholder occurrence with a capture.
    // Quoted string arg "{name}" -> "([^"]*)" ; numeric {name} -> (-?\d+(?:\.\d+)?)
    let mut working = step.text.clone();
    for a in &step.args {
        if a.ty == "string" {
            let ph = format!("\"{{{}}}\"", a.name);
            working = working.replacen(&ph, "\u{0}STR\u{0}", 1);
        } else {
            let ph = format!("{{{}}}", a.name);
            working = working.replacen(&ph, "\u{0}NUM\u{0}", 1);
        }
    }
    // Escape regex metacharacters in the literal text, then swap placeholders.
    let escaped = regex_escape(&working);
    body.push_str(&escaped);
    body = body
        .replace("\u{0}STR\u{0}", "\"([^\"]*)\"")
        .replace("\u{0}NUM\u{0}", "(-?\\d+(?:\\.\\d+)?)");
    let _ = (rest, &mut body);
    rest = ""; let _ = rest;
    out.push_str(&body);
    out.push('$');
    out
}

fn regex_escape(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        if "\\^$.|?*+()[]{}".contains(c) {
            o.push('\\');
        }
        o.push(c);
    }
    o
}
```

> Note: the `\u{0}STR\u{0}` sentinel trick avoids escaping the capture groups themselves. `regex_escape` then escapes the surrounding literal (including the quote chars, which are fine) before the sentinels are swapped for the real capture groups.

- [ ] **Step 2: Add build-deps to `packages/engine/Cargo.toml`**

Under a new `[build-dependencies]` section:
```toml
[build-dependencies]
serde = { workspace = true, features = ["derive"] }
serde_yaml = { workspace = true }
```
(Check the workspace root `Cargo.toml` actually exposes `serde_yaml` as a workspace dep; if not, pin `serde_yaml = "0.9"` and `serde = { version = "1", features = ["derive"] }` directly.)

- [ ] **Step 3: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add bdd/codegen/grammar_model.rs packages/engine/Cargo.toml
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "feat(bdd): rust grammar model + expr/regex translation"
```

### Task 1.2: `build.rs` that emits cucumber step bindings

**Files:**
- Create: `packages/engine/build.rs`
- Create: `bdd/codegen/gen_rust.rs` (the emitter, `include!`d by build.rs)

- [ ] **Step 1: Write `bdd/codegen/gen_rust.rs`** — emits one cucumber attribute-macro fn per grammar step. Each fn parses captures into `Vec<ArgValue>`, appends `literals`, and calls `world.dispatch(action, args, table).await`. The `World`, `ArgValue`, and `dispatch` are defined in the BDD test crate (Task 1.3); the generated file is `include!`d into a module that has them in scope.

```rust
// Emitter: grammar -> Rust cucumber step fns. include!'d by build.rs.
// Relies on grammar_model.rs being include!'d first.
fn emit_rust(grammar: &Grammar) -> String {
    let mut s = String::new();
    s.push_str("// @generated from bdd/grammar.yaml — do not edit.\n");
    s.push_str("use cucumber::{given, when, then};\n\n");
    for (i, step) in grammar.steps.iter().enumerate() {
        let fn_name = format!("step_{}_{}", i, step.id);
        let macro_name = step.keyword.as_str(); // given|when|then
        let attr = if needs_regex(step) {
            format!("regex = r\"{}\"", to_regex(step).replace('"', "\\\""))
        } else {
            format!("expr = \"{}\"", to_cucumber_expr(step).replace('"', "\\\""))
        };

        // Build the function parameter list: cucumber passes captures as fn
        // params (String for {string} and regex string captures; we parse
        // numbers from String ourselves to keep one code path).
        let mut params = String::new();
        let mut pushes = String::new();
        for (n, a) in step.args.iter().enumerate() {
            params.push_str(&format!(", arg{n}: String"));
            match a.ty.as_str() {
                "number" => pushes.push_str(&format!(
                    "    args.push(ArgValue::Num(arg{n}.parse::<f64>().expect(\"numeric arg\")));\n"
                )),
                _ => pushes.push_str(&format!("    args.push(ArgValue::Str(arg{n}));\n")),
            }
        }
        // literals appended after captured args
        let mut lit_pushes = String::new();
        for lit in &step.literals {
            match lit {
                serde_yaml::Value::Bool(b) => {
                    lit_pushes.push_str(&format!("    args.push(ArgValue::Bool({b}));\n"))
                }
                serde_yaml::Value::Number(num) => lit_pushes
                    .push_str(&format!("    args.push(ArgValue::Num({}f64));\n", num.as_f64().unwrap())),
                serde_yaml::Value::String(text) => lit_pushes.push_str(&format!(
                    "    args.push(ArgValue::Str({:?}.to_string()));\n",
                    text
                )),
                _ => {}
            }
        }

        let step_param = if step.datatable { ", step: &cucumber::gherkin::Step" } else { "" };
        let table_expr = if step.datatable {
            "step.table.as_ref().map(|t| t.rows.clone())"
        } else {
            "None"
        };

        s.push_str(&format!("#[{macro_name}({attr})]\n"));
        s.push_str(&format!(
            "async fn {fn_name}(world: &mut RegelrechtWorld{params}{step_param}) {{\n"
        ));
        s.push_str("    let mut args: Vec<ArgValue> = Vec::new();\n");
        s.push_str(&pushes);
        s.push_str(&lit_pushes);
        s.push_str(&format!("    let table = {table_expr};\n"));
        s.push_str(&format!(
            "    world.dispatch(\"{}\", args, table).await;\n",
            step.action
        ));
        s.push_str("}\n\n");
    }
    s
}
```

- [ ] **Step 2: Write `packages/engine/build.rs`**

```rust
use std::path::Path;

include!("../../bdd/codegen/grammar_model.rs");
include!("../../bdd/codegen/gen_rust.rs");

fn main() {
    let grammar_path = Path::new("../../bdd/grammar.yaml");
    println!("cargo:rerun-if-changed=../../bdd/grammar.yaml");
    println!("cargo:rerun-if-changed=../../bdd/codegen/grammar_model.rs");
    println!("cargo:rerun-if-changed=../../bdd/codegen/gen_rust.rs");

    let grammar = load_grammar(grammar_path);
    let code = emit_rust(&grammar);

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("bdd_generated_steps.rs");
    std::fs::write(&dest, code).expect("write generated steps");
}
```

- [ ] **Step 3: Verify it compiles the generator (cheap check)**

Run:
```bash
cd /workspace/regelrecht/.worktrees/bdd-canonical-grammar
cargo build -p regelrecht-engine 2>&1 | tail -20
```
Expected: builds. The generated file exists but is not yet `include!`d anywhere, so it doesn't affect the test binary. If `serde_yaml` workspace dep is missing, fix Cargo.toml per Task 1.1 Step 2.

- [ ] **Step 4: Inspect the generated output**

Run:
```bash
find target -name bdd_generated_steps.rs -exec head -40 {} \;
```
Expected: valid Rust with `#[given(expr = "the calculation date is {string}")]` etc., and `#[then(regex = r"^output \"([^\"]*)\" equals (-?\d+(?:\.\d+)?)$")]` for numeric steps. Eyeball that quoting/escaping is correct.

- [ ] **Step 5: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add packages/engine/build.rs bdd/codegen/gen_rust.rs
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "feat(bdd): build.rs emits cucumber bindings from grammar"
```

### Task 1.3: Generic `World` fields + `ArgValue` + `dispatch`

Refactor `world.rs` to add the generic state the dispatch needs WITHOUT removing the existing typed `external_data`/note fields yet (old steps still use them; both coexist until Task 1.6).

**Files:**
- Modify: `packages/engine/tests/bdd/world.rs`
- Create: `packages/engine/tests/bdd/dispatch.rs`
- Modify: `packages/engine/tests/bdd/main.rs` (wire modules + include generated steps)

- [ ] **Step 1: Add generic fields to `RegelrechtWorld`** in `world.rs`:
  - `pub data_sources: BTreeMap<String, (String, Vec<BTreeMap<String, Value>>)>` (source name → (key field, rows))
  - `pub requested_outputs: Vec<String>` (set by `evaluate`/`evaluate_outputs`)
  Initialize both empty in `Self::new()`.

- [ ] **Step 2: Define `ArgValue`** (put it in `dispatch.rs`, re-exported):

```rust
#[derive(Debug, Clone)]
pub enum ArgValue {
    Str(String),
    Num(f64),
    Bool(bool),
}

impl ArgValue {
    pub fn as_str(&self) -> &str {
        match self { ArgValue::Str(s) => s, _ => panic!("expected Str arg, got {self:?}") }
    }
    pub fn as_num(&self) -> f64 {
        match self { ArgValue::Num(n) => *n, _ => panic!("expected Num arg, got {self:?}") }
    }
    pub fn as_bool(&self) -> bool {
        match self { ArgValue::Bool(b) => *b, _ => panic!("expected Bool arg, got {self:?}") }
    }
}
```

- [ ] **Step 3: Write `dispatch.rs`** — the single hand-written semantics file. `impl RegelrechtWorld { pub async fn dispatch(&mut self, action: &str, args: Vec<ArgValue>, table: Option<Vec<gherkin::Step row repr>>) }`. Implement every action by porting the existing step bodies to generic form. Reuse `helpers::value_conversion`. For data sources and evaluate, MIRROR the exact `service` API used in `when.rs` (read it first). Skeleton with all arms:

```rust
use crate::helpers::value_conversion::{convert_gherkin_value, values_equal_with_tolerance};
use crate::world::RegelrechtWorld;
use regelrecht_engine::Value;
use std::collections::BTreeMap;

pub use crate::dispatch_args::ArgValue; // if ArgValue lives in its own module; else define above

// `table` rows come straight from cucumber's `gherkin::Step.table.rows`:
// Vec<Vec<String>> where row[0] is the header row.
type Rows = Vec<Vec<String>>;

impl RegelrechtWorld {
    pub async fn dispatch(&mut self, action: &str, args: Vec<ArgValue>, table: Option<Rows>) {
        match action {
            "set_calculation_date" => {
                self.calculation_date = args[0].as_str().to_string();
            }
            "load_law" => {
                // All corpus laws are preloaded in Self::new(); verify presence so
                // a typo'd law id fails loudly rather than silently later.
                let law = args[0].as_str();
                assert!(self.service.has_law(law), "law '{law}' is not loaded (preloaded corpus)");
            }
            "set_parameter" => {
                let name = args[0].as_str().to_string();
                let value = match &args[1] {
                    ArgValue::Num(n) => num_to_value(*n),
                    ArgValue::Str(s) => Value::String(s.clone()),
                    ArgValue::Bool(b) => Value::Bool(*b),
                };
                self.parameters.insert(name, value);
            }
            "set_parameters_table" => {
                for (k, v) in rows_to_params(&table.expect("parameters table")) {
                    self.parameters.insert(k, v);
                }
            }
            "set_data_source" => {
                let source = args[0].as_str().to_string();
                let key = args[1].as_str().to_string();
                let rows = rows_to_records(&table.expect("data source table"));
                self.data_sources.insert(source, (key, rows));
            }
            "evaluate" => {
                let output = args[0].as_str().to_string();
                let law = args[1].as_str().to_string();
                self.run_evaluation(&law, &[output]);
            }
            "evaluate_outputs" => {
                let outputs: Vec<String> =
                    args[0].as_str().split(',').map(|s| s.trim().to_string()).collect();
                let law = args[1].as_str().to_string();
                self.run_evaluation(&law, &outputs);
            }
            "assert_succeeds" => assert!(self.error.is_none(), "expected success, got {:?}", self.error),
            "assert_fails" => assert!(self.error.is_some(), "expected failure, but succeeded"),
            "assert_fails_with" => {
                let needle = args[0].as_str().to_lowercase();
                let msg = self.error.as_ref().map(|e| e.to_string().to_lowercase()).unwrap_or_default();
                assert!(msg.contains(&needle), "error {msg:?} does not contain {needle:?}");
            }
            "assert_boolean" => {
                let actual = self.output_value(args[0].as_str());
                assert_eq!(actual, Value::Bool(args[1].as_bool()), "output {}", args[0].as_str());
            }
            "assert_equals" => {
                let expected = match &args[1] {
                    ArgValue::Num(n) => num_to_value(*n),
                    ArgValue::Str(s) => convert_gherkin_value(s),
                    ArgValue::Bool(b) => Value::Bool(*b),
                };
                let actual = self.output_value(args[0].as_str());
                assert!(values_equal_with_tolerance(&actual, &expected),
                    "output {} = {actual:?}, expected {expected:?}", args[0].as_str());
            }
            "assert_null" => {
                let actual = self.output_value(args[0].as_str());
                assert_eq!(actual, Value::Null, "output {}", args[0].as_str());
            }
            "assert_contains" => {
                let actual = self.output_value(args[0].as_str());
                let needle = args[1].as_str();
                match actual {
                    Value::String(s) => assert!(s.contains(needle), "{s:?} !contains {needle:?}"),
                    other => panic!("output {} is {other:?}, not a string", args[0].as_str()),
                }
            }
            "assert_exact_outputs" => {
                let expected: std::collections::BTreeSet<String> =
                    args[0].as_str().split(',').map(|s| s.trim().to_string()).collect();
                let actual = self.result_output_keys();
                assert_eq!(actual, expected, "output key set mismatch");
            }
            "assert_provenance" => {
                self.assert_provenance(args[0].as_str(), args[1].as_str());
            }
            "set_untranslatable_mode" => {
                self.service.set_untranslatable_mode(parse_untranslatable_mode(args[0].as_str()));
            }
            "assert_tainted" => {
                assert!(self.output_is_untranslatable(args[0].as_str()),
                    "output {} not tainted untranslatable", args[0].as_str());
            }
            // ----- notes tier: port notes.rs bodies verbatim -----
            "set_note_articles" => self.note_set_articles(&table.expect("articles table")),
            "set_note_selector_exact" => self.note_selector_exact(args[0].as_str()),
            "set_note_selector_context" => {
                self.note_selector_context(args[0].as_str(), args[1].as_str(), args[2].as_str())
            }
            "set_note_hint_article" => self.note_hint_article(args[0].as_str()),
            "set_note_hint_position" => {
                self.note_hint_position(args[0].as_str(), args[1].as_num() as usize, args[2].as_num() as usize)
            }
            "resolve_note" => self.note_resolve(),
            "assert_note_resolves" => self.note_assert_resolves(args[0].as_str()),
            "assert_note_exact_match" => self.note_assert_exact(),
            "assert_note_fuzzy_match" => self.note_assert_fuzzy(),
            "assert_note_orphaned" => self.note_assert_orphaned(),
            "assert_note_ambiguous" => self.note_assert_ambiguous(),
            other => panic!("unknown action '{other}' — grammar/dispatch out of sync"),
        }
    }
}
```

  Plus the private helpers used above, ported from the existing step bodies:
  - `run_evaluation(&mut self, law: &str, outputs: &[String])` — registers every `self.data_sources` entry onto `self.service` (mirror `when.rs::register_if_present`), then executes the law for the requested outputs, storing `self.result`/`self.error` and `self.requested_outputs`. **Read `when.rs::execute_healthcare_allowance` and `execute_law_for_outputs` to copy the exact service execute signature.**
  - `output_value(&self, name: &str) -> Value` — read named output from `self.result` (panic with a clear message if no result / missing output), mirroring `then.rs::assert_output_value`.
  - `result_output_keys(&self) -> BTreeSet<String>` — from `then.rs::assert_exact_outputs`.
  - `assert_provenance(&self, output: &str, kind: &str)` — match `kind` to `OutputProvenance::{Direct,Reactive,Override}`, port from `then.rs`.
  - `output_is_untranslatable(&self, output: &str) -> bool` — port from `then.rs::assert_output_untranslatable`.
  - note helpers (`note_set_articles`, `note_selector_exact`, ...) — port bodies verbatim from `notes.rs`.
  - free fns `num_to_value(f64) -> Value` (int if integral else float), `rows_to_params(&Rows)`, `rows_to_records(&Rows) -> Vec<BTreeMap<String,Value>>`, `parse_untranslatable_mode(&str)`.

- [ ] **Step 4: Wire modules + include generated steps in `main.rs`**

Add near the other `mod` decls:
```rust
mod dispatch;          // ArgValue + World::dispatch + helpers
```
And in a dedicated module so the generated `#[given/when/then]` fns register:
```rust
mod generated_steps {
    use crate::dispatch::ArgValue;
    use crate::world::RegelrechtWorld;
    include!(concat!(env!("OUT_DIR"), "/bdd_generated_steps.rs"));
}
```
> The generated fns reference `RegelrechtWorld` and `ArgValue` — the `use` lines bring them into scope. `cucumber::gherkin::Step` is referenced fully-qualified in the generated code.

- [ ] **Step 5: Build the test binary (it should still pass — old steps active, generated steps additionally registered)**

Run:
```bash
just bdd 2>&1 | tail -30
```
Expected: **AMBIGUOUS STEP errors** for any phrasing the generated step duplicates that ALSO exists as an old generic step (e.g. `the execution succeeds`, `the output ... is ...` if phrasings collide). This is expected and tells us which old generic steps to retire now. If old phrasings differ from canonical (most domain steps), there's no collision and the suite still passes (generated steps simply unused until features are rewritten).

- [ ] **Step 6: Resolve collisions** by deleting the *generic* duplicated steps from `then.rs`/`given.rs`/`when.rs` whose canonical equivalent is now generated (e.g. `assert_execution_succeeds`, `assert_output_value`, `assert_execution_fails_with`, the provenance asserts, `assert_exact_outputs`, the note steps in `notes.rs`, `set_untranslatable_mode`, `assert_output_untranslatable`). Keep domain-specific steps (bijstand/zorgtoeslag/erfgrensbeplanting/woo/data-table givens) for now — their phrasings don't collide and they keep the un-migrated features green. Re-run `just bdd` until green.

- [ ] **Step 7: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add packages/engine/tests/bdd/
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "feat(bdd): generic World dispatch + generated cucumber steps active"
```

### Task 1.4: codegen-sync binary + `just bdd-codegen`

A tiny bin target regenerates BOTH outputs so CI can diff. For Rust, the build script already writes to `OUT_DIR`; for the *checked-in* JS file we need a runnable generator (Task 2.1). For Rust, the sync check is "does `cargo build` succeed and does the generated file match a fresh emit" — simplest is to also emit a checked-in copy is NOT wanted (OUT_DIR only). So the Rust sync guarantee is implicit: `build.rs` always regenerates from grammar, so Rust can't drift. CI only needs to diff the JS generated file. Add the just recipe now; JS half lands in Phase 2.

**Files:**
- Modify: `Justfile`

- [ ] **Step 1: Add recipe** to `Justfile`:
```just
# Regenerate all BDD step bindings from bdd/grammar.yaml
bdd-codegen:
    node bdd/codegen/gen-js.mjs
    @echo "Rust bindings regenerate automatically via build.rs on next cargo build"
```

- [ ] **Step 2: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add Justfile
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "chore(bdd): just bdd-codegen recipe"
```

---

## Phase 2 — JS codegen + editor switchover

### Task 2.1: JS generator `bdd/codegen/gen-js.mjs`

**Files:**
- Create: `bdd/codegen/gen-js.mjs`
- Create: `frontend/src/gherkin/grammar.generated.js` (generated output — committed)

- [ ] **Step 1: Write `bdd/codegen/gen-js.mjs`** (Node ESM; parses YAML with a tiny dependency-free loader OR `js-yaml` already in frontend deps — use `js-yaml` via a relative import from frontend/node_modules):

```js
#!/usr/bin/env node
// Generate frontend/src/gherkin/grammar.generated.js from bdd/grammar.yaml.
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const yaml = require(join(process.cwd(), 'frontend/node_modules/js-yaml'));

const root = process.cwd();
const grammar = yaml.load(readFileSync(join(root, 'bdd/grammar.yaml'), 'utf8'));

function toPattern(step) {
  // Build an anchored JS regex source. "{name}" -> "([^"]*)" ; {name} -> (-?\d+(?:\.\d+)?)
  let working = step.text;
  for (const a of step.args ?? []) {
    if (a.type === 'string') working = working.replace(`"{${a.name}}"`, '\0S\0');
    else working = working.replace(`{${a.name}}`, '\0N\0');
  }
  const escaped = working.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const body = escaped.replace(/\0S\0/g, '"([^"]*)"').replace(/\0N\0/g, '(-?\\d+(?:\\.\\d+)?)');
  return `^${body}$`;
}

function templateFn(step) {
  // Emit a JS function body that rebuilds the canonical line from ordered args.
  let text = step.text;
  (step.args ?? []).forEach((a, i) => {
    if (a.type === 'string') text = text.replace(`"{${a.name}}"`, `"\${a[${i}]}"`);
    else text = text.replace(`{${a.name}}`, `\${a[${i}]}`);
  });
  return '(a) => `' + text + '`';
}

const entries = grammar.steps.map((step) => {
  const argTypes = (step.args ?? []).map((a) => a.type);
  const literals = step.literals ?? [];
  return `  { id: ${JSON.stringify(step.id)}, action: ${JSON.stringify(step.action)}, ` +
    `keyword: ${JSON.stringify(step.keyword)}, tier: ${JSON.stringify(step.tier ?? 'core')}, ` +
    `datatable: ${step.datatable ? 'true' : 'false'}, ` +
    `pattern: /${toPattern(step)}/, ` +
    `argTypes: ${JSON.stringify(argTypes)}, ` +
    `literals: ${JSON.stringify(literals)}, ` +
    `template: ${templateFn(step)} }`;
});

const out = `// @generated from bdd/grammar.yaml by bdd/codegen/gen-js.mjs — do not edit.
export const GRAMMAR = [
${entries.join(',\n')},
];
`;

const dest = join(root, 'frontend/src/gherkin/grammar.generated.js');
writeFileSync(dest, out);
console.log(`wrote ${dest} (${grammar.steps.length} steps)`);
```

- [ ] **Step 2: Run it**

Run:
```bash
cd /workspace/regelrecht/.worktrees/bdd-canonical-grammar && node bdd/codegen/gen-js.mjs
```
Expected: `wrote .../grammar.generated.js (34 steps)`

- [ ] **Step 3: Eyeball `frontend/src/gherkin/grammar.generated.js`** — verify regexes match the current hand-written ones in `steps.js` (e.g. `/^the calculation date is "([^"]*)"$/`, `/^output "([^"]*)" equals (-?\d+(?:\.\d+)?)$/`). Note: steps.js currently uses `([^"]+)` for some and `([^"]*)` for empty-allowing; the canonical uses `([^"]*)` for string args (allows empty `equals ""`). Confirm that's acceptable (it is — strictly more permissive).

- [ ] **Step 4: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add bdd/codegen/gen-js.mjs frontend/src/gherkin/grammar.generated.js
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "feat(bdd): js generator + generated grammar module"
```

### Task 2.2: Hand-written `actions.js` (WASM dispatch) + thin `steps.js`

**Files:**
- Create: `frontend/src/gherkin/actions.js`
- Modify: `frontend/src/gherkin/steps.js`
- Modify: `frontend/src/gherkin/steps.test.js`

- [ ] **Step 1: Write `frontend/src/gherkin/actions.js`** — the single JS semantics file, one `dispatch(ctx, engine, action, args, table, {loadDependency})` mirroring Rust `dispatch.rs`. Port the existing `steps.js` bodies into action arms. Reuse `parseValue`, `tableToRecords`, `getOutput`, `primitiveEqual` (move these helpers here from steps.js or import them). Core arms only need to be wired for the editor; tiers `notes/untranslatable/provenance` can `throw new Error('tier <x> not supported by the editor engine')` (the editor declares only `core` — Task 2.4). Arms: `set_calculation_date`, `load_law` (await `engine.hasLaw` / `loadDependency`), `set_parameter`, `set_parameters_table`, `set_data_source` (`engine.registerDataSource`), `evaluate` (`engine.execute` → ctx.result/ctx.error/ctx.executed), `assert_succeeds`, `assert_fails`, `assert_fails_with`, `assert_boolean`, `assert_equals`, `assert_null`, `assert_contains`. For unsupported-tier actions throw the tier error.

- [ ] **Step 2: Rewrite `steps.js`** to build step defs from `GRAMMAR` instead of hand-listed patterns:

```js
import { GRAMMAR } from './grammar.generated.js';
import { dispatch } from './actions.js';

/** Parse a regex match's captures into typed args, then append literals. */
function buildArgs(entry, match) {
  const args = entry.argTypes.map((t, i) => {
    const raw = match[i + 1];
    return t === 'number' ? Number(raw) : raw;
  });
  return [...args, ...entry.literals];
}

export function createStepDefinitions({ loadDependency }) {
  return GRAMMAR.map((entry) => ({
    pattern: entry.pattern,
    tier: entry.tier,
    execute: async (ctx, engine, match, step) => {
      const args = buildArgs(entry, match);
      const table = step?.dataTable ?? null;
      await dispatch(ctx, engine, entry.action, args, table, { loadDependency });
    },
  }));
}

export { parseValue } from './actions.js'; // keep the public re-export ScenarioForm uses
```

- [ ] **Step 3: Update `steps.test.js`** — it currently asserts the 18 hand-written patterns + `parseValue`. Keep the `parseValue` tests (now imported from actions.js via steps.js re-export). Replace the pattern-registry tests with: "every core GRAMMAR entry produces a step whose pattern matches its canonical example line." Add a table of `{line, action}` core examples and assert exactly one GRAMMAR core pattern matches each.

- [ ] **Step 4: Run JS tests**

Run:
```bash
cd /workspace/regelrecht/.worktrees/bdd-canonical-grammar/frontend && npm test -- src/gherkin/steps.test.js
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add frontend/src/gherkin/actions.js frontend/src/gherkin/steps.js frontend/src/gherkin/steps.test.js
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "feat(bdd): editor steps executor driven by generated grammar"
```

### Task 2.3: `formMapper.js` consumes generated patterns/templates

The form-state semantics stay hand-written, but the per-step regex and emit-template strings (the drift surface) come from `GRAMMAR`. Map by `action` id.

**Files:**
- Modify: `frontend/src/gherkin/formMapper.js`
- Modify: `frontend/src/gherkin/formMapper.test.js` (only if phrasings change; they shouldn't)

- [ ] **Step 1: Replace the hand-written `PATTERNS` regexes** in formMapper.js with lookups into `GRAMMAR`. Keep the extraction→form-state mapping keyed on `action`. For matching an incoming step line, iterate `GRAMMAR` core entries, test `entry.pattern`, and on match call the action-specific extractor (a `switch (entry.action)` building the same form-state fragments as today). For the two parameter forms (`set_parameter` string vs number) disambiguate by `entry.id` or by `typeof` of the captured value.
  - Special case preserved: the legacy Rust-style `dataSourceRust` pattern (`/^the following (\w+) "([^"]+)" data:$/`) — this phrasing is being RETIRED (Phase 3 rewrites those features to the canonical key form). Drop `dataSourceRust` from formMapper; it is no longer emitted or consumed.

- [ ] **Step 2: Replace `formStateToGherkin` emit strings** with `GRAMMAR` templates. Build a `byAction` index: `const TPL = Object.fromEntries(GRAMMAR.map(e => [e.id, e.template]))`. Emit:
  - calculation date → `TPL.set_calculation_date([date])`
  - dependency → `TPL.load_law([lawId])`
  - parameter (number) → `TPL.set_parameter_number([name, value])`; (string) → `TPL.set_parameter_string([name, value])`
  - data source → header/rows table appended after `TPL.set_data_source([source, key])`
  - execution → `TPL.evaluate([output, law])`
  - assertions → the matching `assert_*` template by id.
  Prefix `Given `/`When `/`Then ` from `entry.keyword` (capitalized) — derive from GRAMMAR so keyword stays single-sourced too.

- [ ] **Step 3: Run formMapper tests**

Run:
```bash
cd /workspace/regelrecht/.worktrees/bdd-canonical-grammar/frontend && npm test -- src/gherkin/formMapper.test.js
```
Expected: PASS (round-trip tests prove canonical phrasings are byte-identical to before). If a test fails on a phrasing diff, fix the GRAMMAR `text` (it is the source of truth) — do NOT special-case in formMapper.

- [ ] **Step 4: Full frontend test + build**

Run:
```bash
cd /workspace/regelrecht/.worktrees/bdd-canonical-grammar/frontend && npm test && npm run build 2>&1 | tail -15
```
Expected: all green, build succeeds.

- [ ] **Step 5: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add frontend/src/gherkin/formMapper.js frontend/src/gherkin/formMapper.test.js
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "feat(bdd): formMapper sources phrasings from generated grammar"
```

### Task 2.4: Wire codegen into the frontend build + declare editor tier

**Files:**
- Modify: `frontend/package.json` (prebuild hook)
- Modify: `frontend/src/gherkin/steps.js` (export supported tiers; skip non-core steps gracefully)

- [ ] **Step 1: Add a `prebuild` (and `pretest`) script** to `frontend/package.json` so the generated file can't go stale locally:
```json
"prebuild": "node ../bdd/codegen/gen-js.mjs",
"pretest": "node ../bdd/codegen/gen-js.mjs"
```
(Confirm `process.cwd()` in gen-js handles being run from `frontend/` — adjust the script to `cd .. && node bdd/codegen/gen-js.mjs` via `"prebuild": "node -e \"process.chdir('..')\" ...` is awkward; simplest: make gen-js resolve `root` as two levels up from its own file instead of `process.cwd()`. Update gen-js to `const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..')`.)

- [ ] **Step 2: Export `SUPPORTED_TIERS = ['core']`** from steps.js. In `createStepDefinitions`, when an incoming feature uses a step whose `entry.tier` is not supported, the executor already throws via the action arm — that's the desired "editor runs core only" behavior. (Bucket B conformance features are never loaded into the editor.)

- [ ] **Step 3: Verify prebuild runs**

Run:
```bash
cd /workspace/regelrecht/.worktrees/bdd-canonical-grammar/frontend && npm run prebuild
```
Expected: `wrote .../grammar.generated.js (34 steps)`

- [ ] **Step 4: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add frontend/package.json bdd/codegen/gen-js.mjs frontend/src/gherkin/steps.js
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "build(bdd): regenerate grammar on frontend prebuild/pretest; editor declares core tier"
```

---

## Phase 3 — Migrate & relocate features into the two buckets

The canonical phrasing mapping (apply mechanically to every old Rust-dialect line):

| Old Rust phrasing | Canonical |
|---|---|
| `Given a citizen with the following data:` + table | `Given the following parameters:` + same table |
| `Given a query with the following data:` + table | `Given the following parameters:` + same table |
| `Given a vreemdelingenwet application with:` + table | `Given the following parameters:` + same table |
| `Given the following RVIG "personal_data" data:` + table | `Given the following "personal_data" data with key "bsn":` + table |
| `Given the following BELASTINGDIENST "box1" data:` (etc.) | `Given the following "box1" data with key "bsn":` |
| `When the bijstandsaanvraag is executed for participatiewet article 43` | `When I evaluate "uitkering_bedrag" of "participatiewet"` (and a sibling line/scenario for `heeft_recht_op_bijstand`) |
| `When the WOO disclosure decision is executed` | `When I evaluate "openbaarmaking_toegestaan" of "wet_open_overheid"` |
| `When the erfgrensbeplanting is requested for burgerlijk_wetboek_boek_5 article 42` | `When I evaluate "minimale_afstand_cm" of "burgerlijk_wetboek_boek_5"` |
| `When the vreemdelingenwet beschikking is executed` | `When I evaluate "minister_is_bevoegd" of "vreemdelingenwet_2000"` |
| `When the healthcare allowance law is executed` | `When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"` |
| `When the law "X" is executed for outputs "a,b"` | `When I evaluate outputs "a, b" of "X"` (tier provenance) |
| `When the untranslatable test law is executed for output "x"` | `When I evaluate "x" of "test_untranslatables"` |
| `When I request the standard premium for year 2025` | `Given the calculation date is "2025-01-01"` + `When I evaluate "standaardpremie" of "regeling_standaardpremie"` |
| `Then the citizen has the right to bijstand` | `Then output "heeft_recht_op_bijstand" is true` |
| `Then the citizen does not have the right to bijstand` | `Then output "heeft_recht_op_bijstand" is false` |
| `Then the uitkering_bedrag is "109171" eurocent` | `Then output "uitkering_bedrag" equals 109171` |
| `Then the reden_afwijzing contains "X"` | `Then output "reden_afwijzing" contains "X"` |
| `Then the output "x" is "y"` | `Then output "x" equals "y"` (or `is true/false`/`equals N`/`is null` by type) |
| `Then the execution fails with "X"` | unchanged |
| `Then the minimale_afstand_cm is "100"` | `Then output "minimale_afstand_cm" equals 100` |
| `Then the minimale_afstand_m is "1"` | `Then output "minimale_afstand_m" equals 1` |
| `Then the standard premium is "X" eurocent` | `Then output "standaardpremie" equals X` |
| `Then the allowance amount is "1358.93" euro` | `Then output "hoogte_zorgtoeslag" equals 135893` (assert raw eurocent output; add `# NB:` noting the euro value) |
| `Then the citizen has the right to healthcare allowance` | `Then output "heeft_recht_op_zorgtoeslag" is true` |
| provenance / untranslatable / notes lines | already canonical-equivalent — tag the feature `@tier:provenance` / `@tier:untranslatable` / `@tier:notes` |

Each migrated bucket-A scenario must add the dependency `Given law "X" is loaded` lines its background needs (mirror `eligibility.feature`). For data-source givens, the key column is the first table column (usually `bsn`); pick the actual key field.

### Task 3.1: Bucket A — `bijstand` → participatiewet scenarios

**Files:**
- Create: `corpus/regulation/nl/wet/participatiewet/scenarios/bijstand.feature`
- (later) Delete: `features/bijstand.feature`

- [ ] **Step 1: Rewrite** `features/bijstand.feature` into the new file using the mapping table. Worked example for the first scenario:

```gherkin
Feature: Bijstand eligibility and amount (Participatiewet)

  Background:
    Given law "afstemmingsverordening_participatiewet_diemen" is loaded

  Scenario: Alleenstaande in Diemen heeft recht op bijstand
    Given the calculation date is "2024-06-01"
    Given the following parameters:
      | key              | value  |
      | gemeente_code    | GM0384 |
      | leeftijd         | 35     |
      | is_alleenstaande | true   |
      | gedragscategorie | geen   |
    When I evaluate "heeft_recht_op_bijstand" of "participatiewet"
    Then the execution succeeds
    Then output "heeft_recht_op_bijstand" is true
    When I evaluate "uitkering_bedrag" of "participatiewet"
    Then output "uitkering_bedrag" equals 109171
```
Apply the same transform to the remaining bijstand scenarios.

- [ ] **Step 2: Do NOT delete `features/bijstand.feature` yet** (the old runner still globs `features/`; deletion happens when the runner switches in Task 4.1). Instead, temporarily REMOVE the old domain bijstand scenarios from `features/bijstand.feature` to avoid double-running — actually simpler: leave `features/` untouched until Task 4.1 flips the runner, then delete the whole `features/` tree in one go. So for now just CREATE the bucket-A file.

- [ ] **Step 3: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add corpus/regulation/nl/wet/participatiewet/scenarios/bijstand.feature
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "feat(bdd): bucket A — participatiewet bijstand scenarios (canonical)"
```

### Task 3.2: Bucket A — `zorgtoeslag`, `woo`, `erfgrensbeplanting`, `bezwaartermijn`

**Files:**
- Modify/extend: `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature` (already canonical — fold in any unique scenarios from `features/zorgtoeslag.feature` not already covered)
- Create: `corpus/regulation/nl/wet/wet_open_overheid/scenarios/openbaarmaking.feature`
- Create: `corpus/regulation/nl/wet/burgerlijk_wetboek_boek_5/scenarios/erfgrensbeplanting.feature`
- Create: `corpus/regulation/nl/wet/vreemdelingenwet_2000/scenarios/bezwaartermijn.feature`

- [ ] **Step 1:** For each, rewrite the corresponding `features/*.feature` per the mapping table, with the right `Given law "..." is loaded` background and `Given the following "..." data with key "...":` data sources. Preserve all `# NB:` comments. For `zorgtoeslag.feature`, diff its scenarios against the existing `eligibility.feature` and only ADD ones not present.

- [ ] **Step 2: Commit** (one commit per law file is fine; or one commit for the batch)

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add corpus/regulation/nl/wet/wet_open_overheid/scenarios/ corpus/regulation/nl/wet/burgerlijk_wetboek_boek_5/scenarios/ corpus/regulation/nl/wet/vreemdelingenwet_2000/scenarios/ corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "feat(bdd): bucket A — woo/erfgrens/bezwaartermijn/zorgtoeslag canonical scenarios"
```

### Task 3.3: Bucket B — conformance suite

**Files:**
- Create: `bdd/conformance/core.feature` (basic core-tier sanity against a simple test law)
- Create: `bdd/conformance/notes.feature` (`@tier:notes`) — port `features/notes.feature` verbatim (already canonical phrasing) + add the tag
- Create: `bdd/conformance/untranslatables.feature` (`@tier:untranslatable`) — rewrite `features/untranslatables.feature` (`evaluate` form + `is tainted as untranslatable`)
- Create: `bdd/conformance/multi_output.feature` (`@tier:provenance`) — rewrite `features/multi_output.feature` (`I evaluate outputs "..."` + provenance asserts + exact-outputs)
- Create: `bdd/conformance/date_operations.feature` (`@tier:core`) — rewrite `features/date_operations.feature`
- Create: `bdd/conformance/einddatum.feature` (`@tier:core`) — rewrite `features/einddatum.feature`
- Create: `bdd/conformance/error_handling.feature` (`@tier:core`) — the engine-conformance scenarios extracted from `features/negative_scenarios.feature` (missing-input errors, null delegation fallthrough); the law-boundary scenarios from negative_scenarios go to the relevant bucket-A files in Task 3.1/3.2.

- [ ] **Step 1:** Add `@tier:<name>` tags on the feature (or scenario) line for any feature using non-core steps. Untagged ⇒ core.

- [ ] **Step 2: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add bdd/conformance/
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "feat(bdd): bucket B — engine conformance suite (tagged by tier)"
```

---

## Phase 4 — Runner switchover + CI

### Task 4.1: Rust runner globs both buckets, supports all tiers, retires old steps

**Files:**
- Modify: `packages/engine/tests/bdd/main.rs`
- Delete: `features/` (whole directory)
- Delete: remaining domain steps in `packages/engine/tests/bdd/steps/{given,when}.rs`; delete `then.rs`/`notes.rs` domain remnants. Delete now-unused typed `external_data: ExternalData` field + struct and note fields IF fully superseded by the generic `data_sources`/note helpers in dispatch.rs.

- [ ] **Step 1: Change `main.rs`** to discover both buckets instead of `features/`:
```rust
// Collect bucket A (corpus scenarios) + bucket B (conformance).
let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
let mut features: Vec<std::path::PathBuf> = Vec::new();
for entry in walkdir::WalkDir::new(root.join("corpus/regulation")).into_iter().flatten() {
    let p = entry.path();
    if p.extension().map(|e| e == "feature").unwrap_or(false)
        && p.components().any(|c| c.as_os_str() == "scenarios") {
        features.push(p.to_path_buf());
    }
}
for entry in walkdir::WalkDir::new(root.join("bdd/conformance")).into_iter().flatten() {
    if entry.path().extension().map(|e| e == "feature").unwrap_or(false) {
        features.push(entry.path().to_path_buf());
    }
}
```
Run cucumber over each collected path. The Rust engine supports ALL tiers, so no tier filtering is needed here (it runs everything). (cucumber-rs can take a directory or be run per-file; iterate and run, or point it at the two roots if the glob semantics suffice. Prefer collecting explicit paths to avoid pulling unrelated `.feature` files.)

- [ ] **Step 2: Delete `features/` and the old domain steps**:
```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar rm -r features/
```
Edit the step files to remove every remaining domain/duplicate step fn; the generated steps now cover the whole canonical vocabulary. Remove dead helpers (`parse_eurocent`, `parse_euro_to_eurocent` if unused; the typed `ExternalData`).

- [ ] **Step 3: Run the full BDD suite (both buckets)**

Run:
```bash
just bdd 2>&1 | tail -40
```
Expected: every bucket-A and bucket-B scenario passes. Failures here are real: either a migration phrasing bug, a dispatch arm bug, or a genuine law/scenario mismatch. Fix migration/dispatch bugs; for a genuine law-vs-scenario mismatch, treat it as the desired signal and adjust the scenario (or leave a `# NB:` if it documents a known engine gap, matching the `assert false as desired` pattern in eligibility.feature).

- [ ] **Step 4: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add -A
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "feat(bdd): runner globs corpus scenarios + conformance; retire features/ and domain steps"
```

### Task 4.2: CI — codegen sync check + run buckets

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add a `bdd-codegen-sync` step** to the relevant job: regenerate the JS file and fail on drift.
```yaml
      - name: BDD grammar codegen is in sync
        run: |
          node bdd/codegen/gen-js.mjs
          git diff --exit-code frontend/src/gherkin/grammar.generated.js \
            || { echo "grammar.generated.js is stale — run 'just bdd-codegen'"; exit 1; }
```
(The Rust side can't drift — `build.rs` regenerates every build — so only the checked-in JS file needs the diff guard.)

- [ ] **Step 2: Ensure `just bdd` already runs in CI** (it does via `just check`/test jobs). Confirm the BDD job now exercises both buckets (it will, since the runner changed). No separate job needed.

- [ ] **Step 3: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add .github/workflows/ci.yml
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "ci(bdd): fail on stale grammar codegen"
```

### Task 4.3: Manual editor smoke test (secure context)

- [ ] **Step 1:** Start the editor dev server bound to `0.0.0.0` on a port in 7100–7300, open via `localhost` (secure context — scenario panel needs `crypto.randomUUID`). Load a law that has scenarios (e.g. `wet_op_de_zorgtoeslag`), confirm the scenario panel parses `eligibility.feature`, executes against WASM, and `formStateToGherkin` round-trips on save (byte-identical phrasing). Confirm editing a parameter and saving emits canonical Gherkin.

- [ ] **Step 2:** If green, note it in the PR description. If the panel errors, debug `actions.js`/`steps.js` wiring before opening the PR.

---

## Phase 5 — Docs & ship

### Task 5.1: Update CLAUDE.md + docs pointers

**Files:**
- Modify: root `CLAUDE.md` (the `features/` bullet → describe the two buckets + `bdd/grammar.yaml`)
- Modify: `packages/engine` test docs if any reference `features/`.

- [ ] **Step 1:** Replace the `features/ - Gherkin BDD feature files` line with a description of `bdd/grammar.yaml` (source of truth), bucket A (`corpus/regulation/**/scenarios/`), bucket B (`bdd/conformance/`), and `just bdd-codegen`.

- [ ] **Step 2: Commit**

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar add CLAUDE.md
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar commit -m "docs(bdd): document canonical grammar + two buckets"
```

### Task 5.2: Full check + open PR

- [ ] **Step 1: Run the full quality gate**

Run:
```bash
cd /workspace/regelrecht/.worktrees/bdd-canonical-grammar
just format && just lint && just bdd
cd frontend && npm test && npm run build
```
Expected: all green.

- [ ] **Step 2: Push and open a draft PR** (English title/body; no Claude branding; ends up signed — main requires signed commits, so ensure commits are `-S` signed or amend-sign before merge).

```bash
git -C /workspace/regelrecht/.worktrees/bdd-canonical-grammar push -u origin feat/bdd-canonical-grammar
gh pr create --draft --repo MinBZK/regelrecht --title "feat(bdd): canonical engine-agnostic BDD feature language" --body "<summary + before/after table from the design doc>"
```

- [ ] **Step 3:** Follow the ci-monitor hook; then run `pr-ship` once green-able.

---

## Self-review notes

- **Spec coverage:** grammar (Task 0.2), codegen Rust (1.2) + JS (2.1), generic World (1.3), editor switchover (2.2–2.4), two buckets (3.1–3.3), runner + CI sync (4.1–4.2), live-law validation in CI (4.1 Step 3 / bucket A globbed), portability via conformance (3.3 + tiers). All design sections map to tasks.
- **Open points resolved:** canonical data-source phrasing = editor key form (mapping table); Rust codegen = attribute-macro fns via build.rs + `include!`; stored-scenario migration = none needed beyond `features/` rewrite (only `eligibility.feature` pre-existed, already canonical); `just bdd` discovery = explicit glob of `corpus/regulation/**/scenarios` + `bdd/conformance`.
- **Type consistency:** `ArgValue::{Str,Num,Bool}` and `dispatch(action, args, table)` identical names across Rust (`dispatch.rs`) and the generated bindings; JS `GRAMMAR` entry shape (`id, action, keyword, tier, datatable, pattern, argTypes, literals, template`) consumed identically by steps.js and formMapper.js.
- **Risk hotspots flagged inline:** exact `service` execute/register signatures (read `when.rs` before writing `run_evaluation`); cucumber ambiguous-step collisions during the coexistence window (Task 1.5 Steps 5–6); gen-js `root` resolution when run from `frontend/` (Task 2.4 Step 1).
