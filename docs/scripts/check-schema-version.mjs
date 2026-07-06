// Assert the schema reference page tracks the latest released schema version.
//
// `reference/schema.md` hand-states the current schema version in three places
// (a "current version is vX.Y.Z" line, an immutable-tag example URL, and the
// top row of the Version History table). None of them are derived from the
// repo, so the page silently rots the moment `schema/latest` is bumped and
// nobody edits the doc: it then advertises an old version as current while
// looking authoritative, and law authors copy a stale `$schema` URL. This
// check makes CI fail when the page falls behind `schema/latest`, so the
// versioned schema and its documentation can never drift apart again.
//
// It reads source files only (the `schema/latest` symlink + the markdown), not
// the astro build, so it runs without `astro build`. The `schema/` tree lives
// outside docs/ and is not in the docs Docker build context, but it IS present
// in the CI repo checkout where this script runs (see the provenance-checks
// job). Non-zero exit fails the gate.

import { readdirSync, readFileSync, existsSync, lstatSync, readlinkSync } from 'node:fs';
import { basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const SCHEMA_DIR = fileURLToPath(new URL('../../schema', import.meta.url));
const PAGE = fileURLToPath(
  new URL('../src/content/docs/reference/schema.md', import.meta.url),
);

if (!existsSync(SCHEMA_DIR)) {
  // Outside the CI checkout (e.g. the docs-only Docker build context) the
  // schema tree is absent. Skip rather than fail: this guard is a CI gate, not
  // a build step, and there is nothing to compare against here.
  console.log('check-schema-version: schema/ not present, skipping (not a full-repo checkout)');
  process.exit(0);
}

// The released version is whatever schema/latest resolves to — the single
// source of truth the rest of the repo already keys off (provenance-checks
// asserts the symlink points at the highest schema/vX.Y.Z directory).
const latestLink = `${SCHEMA_DIR}/latest`;
let latest;
if (existsSync(latestLink) && lstatSync(latestLink).isSymbolicLink()) {
  latest = basename(readlinkSync(latestLink));
} else {
  // Fall back to the highest versioned directory (e.g. checkouts that
  // materialize symlinks as plain dirs).
  latest = readdirSync(SCHEMA_DIR)
    .filter((n) => /^v\d+\.\d+\.\d+$/.test(n))
    .sort((a, b) =>
      a.localeCompare(b, undefined, { numeric: true, sensitivity: 'base' }),
    )
    .at(-1);
}

if (!latest || !/^v\d+\.\d+\.\d+$/.test(latest)) {
  console.error(`check-schema-version FAILED: could not determine latest schema version from ${SCHEMA_DIR}`);
  process.exit(1);
}

const page = readFileSync(PAGE, 'utf8');
const problems = [];

// 1. The prose "current version is vX.Y.Z" line must name the latest version.
const currentLine = page.match(/current schema version is \*\*(v\d+\.\d+\.\d+)\*\*/);
if (!currentLine) {
  problems.push('could not find the "current schema version is **vX.Y.Z**" line');
} else if (currentLine[1] !== latest) {
  problems.push(
    `"current schema version" says ${currentLine[1]} but schema/latest is ${latest}`,
  );
}

// 2. The example immutable-tag URL must point at the latest version's tag+dir.
const expectedUrl = `https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-${latest}/schema/${latest}/schema.json`;
if (!page.includes(expectedUrl)) {
  problems.push(`the example schema URL does not point at ${latest} (expected ${expectedUrl})`);
}

// 3. The Version History table must have a row for the latest version, so the
//    thing that changed is actually described (with its RFC).
const historyRow = new RegExp(`^\\|\\s*${latest.replace(/\./g, '\\.')}\\s*\\|`, 'm');
if (!historyRow.test(page)) {
  problems.push(`the Version History table has no row for ${latest}`);
}

if (problems.length) {
  console.error(`check-schema-version FAILED (schema/latest is ${latest}):`);
  for (const p of problems) console.error('  - ' + p);
  console.error(
    '\nUpdate docs/src/content/docs/reference/schema.md: bump the current-version line ' +
      'and tag URL, and add a Version History row describing what the new version introduces.',
  );
  process.exit(1);
}

console.log(`check-schema-version passed: reference page tracks schema/latest (${latest}).`);
