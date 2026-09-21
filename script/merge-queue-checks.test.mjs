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
// draait. Node's ingebouwde runner, geen dependency: de Pre-commit-baan heeft
// Node maar geen node_modules.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const WORKFLOW_DIR = fileURLToPath(new URL('../.github/workflows/', import.meta.url));

// De verplichte checks op main, zoals de branch protection ze kent:
//   gh api repos/MinBZK/regelrecht/branches/main/protection \
//     --jq '.required_status_checks.contexts[]'
// Wijzigt die lijst, dan hier ook. Staat er hier een naam te veel, dan faalt
// deze test; staat er een te weinig, dan bewaakt niemand die check meer.
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
const SLAAT_OVER_IN_DE_RIJ = new Set(['Protect schema versions']);

/** Elke workflow in .github/workflows, als ruwe tekst. */
function workflows() {
  return readdirSync(WORKFLOW_DIR)
    .filter((f) => f.endsWith('.yml') || f.endsWith('.yaml'))
    .map((f) => ({ file: f, source: readFileSync(WORKFLOW_DIR + f, 'utf8') }));
}

/** Draait deze workflow op het merge_group-event? */
function draaitInDeRij(source) {
  // `on:` staat op kolom 0, de events erhuizen op twee spaties. Genoeg om
  // `merge_group:` als trigger te onderscheiden van het woord in een comment
  // of in een `${{ github.event.merge_group.base_sha }}`.
  return /^ {2}merge_group:\s*$/m.test(source);
}

/** De namen die deze workflow als check publiceert: elke `name:` van een baan. */
function checkNamen(source) {
  const namen = new Set();
  const lines = source.split('\n');
  let inJobs = false;
  for (const line of lines) {
    if (/^jobs:\s*$/.test(line)) { inJobs = true; continue; }
    if (!inJobs) continue;
    if (/^\S/.test(line)) break;
    // De `name:` van een baan staat op vier spaties; een `name:` van een stap
    // staat dieper en begint met een streepje.
    const match = /^ {4}name:\s*(.+?)\s*$/.exec(line);
    if (match) namen.add(match[1].replace(/^['"]|['"]$/g, ''));
  }
  return namen;
}

test('elke verplichte check bestaat als baan in een workflow', () => {
  const alle = workflows();
  for (const check of REQUIRED_CHECKS) {
    const dragers = alle.filter((w) => checkNamen(w.source).has(check));
    assert.ok(
      dragers.length > 0,
      `geen enkele workflow publiceert de verplichte check "${check}" — ` +
        'hernoemd of verwijderd? Dan blijft hij op "Expected" staan en ' +
        'blokkeert hij elke merge.',
    );
  }
});

test('elke verplichte check rapporteert ook in de merge queue', () => {
  const alle = workflows();
  for (const check of REQUIRED_CHECKS) {
    if (SLAAT_OVER_IN_DE_RIJ.has(check)) continue;
    const dragers = alle.filter((w) => checkNamen(w.source).has(check));
    const inDeRij = dragers.filter((w) => draaitInDeRij(w.source));
    assert.ok(
      inDeRij.length > 0,
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
  const alle = workflows();
  for (const check of SLAAT_OVER_IN_DE_RIJ) {
    const dragers = alle.filter((w) => checkNamen(w.source).has(check));
    assert.ok(dragers.length > 0, `"${check}" bestaat niet meer als baan`);
    for (const drager of dragers) {
      assert.ok(
        draaitInDeRij(drager.source),
        `${drager.file} publiceert "${check}" maar draait niet op merge_group; ` +
          'de baan mag daar overslaan, de workflow niet wegvallen.',
      );
    }
  }
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
      checkNamen(source).has(check),
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
