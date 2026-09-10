# LU Wet in Frontend Demo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Put the Luxembourg flight-tax discount law into the frontend demo (Wetten, English Scenario's, Graaf hand-pick, presentatie mini-section) without mixing it into Merijn/Claudia NL stories.

**Architecture:** Curated copy under `corpus/demo/regulation/lu/`, multi-jurisdiction walk in `copy-demo-corpus.mjs`, plus `services.yaml` / `demo-config.yaml` wiring. Canonical `corpus/regulation/lu/` stays untouched except as the source for the YAML body.

**Tech Stack:** Node copy script (`js-yaml`), demo corpus YAML/Gherkin, Vue demo frontend, `just validate-demo` / `just bdd-demo`.

**Spec:** `docs/superpowers/specs/2026-09-10-lu-wet-frontend-demo-design.md`

## Global Constraints

- Demo feature file: **English** Feature + Scenario titles (steps already English).
- Graaf: **do not** add LU to Merijn or Claudia `graph_laws`.
- Presentatie: **two slides** (statement + demo) **immediately before** the existing `closing` slide.
- Demo YAML: `service: ADMIN_FISCALE_LU`, `discoverable: HIDDEN`.
- NL `law_path` values must remain unchanged (relative to `regulation/nl/`).
- LU `law_path`: `wet/vliegbelasting_korting_klimaatneutraal_lu`.
- No portal tiles, no simulation bindings, no third persona.

---

## File map

| File | Responsibility |
|------|----------------|
| `frontend-demo/scripts/copy-demo-corpus.mjs` | Walk all jurisdictions under `corpus/demo/regulation/`; build `public/data` + `index.json` |
| `corpus/demo/regulation/lu/wet/vliegbelasting_korting_klimaatneutraal_lu/2026-07-21.yaml` | Demo law YAML (+ name/service/discoverable) |
| `corpus/demo/regulation/lu/wet/vliegbelasting_korting_klimaatneutraal_lu/scenarios/korting.feature` | English Gherkin for demo |
| `corpus/demo/services.yaml` | `ADMIN_FISCALE_LU` organisation |
| `corpus/demo/demo-config.yaml` | `expanded_paths` + presentatie mini-section |
| `frontend-demo/scripts/copy-demo-corpus.test.mjs` | Node test: multi-jurisdiction index includes LU |

---

### Task 1: Multi-jurisdiction copy script + test

**Files:**
- Modify: `frontend-demo/scripts/copy-demo-corpus.mjs`
- Create: `frontend-demo/scripts/copy-demo-corpus.test.mjs`
- Test: run with `node --test frontend-demo/scripts/copy-demo-corpus.test.mjs`

**Interfaces:**
- Consumes: `corpus/demo/regulation/{cc}/**/*.yaml` and `**/*.feature`
- Produces: `frontend-demo/public/data/laws/...`, `index.json` with `laws[].law_path` relative to jurisdiction root; LU entry id `vliegbelasting_korting_klimaatneutraal_lu` once Task 2 files exist

- [ ] **Step 1: Write the failing test**

Create `frontend-demo/scripts/copy-demo-corpus.test.mjs`:

