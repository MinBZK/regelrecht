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
          - name: dekking
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
    expect(extractRegulationRefs(yamlMain())).toEqual(['zorgverzekeringswet']);
  });
  it('returns [] for a law with no machine_readable inputs', () => {
    expect(extractRegulationRefs({ $id: 'x', articles: [{ number: '1' }] })).toEqual([]);
  });
});

describe('useDependencies.loadAllDependencies', () => {
  it('loads source.regulation deps, returns the $id, and does NOT scan the corpus', async () => {
    // The implementor scan is off the critical path, so loading the law's own
    // deps must make no network call at all (only fetchLawYaml is used).
    const fetchSpy = vi.fn();
    globalThis.fetch = fetchSpy;
    const engine = fakeEngine();
    const fetchLawYaml = vi.fn(async (id) => DEP_YAML[id]);

    const { loadAllDependencies, loading } = useDependencies();
    const mainLawId = await loadAllDependencies(MAIN_LAW, engine, fetchLawYaml);

    expect(mainLawId).toBe('zorgtoeslagwet');
    expect(engine.loaded.has('zorgverzekeringswet')).toBe(true);
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(loading.value).toBe(false);
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
    const fetchLawYaml = vi.fn(async (id) => DEP_YAML[id]);

    const { loadImplementors } = useDependencies();
    await loadImplementors('zorgtoeslagwet', engine, fetchLawYaml, 'mig-1a2b3c4d');

    expect(fetchSpy).toHaveBeenCalledTimes(1);
    expect(fetchSpy.mock.calls[0][0]).toBe(
      '/api/trajects/mig-1a2b3c4d/corpus/laws/zorgtoeslagwet/implementors',
    );
    expect(fetchSpy.mock.calls.some((c) => String(c[0]).includes('limit=1000'))).toBe(false);
    expect(engine.loaded.has('regeling_standaardpremie')).toBe(true);
  });

  it('runs at most once per (trajectRef, lawId)', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(res(['regeling_standaardpremie']));
    globalThis.fetch = fetchSpy;
    const engine = fakeEngine();
    const fetchLawYaml = vi.fn(async (id) => DEP_YAML[id]);

    const { loadImplementors } = useDependencies();
    await loadImplementors('zorgtoeslagwet', engine, fetchLawYaml, 'mig-1a2b3c4d');
    await loadImplementors('zorgtoeslagwet', engine, fetchLawYaml, 'mig-1a2b3c4d');

    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it('is best-effort: a failed scan resolves without throwing and can be retried', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(res(null, false, 500));
    globalThis.fetch = fetchSpy;
    const engine = fakeEngine();
    const { loadImplementors } = useDependencies();
    await expect(
      loadImplementors('zorgtoeslagwet', engine, vi.fn(), null),
    ).resolves.toBeUndefined();
    // A transient failure must not permanently suppress the scan: a second
    // call retries (the guard key was reset on failure).
    await loadImplementors('zorgtoeslagwet', engine, vi.fn(), null);
    expect(fetchSpy).toHaveBeenCalledTimes(2);
  });
});

// Parsed equivalent of MAIN_LAW (plus a self-reference) for the pure test.
function yamlMain() {
  return {
    $id: 'zorgtoeslagwet',
    articles: [
      {
        number: '2',
        machine_readable: {
          execution: {
            input: [
              { name: 'dekking', source: { regulation: 'zorgverzekeringswet', output: 'dekking' } },
              { name: 'self', source: { regulation: 'zorgtoeslagwet', output: 'x' } },
            ],
          },
        },
      },
    ],
  };
}
