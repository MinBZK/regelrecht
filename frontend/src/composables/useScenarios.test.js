import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref } from 'vue';
import { useScenarios, isScenarioMismatch } from './useScenarios.js';

// Helper: shape a Response-like object with headers + body + status.
// Mirrors the harness in useTrajectDocuments.test.js.
function res({ ok = true, status = 200, body = '', etag = null, json = null }) {
  const headers = new Map();
  if (etag) headers.set('etag', etag);
  if (json) headers.set('content-type', 'application/json');
  else if (typeof body === 'string') headers.set('content-type', 'text/plain');
  return {
    ok,
    status,
    headers: {
      get: (k) => headers.get(k.toLowerCase()) ?? null,
    },
    async text() {
      return typeof body === 'string' ? body : JSON.stringify(body);
    },
    async json() {
      return json ?? body;
    },
  };
}

async function waitForScenario(sc, filename) {
  await vi.waitFor(() => {
    expect(sc.selectedScenario.value).toBe(filename);
    expect(sc.featureText.value).not.toBe('');
  });
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('useScenarios optimistic concurrency', () => {
  function mockFetch(onPut) {
    const calls = [];
    globalThis.fetch = vi.fn().mockImplementation(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (opts?.method === 'PUT') return onPut(url, opts);
      if (url.endsWith('/scenarios')) {
        return res({ json: [{ filename: 'basis.feature' }] });
      }
      return res({ body: 'Feature: v1\n', etag: '"v1"' });
    });
    return calls;
  }

  it('captures the ETag on read and sends it back as If-Match on save', async () => {
    const calls = mockFetch(() =>
      res({ etag: '"new"', json: { pr: null, etag: '"new"' } }),
    );

    const sc = useScenarios(ref('wet_a'), ref('tr-12345678'));
    await waitForScenario(sc, 'basis.feature');

    await sc.saveScenario('basis.feature', 'Feature: v2\n');

    const put = calls.find((c) => c.opts?.method === 'PUT');
    expect(put).toBeTruthy();
    expect(put.url).toBe(
      '/api/trajects/tr-12345678/corpus/laws/wet_a/scenarios/basis.feature',
    );
    expect(put.opts.headers['If-Match']).toBe('"v1"');

    // The new ETag is chained: a follow-up save sends the updated value.
    await sc.saveScenario('basis.feature', 'Feature: v3\n');
    const puts = calls.filter((c) => c.opts?.method === 'PUT');
    expect(puts[1].opts.headers['If-Match']).toBe('"new"');
  });

  it('saves a brand-new scenario without an If-Match precondition', async () => {
    const calls = mockFetch(() =>
      res({ etag: '"created"', json: { pr: null, etag: '"created"' } }),
    );

    const sc = useScenarios(ref('wet_a'), ref('tr-12345678'));
    await waitForScenario(sc, 'basis.feature');

    await sc.saveScenario('nieuw.feature', 'Feature: nieuw\n');
    const put = calls.find(
      (c) => c.opts?.method === 'PUT' && c.url.endsWith('nieuw.feature'),
    );
    expect(put.opts.headers['If-Match']).toBeUndefined();
  });

  it('surfaces a 412 as a Dutch conflict error instead of overwriting silently', async () => {
    mockFetch(() => res({ ok: false, status: 412 }));

    const sc = useScenarios(ref('wet_a'), ref('tr-12345678'));
    await waitForScenario(sc, 'basis.feature');

    await expect(
      sc.saveScenario('basis.feature', 'Feature: v2\n'),
    ).rejects.toThrow(/door iemand anders gewijzigd/i);
    expect(sc.saveError.value?.message).toMatch(/herlaad/i);
  });

  it('does not chain the etag or content across a law switch mid-save', async () => {
    // Scenario filenames recur across laws (`basis.feature` exists for
    // both wet_a and wet_b). An in-flight save of wet_a's copy must not
    // re-insert its ETag into the map after the watch cleared it for
    // wet_b, or wet_b's next save carries a foreign If-Match → 412.
    let resolvePut;
    const putGate = new Promise((resolve) => { resolvePut = resolve; });
    const calls = [];
    globalThis.fetch = vi.fn().mockImplementation(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (opts?.method === 'PUT') {
        await putGate;
        return res({ etag: '"a2"', json: { pr: null, etag: '"a2"' } });
      }
      if (url.endsWith('/scenarios')) {
        return res({ json: [{ filename: 'basis.feature' }] });
      }
      if (url.includes('/laws/wet_b/')) {
        return res({ body: 'Feature: b1\n', etag: '"b1"' });
      }
      return res({ body: 'Feature: a1\n', etag: '"a1"' });
    });

    const lawId = ref('wet_a');
    const sc = useScenarios(lawId, ref('tr-12345678'));
    await waitForScenario(sc, 'basis.feature');
    expect(sc.featureText.value).toBe('Feature: a1\n');

    // Kick off the save, then switch laws while the PUT is in flight.
    const save = sc.saveScenario('basis.feature', 'Feature: a2\n');
    lawId.value = 'wet_b';
    await vi.waitFor(() => {
      expect(sc.featureText.value).toBe('Feature: b1\n');
    });

    resolvePut();
    await save;

    // The stale completion must not overwrite the new law's content…
    expect(sc.featureText.value).toBe('Feature: b1\n');

    // …nor poison the new law's etag map: saving wet_b's same-named
    // scenario sends the ETag wet_b's read returned, not the old
    // save's chained one.
    await sc.saveScenario('basis.feature', 'Feature: b2\n');
    const puts = calls.filter((c) => c.opts?.method === 'PUT');
    expect(puts[1].url).toBe(
      '/api/trajects/tr-12345678/corpus/laws/wet_b/scenarios/basis.feature',
    );
    expect(puts[1].opts.headers['If-Match']).toBe('"b1"');
  });
});