```js
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
import test from 'node:test';

const here = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(here, '..');
const repoRoot = resolve(appRoot, '..');
const fixtureRoot = resolve(appRoot, '.tmp-copy-corpus-test');
const regulationRoot = join(fixtureRoot, 'regulation');

test('copy-demo-corpus indexes laws from every jurisdiction directory', () => {
  rmSync(fixtureRoot, { recursive: true, force: true });
  const luDir = join(regulationRoot, 'lu', 'wet', 'demo_lu_law');
  mkdirSync(luDir, { recursive: true });
  writeFileSync(
    join(luDir, '2026-01-01.yaml'),
    [
      '---',
      '$id: demo_lu_law',
      'name: Demo LU Law',
      'service: ADMIN_FISCALE_LU',
      'regulatory_layer: WET',
      'valid_from: "2026-01-01"',
      'articles: []',
      '',
    ].join('\n'),
  );
  mkdirSync(join(luDir, 'scenarios'), { recursive: true });
  writeFileSync(
    join(luDir, 'scenarios', 'demo.feature'),
    'Feature: Demo LU feature\n',
  );

  // Patch: run script with CORPUS_DEMO_ROOT override — implement that env in Step 3.
  const script = join(here, 'copy-demo-corpus.mjs');
  const destDir = join(fixtureRoot, 'public-data');
  const result = spawnSync(process.execPath, [script], {
    env: {
      ...process.env,
      CORPUS_DEMO_ROOT: fixtureRoot,
      DEMO_PUBLIC_DATA_DIR: destDir,
    },
    encoding: 'utf8',
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const index = JSON.parse(readFileSync(join(destDir, 'index.json'), 'utf8'));
  const law = index.laws.find((l) => l.id === 'demo_lu_law');
  assert.ok(law, 'expected LU law in index');
  assert.equal(law.law_path, 'wet/demo_lu_law');
  assert.equal(law.path, '/data/laws/wet/demo_lu_law/2026-01-01.yaml');
  const scenario = index.scenarios.find((s) => s.law_path === 'wet/demo_lu_law');
  assert.ok(scenario, 'expected LU scenario');
  assert.equal(scenario.title, 'Demo LU feature');
  assert.ok(existsSync(join(destDir, 'laws', 'wet', 'demo_lu_law', '2026-01-01.yaml')));

  rmSync(fixtureRoot, { recursive: true, force: true });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test frontend-demo/scripts/copy-demo-corpus.test.mjs`

Expected: FAIL (script ignores `CORPUS_DEMO_ROOT` / still only scans `nl`, or missing env support)

- [ ] **Step 3: Implement multi-jurisdiction walk + env overrides**

Replace the hard-coded `lawsDir` / `destDir` section in `frontend-demo/scripts/copy-demo-corpus.mjs` with:

```js
const corpusDir = process.env.CORPUS_DEMO_ROOT
  ? resolve(process.env.CORPUS_DEMO_ROOT)
  : resolve(repoRoot, 'corpus', 'demo');
const regulationRoot = resolve(corpusDir, 'regulation');
const destDir = process.env.DEMO_PUBLIC_DATA_DIR
  ? resolve(process.env.DEMO_PUBLIC_DATA_DIR)
  : resolve(appRoot, 'public', 'data');

function jurisdictionDirs() {
  if (!existsSync(regulationRoot)) return [];
  return readdirSync(regulationRoot)
    .map((name) => join(regulationRoot, name))
    .filter((p) => statSync(p).isDirectory());
}

rmSync(destDir, { recursive: true, force: true });
mkdirSync(destDir, { recursive: true });

const laws = [];
const scenarios = [];

for (const lawsDir of jurisdictionDirs()) {
  for (const file of walk(lawsDir, (n) => n.endsWith('.yaml'))) {
    const rel = relative(lawsDir, file);
    const text = readFileSync(file, 'utf8');
    const doc = yaml.load(text);
    const dest = join(destDir, 'laws', rel);
    mkdirSync(dirname(dest), { recursive: true });
    writeFileSync(dest, text);
    const outputs = [];
    const inputs = [];
    for (const article of doc.articles ?? []) {
      const ex = article.machine_readable?.execution;
      if (!ex) continue;
      for (const o of ex.output ?? []) outputs.push(o.name);
      for (const i of ex.input ?? []) inputs.push(i.name);
      for (const p of ex.parameters ?? []) inputs.push(p.name);
    }
    laws.push({
      id: doc.$id,
      name: doc.name,
      service: doc.service ?? null,
      discoverable: doc.discoverable ?? null,
      regulatory_layer: doc.regulatory_layer,
      valid_from: doc.valid_from ?? doc.publication_date,
      uuid: doc.uuid ?? null,
      path: `/data/laws/${rel.split('\\').join('/')}`,
      law_path: rel.replace(/\/[^/]+\.yaml$/, ''),
      outputs,
      inputs,
    });
  }

  for (const file of walk(lawsDir, (n) => n.endsWith('.feature'))) {
    const rel = relative(lawsDir, file);
    const dest = join(destDir, 'laws', rel);
    mkdirSync(dirname(dest), { recursive: true });
    cpSync(file, dest);
    const firstLine = readFileSync(file, 'utf8').split('\n').find((l) => l.trim().startsWith('Feature:'));
    scenarios.push({
      path: `/data/laws/${rel.split('\\').join('/')}`,
      law_path: rel.replace(/\/scenarios\/[^/]+\.feature$/, ''),
      title: firstLine ? firstLine.replace(/^\s*Feature:\s*/, '').trim() : rel,
    });
  }
}

// Sidecars: only when using the real corpus (bindings etc. live next to regulation/)
for (const name of ['bindings.yaml', 'profiles.yaml', 'demo-config.yaml', 'services.yaml']) {
  const src = join(corpusDir, name);
  if (existsSync(src)) cpSync(src, join(destDir, name));
}

laws.sort((a, b) => a.id.localeCompare(b.id) || String(a.valid_from).localeCompare(String(b.valid_from)));
scenarios.sort((a, b) => a.path.localeCompare(b.path));
writeFileSync(join(destDir, 'index.json'), JSON.stringify({ laws, scenarios }, null, 2));
console.log(`demo corpus: ${laws.length} law files, ${scenarios.length} feature files → ${relative(appRoot, destDir)}`);
```

