import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useDependencies, extractRegulationRefs } from './useDependencies.js';

// Minimal Response-like stub.
function res(json, ok = true, status = 200) {
  return { ok, status, async json() { return json; }, async text() { return ''; } };
}

// Fake WASM engine: tracks which law ids are loaded, plus every body handed to
// `loadLaw` so multi-version loading can be asserted (the real engine keys
// versions by ($id, valid_from), so several bodies of one id coexist).
function fakeEngine() {
  const loaded = new Set();
  const bodies = [];
  return {
    loaded,
    bodies,
    hasLaw: (id) => loaded.has(id),
    loadLaw: (yamlText) => {
      bodies.push(yamlText);
      const m = yamlText.match(/\$id:\s*(\S+)/);
      if (m) loaded.add(m[1]);
    },
  };
}

const MAIN_LAW = `\
$id: wet_op_de_zorgtoeslag
articles:
  - number: '2'
    machine_readable:
      execution:
        input:
          - name: dekking
            source:
              regulation: zorgverzekeringswet
              output: dekking
`;

// Version lists per law id (the `fetchLawVersions` contract). No further refs,
// so the walk terminates.
const DEP_VERSIONS = {
  zorgverzekeringswet: ['$id: zorgverzekeringswet\narticles: []\n'],
  regeling_standaardpremie: ['$id: regeling_standaardpremie\narticles: []\n'],
};

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('extractRegulationRefs', () => {
  it('collects source.regulation refs and skips self-references', () => {
    expect(extractRegulationRefs(yamlMain())).toEqual(['zorgverzekeringswet']);
  });
  it('returns [] for a law with no machine_readable inputs', () => {
    expect(extractRegulationRefs({ $id: 'x', articles: [{ number: '1' }] })).toEqual([]);
  });
});

describe('useDependencies.loadAllDependencies', () => {
  it('loads source.regulation deps, returns the $id, and does NOT scan the corpus', async () => {
    // The implementor scan is off the critical path, so loading the law's own
    // deps must make no network call at all (only fetchLawVersions is used).
    const fetchSpy = vi.fn();
    globalThis.fetch = fetchSpy;
    const engine = fakeEngine();
    const fetchLawVersions = vi.fn(async (id) => DEP_VERSIONS[id]);

    const { loadAllDependencies, loading } = useDependencies();
    const mainLawId = await loadAllDependencies(MAIN_LAW, engine, fetchLawVersions);

    expect(mainLawId).toBe('wet_op_de_zorgtoeslag');
    expect(engine.loaded.has('zorgverzekeringswet')).toBe(true);
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(loading.value).toBe(false);
  });

  it('loads EVERY version of a dependency (in-force + future), not just one', async () => {
    // Regression for the temporal-federation bug: a referenced law that has a
    // future-dated version must have *all* its versions loaded so the engine
    // can pick the one in force on the scenario date — loading only the future
    // version would fail "not yet in force" for a past-dated scenario.
    const inForce = '$id: zorgverzekeringswet\nvalid_from: \'2025-01-01\'\narticles: []\n';
    const future = '$id: zorgverzekeringswet\nvalid_from: \'2099-01-01\'\narticles: []\n';
    const engine = fakeEngine();
    const fetchLawVersions = vi.fn(async (id) =>
      id === 'zorgverzekeringswet' ? [future, inForce] : DEP_VERSIONS[id],
    );

    const { loadAllDependencies } = useDependencies();
    await loadAllDependencies(MAIN_LAW, engine, fetchLawVersions);

    const zvwBodies = engine.bodies.filter((b) => b.includes('$id: zorgverzekeringswet'));
    expect(zvwBodies).toHaveLength(2);
    expect(zvwBodies).toContain(inForce);
    expect(zvwBodies).toContain(future);
  });

  it('treats an empty version list as a missing dependency (requests harvest)', async () => {
    // /versions returning [] means no version is available — the loader must
    // not silently succeed; it routes the id to the harvest request like a
    // fetch failure. The harvest call is the only network request.
    const fetchSpy = vi.fn(async (url) => {
      if (String(url).includes('/harvest')) return res({ results: [] });
      return res([], false, 404);
    });
    globalThis.fetch = fetchSpy;
    const engine = fakeEngine();
    const fetchLawVersions = vi.fn(async () => []);

    const { loadAllDependencies } = useDependencies();
    await loadAllDependencies(MAIN_LAW, engine, fetchLawVersions);

    expect(engine.loaded.has('zorgverzekeringswet')).toBe(false);
    // The missing dep is routed to harvest, not silently dropped.
    const harvestCalls = fetchSpy.mock.calls.filter((c) => String(c[0]).includes('/harvest'));
    expect(harvestCalls.length).toBeGreaterThan(0);
  });
});

describe('useDependencies.loadImplementors', () => {
  it('resolves implementors with a single request and loads them', async () => {
    const fetchSpy = vi.fn().mockImplementation(async (url) => {
      if (url.endsWith('/implementors')) return res(['regeling_standaardpremie']);
      return res([], false, 404);
    });
    globalThis.fetch = fetchSpy;
    const engine = fakeEngine();
    const fetchLawVersions = vi.fn(async (id) => DEP_VERSIONS[id]);

    const { loadImplementors } = useDependencies();
    await loadImplementors('wet_op_de_zorgtoeslag', engine, fetchLawVersions, 'mig-1a2b3c4d');

    expect(fetchSpy).toHaveBeenCalledTimes(1);
    expect(fetchSpy.mock.calls[0][0]).toBe(
      '/api/trajects/mig-1a2b3c4d/corpus/laws/wet_op_de_zorgtoeslag/implementors',
    );
    expect(fetchSpy.mock.calls.some((c) => String(c[0]).includes('limit=1000'))).toBe(false);
    expect(engine.loaded.has('regeling_standaardpremie')).toBe(true);
  });

  it('runs at most once per (trajectRef, lawId)', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(res(['regeling_standaardpremie']));
    globalThis.fetch = fetchSpy;
    const engine = fakeEngine();
    const fetchLawVersions = vi.fn(async (id) => DEP_VERSIONS[id]);

    const { loadImplementors } = useDependencies();
    await loadImplementors('wet_op_de_zorgtoeslag', engine, fetchLawVersions, 'mig-1a2b3c4d');
    await loadImplementors('wet_op_de_zorgtoeslag', engine, fetchLawVersions, 'mig-1a2b3c4d');

    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it('is best-effort: a failed scan resolves without throwing and can be retried', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(res(null, false, 500));
    globalThis.fetch = fetchSpy;
    const engine = fakeEngine();
    const { loadImplementors } = useDependencies();
    await expect(
      loadImplementors('wet_op_de_zorgtoeslag', engine, vi.fn(), null),
    ).resolves.toBeUndefined();
    // A transient failure must not permanently suppress the scan: a second
    // call retries (the guard key was reset on failure).
    await loadImplementors('wet_op_de_zorgtoeslag', engine, vi.fn(), null);
    expect(fetchSpy).toHaveBeenCalledTimes(2);
  });
});

// Parsed equivalent of MAIN_LAW (plus a self-reference) for the pure test.
function yamlMain() {
  return {
    $id: 'wet_op_de_zorgtoeslag',
    articles: [
      {
        number: '2',
        machine_readable: {
          execution: {
            input: [
              { name: 'dekking', source: { regulation: 'zorgverzekeringswet', output: 'dekking' } },
              { name: 'self', source: { regulation: 'wet_op_de_zorgtoeslag', output: 'x' } },
            ],
          },
        },
      },
    ],
  };
}
