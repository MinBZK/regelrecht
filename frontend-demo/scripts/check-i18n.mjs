/**
 * De twee woordenboeken dekken elkaar, en niemand vertaalt buiten hen om.
 *
 * The vitest suite next to the dictionaries already compares their keys. This
 * check exists for the two things a test inside the app cannot see:
 *
 *  1. A string that never reached a dictionary at all. A `t()` call with a key
 *     that is in neither file renders the key on screen, and nothing else in
 *     the pipeline notices.
 *  2. A locale ternary written into a component. `locale === 'en' ? … : …` in a
 *     `.vue` file is how the docs landing page ended up with its strings spread
 *     over three places; it type-checks, it renders, and it takes the string
 *     out of reach of every parity check there is. It is cheap to refuse here
 *     and expensive to unpick later.
 *
 * Runs at `prebuild`/`predev` next to check-service-map.mjs, in its style:
 * line-oriented reads, a Dutch summary, exit 1. No YAML parser and no
 * dependency on the app being built.
 */
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import en from '../src/i18n/en.js';
import nl from '../src/i18n/nl.js';

const here = dirname(fileURLToPath(import.meta.url));
const srcDir = resolve(here, '..', 'src');
const i18nDir = join(srcDir, 'i18n');
const lawsDir = resolve(here, '..', '..', 'corpus', 'demo', 'regulation', 'nl');
const glossaryFile = resolve(here, '..', '..', 'corpus', 'demo', 'i18n', 'glossary.en.yaml');

function walk(dir, out = [], match = (f) => /\.(vue|js)$/.test(f) && !f.endsWith('.test.js')) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, out, match);
    else if (match(full)) out.push(full);
  }
  return out;
}

const problems = [];