Keep the existing `walk` helper and imports unchanged aside from using `jurisdictionDirs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test frontend-demo/scripts/copy-demo-corpus.test.mjs`

Expected: PASS

Also smoke the real corpus:

Run: `node frontend-demo/scripts/copy-demo-corpus.mjs`

Expected: log line with law/feature counts; NL laws still present in `frontend-demo/public/data/index.json` (e.g. `zorgtoeslagwet`).

- [ ] **Step 5: Commit**

```bash
git add frontend-demo/scripts/copy-demo-corpus.mjs frontend-demo/scripts/copy-demo-corpus.test.mjs
git commit -m "$(cat <<'EOF'
feat(demo): scan all jurisdictions when copying demo corpus.

EOF
)"
```

---

### Task 2: Demo LU YAML + English feature

**Files:**
- Create: `corpus/demo/regulation/lu/wet/vliegbelasting_korting_klimaatneutraal_lu/2026-07-21.yaml`
- Create: `corpus/demo/regulation/lu/wet/vliegbelasting_korting_klimaatneutraal_lu/scenarios/korting.feature`
- Modify: `corpus/demo/services.yaml` (add `ADMIN_FISCALE_LU`)

**Interfaces:**
- Consumes: rule body from `corpus/regulation/lu/wet/vliegbelasting_korting_klimaatneutraal_lu/2026-07-21.yaml`
- Produces: demo law `$id: vliegbelasting_korting_klimaatneutraal_lu` with `service: ADMIN_FISCALE_LU`, `discoverable: HIDDEN`, `name: Vliegbelasting — duurzaamheidskorting (LU)`

- [ ] **Step 1: Add service entry**

Append to `corpus/demo/services.yaml` under `services:` (alphabetically near `ACICT` or at end before EOF — place after `ACICT` block):

```yaml
  ADMIN_FISCALE_LU:
    name: Administration de l'enregistrement (LU)
```

- [ ] **Step 2: Create demo YAML**

Copy canonical YAML to:

`corpus/demo/regulation/lu/wet/vliegbelasting_korting_klimaatneutraal_lu/2026-07-21.yaml`

Then insert these keys after `jurisdictie: LU` (keep all articles/actions identical to canonical):

```yaml
name: Vliegbelasting — duurzaamheidskorting (LU)
service: ADMIN_FISCALE_LU
discoverable: HIDDEN
```

Full header after edit should look like:

```yaml
---
$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.9/schema/v0.5.9/schema.json
$id: vliegbelasting_korting_klimaatneutraal_lu
uuid: b1b1b1b1-b1b1-b1b1-b1b1-b1b1b1b10114
regulatory_layer: WET
jurisdictie: LU
name: Vliegbelasting — duurzaamheidskorting (LU)
service: ADMIN_FISCALE_LU
discoverable: HIDDEN
publication_date: '2026-07-21'
valid_from: '2026-07-21'
officiele_titel: Vliegbelastingwet Luxemburg - Klimaatneutrale kortingsregeling
eli: https://legilux.public.lu/eli/etat/lu/2026/07/21/vliegbelasting_klimaat
url: https://legilux.public.lu/eli/etat/lu/2026/07/21/vliegbelasting_klimaat
```

