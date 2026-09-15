// Copy schema/latest/schema.json into docs/src/data/schema-latest.json.
//
// The schema reference page renders the real schema, so the build needs to
// read it. It cannot: docs/Dockerfile copies only `docs/` into the image
// (`COPY docs/ .`), so `schema/` does not exist where `astro build` runs.
// Importing it across the repo root works locally and fails in production.
//
// So the snapshot is committed, the way `src/research/rules-as-executed.html`
// is: generated elsewhere, committed here, imported normally. The copy is
// mechanical and this script is the only thing that writes it — never edit
// docs/src/data/schema-latest.json by hand.
//
// check-schema-version.mjs fails CI when the snapshot and `schema/latest`
// disagree, so a schema bump that forgets this step is caught rather than
// silently shipping a stale reference page.
//
// Run: npm run sync:schema

import { copyFileSync, existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const SOURCE = fileURLToPath(
  new URL('../../schema/latest/schema.json', import.meta.url),
);
const SNAPSHOT = fileURLToPath(
  new URL('../src/data/schema-latest.json', import.meta.url),
);

if (!existsSync(SOURCE)) {
  console.error(
    `sync-schema FAILED: ${SOURCE} does not exist.\n` +
      'Run this from a full repo checkout; the schema tree lives outside docs/.',
  );
  process.exit(1);
}

// Parse before copying: a malformed schema should fail here, loudly, rather
// than at `astro build` with a stack trace from inside a content collection.
let version;
try {
  version = JSON.parse(readFileSync(SOURCE, 'utf8')).title;
} catch (e) {
  console.error(`sync-schema FAILED: ${SOURCE} is not valid JSON: ${e.message}`);
  process.exit(1);
}

copyFileSync(SOURCE, SNAPSHOT);
console.log(`sync-schema: snapshot updated from schema/latest (${version}).`);
