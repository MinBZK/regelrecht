// Pint het gedrag van deploy-filters vast tegen de echte cargo-graaf van deze
// workspace.
//
// Waarom tegen de echte graaf en niet tegen een verzonnen fixture: de fout die
// dit script uitbant was een lijst die wegdreef van de werkelijkheid. Een
// fixture zou meedrijven. Deze test rekent dezelfde graaf na die de CI gebruikt,
// dus een crate die van eigenaar wisselt komt hier boven water.
//
// De `try`/`catch` in het script vangt alleen gegooide fouten. De gevallen
// hieronder dekken juist de stille kant: een verkeerde `kind === 'dev'`-check of
// een verschoven `dirOf` gooit niets, maar levert `false` op voor een component
// die had moeten uitrollen. Dat is precies de klasse fouten waarvoor het script
// bestaat.
//
// De handmatige helft van de tabel heeft geen cargo-graaf om tegen te rekenen,
// en die helft is met de hand teruglezen even weerloos als de lijst die dit
// script verving. De sectie onderaan geeft hem een eigen bron: de `COPY`-regels
// van de Dockerfiles, gelezen uit de workflow die ze bouwt. Wat een image
// binnenhaalt moet dat image kunnen laten bouwen.

import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

const SCRIPT = fileURLToPath(new URL('./deploy-filters.mjs', import.meta.url));
const REPO = fileURLToPath(new URL('..', import.meta.url));

import { COMPONENTS, componentsFor, prefixesFor, workspaceGraph } from './deploy-filters.mjs';

// Eén keer afleiden; `cargo metadata` is de duurste stap in dit bestand.
const graph = workspaceGraph();

const namesOf = (changed) => componentsFor(changed, graph);
const allComponents = Object.keys(COMPONENTS);

test('dirOf leidt het cratepad af, relatief en met slash', () => {
  assert.equal(graph.dirOf('regelrecht-auth'), 'packages/auth/');
  assert.equal(graph.dirOf('regelrecht-law-model'), 'packages/law-model/');
  assert.equal(graph.dirOf('regelrecht-editor-api'), 'packages/editor-api/');

  // Geen absoluut pad en geen achtergebleven Cargo.toml: daar zou `startsWith`
  // stilzwijgend nooit meer matchen.
  for (const name of graph.byName.keys()) {
    const dir = graph.dirOf(name);
    assert.ok(dir.startsWith('packages/'), `${name} gaf ${dir}`);
    assert.ok(dir.endsWith('/'), `${name} gaf ${dir}`);
    assert.ok(!dir.includes('Cargo.toml'), `${name} gaf ${dir}`);
  }
});

test('de closure van editor-api bevat de twee crates die eerder ontbraken', () => {
  const closure = graph.closure('regelrecht-editor-api');
  assert.ok(closure.has('regelrecht-auth'));
  assert.ok(closure.has('regelrecht-law-model'));

  // law-model zit er indirect in, via pipeline en corpus; shared hangt er weer
  // onder. Valt de recursie weg, dan blijft alleen de directe laag over.
  assert.ok(closure.has('regelrecht-shared'));
  assert.ok(closure.has('regelrecht-engine'));
});

test('een dev-dependency telt niet mee voor de closure', () => {
  // harvester heeft law-model uitsluitend als dev-dependency: die code zit niet
  // in de binary, dus een wijziging erin hoeft dit image niet te bouwen.
  const harvester = graph.closure('regelrecht-harvester');
  assert.ok(harvester.has('regelrecht-shared'));
  assert.ok(
    !harvester.has('regelrecht-law-model'),
    'law-model is een dev-dependency van harvester en hoort niet in de closure',
  );

  // corpus heeft engine alleen als dev-dependency.
  const corpus = graph.closure('regelrecht-corpus');
  assert.ok(corpus.has('regelrecht-law-model'));
  assert.ok(corpus.has('regelrecht-github'));
  assert.ok(
    !corpus.has('regelrecht-engine'),
    'engine is een dev-dependency van corpus en hoort niet in de closure',
  );
});

test('een wijziging in packages/auth laat de editor uitrollen', () => {
  // Het gat waarvoor dit script is geschreven. Zonder de graaf stond auth in
  // geen enkele filter en rolde er niets uit.
  const hit = namesOf(['packages/auth/src/oidc.rs']);
  assert.equal(hit.editor, true);
  assert.equal(hit.admin, true);

  // pipeline leunt niet op auth, dus de drie pipeline-images blijven staan.
  assert.equal(hit['pipeline-api'], false);
  assert.equal(hit['harvester-worker'], false);
  assert.equal(hit['enrich-worker'], false);
  assert.equal(hit.docs, false);
});