// ---- 1. every key a `t()` call asks for exists -----------------------------
//
// Only literal keys can be checked. `t(`app.features.${f.key}`)` is a template
// and is skipped here; the dictionaries' own test covers those by comparing
// the two sides, and a missing one falls back to Dutch rather than breaking.
const used = new Map();
for (const file of walk(srcDir)) {
  if (file.startsWith(i18nDir)) continue;
  const text = readFileSync(file, 'utf8');
  // De `(` hoort in de alternatie en niet erachter: stond hij erbuiten, dan
  // eiste de plural-tak een tweede haakje (`t.plural(x, ('key'`) en werd élke
  // `t.plural`-aanroep stil overgeslagen — de vorm die deze check juist moet
  // zien. Beide soorten aanhalingstekens, want niets houdt iemand bij `"` weg.
  for (const m of text.matchAll(/\bt(?:\.plural\(\s*[^,]+,\s*|\(\s*)['"]([a-z][\w.]*)['"]/g)) {
    if (!used.has(m[1])) used.set(m[1], relative(srcDir, file));
  }
}

for (const [key, file] of used) {
  // A plural key is stored as `.one`/`.other`, so the bare stem counts as
  // present when either form is there.
  const known = key in nl || `${key}.one` in nl || `${key}.other` in nl;
  if (!known) problems.push(`onbekende sleutel ${key} (${file})`);
}

// ---- 2. no component decides language on its own ---------------------------
for (const file of walk(srcDir)) {
  if (file.startsWith(i18nDir)) continue;
  const rel = relative(srcDir, file);
  // The router legitimately branches on locale: it is what builds the paths.
  if (rel === 'router.js') continue;
  const text = readFileSync(file, 'utf8');
  const lines = text.split('\n');
  for (const [i, line] of lines.entries()) {
    // Any branch on the locale, not only a ternary: `locale === 'nl' && x` is
    // the same decision written differently, and an author reaching for it is
    // making the choice this check exists to catch.
    if (!/locale(?:\.value)?\s*===\s*'(?:nl|en)'/.test(line)) continue;
    // A branch may be deliberate — falling back to Dutch corpus content that
    // has no translation yet. It then says so on the line above, and the
    // comment is what a reviewer reads.
    const excused = lines.slice(Math.max(0, i - 10), i).some((l) => /i18n-ok:/.test(l));
    if (!excused) {
      problems.push(`taal-vertakking in ${rel}:${i + 1} — zet de tekst in nl.js/en.js, of verantwoord hem met een \`i18n-ok:\`-commentaar`);
    }
  }
}

// ---- 3. de woordenlijst dekt elke veldnaam die de demo kan tonen -----------
//
// Deze controle kruist de grens tussen code en corpus, en dat is precies wat de
// vitest-suite niet kan zien: een nieuwe wet brengt nieuwe veldnamen mee, en die
// vallen stil terug op het Nederlands. Niet fataal — een Nederlands label is een
// schoonheidsfout en geen onwaarheid — maar wel iets wat iemand hoort te zien,
// dus het telt hier mee en wordt op frequentie geprint zodat de volgende ronde
// zichzelf sorteert.
function readGlossaryBlock(text, block) {
  const out = new Set();
  let inBlock = false;
  for (const raw of text.split('\n')) {
    const line = raw.replace(/\s+$/, '');
    if (!line || line.startsWith('#')) continue;
    if (line === `${block}:`) { inBlock = true; continue; }
    if (/^\S/.test(line)) { inBlock = false; continue; }
    if (!inBlock) continue;
    const m = /^ {2}("?)([a-z0-9][a-z0-9_]*)\1:\s*\S/.exec(line);
    if (m) out.add(m[2]);
  }
  return out;
}

// Dezelfde afkortingen die format.js kent; die horen bewust niet in de lijst.
const ABBREVIATIONS = new Set(['agp', 'aow', 'bsn', 'bbz', 'brp', 'cbs', 'haccp', 'kvk', 'nvwa', 'sbi', 'svh', 'vog', 'wia', 'ww', 'zvw']);

if (existsSync(glossaryFile)) {
  const text = readFileSync(glossaryFile, 'utf8');
  const words = readGlossaryBlock(text, 'words');
  const wholeNames = readGlossaryBlock(text, 'names');

  const fieldNames = new Map();
  for (const file of walk(lawsDir, [], (f) => f.endsWith('.yaml'))) {
    for (const line of readFileSync(file, 'utf8').split('\n')) {
      const m = /^\s*-\s+name:\s+([a-z][a-z0-9_]*)\s*$/.exec(line);
      if (m) fieldNames.set(m[1], (fieldNames.get(m[1]) ?? 0) + 1);
    }
  }

  const missing = new Map();
  for (const [name, count] of fieldNames) {
    if (wholeNames.has(name)) continue;
    for (const token of name.split('_')) {
      if (words.has(token) || ABBREVIATIONS.has(token)) continue;
      missing.set(token, (missing.get(token) ?? 0) + count);
    }
  }

  if (missing.size) {
    const top = [...missing.entries()].sort((a, b) => b[1] - a[1]).slice(0, 12);
    problems.push(
      `de woordenlijst mist ${missing.size} woord(en); label(s) vallen terug op het Nederlands. ` +
        'Draai `node scripts/i18n-report.mjs --missing`. Meest voorkomend: ' +
        top.map(([w, n]) => `${w} (${n}x)`).join(', '),
    );
  }
}

if (problems.length) {
  console.error('i18n en het democorpus lopen uiteen:\n');
  for (const p of problems) console.error('  ' + p);
  console.error(`\n${problems.length} probleem(en).`);
  process.exit(1);
}

const glossaryWords = existsSync(glossaryFile) ? readGlossaryBlock(readFileSync(glossaryFile, 'utf8'), 'words').size : 0;
console.log(
  `i18n: ${Object.keys(nl).length} sleutels in nl en ${Object.keys(en).length} in en, ${used.size} gebruikt in de app` +
    (glossaryWords ? `, woordenlijst ${glossaryWords} woorden.` : '.'),
);
