/**
 * Scenario runner: parses .feature files and executes them against the
 * in-browser WASM engine using the step definitions.
 *
 * Individual scenarios can be re-run with overridden Given-values
 * (`overrides`: parameternaam → nieuwe waarde), zodat de webinterface een
 * speeltuin wordt: pas de gegevens aan en zie wat de wet doet.
 */

import { parseFeature } from './parser.js';
import { stepDefinitions } from './steps.js';

class ExecutionContext {
  constructor() {
    this.calculationDate = null;
    this.parameters = {};
    this.result = null;
    this.error = null;
    this.executed = false;
  }
}

function findStep(text) {
  for (const def of stepDefinitions) {
    const match = def.pattern.exec(text);
    if (match) return { def, match };
  }
  return null;
}

/** Apply value-overrides to a step's data table (matched on the key column). */
function applyOverrides(step, overrides) {
  if (!overrides || !step.dataTable) return step;
  return {
    ...step,
    dataTable: step.dataTable.map((row) =>
      overrides[row[0]] !== undefined ? [row[0], String(overrides[row[0]])] : row,
    ),
  };
}

/**
 * Run a list of (background + scenario) steps.
 *
 * @returns {{ passed: boolean, steps: Array, outputs: object|null }}
 *   `outputs` are the engine outputs of the last execution, so the caller
 *   can show what the law computed regardless of the Then-assertions.
 */
export async function runSteps(steps, engine, overrides = null) {
  const ctx = new ExecutionContext();
  const stepResults = [];
  let failed = false;

  for (const original of steps) {
    const step = applyOverrides(original, overrides);
    if (failed) {
      stepResults.push({ ...step, status: 'overgeslagen', error: null });
      continue;
    }
    const found = findStep(step.text);
    if (!found) {
      stepResults.push({
        ...step,
        status: 'mislukt',
        error: `Geen stapdefinitie voor: ${step.text}`,
      });
      failed = true;
      continue;
    }
    try {
      await found.def.execute(ctx, engine, found.match, step);
      stepResults.push({ ...step, status: 'geslaagd', error: null });
    } catch (e) {
      stepResults.push({ ...step, status: 'mislukt', error: String(e?.message ?? e) });
      failed = true;
    }
  }

  return {
    passed: !failed,
    steps: stepResults,
    outputs: ctx.result?.outputs ?? null,
  };
}

/**
 * Run one feature file.
 *
 * @returns {{ feature: string, scenarios: Array<{name, passed, steps, outputs}> }}
 */
export async function runFeature(featureText, engine) {
  const parsed = parseFeature(featureText);
  const scenarios = [];

  for (const scenario of parsed.scenarios) {
    const allSteps = [...(parsed.background ?? []), ...scenario.steps];
    const uitkomst = await runSteps(allSteps, engine);
    scenarios.push({ name: scenario.name, ...uitkomst });
  }

  return { feature: parsed.feature, scenarios };
}
