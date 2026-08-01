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

import { test } from 'node:test';
import assert from 'node:assert/strict';

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