(rest unchanged from canonical)

- [ ] **Step 3: Create English feature**

Create `corpus/demo/regulation/lu/wet/vliegbelasting_korting_klimaatneutraal_lu/scenarios/korting.feature`:

```gherkin
Feature: Flight tax sustainability discount (LU)

  Background:
    Given the calculation date is "2026-07-21"

  Scenario: Climate-neutral domestic flight → discount
    Given the following parameters:
      | klimaatneutraal       | true |
      | binnenlands           | true |
      | afstand_km            | 200  |
      | vliegbelasting_bedrag | 45   |
    When I evaluate outputs "recht_op_korting, korting_bedrag" of "vliegbelasting_korting_klimaatneutraal_lu"
    Then the execution succeeds
    Then output "recht_op_korting" is true
    Then output "korting_bedrag" equals 22.5

  Scenario: Non-climate-neutral flight → no discount
    Given the following parameters:
      | klimaatneutraal       | false |
      | binnenlands           | false |
      | afstand_km            | 300   |
      | vliegbelasting_bedrag | 60    |
    When I evaluate outputs "recht_op_korting, korting_bedrag" of "vliegbelasting_korting_klimaatneutraal_lu"
    Then the execution succeeds
    Then output "recht_op_korting" is false
    Then output "korting_bedrag" equals 0

  Scenario: Climate-neutral short-haul flight → discount
    Given the following parameters:
      | klimaatneutraal       | true  |
      | binnenlands           | false |
      | afstand_km            | 480   |
      | vliegbelasting_bedrag | 50    |
    When I evaluate outputs "recht_op_korting, korting_bedrag" of "vliegbelasting_korting_klimaatneutraal_lu"
    Then the execution succeeds
    Then output "recht_op_korting" is true
    Then output "korting_bedrag" equals 25

  Scenario: Climate-neutral long-haul flight → no discount
    Given the following parameters:
      | klimaatneutraal       | true  |
      | binnenlands           | false |
      | afstand_km            | 1200  |
      | vliegbelasting_bedrag | 80    |
    When I evaluate outputs "recht_op_korting, korting_bedrag" of "vliegbelasting_korting_klimaatneutraal_lu"
    Then the execution succeeds
    Then output "recht_op_korting" is false
    Then output "korting_bedrag" equals 0
```

- [ ] **Step 4: Validate + BDD**

Run: `just validate-demo`

Expected: exit 0 (all demo YAMLs valid, including LU).

Run: `just bdd-demo` filtered if possible, or full bucket:

```bash
cd packages/engine && BDD_BUCKET=corpus REGULATION_PATH="$(pwd)/../../corpus/demo/regulation" \
  cargo test --test bdd Flight_tax -- --nocapture
```

Expected: the four English scenarios pass (cucumber may mangle spaces in filter — if filter finds nothing, run full `just bdd-demo` and confirm LU scenarios in output).

Run: `node frontend-demo/scripts/copy-demo-corpus.mjs`

Expected: `index.json` contains law id `vliegbelasting_korting_klimaatneutraal_lu`, scenario title `Flight tax sustainability discount (LU)`, and `inputs` includes `klimaatneutraal` (from parameters indexing).

- [ ] **Step 5: Commit**

```bash
git add \
  corpus/demo/regulation/lu/wet/vliegbelasting_korting_klimaatneutraal_lu/ \
  corpus/demo/services.yaml
git commit -m "$(cat <<'EOF'
feat(demo): add Luxembourg flight-tax law with English scenarios.

EOF
)"
```

---

### Task 3: demo-config — expanded_paths + presentatie mini-section

**Files:**
- Modify: `corpus/demo/demo-config.yaml`

**Interfaces:**
- Consumes: law id `vliegbelasting_korting_klimaatneutraal_lu` (Wetten `expandedFor` checks `cfg[law.id] ?? cfg[law.law_path]`)
- Produces: two slides before `kind: closing`; no changes to Merijn/Claudia `graph_laws`

- [ ] **Step 1: Add expanded_paths**