test('een wijziging in packages/law-model raakt alle crate-componenten', () => {
  const hit = namesOf(['packages/law-model/src/lib.rs']);
  assert.equal(hit.editor, true);
  assert.equal(hit.admin, true);
  assert.equal(hit['pipeline-api'], true);
  assert.equal(hit['harvester-worker'], true);
  assert.equal(hit['enrich-worker'], true);

  // De componenten zonder Rust-image blijven er buiten.
  assert.equal(hit.docs, false);
  assert.equal(hit.grafana, false);
  assert.equal(hit.lawmaking, false);
});

test('een pad dat bij geen enkel component hoort levert niets op', () => {
  // tui en arch-extract zitten in geen enkele closure: het zijn crates zonder
  // image. Zou hier iets `true` worden, dan is de closure te ruim en zegt een
  // groene filter niets meer.
  for (const path of [
    'packages/tui/src/main.rs',
    'packages/arch-extract/src/lib.rs',
    'README.md',
    'REVIEW.md',
  ]) {
    const hit = namesOf([path]);
    const geraakt = allComponents.filter((name) => hit[name]);
    assert.deepEqual(geraakt, [], `${path} raakte ${geraakt.join(', ')}`);
  }
});

test('de handmatige paden buiten de graaf blijven werken', () => {
  assert.equal(namesOf(['docs/src/content/docs/index.md']).docs, true);
  assert.equal(namesOf(['docs/src/content/docs/index.md']).editor, false);

  assert.equal(namesOf(['packages/grafana/dashboards/pipeline.json']).grafana, true);
  assert.equal(namesOf(['packages/grafana/dashboards/pipeline.json']).editor, false);

  assert.equal(namesOf(['frontend-lawmaking/src/App.vue']).lawmaking, true);
  assert.equal(namesOf(['frontend-lawmaking/src/App.vue']).editor, false);

  assert.equal(namesOf(['frontend/src/main.ts']).editor, true);
  assert.equal(namesOf(['frontend/src/main.ts']).lawmaking, false);

  // frontend-shared zit in drie frontends tegelijk.
  const shared = namesOf(['packages/frontend-shared/src/auth.ts']);
  assert.equal(shared.editor, true);
  assert.equal(shared.admin, true);
  assert.equal(shared.lawmaking, true);
});

test('de skills raken alleen de enrich-worker', () => {
  // Drie componenten hangen aan dezelfde crate; alleen de enrich-worker neemt
  // de skills mee in zijn image.
  const hit = namesOf(['.claude/skills/law-generate/SKILL.md']);
  assert.equal(hit['enrich-worker'], true);
  assert.equal(hit['harvester-worker'], false);
  assert.equal(hit['pipeline-api'], false);
});

test('een workspace-brede wijziging raakt elk Rust-image', () => {
  for (const path of ['packages/Cargo.lock', 'packages/Cargo.toml', 'rust-toolchain.toml']) {
    const hit = namesOf([path]);
    assert.equal(hit.editor, true, path);
    assert.equal(hit.admin, true, path);
    assert.equal(hit['pipeline-api'], true, path);
    assert.equal(hit.docs, false, path);
  }
});

test('zonder lijst met gewijzigde bestanden bouwt alles', () => {
  // Fail-open: weten we niets, dan is een overbodige build goedkoper dan een
  // stille overslag.
  const hit = componentsFor([], null);
  for (const name of allComponents) {
    assert.equal(hit[name], true, name);
  }
});

test('elk component in de tabel levert bruikbare prefixen op', () => {
  // Vangt een tikfout in een cratenaam: die gooit, en de fallback zou dan bij
  // elke run alles bouwen zonder dat iemand het merkt.
  for (const [name, spec] of Object.entries(COMPONENTS)) {
    const prefixes = prefixesFor(spec, graph);
    assert.ok(prefixes.length > 0, `${name} leverde geen prefixen op`);
    for (const prefix of prefixes) {
      assert.equal(typeof prefix, 'string');
      assert.ok(prefix.length > 0, `${name} leverde een lege prefix op`);
    }
  }
});

// --- Het vangnet zelf ---
//
// De tests hierboven dekken de stille kant: een verkeerde check levert `false`
// op zonder te gooien. De twee hieronder dekken de luide kant, en die is het
// vangnet waar de rest op leunt. Gaat er iets stuk in de afleiding, dan moet
// alles gebouwd worden; blijft dat vangnet ongetest, dan kan het wegvallen
// zonder dat iemand het merkt, en is een storing weer een stille overslag.

