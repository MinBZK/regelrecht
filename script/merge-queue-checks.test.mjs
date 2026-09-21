// Main draait op een merge queue: GitHub bouwt een eigen branch met main plus
// de wachtende pull requests erin en mergt pas als de verplichte checks dáár
// groen zijn. Diezelfde lijst verplichte checks geldt in de rij als op de pull
// request — een aparte lijst bestaat niet.
//
// Een check die in de rij nooit rapporteert blijft op "Expected" staan en laat
// elke entry hangen tot de status-check-timeout hem eruit gooit. Dat gebeurt
// stil: de melding staat op de queue-branch, niet als rode check op de pull
// request. Eén hernoemde baan is genoeg, en dan is de rij kapot zonder dat
// iets roods dat zegt.
//
// Deze test bindt daarom elke verplichte check aan een baan die in de rij
// draait. Node's ingebouwde runner en een eigen minilezer voor de twee stukken
// YAML die ertoe doen: de Pre-commit-baan heeft Node maar geen node_modules,
// en PyYAML staat daar niet gegarandeerd.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const WORKFLOW_DIR = fileURLToPath(new URL('../.github/workflows/', import.meta.url));

// De verplichte checks op main, zoals de branch protection ze kent:
//   gh api repos/MinBZK/regelrecht/branches/main/protection \
//     --jq '.required_status_checks.contexts[]'
//
// Deze lijst is met de hand bijgehouden en de twee richtingen zijn níét
// symmetrisch. Staat er een naam te veel, dan faalt deze test luid en herstelt
// zich vanzelf. Ontbreekt er een, dan blijft alles groen en bewaakt niemand die
// check meer — stil, en zonder spoor in een review, want de branch protection
// wijzigt met een knop en niet met een commit. Zet dus bij elke wijziging daar
// deze lijst mee om.
const REQUIRED_CHECKS = [
  'Pre-commit',
  'WASM Build',
  'Protect schema versions',
  'Security Audit',
  'Test',
  'Validate PR title',
  'Claude review completed',
];

// `Protect schema versions` hoort bewust niet in de rij te draaien: de baan
// vergelijkt tegen `origin/main`, en in de rij is HEAD de hele groep in plaats
// van één pull request. Zou hij daar draaien, dan sleept een schending van de
// ene pull request de andere mee. Overslaan mag, want een baan die op een
// `if:` overslaat meldt `skipped`, en dat telt bij GitHub als geslaagd. Wat
// niet mag is dat de hele wórkflow wegvalt — dan rapporteert er niets.
const SLAAT_OVER_IN_DE_RIJ = new Map([
  ['Protect schema versions', 'ci.yml'],
]);

/**
 * De top-level blokken van een workflow, op inspringing in plaats van op losse
 * regexes: `{ on: [regels], jobs: [regels], ... }`. Comments en lege regels
 * gaan eruit, zodat `jobs:  # alle banen` net zo goed leest als `jobs:`.
 *
 * Genoeg YAML voor wat hier nodig is, en bewust niet meer. Een echte parser
 * zou beter zijn, maar die is er niet zonder dependency; wat deze lezer niet
 * aankan faalt daarom zichtbaar in plaats van stil (zie `sleutelsVan`).
 */
function topBlokken(source) {
  const blokken = new Map();
  let huidig = null;
  for (const raw of source.split('\n')) {
    const line = raw.replace(/\s+$/, '');
    if (line === '' || /^\s*#/.test(line)) continue;
    const top = /^([A-Za-z_][A-Za-z0-9_-]*):(.*)$/.exec(line);
    if (top) {
      huidig = top[1];
      blokken.set(huidig, { inline: top[2].trim(), lines: [] });
      continue;
    }
    if (huidig) blokken.get(huidig).lines.push(line);
  }
  return blokken;
}

/** Het commentaar van een regel eraf, buiten quotes om. */
function zonderComment(text) {
  let out = '';
  let quote = null;
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (quote) {
      out += c;
      if (c === quote) quote = null;
      continue;
    }
    if (c === '"' || c === "'") { quote = c; out += c; continue; }
    // Een `#` telt alleen als comment met witruimte ervoor (of aan het begin),
    // zodat `name: build#2` heel blijft.
    if (c === '#' && (i === 0 || /\s/.test(text[i - 1]))) break;
    out += c;
  }
  return out.replace(/\s+$/, '');
}

