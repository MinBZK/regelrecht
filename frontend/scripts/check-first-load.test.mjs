// Node's built-in test runner, so this costs no dependency. Run with
// `just first-load-test` (part of `just check`).
//
// The guard itself is the only thing standing between a silent bundling
// regression and a doubled first load, and it is wired into `npm run build` —
// a bug in it would surface as a failing Docker image build, or not at all.
// So the fixtures here are the shapes a real `dist/` takes: the current build,
// and the two ways echarts gets back into the entry graph.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { checkFirstLoad } from './check-first-load.mjs';

/**
 * Shared chunks index.html preloads in a real build: the rolldown runtime, the
 * vue runtime, a handful of composables. Named as vite hashes them.
 */
const SHARED_CHUNKS = [
  'rolldown-runtime-DK3Fl9T5.js',
  'runtime-core.esm-bundler-yi8_EWx1.js',
  'useApi-CROJJdhE-DDjPdHJd.js',
  'useLaw-BmNkokO3.js',
];

/** Lazy route chunks that exist on disk but are imported at navigation time. */
const ROUTE_CHUNKS = ['AccountRequestView-DjPvsnLO.js', 'DataTable-CvK3xaic.js'];

/**
 * Write a dist that matches what vite emits: hashed files under assets/, an
 * index.html with the entry as a module script and every eagerly-fetched chunk
 * as a modulepreload.
 */
async function dist({ assets, entry = 'index-SWwzPvTN.js', preloads = SHARED_CHUNKS }) {
  const dir = await mkdtemp(join(tmpdir(), 'check-first-load-'));
  await mkdir(join(dir, 'assets'));
  for (const name of assets) await writeFile(join(dir, 'assets', name), '/* chunk */\n');

  const html = `<!DOCTYPE html>
<html lang="nl">
<head>
  <meta charset="UTF-8">
  <title>RegelRecht</title>
  <link rel="icon" type="image/svg+xml" href="/regelrecht-icon.svg">
  <script type="module" crossorigin src="/assets/${entry}"></script>
${preloads.map((p) => `  <link rel="modulepreload" crossorigin href="/assets/${p}">`).join('\n')}
  <link rel="stylesheet" crossorigin href="/assets/index-aazph-g7.css">
</head>
<body>
  <div id="app" style="height: 100%;"></div>
</body>
</html>
`;
  await writeFile(join(dir, 'index.html'), html);
  return dir;
}

async function withDist(spec, assertions) {
  const dir = await dist(spec);
  try {
    await assertions(dir);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

test('a build with echarts in its own chunk passes', async () => {
  await withDist(
    {
      assets: [
        'index-SWwzPvTN.js',
        'echarts-B-2PdTzp.js',
        'OverviewView-DGcaNwgn.js',
        ...SHARED_CHUNKS,
        ...ROUTE_CHUNKS,
        'index-aazph-g7.css',
      ],
    },
    (dir) => {
      const { problems, echartsChunk, loaded } = checkFirstLoad(dir);
      assert.deepEqual(problems, []);
      assert.equal(echartsChunk, 'echarts-B-2PdTzp.js');
      // The entry plus its preloads; the stylesheet is not a JS request.
      assert.equal(loaded.length, SHARED_CHUNKS.length + 1);
    },
  );
});

test('a build without the codeSplitting group fails', async () => {
  // What the build looks like when the group stops applying: echarts is folded
  // into the route chunk, so there is no echarts-*.js left. index.html is
  // unchanged, which is exactly why this is invisible without the guard.
  await withDist(
    {
      assets: ['index-SWwzPvTN.js', 'OverviewView-DGcaNwgn.js', ...SHARED_CHUNKS],
    },
    (dir) => {
      const { problems } = checkFirstLoad(dir);
      assert.equal(problems.length, 1);
      assert.match(problems[0], /no echarts-\*\.js chunk/);
    },
  );
});

test('an echarts chunk that index.html preloads fails', async () => {
  await withDist(
    {
      assets: ['index-SWwzPvTN.js', 'echarts-B-2PdTzp.js', ...SHARED_CHUNKS],
      preloads: [...SHARED_CHUNKS, 'echarts-B-2PdTzp.js'],
    },
    (dir) => {
      const { problems } = checkFirstLoad(dir);
      assert.equal(problems.length, 1);
      assert.match(problems[0], /first load/);
      // The build log has to name the chunk, or the failure is a riddle.
      assert.match(problems[0], /echarts-B-2PdTzp\.js/);
    },
  );
});

test('an OverviewView chunk loaded by a script tag fails', async () => {
  // A preload is not the only way in: an eager import of the route puts the
  // chunk in the entry graph and vite emits it as a module script.
  await withDist(
    {
      assets: ['index-SWwzPvTN.js', 'echarts-B-2PdTzp.js', 'OverviewView-DGcaNwgn.js'],
      entry: 'OverviewView-DGcaNwgn.js',
      preloads: SHARED_CHUNKS,
    },
    (dir) => {
      const { problems } = checkFirstLoad(dir);
      assert.equal(problems.length, 1);
      assert.match(problems[0], /OverviewView-DGcaNwgn\.js/);
    },
  );
});

test('both failures are reported in one run', async () => {
  await withDist(
    {
      assets: ['index-SWwzPvTN.js', 'OverviewView-DGcaNwgn.js'],
      preloads: [...SHARED_CHUNKS, 'OverviewView-DGcaNwgn.js'],
    },
    (dir) => {
      assert.equal(checkFirstLoad(dir).problems.length, 2);
    },
  );
});
