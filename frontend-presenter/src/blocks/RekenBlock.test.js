// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import RekenBlock from './RekenBlock.vue';

// The block's own job: fetch the scenario, run it on the deck's engine, show
// outcome, expectations and trace. The engine is a stand-in here; the real one
// is exercised in src/engine/runScenario.test.js.
const engine = {
  loaded: new Set(),
  hasLaw: (id) => engine.loaded.has(id),
  clearDataSources: () => {},
  registerDataSource: () => {},
  executeMultipleWithTrace: () => ({
    outputs: { heeft_recht_op_zorgtoeslag: true, hoogte_zorgtoeslag: 157731 },
    trace: { root: { node_type: 'Article', name: '2', children: [{ node_type: 'Resolve', name: '$toetsingsinkomen', result: 79547, children: [] }] } },
  }),
};
vi.mock('../engine/engine.js', () => ({
  deckEngine: async () => ({
    engine,
    loadLaw: async (id) => engine.loaded.add(id),
    units: new Map([['wet_op_de_zorgtoeslag/hoogte_zorgtoeslag', 'eurocent']]),
    failures: [],
    exclusive: (fn) => fn(),
  }),
}));

const FEATURE = `Feature: Zorgtoeslag
  Scenario: Alleenstaande
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "1"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "hoogte_zorgtoeslag" equals 157731
`;

beforeEach(() => {
  vi.stubGlobal('fetch', vi.fn(async (url) => ({ ok: true, json: async () => ({ feature: 'x', text: FEATURE, url }) })));
});
afterEach(() => vi.unstubAllGlobals());

const mountBlock = (spec) => mount(RekenBlock, { props: { spec }, global: { provide: { 'presenter:deck': null } } });

describe('RekenBlock', () => {
  it('shows the outcome in euros and every expectation of the scenario', async () => {
    const w = mountBlock({ scenario: 'wet_op_de_zorgtoeslag/eligibility.feature', naam: 'Alleenstaande' });
    await flushPromises();
    expect(fetch).toHaveBeenCalledWith(expect.stringContaining('feature=wet_op_de_zorgtoeslag%2Feligibility.feature'));
    const rows = w.findAll('.reken-outputs tr').map((r) => [r.find('th').text(), r.find('td').text()]);
    expect(rows).toEqual([
      ['heeft recht op zorgtoeslag', 'ja'],
      ['hoogte zorgtoeslag', expect.stringMatching(/^€\s1\.577,31$/)],
    ]);
    expect(w.findAll('.reken-checks li.passed')).toHaveLength(2);
    expect(w.find('.reken-trace').exists()).toBe(false);
  });

  it('shows only the outputs asked for, and the trace when asked', async () => {
    const w = mountBlock({ scenario: 'x/y.feature', uitvoer: 'hoogte_zorgtoeslag', spoor: true });
    await flushPromises();
    expect(w.findAll('.reken-outputs tr')).toHaveLength(1);
    expect(w.find('.reken-trace').text()).toContain('toetsingsinkomen');
  });

  it('shows the server error, e.g. when the engine is not built', async () => {
    fetch.mockResolvedValueOnce({ ok: false, json: async () => ({ error: 'Scenario-bestand niet gevonden: x/y.feature' }) });
    const w = mountBlock({ scenario: 'x/y.feature' });
    await flushPromises();
    expect(w.find('.wet-error').text()).toBe('Scenario-bestand niet gevonden: x/y.feature');
  });
});