test('een onbekende crate in de tabel gooit, zodat het vangnet aanslaat', () => {
  assert.throws(
    () => prefixesFor({ crate: 'regelrecht-bestaat-niet', paths: [] }, graph),
    /niet gevonden in de workspace/,
  );
});

test('faalt de afleiding, dan bouwt het script alles met een zichtbare waarschuwing', () => {
  // Het script als geheel, niet de losse functies: het vangnet zit in de
  // top-level try/catch. Zonder cargo op PATH faalt `cargo metadata`.
  const result = spawnSync(process.execPath, [SCRIPT, 'packages/auth/src/lib.rs'], {
    encoding: 'utf8',
    env: { ...process.env, PATH: '/nonexistent' },
  });

  assert.equal(result.status, 0, 'het script hoort niet te falen, maar alles te bouwen');
  assert.match(result.stdout, /::warning::/, 'de fallback hoort zichtbaar te zijn in het log');
  for (const name of allComponents) {
    assert.match(
      result.stdout,
      new RegExp(`^${name}=true$`, 'm'),
      `${name} hoort op true te staan als de afleiding faalt`,
    );
  }
});

test('een schemawijziging raakt elk Rust-image', () => {
  // Elk Rust-image doet `COPY schema/`, en corpus bakt er een schema uit in met
  // include_str!. Zonder deze regel levert een schemawijziging stille,
  // verouderde images op: de gevaarlijkste uitkomst, want niets wordt rood.
  const hit = namesOf(['schema/v0.5.6/schema.json']);
  for (const name of ['editor', 'admin', 'harvester-worker', 'enrich-worker', 'pipeline-api']) {
    assert.equal(hit[name], true, name);
  }
});

test('een Dockerfile raakt alleen de images die hem gebruiken', () => {
  // packages/pipeline/Dockerfile ligt in de cratemap van pipeline, en editor en
  // admin hangen via de graaf aan die crate. Ze gebruiken die Dockerfile niet,
  // dus ze horen niet mee te bouwen.
  const pipeline = namesOf(['packages/pipeline/Dockerfile']);
  assert.equal(pipeline['harvester-worker'], true);
  assert.equal(pipeline['enrich-worker'], true);
  assert.equal(pipeline['pipeline-api'], true);
  assert.equal(pipeline.editor, false);
  assert.equal(pipeline.admin, false);

  // En andersom blijft de crate zelf wel gewoon werken.
  const source = namesOf(['packages/pipeline/src/worker.rs']);
  assert.equal(source.editor, true);
  assert.equal(source.admin, true);
});

test('een npm-bump in de wortel bouwt beide frontend-images', () => {
  // `npm ci` in frontend/Dockerfile en frontend-lawmaking/Dockerfile draait over
  // de hele workspace, dus beide images kopiëren deze drie bestanden.
  for (const path of ['package.json', 'package-lock.json', '.npmrc']) {
    const hit = namesOf([path]);
    assert.equal(hit.editor, true, path);
    assert.equal(hit.lawmaking, true, path);
  }
});

// --- De handmatige helft, tegen de Dockerfiles ---

// Welke Dockerfile (en welk --target en welke buildcontext) hoort bij welk
// component: uit deploy.yml, niet uit een tweede handgeschreven tabel. Elke
// build-job roept build-image.yml aan met een `cache-scope` die de componentnaam
// is - dezelfde naam die dit script emit.
function workflowImages() {
  const text = readFileSync(`${REPO}.github/workflows/deploy.yml`, 'utf8');
  const blocks = text.split(/\n {2}(?=[\w-]+:\n)/);
  const images = {};
  for (const block of blocks) {
    if (!block.includes('build-image.yml')) continue;
    const pick = (key) => block.match(new RegExp(`^\\s+${key}:\\s*(\\S+)\\s*$`, 'm'))?.[1];
    const name = pick('cache-scope');
    const dockerfile = pick('dockerfile');
    assert.ok(name && dockerfile, `build-job zonder cache-scope of dockerfile:\n${block.slice(0, 200)}`);
    // De context is repo-relatief; './packages/grafana' wordt 'packages/grafana/'.
    const rawContext = pick('context');
    const context = rawContext ? `${rawContext.replace(/^\.\/?/, '').replace(/\/$/, '')}/` : '';
    images[name] = { dockerfile, target: pick('target') ?? null, context };
  }
  return images;
}

