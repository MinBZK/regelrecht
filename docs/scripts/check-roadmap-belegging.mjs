// Report werkpakketten whose belegging has been sitting on 'opgepakt' for a
// long time.
//
// A frontmatter field cannot expire. Someone writes `stand: opgepakt`, the
// work stalls, and the card stays coloured for a year while the roadmap reads
// as if it is covered. Nothing in the build can tell that apart from work that
// is genuinely under way, because the difference is not in the file.
//
// This REPORTS and always exits 0, like check-roadmap-rfcs.mjs beside it.
// Blocking would hold up unrelated work until someone chased down whether a
// colleague is still on a werkpakket — editorial lag, which per the reasoning
// already written into require-werkpakket.sh gets worked around rather than
// satisfied. The build's own checks cover what makes a page wrong (a claim
// without a date, a date in the future, 'vrij' on finished work); this is the
// softer question of whether a claim is still true.
//
// What deliberately does NOT happen: a field that flips itself back to 'vrij'
// after N months. Then the frontmatter no longer records what anyone said, and
// someone who IS working on it loses their claim without noticing.
//
// Reads source files, not the build, so it runs without `astro build`.
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const WP_DIR = fileURLToPath(
  new URL('../src/content/roadmap/werkpakketten', import.meta.url),
);

// Six months. A conversation-starter, not a rule: long enough that a werkpakket
// of any real size is expected to still be running, short enough that a claim
// nobody remembers making surfaces within a release cycle.
const MAANDEN_DREMPEL = 6;

const field = (text, name) =>
  (text.match(new RegExp(`^${name}:\\s*(.*)$`, 'm')) ?? [])[1]?.trim() ?? '';

// The belegging block, read without a YAML parser: this script deliberately has
// no dependencies, and the shape is two indented keys under one top-level one.
const belegging = (text) => {
  const blok = text.match(/^belegging:\n((?:[ \t]+.*\n)*)/m);
  if (!blok) return { stand: '' };
  const lees = (naam) =>
    (blok[1].match(new RegExp(`^\\s+${naam}:\\s*'?([^'\n]*)'?\\s*$`, 'm')) ??
      [])[1]?.trim() ?? '';
  return { stand: lees('stand'), sinds: lees('sinds') };
};

const maandenGeleden = (iso, nu) => {
  const toen = new Date(`${iso}T00:00:00Z`);
  return (
    (nu.getUTCFullYear() - toen.getUTCFullYear()) * 12 +
    (nu.getUTCMonth() - toen.getUTCMonth()) -
    (nu.getUTCDate() < toen.getUTCDate() ? 1 : 0)
  );
};

const nu = new Date();
const verjaard = readdirSync(WP_DIR)
  .filter((f) => f.endsWith('.md'))
  .map((f) => {
    const text = readFileSync(`${WP_DIR}/${f}`, 'utf8');
    return {
      id: f.replace(/\.md$/, ''),
      titel: field(text, 'titel').replace(/^'(.*)'$/, '$1'),
      ...belegging(text),
    };
  })
  .filter((wp) => wp.stand === 'opgepakt' && wp.sinds)
  .map((wp) => ({ ...wp, maanden: maandenGeleden(wp.sinds, nu) }))
  .filter((wp) => wp.maanden >= MAANDEN_DREMPEL)
  .sort((a, b) => b.maanden - a.maanden);

if (verjaard.length === 0) {
  console.log(
    `Belegging: geen werkpakket staat langer dan ${MAANDEN_DREMPEL} maanden op 'opgepakt'.`,
  );
} else {
  console.log(
    `Belegging: ${verjaard.length} werkpakket(ten) staan langer dan ` +
      `${MAANDEN_DREMPEL} maanden op 'opgepakt'. Dat is geen fout — het is een ` +
      'vraag of de claim nog klopt:',
  );
  for (const wp of verjaard) {
    console.log(`  ${wp.id.padEnd(50)} sinds ${wp.maanden} maanden`);
  }
}