/** Een scalar uitpakken: quotes eraf, comment eraf. */
function scalar(text) {
  const kaal = zonderComment(text).trim();
  const m = /^(['"])(.*)\1$/.exec(kaal);
  return m ? m[2] : kaal;
}

/**
 * De sleutels van een blok, of van een inline-lijst. Dekt beide vormen waarin
 * een `on:` geschreven wordt:
 *
 *     on:                       on: [push, merge_group]
 *       push:
 *       merge_group:
 */
function sleutelsVan(blok) {
  if (!blok) return new Set();
  const inline = zonderComment(blok.inline).trim();
  if (inline.startsWith('[')) {
    return new Set(
      inline.replace(/^\[|\]$/g, '').split(',').map((s) => scalar(s)).filter(Boolean),
    );
  }
  // Blokvorm: de sleutels staan één niveau in, en dat niveau is dat van de
  // eerste regel. Diepere regels zijn hun inhoud.
  const eerste = blok.lines.find((l) => l.trim() !== '');
  if (!eerste) return new Set();
  const diepte = eerste.length - eerste.trimStart().length;
  const sleutels = new Set();
  for (const line of blok.lines) {
    const inspring = line.length - line.trimStart().length;
    if (inspring !== diepte) continue;
    const m = /^([A-Za-z_][A-Za-z0-9_-]*):/.exec(line.trim());
    if (m) sleutels.add(m[1]);
  }
  return sleutels;
}

/** Draait deze workflow op het merge_group-event? */
function draaitInDeRij(source) {
  // Alleen de sleutels van het `on:`-blok tellen. Een baan, een `env:`- of een
  // `outputs:`-sleutel die toevallig `merge_group` heet is géén trigger, en een
  // losse regex op het hele bestand zou dat verschil niet zien.
  const blokken = topBlokken(source);
  // In YAML is een kale `on` de booleaanse waarde true, dus sommige parsers en
  // schrijvers maken er `"on"` van; beide vormen komen hier langs.
  const blok = blokken.get('on') ?? blokken.get('"on"') ?? blokken.get("'on'");
  return sleutelsVan(blok).has('merge_group');
}

/**
 * De banen van een workflow als `Map<jobKey, {naam, body}>`. De naam is de
 * `name:` die direct onder de baan hangt; ontbreekt die, dan is de check-naam
 * de sleutel zelf, precies zoals GitHub het doet.
 */
function banen(source) {
  const blok = topBlokken(source).get('jobs');
  const out = new Map();
  if (!blok) return out;
  const eerste = blok.lines.find((l) => l.trim() !== '');
  if (!eerste) return out;
  const diepte = eerste.length - eerste.trimStart().length;

  let key = null;
  for (const line of blok.lines) {
    const inspring = line.length - line.trimStart().length;
    if (inspring === diepte) {
      const m = /^([A-Za-z_][A-Za-z0-9_.-]*):/.exec(line.trim());
      key = m ? m[1] : null;
      if (key) out.set(key, { naam: key, body: [], naamGezet: false });
      continue;
    }
    if (!key) continue;
    const baan = out.get(key);
    baan.body.push(line);
    // Alleen de `name:` één niveau onder de baan is de checknaam. Een `name:`
    // van een stap staat dieper en hoort bij die stap.
    if (inspring === diepte * 2 && !baan.naamGezet) {
      const m = /^name:\s*(.+)$/.exec(line.trim());
      if (m) { baan.naam = scalar(m[1]); baan.naamGezet = true; }
    }
  }
  return out;
}

/** Elke workflow in .github/workflows, als ruwe tekst. */
function workflows() {
  return readdirSync(WORKFLOW_DIR)
    .filter((f) => f.endsWith('.yml') || f.endsWith('.yaml'))
    .map((f) => ({ file: f, source: readFileSync(WORKFLOW_DIR + f, 'utf8') }));
}

/** De workflows die een baan met deze checknaam publiceren. */
function dragersVan(check) {
  return workflows().filter((w) =>
    [...banen(w.source).values()].some((b) => b.naam === check),
  );
}

test('de minilezer leest de workflows die hier beoordeeld worden', () => {
  // Zonder deze assertie zou een lezer die niets meer vindt élke test hieronder
  // groen maken: geen banen, geen tegenspraak. Faalt hij, dan is de YAML een
  // vorm die deze lezer niet kent, en dat hoort zichtbaar te zijn.
  for (const w of workflows()) {
    const gevonden = banen(w.source);
    assert.ok(
      gevonden.size > 0,
      `${w.file}: geen enkele baan gevonden — schrijfwijze die deze lezer ` +
        'niet kent? Dan bewaakt hij dit bestand niet.',
    );
  }
});

test('elke verplichte check bestaat als baan in een workflow', () => {
  for (const check of REQUIRED_CHECKS) {
    assert.ok(
      dragersVan(check).length > 0,
      `geen enkele workflow publiceert de verplichte check "${check}" — ` +
        'hernoemd of verwijderd? Dan blijft hij op "Expected" staan en ' +
        'blokkeert hij elke merge.',
    );
  }
});

test('elke verplichte check rapporteert ook in de merge queue', () => {
  for (const check of REQUIRED_CHECKS) {
    if (SLAAT_OVER_IN_DE_RIJ.has(check)) continue;
    const dragers = dragersVan(check);
    assert.ok(
      dragers.some((w) => draaitInDeRij(w.source)),
      `"${check}" wordt gepubliceerd door ${dragers.map((w) => w.file).join(', ')}, ` +
        'maar geen daarvan draait op merge_group. In de rij rapporteert die ' +
        'check dan nooit en loopt elke entry vast op de status-check-timeout.',
    );
  }
});

test('een check die in de rij overslaat, doet dat vanuit een draaiende workflow', () => {
  // `skipped` telt als geslaagd, maar alleen als de workflow zélf draait. Valt
  // de hele workflow weg, dan rapporteert er niets en staat de check op
  // "Expected".
  for (const [check, bestand] of SLAAT_OVER_IN_DE_RIJ) {
    const dragers = dragersVan(check);
    assert.ok(dragers.length > 0, `"${check}" bestaat niet meer als baan`);
    for (const drager of dragers) {
      assert.ok(
        draaitInDeRij(drager.source),
        `${drager.file} publiceert "${check}" maar draait niet op merge_group; ` +
          'de baan mag daar overslaan, de workflow niet wegvallen.',
      );
    }
    assert.ok(
      dragers.some((w) => w.file === bestand),
      `"${check}" hoort uit ${bestand} te komen`,
    );
  }
});

test('de baan die in de rij overslaat, houdt de voorwaarde die dat doet', () => {
  // De reden dat `Protect schema versions` in de rij overslaat is inhoudelijk:
  // zijn vergelijking leest daar de hele groep in plaats van één pull request.
  // Zonder deze assertie bewaakt niets die keuze, en dan staat er straks een
  // `if: always()` waar de comment nog uitlegt waarom dat juist niet moet.
  for (const [check] of SLAAT_OVER_IN_DE_RIJ) {
    for (const drager of dragersVan(check)) {
      const baan = [...banen(drager.source).values()].find((b) => b.naam === check);
      const voorwaarde = baan.body
        .map((l) => l.trim())
        .find((l) => l.startsWith('if:'));
      assert.ok(
        voorwaarde && /pull_request/.test(voorwaarde) && !/merge_group/.test(voorwaarde),
        `"${check}" in ${drager.file} hoort een \`if:\` te houden die hem in de ` +
          `rij laat overslaan; nu: ${voorwaarde ?? '(geen if:)'}`,
      );
    }
  }
});

test('ci-gate.test.mjs kent dezelfde verplichte checks als deze test', () => {
  // Daar staat dezelfde waarheid in een andere vorm: `EIGEN_REQUIRED_CHECK`
  // somt de baan-sleutels op die een verplichte check zijn en daarom niet aan
  // de Test-poort hoeven te hangen, met de checknaam ernaast als comment. Twee
  // handgeschreven lijsten over hetzelfde lopen uiteen, en dan hangt er een
  // baan aan niets terwijl beide bestanden beweren dat het klopt. Deze
  // assertie bindt ze: elke baan-sleutel daar hoort hier een verplichte check
  // te zijn, en omgekeerd hoort elke verplichte check uit ci.yml daar te staan.
  const source = readFileSync(
    fileURLToPath(new URL('./ci-gate.test.mjs', import.meta.url)),
    'utf8',
  );
  const blok = /const EIGEN_REQUIRED_CHECK = new Set\(\[([\s\S]*?)\]\)/.exec(source);
  assert.ok(blok, 'EIGEN_REQUIRED_CHECK niet gevonden in ci-gate.test.mjs');
  // Per regel, en wat uitgecommentarieerd staat telt niet mee: een weggehaalde
  // sleutel die als comment blijft staan zou anders nog steeds meetellen, en
  // dan bindt deze assertie niets.
  const sleutels = new Set(
    blok[1]
      .split('\n')
      .map((line) => line.replace(/\/\/.*$/, ''))
      .flatMap((line) => [...line.matchAll(/'([^']+)'/g)].map((m) => m[1])),
  );

  const ciBanen = banen(readFileSync(WORKFLOW_DIR + 'ci.yml', 'utf8'));
  const hier = new Set(
    [...ciBanen].filter(([, b]) => REQUIRED_CHECKS.includes(b.naam)).map(([key]) => key),
  );

  assert.deepEqual(
    [...sleutels].sort(),
    [...hier].sort(),
    'EIGEN_REQUIRED_CHECK in ci-gate.test.mjs en REQUIRED_CHECKS hier zijn ' +
      'het oneens over welke banen in ci.yml een verplichte check zijn.',
  );
});

test('de twee poorten uit de pull request worden in de rij vervangen', () => {
  // `Validate PR title` en `Claude review completed` lezen allebei iets dat
  // alleen een pull request heeft. In de rij komen ze uit een eigen workflow;
  // draaien ze daar uit hun oorspronkelijke workflow, dan leest die een lege
  // PR-context en blokkeert hij alles.
  for (const [check, origineel] of [
    ['Validate PR title', 'pr-title.yml'],
    ['Claude review completed', 'claude-code-review.yml'],
  ]) {
    const source = readFileSync(WORKFLOW_DIR + origineel, 'utf8');
    assert.ok(
      [...banen(source).values()].some((b) => b.naam === check),
      `${origineel} publiceert "${check}" niet meer`,
    );
    assert.ok(
      !draaitInDeRij(source),
      `${origineel} draait op merge_group, maar leest PR-eigen velden die ` +
        'daar leeg zijn. Die poort hoort in de rij uit merge-queue-gates.yml ' +
        'te komen.',
    );
  }
});