// Een Dockerfile als stages: per stage de basis (`FROM x AS y`), de stages
// waaruit hij kopieert (`COPY --from=`), en de bronpaden die uit de repo komen.
// Regels die niet in die vorm passen komen als `unparsed` terug: die horen
// zichtbaar overgeslagen te worden, niet stil.
function parseDockerfile(text) {
  const stages = [];
  const unparsed = [];
  let current = null;
  // Een instructie mag over meerdere regels lopen met een backslash; plak die
  // eerst terug tot één logische regel.
  const logical = [];
  let pending = null;
  for (const raw of text.split('\n')) {
    const line = raw.trim();
    const continues = line.endsWith('\\');
    const body = continues ? line.slice(0, -1).trim() : line;
    if (pending !== null) {
      pending = `${pending} ${body}`;
      if (!continues) {
        logical.push(pending);
        pending = null;
      }
    } else if (continues) {
      pending = body;
    } else {
      logical.push(line);
    }
  }
  if (pending !== null) logical.push(pending);

  for (const line of logical) {
    if (line.startsWith('#') || line === '') continue;
    const from = line.match(/^FROM\s+(\S+)(?:\s+AS\s+(\S+))?/i);
    if (from) {
      current = { name: from[2] ?? `#${stages.length}`, base: from[1], fromStages: [], sources: [] };
      stages.push(current);
      continue;
    }
    if (!/^COPY\s/i.test(line)) continue;
    if (!current) {
      unparsed.push(line);
      continue;
    }
    const tokens = line.slice('COPY'.length).trim().split(/\s+/);
    const flags = tokens.filter((t) => t.startsWith('--'));
    const args = tokens.filter((t) => !t.startsWith('--'));
    const fromFlag = flags.find((f) => f.startsWith('--from='));
    if (fromFlag) {
      // Kopie uit een eerdere buildfase, niet uit de repo.
      current.fromStages.push(fromFlag.slice('--from='.length));
      continue;
    }
    if (args.length < 2 || line.includes('"') || line.includes('[')) {
      unparsed.push(line);
      continue;
    }
    // Een glob dekt alles onder zijn vaste kop; die kop is wat een filter moet raken.
    for (const src of args.slice(0, -1)) current.sources.push(src.split(/[*?]/)[0]);
  }

  return { stages, unparsed };
}

// De bronpaden die dit image echt binnenhaalt: het doelstadium plus alles waar
// het via `FROM` en `COPY --from=` op leunt.
function sourcesForTarget(stages, target) {
  const byName = new Map(stages.map((s) => [s.name, s]));
  const start = target ? byName.get(target) : stages[stages.length - 1];
  assert.ok(start, `stage ${target} niet gevonden`);

  const seen = new Set();
  const walk = (stage) => {
    if (!stage || seen.has(stage.name)) return;
    seen.add(stage.name);
    walk(byName.get(stage.base));
    for (const dep of stage.fromStages) walk(byName.get(dep));
  };
  walk(start);

  return [...seen].flatMap((name) => byName.get(name).sources);
}

test('de bouwjobs in deploy.yml dekken precies de componenten uit de tabel', () => {
  assert.deepEqual(Object.keys(workflowImages()).sort(), allComponents.sort());
});

test('elk bronpad dat een image kopieert laat dat image ook bouwen', (t) => {
  // De structurele tegenhanger van de cargo-graaf: de Dockerfile zegt wat het
  // image binnenhaalt, dus dat is de lijst die de filters moeten dekken. Blijft
  // er één achter, dan bouwt een wijziging in dat pad een verouderd image - of
  // helemaal geen image, en dan wordt er niets rood.
  for (const [name, image] of Object.entries(workflowImages())) {
    const text = readFileSync(`${REPO}${image.dockerfile}`, 'utf8');
    const { stages, unparsed } = parseDockerfile(text);
    for (const line of unparsed) {
      t.diagnostic(`${image.dockerfile}: COPY-regel overgeslagen, niet te lezen: ${line}`);
    }

    const prefixes = prefixesFor(COMPONENTS[name], graph);
    const sources = sourcesForTarget(stages, image.target);
    assert.ok(sources.length > 0, `${name}: geen bronpaden gelezen uit ${image.dockerfile}`);

    for (const src of sources) {
      const path = `${image.context}${src}`;
      assert.ok(
        prefixes.some((p) => path.startsWith(p)),
        `${name} kopieert ${path} (${image.dockerfile}) maar geen enkele filter dekt dat pad`,
      );
    }
  }
});
