// Report which implemented RFCs no werkpakket on the roadmap points at.
//
// An RFC with `implementation: Implemented` describes work that is done, and
// one on `Partially implemented` work that is under way; both belong somewhere
// in the roadmap. Nothing enforces that: an RFC lands, the roadmap is updated
// later or not at all, and the gap is invisible.
//
// This REPORTS and always exits 0. Failing would put the roadmap in the way of
// the work it describes — an RFC could not merge until someone edited a
// werkpakket — and a gate that blocks on editorial lag gets worked around
// rather than satisfied. The roadmap's own build already fails on the errors
// that produce a wrong page (a non-existent RFC number, a dangling reference);
// this is the softer question of coverage.
//
// Reads source files, not the build, so it runs without `astro build`.
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const RFC_DIR = fileURLToPath(new URL('../src/content/rfcs', import.meta.url));
const WP_DIR = fileURLToPath(
  new URL('../src/content/roadmap/werkpakketten', import.meta.url),
);

// RFC-000 is the RFC process itself; it describes no work a werkpakket could
// carry, so counting it would make the report permanently non-empty.
const EXEMPT = new Set(['RFC-000']);

const field = (text, name) =>
  (text.match(new RegExp(`^${name}:\\s*(.*)$`, 'm')) ?? [])[1]?.trim() ?? '';

const rfcs = readdirSync(RFC_DIR)
  .filter((f) => /^rfc-\d+\.md$/.test(f))
  .map((f) => {
    const text = readFileSync(`${RFC_DIR}/${f}`, 'utf8');
    return {
      num: parseInt(f.match(/\d+/)[0], 10),
      id: `RFC-${f.match(/\d+/)[0]}`,
      title: field(text, 'title').replace(/"/g, '').replace(/^RFC-\d+:\s*/, ''),
      implementation: field(text, 'implementation'),
    };
  })
  .filter((r) => !EXEMPT.has(r.id));

// Partially implemented counts too: it describes work that is under way, and
// an RFC whose build is half done is exactly the kind the roadmap should be
// showing. Only "Not implemented" (drafts included) may float free.
const groups = [
  { label: 'geïmplementeerde', status: 'Implemented' },
  { label: 'deels geïmplementeerde', status: 'Partially implemented' },
];

// Every RFC number any werkpakket references, from the `rfcs:` sequence.
const referenced = new Set();
for (const f of readdirSync(WP_DIR).filter((x) => x.endsWith('.md'))) {
  const text = readFileSync(`${WP_DIR}/${f}`, 'utf8');
  // \r? throughout, and [ \t] rather than \s for the indent: a file written on
  // Windows has CRLF endings, and without this the block simply does not match
  // — under-reporting coverage instead of failing, which is the hardest kind of
  // wrong to notice. (\s would also swallow the newline and span list items.)
  const block = text.match(/^rfcs:\r?\n((?:[ \t]+-[ \t]*\d+\r?\n)+)/m);
  if (!block) continue;
  for (const m of block[1].matchAll(/-\s*(\d+)/g)) {
    referenced.add(parseInt(m[1], 10));
  }
}

let onvolledig = false;
for (const { label, status } of groups) {
  const all = rfcs.filter((r) => r.implementation === status);
  const missing = all.filter((r) => !referenced.has(r.num));
  if (missing.length === 0) {
    console.log(
      `Roadmap-RFC-dekking: alle ${all.length} ${label} RFC(s) hangen aan ` +
        `een werkpakket (${[...EXEMPT].join(', ')} uitgezonderd).`,
    );
  } else {
    onvolledig = true;
    console.log(
      `Roadmap-RFC-dekking: ${missing.length} van ${all.length} ${label} ` +
        'RFC(s) hangen nog aan geen enkel werkpakket:',
    );
    for (const r of missing) console.log(`  ${r.id}  ${r.title}`);
  }
}
if (onvolledig) {
  console.log(
    '\nDat is een melding, geen fout: niet elke RFC hoort bij een werkpakket, ' +
      'en de roadmap mag achterlopen op het werk dat hij beschrijft.',
  );
}