Under `expanded_paths:` in `corpus/demo/demo-config.yaml`, add:

```yaml
  vliegbelasting_korting_klimaatneutraal_lu:
    - articles.*.machine_readable.execution.parameters
    - articles.*.machine_readable.execution.output
    - articles.*.machine_readable.execution.actions
```

Do **not** edit `profiles.merijn.graph_laws` or `profiles.claudia.graph_laws`.

- [ ] **Step 2: Insert presentatie mini-section**

Immediately **before** the existing `- kind: closing` entry under `slides:`, insert:

```yaml
  - kind: statement
    overline: Ook buiten Nederland
    lines:
      - Dezelfde engine
      - werkt voor **andere jurisdicties**
      - hier: **Luxemburg**
  - kind: demo
    overline: Luxemburg
    title: Vliegbelasting, machine-uitvoerbaar
    lead: Een LU-demo-wet met Engelse scenario's — dezelfde engine als de Nederlandse wetten.
    bullets:
      - Klimaatneutraal én (binnenlands of korter dan 500 km) geeft **50% korting**.
      - Scenario's staan in het **Engels** voor internationale demos.
      - In de graaf kies je deze wet **los** bij; hij zit niet in het Merijn-verhaal.
    route: /scenarios
```

Leave the closing slide unchanged after these two.

- [ ] **Step 3: Regenerate public data and spot-check index/config**

Run: `node frontend-demo/scripts/copy-demo-corpus.mjs`

Verify:

```bash
node -e '
const i=require("./frontend-demo/public/data/index.json");
const c=require("fs").readFileSync("frontend-demo/public/data/demo-config.yaml","utf8");
const law=i.laws.find(l=>l.id==="vliegbelasting_korting_klimaatneutraal_lu");
if(!law) throw new Error("missing law");
if(law.service!=="ADMIN_FISCALE_LU") throw new Error("bad service");
const sc=i.scenarios.find(s=>s.title.includes("Flight tax"));
if(!sc) throw new Error("missing English feature");
if(!c.includes("Ook buiten Nederland")) throw new Error("missing slides");
if(c.match(/graph_laws:[\s\S]*vliegbelasting/)) throw new Error("LU leaked into graph_laws");
console.log("ok", law.law_path, sc.title);
'
```

Expected: prints `ok wet/vliegbelasting_korting_klimaatneutraal_lu Flight tax sustainability discount (LU)`

- [ ] **Step 4: Manual UI check (running demo)**

If Vite is not running: `cd frontend-demo && npx vite --port 7400 --strictPort --host 127.0.0.1`

Check:

1. **Wetten** — group Administration de l'enregistrement (LU) shows the law; open it; YAML expansion includes parameters/output/actions.
2. **Scenario's** — feature title in English; run all four scenarios → pass.
3. **Graaf** — preset Verhaal has no LU; sidebar can toggle the LU law alone.
4. **Presentatie** — two new slides before closing; demo slide navigates to `/scenarios`.

- [ ] **Step 5: Commit**

```bash
git add corpus/demo/demo-config.yaml
git commit -m "$(cat <<'EOF'
feat(demo): wire LU law into Wetten expansion and presentatie.

EOF
)"
```

---

## Self-review (plan vs spec)

| Spec requirement | Task |
|------------------|------|
| Curated demo copy under `regulation/lu/` | Task 2 |
| English Feature/Scenario titles | Task 2 Step 3 |
| Multi-jurisdiction `copy-demo-corpus.mjs` | Task 1 |
| Index `execution.parameters` as inputs | Task 1 Step 3 |
| `ADMIN_FISCALE_LU` in services | Task 2 Step 1 |
| `discoverable: HIDDEN` | Task 2 Step 2 |
| `expanded_paths` | Task 3 Step 1 |
| Graaf hand-pick only (no Merijn/Claudia graph_laws) | Task 3 (explicit non-edit) + verify script |
| Presentatie mini-section before closing | Task 3 Step 2 |
| validate-demo / bdd-demo | Task 2 Step 4 |
| No portal/simulation/bindings | Not in any task |

Placeholder scan: none. Type/name consistency: `$id` / service / law_path match across tasks.
