// Draait alle Gherkin-scenario's uit de corpus-scenariomappen tegen de
// WASM-engine, via de gevendorde JS-runner (zelfde canonieke grammatica als
// de Rust BDD-runner van regelrecht).
import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, statSync, existsSync } from 'fs';
import { resolve, dirname, relative } from 'path';
import { fileURLToPath } from 'url';
import { parseFeature } from '../src/gherkin/parser.js';
import { createStepDefinitions, SUPPORTED_TIERS } from '../src/gherkin/steps.js';
import { ExecutionContext } from '../src/gherkin/context.js';
import { corpusDir as casusCorpusDir } from './helpers/casusPaden.js';
import { createEngineWithLaws, lawsDir } from './helpers/nodeEngine.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const corpusDir = casusCorpusDir;

function findFeatureFiles(dir) {
  if (!existsSync(dir)) return [];
  const results = [];
  for (const entry of readdirSync(dir)) {
    const full = resolve(dir, entry);
    if (statSync(full).isDirectory()) results.push(...findFeatureFiles(full));
    else if (entry.endsWith('.feature')) results.push(full);
  }
  return results;
}

const featureFiles = findFeatureFiles(corpusDir);

// Wetten staan al in de engine; een load_law-stap hoeft niets bij te laden.
const stepDefs = createStepDefinitions({ loadDependency: async () => {} });

async function runScenario(engine, background, scenario) {
  const ctx = new ExecutionContext();
  const steps = [...(background ?? []), ...scenario.steps];
  for (const step of steps) {
    const def = stepDefs.find((d) => d.pattern.test(step.text));
    if (!def) throw new Error(`Geen stapdefinitie voor: "${step.text}"`);
    if (!SUPPORTED_TIERS.includes(def.tier)) {
      throw new Error(`Stap "${step.text}" vergt tier ${def.tier}`);
    }
    const match = step.text.match(def.pattern);
    await def.execute(ctx, engine, match, step);
  }
}

describe.each(featureFiles.map((f) => [relative(corpusDir, f), f]))(
  '%s',
  (label, file) => {
    const { background, scenarios } = parseFeature(readFileSync(file, 'utf-8'));
    const runnable = scenarios.filter((s) => !s.tags.includes('@wip'));

    it.each(runnable.map((s) => [s.name, s]))('%s', async (name, scenario) => {
      // Verse engine per scenario: datasources en parameters lekken anders
      // tussen scenario's door.
      const engine = await createEngineWithLaws();
      await runScenario(engine, background, scenario);
      expect(true).toBe(true);
    });

    if (runnable.length === 0) {
      it.skip('geen (niet-@wip) scenario\'s', () => {});
    }
  },
);

if (featureFiles.length === 0) {
  describe('scenario\'s', () => {
    it.skip('geen feature-bestanden gevonden', () => {});
  });
}
