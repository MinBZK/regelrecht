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
const VERBODEN = /\$\{\{\s*(runner|steps|job|env)\.|hashFiles\s*\(/;

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

for (const naam of bestanden) {
  test(`${naam}: env op jobniveau gebruikt geen contexten die er nog niet zijn`, () => {
    const bron = readFileSync(DIR + naam, 'utf8');
    for (const [nummer, regel] of jobEnvRegels(bron)) {
      assert.ok(
        !VERBODEN.test(regel),
        `${naam}:${nummer} gebruikt een context die op jobniveau niet bestaat, ` +
          `waardoor Actions het hele bestand afkeurt: ${regel.trim()}`,
      );
    }
  });
}
