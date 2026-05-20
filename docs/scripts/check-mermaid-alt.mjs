// Assert every rendered Mermaid diagram has an accessible name.
//
// The accessibility gate (pa11y) hides mermaid SVGs from the contrast runners
// (their foreignObject/attribute structure defeats axe), which would also hide
// a missing aria-label. This check covers that gap directly against the build
// output: every inline mermaid <svg> must carry role="img" and a non-empty
// aria-label (set by rehype-mermaid-alt). Run after `astro build`; non-zero
// exit fails the gate.

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

// Match each <svg …> opening tag whose id starts with "mermaid".
const SVG_OPEN = /<svg\b[^>]*\bid="mermaid[^"]*"[^>]*>/g;

const failures = [];
let total = 0;

for (const file of htmlFiles(DIST)) {
  const html = readFileSync(file, 'utf8');
  const tags = html.match(SVG_OPEN) ?? [];
  for (const tag of tags) {
    total++;
    const hasLabel = /\baria-label="[^"]+"/.test(tag);
    const hasImgRole = /\brole="img"/.test(tag);
    if (!hasLabel || !hasImgRole) {
      const id = tag.match(/\bid="([^"]*)"/)?.[1] ?? '?';
      failures.push(
        `${file.replace(DIST, 'dist')}: <svg id="${id}"> missing ` +
          [!hasImgRole && 'role="img"', !hasLabel && 'aria-label'].filter(Boolean).join(' + '),
      );
    }
  }
}

if (failures.length) {
  console.error(`Mermaid accessible-name check FAILED (${failures.length}/${total}):`);
  for (const f of failures) console.error(`  ${f}`);
  process.exit(1);
}
console.log(`Mermaid accessible-name check passed: ${total} diagram(s), all labelled.`);
