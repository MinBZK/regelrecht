// Assert no heading-rank skips (e.g. h2 → h5) in any built page.
//
// htmlcs's WCAG 1.3.1 check does not flag rank skips, and axe's heading-order
// rule sits under axe's "best-practice" tag, which pa11y disables when the
// standard is WCAG2AA. The gate therefore does not catch a hierarchy regression
// like the H2 → H5 footer headings we just fixed. This check covers that gap
// directly against the build output. Run after `astro build`; non-zero exit
// fails the gate.

import { readdirSync, statSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIST = fileURLToPath(new URL('../dist', import.meta.url));

function htmlFiles(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...htmlFiles(full));
    else if (entry.endsWith('.html')) out.push(full);
  }
  return out;
}

// Capture each heading's level and a short text snippet for the error message.
const HEADING_RE = /<h([1-6])\b[^>]*>([\s\S]*?)<\/h\1>/gi;

const failures = [];
let totalHeadings = 0;

for (const file of htmlFiles(DIST)) {
  const html = readFileSync(file, 'utf8');
  let last = 0;
  let m;
  HEADING_RE.lastIndex = 0;
  while ((m = HEADING_RE.exec(html)) !== null) {
    totalHeadings++;
    const lvl = +m[1];
    const text = m[2].replace(/<[^>]+>/g, '').trim().slice(0, 60);
    if (last && lvl > last + 1) {
      failures.push(
        `${file.replace(DIST, 'dist')}: h${last} → h${lvl} jump at "${text}"`,
      );
    }
    last = lvl;
  }
}

if (failures.length) {
  console.error(`Heading-order check FAILED (${failures.length} jump(s) across ${totalHeadings} headings):`);
  for (const f of failures) console.error('  ' + f);
  process.exit(1);
}

console.log(`Heading-order check passed: ${totalHeadings} heading(s), no rank skips.`);
