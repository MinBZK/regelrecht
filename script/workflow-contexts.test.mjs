// Een `env:` op jobniveau mag alleen contexten gebruiken die er op dat moment
// al zijn: github, needs, strategy, matrix, vars, secrets en inputs. Noemt hij
// `runner`, `steps`, `job`, `env` of `hashFiles()`, dan keurt Actions het hele
// workflowbestand af. Er draait dan niets, en dat is niet te zien aan de
// uitslag van één baan: de run verschijnt met nul jobs, heet naar zijn
// bestandspad, en negeert zijn eigen branch-filters.
//
// Dat is op 8 augustus 2026 gebeurd met security-advisories.yml, die daardoor
// vanaf zijn eerste run nooit heeft gedraaid terwijl hij de advisories in de
// afhankelijkheden hoorde te bewaken.
//
// Regelgewijs geparsed, net als ci-gate.test.mjs: de Pre-commit-baan heeft Node
// maar geen node_modules.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('../.github/workflows/', import.meta.url));
const EXPRESSIE = /\$\{\{([\s\S]*?)\}\}/g;
const VERBODEN = /\b(runner|steps|job|env)\.|\bhashFiles\s*\(/;

/**
 * Kijkt in de hele expressie en niet alleen naar het eerste woord erachter:
 * `${{ format('{0}/x', runner.temp) }}` wordt door Actions net zo hard
 * afgekeurd als `${{ runner.temp }}`.
 */
function verbodenContext(regel) {
  for (const [, expressie] of regel.matchAll(EXPRESSIE)) {
    if (VERBODEN.test(expressie)) return true;
  }
  return false;
}

/**
 * De regels van elk `env:`-blok dat direct onder een baan hangt, dus op inspring
 * vier binnen `jobs:`. Een `env:` onder een stap zit dieper en telt niet mee.
 */
function jobEnvRegels(bron) {
  const regels = bron.split('\n');
  const gevonden = [];
  let inJobs = false;
  let inEnv = false;

  for (const [i, regel] of regels.entries()) {
    if (/^jobs:\s*$/.test(regel)) { inJobs = true; continue; }
    if (!inJobs) continue;
    if (/^\S/.test(regel)) break;

    if (inEnv) {
      if (/^ {6}\S/.test(regel)) { gevonden.push([i + 1, regel]); continue; }
      inEnv = false;
    }
    if (/^ {4}env:\s*$/.test(regel)) inEnv = true;
  }
  return gevonden;
}

const bestanden = readdirSync(DIR).filter((n) => n.endsWith('.yml') || n.endsWith('.yaml'));

test('er zijn workflows om te controleren', () => {
  assert.ok(bestanden.length > 0, `geen workflowbestanden gevonden in ${DIR}`);
});

// Zonder deze twee bewijst de suite alleen dat de huidige bestanden schoon zijn,
// en zou een detectie die stilvalt er precies zo uitzien.

const FOUT = `name: X
jobs:
  een:
    runs-on: ubuntu-latest
    env:
      UIT: \${{ runner.temp }}/iets
    steps:
      - run: 'true'
`;

const GOED = `name: X
jobs:
  een:
    runs-on: ubuntu-latest
    env:
      REPO: \${{ github.repository }}
    steps:
      - name: stap met een eigen env
        env:
          UIT: \${{ runner.temp }}/iets
        run: 'true'
`;

test('een verboden context op jobniveau wordt gevonden', () => {
  const geraakt = jobEnvRegels(FOUT).filter(([, regel]) => verbodenContext(regel));
  assert.equal(geraakt.length, 1);
  assert.equal(geraakt[0][0], 6);
});

test('een verboden context verpakt in een functie wordt ook gevonden', () => {
  const verpakt = FOUT.replace(
    '${{ runner.temp }}/iets',
    "${{ format('{0}/iets', runner.temp) }}",
  );
  assert.equal(jobEnvRegels(verpakt).filter(([, regel]) => verbodenContext(regel)).length, 1);
});

test('dezelfde context in een env onder een stap blijft ongemoeid', () => {
  assert.equal(jobEnvRegels(GOED).filter(([, regel]) => verbodenContext(regel)).length, 0);
});

for (const naam of bestanden) {
  test(`${naam}: env op jobniveau gebruikt geen contexten die er nog niet zijn`, () => {
    const bron = readFileSync(DIR + naam, 'utf8');
    for (const [nummer, regel] of jobEnvRegels(bron)) {
      assert.ok(
        !verbodenContext(regel),
        `${naam}:${nummer} gebruikt een context die op jobniveau niet bestaat, ` +
          `waardoor Actions het hele bestand afkeurt: ${regel.trim()}`,
      );
    }
  });
}
