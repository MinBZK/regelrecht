import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useDependencies, extractRegulationRefs } from './useDependencies.js';

// Minimal Response-like stub.
function res(json, ok = true, status = 200) {
  return { ok, status, async json() { return json; }, async text() { return ''; } };
}

// Fake WASM engine: tracks which laws were loaded.
function fakeEngine() {
  const loaded = new Set();
  return {
    loaded,
    hasLaw: (id) => loaded.has(id),
    loadLaw: (yamlText) => {
      const m = yamlText.match(/\$id:\s*(\S+)/);
      if (m) loaded.add(m[1]);
    },
  };
}

const MAIN_LAW = `\
$id: zorgtoeslagwet
articles:
  - number: '2'
    machine_readable:
      execution:
        input:
          - name: standaardpremie
            source:
              regulation: zorgverzekeringswet
              output: dekking
`;

// Dependency YAMLs with no further refs, so the walk terminates.
const DEP_YAML = {
  zorgverzekeringswet: '$id: zorgverzekeringswet\narticles: []\n',
  regeling_standaardpremie: '$id: regeling_standaardpremie\narticles: []\n',
};

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('extractRegulationRefs', () => {
  it('collects source.regulation refs and skips self-references', () => {
    const law = yamlMain();
    expect(extractRegulationRefs(law)).toEqual(['zorgverzekeringswet']);
  });
  it('returns [] for a law with no machine_readable inputs', () => {
    expect(extractRegulationRefs({ $id: 'x', articles: [{ number: '1' }] })).toEqual([]);
  });
});

describe('useDependencies.loadAllDependencies', () => {
  it('resolves implementors via a single implementors request (no per-law corpus scan)', async () => {
    const fetchSpy = vi.fn().mockImplementation(async (url) => {
      if (url.endsWith('/implementors')) return res(['regeling_standaardpremie']);
      return res([], false, 404);
    });
    globalThis.fetch = fetchSpy;

    const engine = fakeEngine();
    const fetchLawYaml = vi.fn(async (id) => DEP_YAML[id]);

    const { loading, loadedDeps, loadAllDependencies } = useDependencies();
    await loadAllDependencies(MAIN_LAW, engine, fetchLawYaml, 'mig-1a2b3c4d');

    // Exactly one network request, and it's the implementors endpoint for the
    // main law under the active traject — NOT a `?limit=1000` corpus listing.
    expect(fetchSpy).toHaveBeenCalledTimes(1);
    expect(fetchSpy.mock.calls[0][0]).toBe(
      '/api/trajects/mig-1a2b3c4d/corpus/laws/zorgtoeslagwet/implementors',
    );
    expect(fetchSpy.mock.calls.some((c) => String(c[0]).includes('limit=1000'))).toBe(false);

    // Both the source.regulation dep and the implementor are loaded.
    expect(engine.loaded.has('zorgverzekeringswet')).toBe(true);
    expect(engine.loaded.has('regeling_standaardpremie')).toBe(true);
    expect(loadedDeps.value).toContain('zorgverzekeringswet');
    expect(loadedDeps.value).toContain('regeling_standaardpremie');
    expect(loading.value).toBe(false);
  });

  it('treats implementor-discovery failure as best-effort', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res(null, false, 500));
    const engine = fakeEngine();
    const fetchLawYaml = vi.fn(async (id) => DEP_YAML[id]);

    const { error, loadAllDependencies } = useDependencies();
    await loadAllDependencies(MAIN_LAW, engine, fetchLawYaml, null);

    // The source.regulation dep still loads; the failed implementors call
    // does not surface as an error.
    expect(engine.loaded.has('zorgverzekeringswet')).toBe(true);
    expect(error.value).toBe(null);
  });
});

// Parse MAIN_LAW once for the pure-function test.
function yamlMain() {
  return {
    $id: 'zorgtoeslagwet',
    articles: [
      {
        number: '2',
        machine_readable: {
          execution: {
            input: [
              { name: 'standaardpremie', source: { regulation: 'zorgverzekeringswet', output: 'dekking' } },
              { name: 'self', source: { regulation: 'zorgtoeslagwet', output: 'x' } },
            ],
          },
        },
      },
    ],
  };
}
