// @vitest-environment node
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { describe, expect, it } from 'vitest';
import { indexCorpus, readAllLaws } from '../../server/presenterApi.js';
import { featureTags, nullForUndefined, pickScenario, runScenario } from './runScenario.js';

/**
 * A stand-in engine: it knows which laws are loaded and returns fixed outputs
 * for one law, so the runner's own logic can be tested without the WASM.
 */
function fakeEngine(outputs) {
  const loaded = new Set();
  const calls = [];
  return {
    loaded,
    calls,
    hasLaw: (id) => loaded.has(id),
    clearDataSources: () => calls.push('clear'),
    registerDataSource: (name) => calls.push(`data:${name}`),
    registerDataSourceForLaw: () => {},
    executeMultipleWithTrace(law, names, params, date) {
      calls.push(`run:${law}:${names.join(',')}:${date}`);
      if (!loaded.has(law)) throw { error: `LawNotFound: ${law}` };
      return { outputs, trace: { root: { node_type: 'Article', name: '2', children: [] } }, trace_text: '…' };
    },
    execute() {
      throw new Error('the runner should evaluate with a trace');
    },
  };
}

const FEATURE = `Feature: Voorbeeld

  Background:
    Given the calculation date is "2025-01-01"
    Given law "afhankelijke_wet" is loaded

  Scenario: Eerste geval
    Given the following "box1" data with key "bsn":
      | bsn | loon |
      | 1   | 100  |
    Given parameter "bsn" is "1"
    When I evaluate "recht" of "hoofdwet"
    Then the execution succeeds
    Then output "recht" is true
    And output "bedrag" equals 500

  @wip
  Scenario: Tweede geval
    Given parameter "bsn" is "2"
    When I evaluate "recht" of "hoofdwet"
    Then output "recht" is false
`;

