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

function frontmatter(src) {
  const fm = src.match(/^---\n([\s\S]*?)\n---/);
  return fm ? fm[1] : '';
}

function field(fm, name) {
  const line = fm.match(new RegExp(`^${name}:\\s*(.+)$`, 'm'));
  return line ? line[1].trim().replace(/^['"]|['"]$/g, '') : null;
}

const rfcs = [];
for (const entry of readdirSync(RFC_DIR)) {
  const m = entry.match(/^rfc-(\d+)\.md$/);
  if (!m) continue;
  const fm = frontmatter(readFileSync(join(RFC_DIR, entry), 'utf8'));
  rfcs.push({
    num: Number(m[1]),
    file: entry,
    status: field(fm, 'status'),
    reservedBy: field(fm, 'reserved_by'),
  });
}
rfcs.sort((a, b) => a.num - b.num);

const problems = [];

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

// 2. A reservation must be traceable. The content schema enforces this at build
//    time; repeated here so the source check catches it without a build, and so
//    the rule is stated where the numbering rule lives.
for (const r of rfcs) {
  if (r.status === 'Reserved' && !r.reservedBy) {
    problems.push(
      `${r.file} is Reserved but names no pull request in 'reserved_by'.\n` +
        `  A reservation nobody can trace back to a branch blocks the number forever.`,
    );
  }
  if (r.status !== 'Reserved' && r.reservedBy) {
    problems.push(
      `${r.file} carries 'reserved_by' but its status is '${r.status}'.\n` +
        `  That field belongs to a Reserved placeholder only.`,
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
