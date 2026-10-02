// Assert the writing rules of the basiswerk (src/content/docs/basiswerk/).
//
// The basiswerk is a reading text with a fixed rhythm per chapter: an opening
// question, prose, at most one schema, at most three sources. A chapter that
// grows past fifteen minutes should be split. These rules were enforced by the
// renderer of the site the basiswerk came from; here they run against the
// source, so a change made through GitHub's editor is checked like any other.
//
//   - reading time per chapter at most 15 minutes (200 words a minute, a
//     schema counts as 120 words: it is looked at, not read)
//   - at most one <Schema> per chapter
//   - at most three items in <Bronnen>
//   - every term in the glossary (src/lib/basiswerk/begrippen.js) occurs in
//     the chapter the glossary says introduces it
//   - every <Schema code> exists in src/lib/basiswerk/schemas.js
//   - every page ends with <Conceptnoot />: the basiswerk is a draft
//   - every link into the position paper (/research/rules-as-executed#…),
//     in the chapters and in the RegelRecht layer of the schemas, names a
//     section the paper has
//
// Non-zero exit fails the gate.

import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { begrippen } from '../src/lib/basiswerk/begrippen.js';
import { schemas } from '../src/lib/basiswerk/schemas.js';

const paperSections = new Set(
  JSON.parse(readFileSync(new URL('../src/research/rules-as-executed.headings.json', import.meta.url), 'utf8')).map((h) => h.slug),
);

const DIR = fileURLToPath(new URL('../src/content/docs/basiswerk/', import.meta.url));
const WORDS_PER_MINUTE = 200;
const SCHEMA_WORDS = 120;
const MAX_MINUTES = 15;

const problems = [];
const text = {};

for (const file of readdirSync(DIR).filter((f) => f.endsWith('.mdx'))) {
  const name = file.replace(/\.mdx$/, '');
  const src = readFileSync(DIR + file, 'utf8');
  const body = src.replace(/^---[\s\S]*?\n---\n/, '');

  const schemaCodes = [...body.matchAll(/<Schema\s+code="([^"]+)"/g)].map((m) => m[1]);
  const bronnen = body.match(/<Bronnen\s+items=\{(\[[\s\S]*?\])\}\s*\/>/);
  const bronCount = bronnen ? JSON.parse(bronnen[1]).length : 0;

  // Words a reader reads: drop imports, components' markup and link targets.
  const prose = body
    .replace(/^import .*$/gm, '')
    .replace(/<Bronnen[\s\S]*?\/>/g, '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\]\([^)]*\)/g, ']');
  const words = prose.split(/\s+/).filter((w) => /\p{L}/u.test(w)).length;
  const minutes = Math.round((words + schemaCodes.length * SCHEMA_WORDS) / WORDS_PER_MINUTE);

  if (minutes > MAX_MINUTES) problems.push(`${name}: ${minutes} min, more than ${MAX_MINUTES}; split the chapter`);
  if (schemaCodes.length > 1 && name !== 'bijlage-schemas') problems.push(`${name}: ${schemaCodes.length} schemas, at most one per chapter`);
  if (!/<Conceptnoot \/>\s*$/.test(body)) problems.push(`${name}: does not end with <Conceptnoot />`);
  if (bronCount > 3) problems.push(`${name}: ${bronCount} sources in <Bronnen>, at most three`);
  for (const code of schemaCodes) if (!schemas[code]) problems.push(`${name}: <Schema code="${code}"> is not in schemas.js`);
  for (const m of body.matchAll(/\/research\/rules-as-executed#([^)\s"]+)/g)) {
    if (!paperSections.has(m[1])) problems.push(`${name}: link to #${m[1]}, which is not a section of Rules as Executed`);
  }

  text[`/basiswerk/${name}`] = prose.toLowerCase();
}

for (const [code, s] of Object.entries(schemas)) {
  for (const r of s.regelrecht ?? []) {
    if (!paperSections.has(r.paper)) problems.push(`schemas.js ${code}: RegelRecht note points at #${r.paper}, not a section of the paper`);
  }
}

for (const b of begrippen) {
  if (!(b.waar in text)) problems.push(`begrippen: "${b.term}" points at ${b.waar}, which does not exist`);
  else if (!text[b.waar].includes(b.term.toLowerCase())) problems.push(`begrippen: "${b.term}" does not occur in ${b.waar}`);
}

if (problems.length) {
  console.error(`check-basiswerk: ${problems.length} problem(s):\n  - ${problems.join('\n  - ')}`);
  process.exit(1);
}
console.log(`check-basiswerk passed: ${Object.keys(text).length} chapter(s), ${begrippen.length} glossary term(s).`);
