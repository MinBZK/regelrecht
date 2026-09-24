/**
 * Wat er nog vertaald moet worden aan de veldnamen in het democorpus.
 *
 * The 551 distinct field names in the demo corpus decompose into roughly 600
 * distinct word tokens, and the head of that distribution is short (`is` ×47,
 * `heeft` ×43, `partner` ×37). Translating the tokens and composing the labels
 * is therefore far less work than translating the names one by one, and it is
 * the shape this report produces: one line per token, most frequent first,
 * with up to three names it appears in so the translator can see the context a
 * bare word does not give.
 *
 * Read-only. It writes nothing and decides nothing; it is the input for the
 * glossary, and `check-i18n.mjs` is what later holds that glossary to account.
 *
 * Usage:
 *   node scripts/i18n-report.mjs            # tokens, most frequent first
 *   node scripts/i18n-report.mjs --names    # the whole names, alphabetically
 *   node scripts/i18n-report.mjs --missing  # only what the glossary lacks
 */
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const lawsDir = resolve(here, '..', '..', 'corpus', 'demo', 'regulation', 'nl');
const glossaryFile = resolve(here, '..', '..', 'corpus', 'demo', 'i18n', 'glossary.en.yaml');

function walk(dir, out = []) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, out);
    else if (full.endsWith('.yaml')) out.push(full);
  }
  return out;
}

// Line-oriented, like the other corpus guards: this runs where no frontend
// dependency is installed, and the shape it needs is one flat line.
const names = new Map();
for (const file of walk(lawsDir)) {
  for (const line of readFileSync(file, 'utf8').split('\n')) {
    const m = /^\s*-\s+name:\s+([a-z][a-z0-9_]*)\s*$/.exec(line);
    if (m) names.set(m[1], (names.get(m[1]) ?? 0) + 1);
  }
}

/** The `words:` block of the glossary, if it exists yet. */
function readGlossaryWords() {
  if (!existsSync(glossaryFile)) return new Set();
  const words = new Set();
  let inWords = false;
  for (const raw of readFileSync(glossaryFile, 'utf8').split('\n')) {
    const line = raw.replace(/\s+$/, '');
    if (!line || line.startsWith('#')) continue;
    if (/^words:\s*$/.test(line)) { inWords = true; continue; }
    if (/^\S/.test(line)) { inWords = false; continue; }
    if (!inWords) continue;
    const m = /^ {2}([a-z][a-z0-9_]*):\s*\S/.exec(line);
    if (m) words.add(m[1]);
  }
  return words;
}

const known = readGlossaryWords();
const onlyMissing = process.argv.includes('--missing');

if (process.argv.includes('--names')) {
  for (const name of [...names.keys()].sort()) console.log(name);
  console.error(`\n${names.size} verschillende veldnamen.`);
} else {
  // Per token: how often, and a few names to read it in. A word alone is
  // ambiguous ("vermogen" is capacity or assets); the examples are what makes
  // the choice decidable without opening the corpus.
  const tokens = new Map();
  for (const name of names.keys()) {
    for (const token of name.split('_')) {
      if (!tokens.has(token)) tokens.set(token, { count: 0, examples: [] });
      const t = tokens.get(token);
      t.count += 1;
      if (t.examples.length < 3) t.examples.push(name);
    }
  }

  const rows = [...tokens.entries()]
    .filter(([token]) => !onlyMissing || !known.has(token))
    .sort((a, b) => b[1].count - a[1].count || a[0].localeCompare(b[0]));

  for (const [token, { count, examples }] of rows) {
    console.log(`${String(count).padStart(4)}  ${token.padEnd(28)} ${examples.join(', ')}`);
  }
  console.error(
    `\n${rows.length} van ${tokens.size} tokens${onlyMissing ? ' nog zonder vertaling' : ''}, uit ${names.size} veldnamen.` +
      (known.size ? ` De woordenlijst kent er ${known.size}.` : ' Er is nog geen woordenlijst.'),
  );
}
