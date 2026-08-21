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
 * (local build, CI, the Docker image) gets the check. `just first-load-test`
 * pins the guard itself against a fixture dist, without a build.
 */
import { readdirSync, readFileSync, realpathSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

/** Chunk names that belong to the lazy overview route, never to the entry. */
const LAZY_ROUTE_CHUNK = /\/(echarts|OverviewView)-[^/]*\.js$/;

/**
 * Everything index.html makes the browser fetch before it can render: the
 * entry script plus every modulepreload. Both forms matter — a preload is a
 * request the browser fires immediately, so it counts as first load even
 * though nothing imports it yet.
 */
function firstLoadRequests(html) {
  return [
    ...html.matchAll(/<link[^>]*rel="modulepreload"[^>]*href="([^"]+)"/g),
    ...html.matchAll(/<script[^>]*src="([^"]+)"/g),
  ].map((m) => m[1]);
}

/**
 * @returns {{problems: string[], echartsChunk: string|undefined, loaded: string[]}}
 */
export function checkFirstLoad(dist) {
  const problems = [];

  const assets = readdirSync(join(dist, 'assets'));
  const echartsChunks = assets.filter((f) => /^echarts-.*\.js$/.test(f));
  if (echartsChunks.length === 0) {
    problems.push(
      'no echarts-*.js chunk in dist/assets. The codeSplitting group in ' +
        'vite.config.js no longer applies, so echarts is back in a route ' +
        'chunk (and likely in the first load).',
    );
  }

  const loaded = firstLoadRequests(readFileSync(join(dist, 'index.html'), 'utf8'));
  const offenders = loaded.filter((href) => LAZY_ROUTE_CHUNK.test(href));
  if (offenders.length > 0) {
    problems.push(
      `index.html pulls lazy-route payload into the first load: ${offenders.join(', ')}. ` +
        'See the codeSplitting comment in vite.config.js.',
    );
  }

  return { problems, echartsChunk: echartsChunks[0], loaded };
}

function main() {
  const dist = process.argv[2] || 'dist';
  const { problems, echartsChunk, loaded } = checkFirstLoad(dist);

  if (problems.length > 0) {
    for (const problem of problems) console.error(`check-first-load: ${problem}`);
    process.exit(1);
  }

  console.log(
    `check-first-load: ok (${loaded.length} static JS requests, echarts stays in ${echartsChunk})`,
  );
}

// Run only when invoked directly, so the test can import the check without
// exiting the test process on a fixture that is meant to fail.
const invokedDirectly =
  process.argv[1] && realpathSync(fileURLToPath(import.meta.url)) === realpathSync(process.argv[1]);

if (invokedDirectly) main();
