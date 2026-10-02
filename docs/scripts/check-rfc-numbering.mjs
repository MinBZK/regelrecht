// Assert the RFC sequence has no holes, and that every hole-filler is honest.
//
// A gap in the published sequence is the bug that produces duplicate RFCs. The
// number of an unmerged RFC is invisible: until its pull request lands, the
// site shows 024, 026 and nothing in between, so the next author reads 025 as
// free and takes it. That happened twice — RFC-025 (pull requests 933 and a
// later branch) and RFC-036 (pull request 1298 against the RFC-036 that merged
// first) — and the second one left a colleague with a merge conflict on a file
// they believed they were creating.
//
// RFC-000 already says a number is never reused once assigned. This check is
// what makes an assignment visible: a claimed number gets a `status: Reserved`
// placeholder on the default branch the day its pull request opens, so the
// sequence the next author reads has no gaps to walk into.
//
// It reads frontmatter from the source files, not the build, so it runs
// without `astro build`. Non-zero exit fails the gate.

import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RFC_DIR = fileURLToPath(new URL('../src/content/rfcs', import.meta.url));

// Tolerate CRLF. A file saved with Windows line endings would otherwise not
// match at all, `frontmatter()` would return '', every field would read as
// null, and the consistency rules below would quietly skip it — a checker that
// reports "passed" on a file it could not read is worse than one that fails.
function frontmatter(src) {
  const fm = src.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  return fm ? fm[1] : null;
}

function field(fm, name) {
  const line = fm.match(new RegExp(`^${name}:[ \\t]*(.+?)[ \\t]*\\r?$`, 'm'));
  return line ? line[1].trim().replace(/^['"]|['"]$/g, '') : null;
}

const rfcs = [];
const problems = [];

for (const entry of readdirSync(RFC_DIR)) {
  const m = entry.match(/^rfc-(\d+)\.md$/);
  if (!m) continue;
  const fm = frontmatter(readFileSync(join(RFC_DIR, entry), 'utf8'));
  if (fm === null) {
    problems.push(
      `${entry} has no frontmatter block, so its status cannot be read.\n` +
        `  Every RFC opens with a '---' block carrying at least a title and a status.`,
    );
    continue;
  }
  rfcs.push({
    num: Number(m[1]),
    file: entry,
    status: field(fm, 'status'),
    reservedBy: field(fm, 'reserved_by'),
  });
}
rfcs.sort((a, b) => a.num - b.num);

// 0. One file per number. `rfc-01.md` and `rfc-001.md` both parse to 1, so
//    without this the two would sit side by side and the gap check would see a
//    complete sequence. That is the very collision this script exists to stop,
//    reached through a filename variant instead of through two branches.
const byNumber = new Map();
for (const r of rfcs) {
  const existing = byNumber.get(r.num);
  if (existing) {
    problems.push(
      `RFC-${String(r.num).padStart(3, '0')} is claimed by two files: ${existing.file} and ${r.file}.\n` +
        `  One file per number, named rfc-NNN.md with three digits.`,
    );
    continue;
  }
  byNumber.set(r.num, r);
}

// 1. No holes. Every number from 000 up to the highest must exist as a file,
//    even if only as a Reserved placeholder.
const highest = rfcs[rfcs.length - 1]?.num ?? 0;
const present = new Set(rfcs.map((r) => r.num));
const missing = [];
for (let n = 0; n <= highest; n++) {
  if (!present.has(n)) missing.push(n);
}
if (missing.length) {
  const list = missing
    .map((n) => `rfc-${String(n).padStart(3, '0')}.md`)
    .join(', ');
  problems.push(
    `Gap in the RFC sequence: ${list} missing, while RFC-${String(highest).padStart(3, '0')} exists.\n` +
      `  A gap reads as a free number and gets claimed twice. If an open pull request holds it,\n` +
      `  add a placeholder with 'status: Reserved' and a 'reserved_by' link. If the number was\n` +
      `  spent on a proposal that was dropped, the placeholder says so with 'status: Rejected'.\n` +
      `  RFC-000: a number is never reused once assigned.`,
  );
}

// 2. A reservation must be traceable, and a placeholder must say it is one.
//    The content schema enforces the same two rules at build time; they are
//    repeated here so the source check catches them without a build.
//
//    Keep these in step with the superRefine in src/content.config.ts. A
//    placeholder is any file carrying `reserved_by`, and it sits at `Reserved`
//    while its pull request is open or `Rejected` once that pull request
//    closed unmerged — the number is spent either way. Allowing only
//    `Reserved` here would reject the end state the schema accepts and
//    RFC-000 documents, which is how these two guards drifted apart once.
const PLACEHOLDER_STATUSES = new Set(['Reserved', 'Rejected']);

for (const r of rfcs) {
  if (r.status === 'Reserved' && !r.reservedBy) {
    problems.push(
      `${r.file} is Reserved but names no pull request in 'reserved_by'.\n` +
        `  A reservation nobody can trace back to a branch blocks the number forever.`,
    );
  }
  if (r.reservedBy && !PLACEHOLDER_STATUSES.has(r.status)) {
    problems.push(
      `${r.file} carries 'reserved_by' but its status is '${r.status}'.\n` +
        `  That field belongs to a placeholder: 'Reserved' while its pull request is\n` +
        `  open, or 'Rejected' once it closed unmerged. Any other status means the real\n` +
        `  RFC landed, and then 'reserved_by' should go.`,
    );
  }
}

if (problems.length) {
  console.error('RFC numbering check failed:\n');
  for (const p of problems) console.error(`- ${p}\n`);
  process.exit(1);
}

const reserved = rfcs.filter((r) => r.status === 'Reserved');
console.log(
  `RFC numbering check passed: ${rfcs.length} RFC(s), numbered 000-${String(highest).padStart(3, '0')} with no gaps` +
    (reserved.length
      ? `, ${reserved.length} reserved (${reserved.map((r) => `RFC-${String(r.num).padStart(3, '0')}`).join(', ')}).`
      : '.'),
);