describe('isScenarioMismatch', () => {
  it('is false when the file targets the opened law', () => {
    expect(isScenarioMismatch({ target_law_ids: ['wet_a'] }, 'wet_a')).toBe(false);
  });
  it('is false when targets are unknown (no execution step yet)', () => {
    expect(isScenarioMismatch({ target_law_ids: [] }, 'wet_a')).toBe(false);
    expect(isScenarioMismatch({}, 'wet_a')).toBe(false);
  });
  it('is true when the file only targets other laws', () => {
    expect(isScenarioMismatch({ target_law_ids: ['wet_b', 'wet_c'] }, 'wet_a')).toBe(true);
  });
});

describe('useScenarios auto-select', () => {
  function mockList(entries) {
    globalThis.fetch = vi.fn().mockImplementation(async (url) => {
      if (String(url).endsWith('/scenarios')) return res({ json: entries });
      return res({ body: 'Feature: x\n', etag: '"v1"' });
    });
  }

  it('prefers the first file that targets the opened law', async () => {
    mockList([
      { filename: 'other.feature', target_law_ids: ['andere_wet'] },
      { filename: 'matching.feature', target_law_ids: ['wet_a'] },
    ]);

    const s = useScenarios(ref('wet_a'));
    await s.fetchScenarios();
    expect(s.selectedScenario.value).toBe('matching.feature');
  });

  it('prefers an explicit match over a file with unknown targets', async () => {
    mockList([
      { filename: 'wip.feature', target_law_ids: [] },
      { filename: 'matching.feature', target_law_ids: ['wet_a'] },
    ]);

    const s = useScenarios(ref('wet_a'));
    await s.fetchScenarios();
    expect(s.selectedScenario.value).toBe('matching.feature');
  });

  it('prefers unknown targets over a known mismatch', async () => {
    mockList([
      { filename: 'other.feature', target_law_ids: ['andere_wet'] },
      { filename: 'wip.feature', target_law_ids: [] },
    ]);

    const s = useScenarios(ref('wet_a'));
    await s.fetchScenarios();
    expect(s.selectedScenario.value).toBe('wip.feature');
  });

  it('falls back to the first file when none match', async () => {
    mockList([
      { filename: 'a.feature', target_law_ids: ['andere_wet'] },
      { filename: 'b.feature', target_law_ids: ['nog_een_wet'] },
    ]);

    const s = useScenarios(ref('wet_a'));
    await s.fetchScenarios();
    expect(s.selectedScenario.value).toBe('a.feature');
  });
});
