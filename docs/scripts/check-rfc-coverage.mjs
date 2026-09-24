// Assert the documentation-coverage matrix lists every RFC that owes prose.
//
// `reference/documentation-coverage.md` is a hand-maintained table claiming,
// per RFC, where its prose coverage lives (or that it is on the backlog). A
// hand-maintained coverage claim rots the moment an RFC is accepted or built
// and nobody updates the table: it then asserts coverage that does not exist
// while looking authoritative. This check fails CI when such an RFC is missing
// from the page, so the matrix grows with the RFC set.
//
// An RFC owes coverage when any of three things holds:
//
// - `status: Accepted`: the decision has been taken.
// - `implementation: Implemented`: the construct is live whatever the
//   acceptance ceremony says. A reader meets the field in a law file, not the
//   status tag. Keying on `Accepted` alone let `voids` ship in schema v0.7.0,
//   a field separating "no entitlement" from "an entitlement of zero", with no
//   prose page at all, because RFC-041 that defined it sits at `Proposed`.
// - `implementation: Partially implemented`: the built half is just as live.
//   Keying on the first two let the partly built enricher RFCs 026 and 027
//   go without a row.
//
// What it does not do: judge whether the page a row points at is any good.
// It guarantees a row exists; accuracy stays with whoever accepts or builds
// the RFC. Any mention of the id on the page counts as tracked, so a Backlog
// line is enough. It reads source files (frontmatter plus the markdown), not
// the build, so it runs without `astro build`. Non-zero exit fails the gate.

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

// Every RFC that owes prose coverage, from frontmatter: the source of truth,
// not a list to keep in sync by hand. See the header for the three conditions.
const OWES_COVERAGE_IMPLEMENTATION = new Set(['Implemented', 'Partially implemented']);
const rfcsOwingCoverage = [];
for (const entry of readdirSync(RFC_DIR)) {
  const m = entry.match(/^(rfc-\d+)\.md$/);
  if (!m) continue;
  const src = readFileSync(join(RFC_DIR, entry), 'utf8');
  const status = frontmatterField(src, 'status');
  const implementation = frontmatterField(src, 'implementation');
  if (status === 'Accepted' || OWES_COVERAGE_IMPLEMENTATION.has(implementation)) {
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
    `Accepted, Implemented or Partially implemented RFC(s) (excluding ${[...EXEMPT].join(', ')}) are tracked.`,
);
