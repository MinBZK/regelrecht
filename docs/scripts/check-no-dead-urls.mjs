// Assert no URL the site already publishes stops resolving.
//
// A documentation URL is a promise. It gets cited in an RFC, pasted into an
// issue, bookmarked, linked from a werkpakket, and mailed to someone outside
// this repository. Renaming a page is a reasonable thing to do; making the old
// address disappear is not, because the cost lands on a reader who has no way
// to know where it went. The rule this enforces: an old URL keeps working.
//
// `astro.config.mjs` has a `redirects` map for exactly this. Renaming a page is
// then two steps rather than one, and this check is what makes the second step
// non-optional.
//
// It compares the routes built here against the routes built from the base
// branch. A route present there and absent here fails, and the message names
// the redirect line that fixes it. Non-zero exit fails the gate.
//
// ## Why it compares builds rather than a checked-in list
//
// A list of known URLs in the repository is a second source of truth that
// drifts, and the first time it drifts it either blocks a legitimate change or
// silently stops covering a page. The build already knows every route it emits;
// asking the base branch what it emitted is the same question asked of the
// state that is actually live.
//
// ## What it does not cover
//
// Anchors within a page. A heading can be renamed and this check stays green,
// because the route still resolves. That is a real gap and a narrower one than
// a vanished page: a link to a missing anchor lands on the right page rather
// than on a 404.
//
// Usage: node check-no-dead-urls.mjs <base-routes-file>
// where the file holds one route per line, from a build of the base branch.

import { readdirSync, statSync, readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIST = fileURLToPath(new URL('../dist', import.meta.url));
const CONFIG = fileURLToPath(new URL('../astro.config.mjs', import.meta.url));

const baseFile = process.argv[2];
if (!baseFile) {
  console.error('usage: check-no-dead-urls.mjs <base-routes-file>');
  process.exit(2);
}

if (!existsSync(baseFile)) {
  // No base routes means nothing to compare against. That happens on a first
  // run or when the base build failed for its own reasons; neither is evidence
  // that a URL broke, so skip loudly rather than inventing a verdict.
  console.log(`check-no-dead-urls: no base route list at ${baseFile}, skipping`);
  process.exit(0);
}

if (!existsSync(DIST)) {
  console.error('check-no-dead-urls FAILED: no docs/dist, run `astro build` first');
  process.exit(1);
}

function routes(dir, prefix = '') {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      out.push(...routes(full, `${prefix}/${entry}`));
    } else if (entry === 'index.html') {
      out.push(prefix === '' ? '/' : prefix);
    }
  }
  return out;
}

const here = new Set(routes(DIST));
const base = readFileSync(baseFile, 'utf8')
  .split('\n')
  .map((l) => l.trim())
  .filter(Boolean);

const gone = base.filter((r) => !here.has(r));

if (gone.length) {
  console.error(
    `check-no-dead-urls FAILED: ${gone.length} URL(s) the site already publishes no longer resolve:\n`,
  );
  for (const r of gone) console.error(`  ${r}`);
  console.error(
    '\nAn address that has been published stays reachable. If the page moved, add a\n' +
      'redirect in docs/astro.config.mjs:\n',
  );
  for (const r of gone) console.error(`    '${r}': '/the/new/path',`);
  console.error(
    '\nIf the page was deliberately removed rather than moved, redirect it to the page\n' +
      'that now covers the subject.',
  );
  process.exit(1);
}

// A redirect is only worth having if it points somewhere. Catch the case where
// one is added for a page that does not exist, which would otherwise look like
// a kept promise and serve a redirect loop or a 404.
const config = readFileSync(CONFIG, 'utf8');
const redirectBlock = config.match(/redirects:\s*\{([^}]*)\}/s);
const dangling = [];
if (redirectBlock) {
  for (const m of redirectBlock[1].matchAll(/'([^']+)'\s*:\s*'([^']+)'/g)) {
    if (!here.has(m[2])) dangling.push(`${m[1]} -> ${m[2]}`);
  }
}

if (dangling.length) {
  console.error('check-no-dead-urls FAILED: redirect target(s) do not exist:\n');
  for (const d of dangling) console.error(`  ${d}`);
  process.exit(1);
}

console.log(
  `check-no-dead-urls passed: all ${base.length} published URL(s) still resolve.`,
);
