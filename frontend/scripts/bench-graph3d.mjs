/**
 * Frame-time benchmark for the 3D corpus graph.
 *
 *   node scripts/bench-graph3d.mjs [--quick] [--case name]
 *
 * Starts a Vite dev server and a headless Chromium in this one process, runs
 * the cases in `bench-graph3d.html`, prints a table, and shuts both down. No
 * daemon is left behind: the whole measurement lives and dies with the command.
 *
 * On a machine without a GPU (no /dev/dri) Chromium falls back to SwiftShader,
 * which rasterises on the CPU. The numbers are then a floor, not a prediction
 * of what a user with a real GPU sees; the ratios between cases still hold, and
 * the CPU-side costs (graph build, buffer packing, label selection, picking
 * readback) are the same either way. The header of the output says which of the
 * two you are looking at.
 *
 * Two frame numbers are reported. `p50/p95` are submission times as the main
 * thread sees them; `wall` is the block wall time per frame with a `finish()`
 * at the end, which is what the user actually waits for. On a GPU they are
 * close together; on SwiftShader the wall number is the only honest one.
 */

import { readFile, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { createServer } from 'vite';
import { chromium } from '@playwright/test';

// Where the graph builder drops its payloads. Served to the page under
// /corpusgraaf/ so the measurement includes fetch and decode of the real file.
const PAYLOAD_DIR = process.env.CORPUSGRAAF_DIR || '/tmp/corpusgraaf';

const args = process.argv.slice(2);
const quick = args.includes('--quick');
const only = args.includes('--case') ? args[args.indexOf('--case') + 1] : null;

// The corpus measured today: 4.138 laws. The larger cases are the design's
// H2 and H3 horizons, and the last one is there to find the wall.
const CASES = [
  // The real corpus first: measured payloads, not synthetic ones.
  {
    name: 'echt-wetniveau',
    file: '/corpusgraaf/corpus-wetniveau.rrgraph',
    nodes: 4169,
    edges: 30142,
  },
  {
    name: 'echt-artikelniveau',
    file: '/corpusgraaf/corpus-artikelniveau.rrgraph',
    lawLevelOnly: false,
    nodes: 205585,
    edges: 259136,
    labels: false,
    frames: 60,
  },
  {
    name: 'echt-artikel-labels',
    file: '/corpusgraaf/corpus-artikelniveau.rrgraph',
    lawLevelOnly: false,
    nodes: 205585,
    edges: 259136,
    labelBudget: 400,
    frames: 60,
  },
  { name: 'corpus-nu', nodes: 4138, edges: 50000 },
  { name: 'corpus-nu-geen-labels', nodes: 4138, edges: 50000, labels: false },
  { name: 'labels-2000', nodes: 4138, edges: 50000, labelBudget: 2000 },
  { name: 'labels-5000', nodes: 4138, edges: 50000, labelBudget: 5000 },
  { name: 'h2-klein', nodes: 25000, edges: 250000 },
  { name: 'h2-groot', nodes: 100000, edges: 500000 },
  { name: 'kanten-1m', nodes: 100000, edges: 1000000 },
  { name: 'kanten-2m', nodes: 100000, edges: 2000000 },
  { name: 'kanten-5m', nodes: 150000, edges: 5000000, labels: false },
  { name: 'h3-500k', nodes: 500000, edges: 1500000, labels: false },
  // 4.000 is the renderer's default limit for the highlighted subgraph; the
  // larger ones are there to find where thickness stops being affordable.
  { name: 'dikke-kanten-4k', nodes: 4138, edges: 50000, thickEdges: 4000 },
  { name: 'dikke-kanten-20k', nodes: 4138, edges: 50000, thickEdges: 20000 },
  { name: 'dikke-kanten-100k', nodes: 4138, edges: 200000, thickEdges: 100000 },
  { name: 'dikke-kanten-300k', nodes: 4138, edges: 400000, thickEdges: 300000 },
  { name: 'sterknoop-3000', nodes: 4138, edges: 60000, hubs: 1 },
];

const QUICK = new Set(['echt-wetniveau', 'echt-artikelniveau', 'corpus-nu']);

const COLUMNS = [
  ['case', 22],
  ['nodes', 8],
  ['edges', 9],
  ['p50 ms', 8],
  ['p95 ms', 8],
  ['wall ms', 8],
  ['fps', 7],
  ['build ms', 9],
  ['pick ms', 8],
  ['label ms', 9],
  ['calls', 6],
  ['heap MB', 8],
];

function row(values) {
  return COLUMNS.map(([, w], i) => String(values[i] ?? '').padStart(w)).join(' ');
}

/**
 * Serve the graph builder's payloads under /corpusgraaf.
 *
 * It has to be a plugin, not `server.middlewares.use` after the fact: Vite
 * installs its own middlewares (including the HTML fallback) first, and an
 * unknown path would come back as index.html instead of the payload - which
 * looks exactly like a corrupt file to the reader.
 */
function payloadPlugin() {
  return {
    name: 'corpusgraaf-payloads',
    configureServer(server) {
      server.middlewares.use('/corpusgraaf', async (req, res, next) => {
        const name = req.url.split('?')[0].replace(/^\//, '');
        if (!/^[\w.-]+$/.test(name)) return next();
        try {
          const path = join(PAYLOAD_DIR, name);
          await stat(path);
          const body = await readFile(path);
          res.setHeader('content-type', 'application/octet-stream');
          res.setHeader('content-length', body.length);
          res.end(body);
        } catch {
          res.statusCode = 404;
          res.end('geen payload');
        }
      });
    },
  };
}

async function main() {
  const server = await createServer({
    root: new URL('..', import.meta.url).pathname,
    // HMR off: a source edit while a measurement runs would reload the page
    // mid-case and destroy the harness, turning every later case into an
    // execution-context error instead of a number.
    server: { port: 0, host: '127.0.0.1', hmr: false },
    logLevel: 'warn',
    plugins: [payloadPlugin()],
  });
  await server.listen();
  const port = server.config.server.port || server.httpServer.address().port;
  const url = `http://127.0.0.1:${port}/bench-graph3d.html`;

  const launchArgs = [
    '--use-gl=angle',
    '--use-angle=swiftshader',
    '--enable-unsafe-swiftshader',
    '--js-flags=--expose-gc',
    '--enable-precise-memory-info',
    // Without this the measurement is pinned to the display refresh and every
    // case that fits in 16 ms reports 16 ms.
    '--disable-frame-rate-limit',
  ];
  let browser = await chromium.launch({ args: launchArgs });

  // A fresh page per case. The large cases allocate hundreds of megabytes of
  // typed arrays and GPU buffers, and a renderer that has already been through
  // five of them measures its predecessors' fragmentation as much as its own
  // work. If the browser died with the previous case, relaunch it.
  async function freshPage() {
    if (!browser.isConnected()) browser = await chromium.launch({ args: launchArgs });
    const p = await browser.newPage({ viewport: { width: 1600, height: 900 } });
    p.on('pageerror', (err) => console.error('page error:', err.message));
    p.on('console', (msg) => {
      if (msg.type() === 'error') console.error('console:', msg.text());
    });
    await p.goto(url, { waitUntil: 'load' });
    await p.waitForFunction(() => !!window.__graphBench, null, { timeout: 60000 });
    return p;
  }

  let page = await freshPage();

  const renderer = await page.evaluate(() => {
    const canvas = document.createElement('canvas');
    const gl = canvas.getContext('webgl2') || canvas.getContext('webgl');
    if (!gl) return 'geen WebGL';
    const ext = gl.getExtension('WEBGL_debug_renderer_info');
    return ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
  });

  const fontsPresent = await page.evaluate(() => {
    const c = document.createElement('canvas').getContext('2d');
    c.font = '32px sans-serif';
    return c.measureText('n').width > 0;
  });

  console.log(`\nrenderer: ${renderer}`);
  if (!fontsPresent) {
    console.log(
      'LET OP: geen enkel systeemlettertype beschikbaar in deze browser, dus de\n' +
        '        labellaag kan hier niet gemeten worden (elke glyph meet nul breed).\n' +
        '        Draai met FONTCONFIG_FILE naar een config met een lettertype erin.',
    );
  }
  console.log(`viewport: 1600x900\n`);
  console.log(row(COLUMNS.map(([n]) => n)));
  console.log(COLUMNS.map(([, w]) => '-'.repeat(w)).join(' '));

  const results = [];
  for (const c of CASES) {
    if (only && c.name !== only) continue;
    if (!only && quick && !QUICK.has(c.name)) continue;
    let res;
    try {
      res = await page.evaluate((cfg) => window.__graphBench.runCase(cfg), {
        frames: quick ? 60 : 120,
        ...c,
      });
    } catch (err) {
      // A crash here is a result too: this size took the tab down. Say so and
      // carry on with a fresh page instead of failing every later case.
      console.log(row([c.name, c.nodes, c.edges, 'STUK', String(err.message).slice(0, 30)]));
      results.push({ name: c.name, error: String(err.message) });
      await page.close().catch(() => {});
      page = await freshPage();
      continue;
    }
    results.push({ name: c.name, ...res });
    console.log(
      row([
        c.name,
        res.nodes,
        res.edges,
        res.frameP50,
        res.frameP95,
        res.wallPerFrame,
        res.fps,
        res.buildMs,
        res.pickP50,
        res.labelP50,
        res.drawCalls,
        res.heapMB ?? '-',
      ]),
    );
    await page.close().catch(() => {});
    page = await freshPage();
    if (res.aborted) {
      console.log(
        `${' '.repeat(6)}(afgebroken na ${res.framesMeasured} frames: deze omvang haalt de deadline niet)`,
      );
    }
  }

  if (process.env.BENCH_JSON) {
    console.log('\nJSON:');
    console.log(JSON.stringify({ renderer, results }, null, 2));
  }

  await browser.close();
  await server.close();
}

main().catch(async (err) => {
  console.error(err);
  process.exit(1);
});
