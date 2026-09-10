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