describe('pickScenario', () => {
  const parsed = { scenarios: [{ name: 'A' }, { name: 'B' }] };
  it('picks by exact title and names the choices when it cannot', () => {
    expect(pickScenario(parsed, 'B').name).toBe('B');
    expect(() => pickScenario(parsed, 'C')).toThrow(/"A", "B"/);
    expect(() => pickScenario(parsed)).toThrow(/2 scenario's/);
    expect(pickScenario({ scenarios: [{ name: 'X' }] }).name).toBe('X');
  });
});

describe('runScenario (fake engine)', () => {
  const load = (engine) => async (id) => engine.loaded.add(id);

  it('loads the background laws and the law under test, and evaluates with a trace', async () => {
    const engine = fakeEngine({ recht: true, bedrag: 500 });
    const r = await runScenario({ engine, text: FEATURE, name: 'Eerste geval', loadLaw: load(engine) });
    expect([...engine.loaded]).toEqual(['afhankelijke_wet', 'hoofdwet']);
    expect(engine.calls).toContain('data:box1');
    expect(engine.calls).toContain('run:hoofdwet:recht:2025-01-01');
    expect(r.failed).toBe(false);
    expect(r.run).toMatchObject({ law: 'hoofdwet', asked: ['recht'], outputs: { recht: true, bedrag: 500 } });
    expect(r.run.trace).toMatchObject({ node_type: 'Article' });
    // `And` after `Then` is an expectation too.
    expect(r.steps.filter((s) => s.kind === 'Then').map((s) => s.text)).toEqual([
      'the execution succeeds',
      'output "recht" is true',
      'output "bedrag" equals 500',
    ]);
    // Clean before and after, so the next run starts empty.
    expect(engine.calls[0]).toBe('clear');
    expect(engine.calls.at(-1)).toBe('clear');
  });

  it('reports a failed expectation and marks the rest as not run', async () => {
    const engine = fakeEngine({ recht: false, bedrag: 500 });
    const r = await runScenario({ engine, text: FEATURE, name: 'Eerste geval', loadLaw: load(engine) });
    expect(r.failed).toBe(true);
    const then = r.steps.filter((s) => s.kind === 'Then');
    expect(then.map((s) => s.status)).toEqual(['passed', 'failed', 'skipped']);
    expect(then[1].message).toBeTruthy();
  });

  it('keeps the tags, so the slide can say a scenario is @wip', async () => {
    const engine = fakeEngine({ recht: true });
    const r = await runScenario({ engine, text: FEATURE, name: 'Tweede geval', loadLaw: load(engine) });
    expect(r.scenario.tags).toContain('@wip');
    expect(r.failed).toBe(true);
  });

  it('reports an engine error instead of throwing', async () => {
    const engine = fakeEngine({});
    const r = await runScenario({ engine, text: FEATURE, name: 'Eerste geval', loadLaw: async () => {} });
    expect(r.error).toMatch(/LawNotFound/);
    expect(r.steps.find((s) => s.text === 'the execution succeeds').status).toBe('failed');
  });

  it('fails on a step outside the grammar', async () => {
    const engine = fakeEngine({});
    const text = 'Feature: X\n  Scenario: Y\n    Given iets wat niemand kent\n';
    const r = await runScenario({ engine, text, loadLaw: async () => {} });
    expect(r.steps[0]).toMatchObject({ status: 'failed', message: expect.stringMatching(/grammatica/) });
  });
});

// The real engine on the real zorgtoeslag scenario, when `just wasm-build` has
// run on this machine. CI's frontend job does not build the WASM, so there it
// is skipped; the fake-engine tests above cover the runner itself.
const repo = path.resolve(import.meta.dirname, '../../..');
const pkg = path.join(repo, 'frontend/public/wasm/pkg');
const haveWasm = fs.existsSync(path.join(pkg, 'regelrecht_engine_bg.wasm'));

describe.skipIf(!haveWasm)('runScenario (real engine, whole corpus)', () => {
  // One engine with the whole corpus, loaded the way the reken block loads it
  // (readAllLaws): the same set the Rust BDD runner has.
  let engine;
  let failures;
  const corpus = path.join(repo, 'corpus/regulation');
  const setup = async () => {
    if (engine) return;
    const wasm = await import(pathToFileURL(path.join(pkg, 'regelrecht_engine.js')).href);
    await wasm.default({ module_or_path: fs.readFileSync(path.join(pkg, 'regelrecht_engine_bg.wasm')) });
    engine = new wasm.WasmEngine();
    failures = [];
    for (const v of readAllLaws(indexCorpus([corpus]), null, { root: corpus }).versions) {
      try {
        engine.loadLaw(v.text);
      } catch (e) {
        failures.push(`${v.law} ${v.source}: ${e}`);
      }
    }
  };
  const noLoad = async (id) => {
    throw new Error(`law ${id} is not in the corpus`);
  };

  it('loads every law of the corpus', async () => {
    await setup();
    expect(failures).toEqual([]);
  }, 60_000);

  it('computes the zorgtoeslag of the example slide, with every expectation holding', async () => {
    await setup();
    const text = fs.readFileSync(path.join(corpus, 'nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature'), 'utf8');
    const r = await runScenario({ engine, text, name: 'Meerderjarige met actieve polis heeft recht op zorgtoeslag', loadLaw: noLoad });
    expect(r.error).toBeNull();
    expect(r.steps.filter((s) => s.status !== 'passed')).toEqual([]);
    expect(r.run.outputs.hoogte_zorgtoeslag).toBe(157731);
    expect(r.run.trace).toBeTruthy();
  }, 60_000);

  // Every scenario that CI runs (all but @wip) passes here too, or only has
  // steps of a tier the browser does not check. A scenario that is green in
  // CI and red on a slide would be the worst thing this block can do.
  it('agrees with CI on every non-@wip scenario of the corpus', async () => {
    await setup();
    const features = fs.globSync('**/*.feature', { cwd: corpus }).sort();
    const disagreements = [];
    let ran = 0;
    for (const rel of features) {
      const text = fs.readFileSync(path.join(corpus, rel), 'utf8');
      if (featureTags(text).includes('@wip')) continue;
      const { parseFeature } = await import('@regelrecht/frontend-shared/gherkin');
      for (const sc of parseFeature(text).scenarios) {
        if (sc.tags.includes('@wip')) continue;
        ran += 1;
        const r = await runScenario({ engine, text, name: sc.name, loadLaw: noLoad });
        const bad = r.steps.find((s) => s.status === 'failed');
        if (bad) disagreements.push(`${rel} / ${sc.name}: ${bad.text}: ${bad.message}`);
      }
    }
    expect(ran).toBeGreaterThan(50);
    expect(disagreements).toEqual([]);
  }, 180_000);
});

describe('nullForUndefined', () => {
  it('turns a key the binding sent as undefined back into null, and leaves the rest', () => {
    expect(nullForUndefined({ a: undefined, b: 0, c: false, d: null, e: 'x' })).toEqual({ a: null, b: 0, c: false, d: null, e: 'x' });
    expect(nullForUndefined(undefined)).toEqual({});
  });
});

describe('featureTags', () => {
  it('reads tags above Feature:, not those of scenarios', () => {
    expect(featureTags('@wip @slow\nFeature: X\n  @other\n  Scenario: Y\n')).toEqual(['@wip', '@slow']);
    expect(featureTags('Feature: X\n  @wip\n  Scenario: Y\n')).toEqual([]);
  });
});

describe('runScenario (fake engine, review cases)', () => {
  it('lets "the execution fails with" match the text of an engine error object', async () => {
    const engine = fakeEngine({});
    engine.loaded.add('hoofdwet');
    engine.executeMultipleWithTrace = () => {
      throw { error: 'Missing required parameter: informatie_datum', trace: null };
    };
    const text = 'Feature: X\n  Scenario: Y\n    Given the calculation date is "2025-01-01"\n    When I evaluate "a" of "hoofdwet"\n    Then the execution fails with "informatie_datum"\n';
    const r = await runScenario({ engine, text, loadLaw: async () => {} });
    expect(r.failed).toBe(false);
    expect(r.error).toBe('Missing required parameter: informatie_datum');
  });

  it('marks a step of another tier as not checked instead of failing', async () => {
    const engine = fakeEngine({ a: 1 });
    engine.loaded.add('hoofdwet');
    const { GRAMMAR } = await import('@regelrecht/frontend-shared/gherkin');
    const other = GRAMMAR.find((g) => g.tier !== 'core' && g.action !== 'evaluate_outputs' && g.keyword === 'then');
    const stepText = other.template(other.argTypes.map(() => 'x'));
    const text = `Feature: X\n  Scenario: Y\n    Given the calculation date is "2025-01-01"\n    When I evaluate "a" of "hoofdwet"\n    Then ${stepText}\n`;
    const r = await runScenario({ engine, text, loadLaw: async () => {} });
    expect(r.failed).toBe(false);
    expect(r.steps.at(-1)).toMatchObject({ status: 'unsupported', message: expect.stringContaining(other.tier) });
  });
});
