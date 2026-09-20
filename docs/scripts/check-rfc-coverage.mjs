// Assert the documentation-coverage matrix lists every Accepted RFC.
//
// `reference/documentation-coverage.md` is a hand-maintained table claiming,
// per Accepted RFC, where its prose coverage lives (or that it is on the
// backlog). A hand-maintained coverage claim rots the moment a new RFC is
// accepted and nobody updates the table: it then asserts coverage that does
// not exist while looking authoritative. This check makes the table fail CI
// when an Accepted RFC is missing from it, so the matrix stays honest as the
// RFC set grows. It reads source files (frontmatter + the markdown table),
// not the build, so it runs without `astro build`. Non-zero exit fails the gate.

import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RFC_DIR = fileURLToPath(new URL('../src/content/rfcs', import.meta.url));
const COVERAGE = fileURLToPath(
  new URL('../src/content/docs/reference/documentation-coverage.md', import.meta.url),
);

// RFC-000 is the process itself; it documents itself and the coverage page says
// so in prose rather than the table. Any other Accepted RFC must appear.
const EXEMPT = new Set(['RFC-000']);

// Pull a field out of YAML frontmatter (first --- ... --- block).
function frontmatterField(src, name) {
  const fm = src.match(/^---\n([\s\S]*?)\n---/);
  if (!fm) return null;
  const line = fm[1].match(new RegExp(`^${name}:\\s*(.+)$`, 'm'));
  if (!line) return null;
  return line[1].trim().replace(/^['"]|['"]$/g, '');
}

// Every RFC that owes prose coverage, from frontmatter — the source of truth,
// not a list to keep in sync by hand.
//
// Two ways to owe it, because either one alone leaves a hole. `Accepted` is the
// decision having been taken. `Implemented` is the construct being live in the
// shipped schema, which obliges coverage whatever the acceptance ceremony says:
// a reader hits the field in a law file, not the status tag. Keying on
// `Accepted` alone let `voids` ship in schema v0.7.0 — a field distinguishing
// "no entitlement" from "an entitlement of zero" — with no prose page at all,
// because RFC-041 that defined it sits at `Proposed`.
const rfcsOwingCoverage = [];
for (const entry of readdirSync(RFC_DIR)) {
  const m = entry.match(/^(rfc-\d+)\.md$/);
  if (!m) continue;
  const src = readFileSync(join(RFC_DIR, entry), 'utf8');
  const status = frontmatterField(src, 'status');
  const implementation = frontmatterField(src, 'implementation');
  if (status === 'Accepted' || implementation === 'Implemented') {
    rfcsOwingCoverage.push(m[1].toUpperCase());
  }
}
rfcsOwingCoverage.sort();

// Every RFC id mentioned anywhere in the coverage page (table rows or the
// Backlog prose both count as "tracked").
const coverageSrc = readFileSync(COVERAGE, 'utf8');
const tracked = new Set(coverageSrc.match(/RFC-\d+/g) ?? []);

const missing = rfcsOwingCoverage.filter((id) => !EXEMPT.has(id) && !tracked.has(id));

if (missing.length) {
  console.error(
    `RFC coverage check FAILED: ${missing.length} RFC(s) absent from ` +
      `documentation-coverage.md:`,
  );
  for (const id of missing) console.error('  ' + id);
  console.error(
    'Add a row to the coverage table (or a Backlog entry) for each, then re-run.',
  );
  process.exit(1);
}

console.log(
  `RFC coverage check passed: all ${rfcsOwingCoverage.length - rfcsOwingCoverage.filter((id) => EXEMPT.has(id)).length} ` +
    `Accepted or Implemented RFC(s) (excluding ${[...EXEMPT].join(', ')}) are tracked.`,
);
