#!/usr/bin/env node
/**
 * Postbuild guard for the editor's first load.
 *
 * The `codeSplitting` group in vite.config.js keeps echarts (~550 KB) out of
 * the static entry graph; without it, rolldown parks its shared runtime
 * helper inside the OverviewView chunk and index.html modulepreloads the
 * whole echarts payload on every page. That regression is silent: the build
 * still succeeds, the app still works, the first load just doubles.
 *
 * This script makes it loud. It fails the build when:
 *  - the dedicated echarts chunk is missing from dist/assets (the
 *    codeSplitting group no longer applies, e.g. the option was renamed or
 *    removed in a Vite/rolldown upgrade), or
 *  - index.html preloads or directly loads an echarts or OverviewView chunk
 *    (echarts rejoined the static entry graph some other way).
 *
 * Runs as part of `npm run build`, so every path that produces the artifact
 * (local build, CI, the Docker image) gets the check.
 */
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const dist = process.argv[2] || 'dist';

const assets = readdirSync(join(dist, 'assets'));
const echartsChunks = assets.filter((f) => /^echarts-.*\.js$/.test(f));
if (echartsChunks.length === 0) {
  console.error(
    'check-first-load: no echarts-*.js chunk in dist/assets. The ' +
      "codeSplitting group in vite.config.js no longer applies, so echarts " +
      'is back in a route chunk (and likely in the first load).',
  );
  process.exit(1);
}

const html = readFileSync(join(dist, 'index.html'), 'utf8');
const loaded = [
  ...html.matchAll(/<link[^>]*rel="modulepreload"[^>]*href="([^"]+)"/g),
  ...html.matchAll(/<script[^>]*src="([^"]+)"/g),
].map((m) => m[1]);

const offenders = loaded.filter((href) => /\/(echarts|OverviewView)-[^/]*\.js$/.test(href));
if (offenders.length > 0) {
  console.error(
    'check-first-load: index.html pulls lazy-route payload into the first ' +
      `load: ${offenders.join(', ')}. See the codeSplitting comment in ` +
      'vite.config.js.',
  );
  process.exit(1);
}

console.log(
  `check-first-load: ok (${loaded.length} static JS requests, echarts stays in ${echartsChunks[0]})`,
);
