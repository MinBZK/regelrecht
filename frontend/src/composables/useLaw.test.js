import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useLaw, fetchLaw } from './useLaw.js';

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

describe('useLaw fetch dedup (single-flight)', () => {
  it('shares one GET between concurrent callers for the same key', async () => {
    let callCount = 0;
    globalThis.fetch = vi.fn().mockImplementation(async () => {
      callCount += 1;
      return res({ body: '$id: wet_single_flight\nname: V1\n', etag: '"v1"' });
    });

    // Two concurrent fetches for the same key (mirrors an editor mount:
    // useLaw().load() and fetchLaw() racing the same id against an empty cache).
    const p1 = fetchLaw('tr-dedupe', 'wet_single_flight');
    const p2 = fetchLaw('tr-dedupe', 'wet_single_flight');
    // The second caller joins the in-flight request instead of firing its own.
    expect(callCount).toBe(1);

    const [e1, e2] = await Promise.all([p1, p2]);
    // Both callers get the same resolved entry, from one GET.
    expect(e1).toBe(e2);
    expect(e1.law.$id).toBe('wet_single_flight');
    expect(callCount).toBe(1);
  });

  it('clears the pending entry on settle so a later fresh load re-fetches', async () => {
    let callCount = 0;
    globalThis.fetch = vi.fn().mockImplementation(async () => {
      callCount += 1;
      return res({ body: '$id: wet_pending_clear\nname: V1\n', etag: '"v1"' });
    });

    // First fresh load fires GET #1 and settles.
    const a = useLaw('wet_pending_clear', null, 'tr-dedupe');
    await waitForLoaded(a);
    expect(callCount).toBe(1);

    // A subsequent fresh load (load() never reads the cache) fires a new GET —
    // proving the settled promise wasn't pinned in the pending map.
    const b = useLaw('wet_pending_clear', null, 'tr-dedupe');
    await waitForLoaded(b);
    expect(callCount).toBe(2);
  });

  it('caches under the resolved $id so a later fetch by canonical id is a hit', async () => {
    let callCount = 0;
    globalThis.fetch = vi.fn().mockImplementation(async () => {
      callCount += 1;
      return res({ body: '$id: wet_canonical\nname: V1\n', etag: '"v1"' });
    });

    // Requested by a slug that differs from the body's `$id`.
    const bySlug = await fetchLaw('tr-dedupe', 'wet-canonical-slug');
    expect(callCount).toBe(1);
    expect(bySlug.law.$id).toBe('wet_canonical');

    // The dual-cache write under the resolved `$id` means a later fetch by
    // the canonical id is served from cache — no extra GET.
    const byId = await fetchLaw('tr-dedupe', 'wet_canonical');
    expect(byId).toBe(bySlug);
    expect(callCount).toBe(1);
  });

  it('does not pin a rejected fetch — the next call retries', async () => {
    let callCount = 0;
    globalThis.fetch = vi.fn().mockImplementation(async () => {
      callCount += 1;
      if (callCount === 1) throw new Error('network down');
      return res({ body: '$id: wet_retry\nname: V1\n', etag: '"v1"' });
    });

    // First call fails; a failed fetch must not pin a rejected promise.
    await expect(fetchLaw('tr-dedupe', 'wet_retry')).rejects.toThrow(
      /network down/i,
    );
    expect(callCount).toBe(1);

    // The retry gets a fresh GET (nothing was cached, nothing pinned) and
    // succeeds.
    const entry = await fetchLaw('tr-dedupe', 'wet_retry');
    expect(entry.law.$id).toBe('wet_retry');
    expect(callCount).toBe(2);
  });
});
