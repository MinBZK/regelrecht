import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref } from 'vue';
import { useScenarios } from './useScenarios.js';

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
});
