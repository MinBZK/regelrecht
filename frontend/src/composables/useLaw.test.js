import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useLaw } from './useLaw.js';

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

async function waitForLoaded(law) {
  await vi.waitFor(() => {
    expect(law.loading.value).toBe(false);
  });
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('useLaw optimistic concurrency', () => {
  it('captures the ETag on load and sends it back as If-Match on save', async () => {
    const calls = [];
    globalThis.fetch = vi.fn().mockImplementation(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (opts?.method === 'PUT') {
        return res({ etag: '"new"', json: { pr: null, etag: '"new"' } });
      }
      return res({ body: '$id: wet_etag_chain\nname: V1\n', etag: '"v1"' });
    });

    const law = useLaw('wet_etag_chain', null, 'tr-12345678');
    await waitForLoaded(law);
    expect(law.currentEtag.value).toBe('"v1"');

    await law.saveLaw('$id: wet_etag_chain\nname: V2\n');

    const put = calls.find((c) => c.opts?.method === 'PUT');
    expect(put).toBeTruthy();
    expect(put.url).toBe('/api/trajects/tr-12345678/corpus/laws/wet_etag_chain');
    expect(put.opts.headers['If-Match']).toBe('"v1"');
    // The new ETag from the save response is chained for the next save.
    expect(law.currentEtag.value).toBe('"new"');
  });

  it('omits If-Match when the load carried no ETag (older deployments)', async () => {
    const calls = [];
    globalThis.fetch = vi.fn().mockImplementation(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (opts?.method === 'PUT') return res({ json: { pr: null } });
      return res({ body: '$id: wet_no_etag\nname: V1\n' });
    });

    const law = useLaw('wet_no_etag', null, 'tr-12345678');
    await waitForLoaded(law);
    expect(law.currentEtag.value).toBeNull();

    await law.saveLaw('$id: wet_no_etag\nname: V2\n');
    const put = calls.find((c) => c.opts?.method === 'PUT');
    expect(put.opts.headers['If-Match']).toBeUndefined();
  });

  it('surfaces a 412 as a Dutch conflict error instead of overwriting silently', async () => {
    globalThis.fetch = vi.fn().mockImplementation(async (_url, opts) => {
      if (opts?.method === 'PUT') return res({ ok: false, status: 412 });
      return res({ body: '$id: wet_conflict\nname: V1\n', etag: '"v1"' });
    });

    const law = useLaw('wet_conflict', null, 'tr-12345678');
    await waitForLoaded(law);

    await expect(law.saveLaw('$id: wet_conflict\nname: V2\n')).rejects.toThrow(
      /door iemand anders gewijzigd/i,
    );
    expect(law.saveError.value?.message).toMatch(/herlaad/i);
    // The stale ETag is kept — the user reloads to pick up a fresh one.
    expect(law.currentEtag.value).toBe('"v1"');
  });
});
