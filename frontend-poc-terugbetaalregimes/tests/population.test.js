import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import * as yaml from 'js-yaml';
import { generatePopulation } from '../src/sim/population.js';
import { aggregate } from '../src/sim/metrics.js';
import { simulate } from '../src/sim/simulate.js';
import { createEngineWithLaws } from './helpers/nodeEngine.js';
import { dataDir } from './helpers/casusPaden.js';

const distFile = resolve(dataDir, 'distributions.yaml');
const distributions = yaml.load(readFileSync(distFile, 'utf-8'));

const OPTIONS = { startJaar: 2026 };

/**
 * Draai een populatie door de engine en aggregeer, zoals simWorker.js doet
 * (inclusief clearDataSources() tussen records om de engine-cache vlak te
 * houden — anders loopt de doorlooptijd per record op).
 */
function simulatePopulation(engine, records) {
  const results = [];
  for (const record of records) {
    const { totals } = simulate(engine, record, record.keuzes, OPTIONS);
    results.push({ record, totals });
    engine.clearDataSources();
  }
  return aggregate(results);
}

describe('populatie', () => {
  it('is deterministisch: dezelfde seed geeft een identieke aggregatie', async () => {
    const engine = await createEngineWithLaws();
    const a = simulatePopulation(engine, generatePopulation(distributions, 200, 12345));
    const b = simulatePopulation(engine, generatePopulation(distributions, 200, 12345));
    expect(a).toEqual(b);
  });

  it('een andere seed geeft een andere populatie', () => {
    const a = generatePopulation(distributions, 200, 1);
    const b = generatePopulation(distributions, 200, 2);
    expect(a).not.toEqual(b);
  });

  it('SF15-oud heeft meer betalingsproblemen dan SF15-nieuw (draagkracht-priors)', async () => {
    const engine = await createEngineWithLaws();
    const agg = simulatePopulation(engine, generatePopulation(distributions, 200, 777));
    const oud = agg.regimes.SF15_OUD;
    const nieuw = agg.regimes.SF15_NIEUW;
    expect(oud).toBeDefined();
    expect(nieuw).toBeDefined();
    // SF15-oud vraagt draagkracht veel minder vaak aan (prior 0,37 vs 0,90) en
    // betaalt daardoor vaker het volle wettelijke maandbedrag → meer problemen.
    expect(oud.pctBetalingsprobleem).toBeGreaterThan(nieuw.pctBetalingsprobleem);
  });
});
