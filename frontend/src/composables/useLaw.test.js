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
    // The stale ETag is kept - the user reloads to pick up a fresh one.
    expect(law.currentEtag.value).toBe('"v1"');
  });

  // Review-modus approval (EditorView's handleLawSave) saves a task's full
  // proposed YAML instead of the article-scoped splice `currentLawYaml`
  // builds. `saveLaw` takes arbitrary YAML text and PUTs it verbatim, so no
  // second save path exists for that - this test pins the contract review
  // mode relies on: the PUT body is exactly the override text passed in
  // (here, a full two-article proposal unrelated to the loaded law's single
  // article), and If-Match still carries the currently loaded ETag.
  it('sends an arbitrary override YAML verbatim as the PUT body (review-mode full-save)', async () => {
    const calls = [];
    globalThis.fetch = vi.fn().mockImplementation(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (opts?.method === 'PUT') {
        return res({ etag: '"v2"', json: { pr: null, etag: '"v2"' } });
      }
      return res({
        body: '$id: wet_review\narticles:\n  - number: "1"\n    text: origineel\n',
        etag: '"v1"',
      });
    });

    const law = useLaw('wet_review', null, 'tr-12345678');
    await waitForLoaded(law);
    expect(law.currentEtag.value).toBe('"v1"');

    const overrideYaml =
      '$id: wet_review\narticles:\n  - number: "1"\n    text: voorgesteld\n' +
      '  - number: "2"\n    text: nieuw artikel\n';
    await law.saveLaw(overrideYaml);

    const put = calls.find((c) => c.opts?.method === 'PUT');
    expect(put).toBeTruthy();
    expect(put.opts.body).toBe(overrideYaml);
    expect(put.opts.headers['If-Match']).toBe('"v1"');
    expect(law.currentEtag.value).toBe('"v2"');
    // The local law state converges on the full override, not a splice -
    // both articles from the proposal are present after the save.
    expect(law.articles.value.map((a) => a.number)).toEqual(['1', '2']);
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

  it('shares one GET between load() and a concurrent fetchLaw (editor mount)', async () => {
    // The exact regression this PR fixes: on editor mount, useLaw().load()
    // fetches the routed law while the persisted-tab label loader
    // fetchLaw()s the same id in parallel against an empty cache.
    let callCount = 0;
    let release;
    globalThis.fetch = vi.fn().mockImplementation(async () => {
      callCount += 1;
      // Hold the GET open so the second caller arrives while it's in flight.
      await new Promise((resolve) => { release = resolve; });
      return res({ body: '$id: wet_mount_race\nname: V1\n', etag: '"v1"' });
    });

    const law = useLaw('wet_mount_race', null, 'tr-dedupe');
    const labelFetch = fetchLaw('tr-dedupe', 'wet_mount_race');
    expect(callCount).toBe(1);

    release();
    const entry = await labelFetch;
    await waitForLoaded(law);

    expect(callCount).toBe(1);
    expect(entry.law.$id).toBe('wet_mount_race');
    // Both consumers converge on the same fetched content.
    expect(law.rawYaml.value).toBe(entry.rawYaml);
    expect(law.currentEtag.value).toBe('"v1"');
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

    // A subsequent fresh load (load() never reads the cache) fires a new GET -
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
    // the canonical id is served from cache - no extra GET.
    const byId = await fetchLaw('tr-dedupe', 'wet_canonical');
    expect(byId).toBe(bySlug);
    expect(callCount).toBe(1);
  });

  it('a traject switch during a direct-URL load cannot poison the new traject cache', async () => {
    // The direct-URL branch caches under the traject the load was issued
    // for. If a cross-traject switchLaw lands while the response body is
    // still streaming, the late cache write must go to the OLD traject's
    // key - never the new one, where it would shadow the real law.
    let releaseText;
    const calls = [];
    globalThis.fetch = vi.fn().mockImplementation(async (url) => {
      calls.push(url);
      if (url === '/direct/wet_direct.yaml') {
        return {
          ok: true,
          status: 200,
          headers: { get: (k) => (k.toLowerCase() === 'etag' ? '"d1"' : null) },
          // Body held open so the traject switch can land mid-flight.
          text: () => new Promise((resolve) => { releaseText = resolve; }),
        };
      }
      return res({ body: `$id: ${url.split('/').pop()}\nname: X\n`, etag: '"x1"' });
    });

    const law = useLaw('/direct/wet_direct.yaml', null, 'tr-old');
    await vi.waitFor(() => expect(releaseText).toBeDefined());
    // Cross-traject navigation while the direct body is still streaming.
    await law.switchLaw('wet_other', null, 'tr-new');
    releaseText('$id: wet_direct\nname: Direct\n');
    await waitForLoaded(law);

    // Fetching the same id in the new traject must go to the network -
    // a cache hit here would serve the direct-URL body as tr-new content.
    const before = calls.length;
    const entry = await fetchLaw('tr-new', 'wet_direct');
    expect(calls.length).toBe(before + 1);
    expect(calls[calls.length - 1]).toBe('/api/trajects/tr-new/corpus/laws/wet_direct');
    expect(entry.law.$id).toBe('wet_direct');
  });

  it('does not pin a rejected fetch - the next call retries', async () => {
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

describe('useLaw law_create-flow (seedFromYaml + createLaw)', () => {
  it('seedFromYaml vult de editorstate zonder netwerk en wist de load-fout', async () => {
    // De load 404't (de wet bestaat nog niet in het traject).
    globalThis.fetch = vi.fn().mockImplementation(async () =>
      res({ ok: false, status: 404, body: 'Law not found' }),
    );
    const law = useLaw('nieuwe_wet', null, 'tr-12345678');
    await waitForLoaded(law);
    expect(law.error.value).toBeTruthy();

    const seeded = law.seedFromYaml(
      '$id: nieuwe_wet\narticles:\n  - number: "1"\n    text: t\n',
    );
    expect(seeded).toBe(true);
    expect(law.error.value).toBeNull();
    expect(law.law.value.$id).toBe('nieuwe_wet');
    expect(law.selectedArticleNumber.value).toBe('1');
    expect(law.currentEtag.value).toBeNull();
  });

  it('seedFromYaml laat de foutstaat staan bij kapotte YAML', async () => {
    globalThis.fetch = vi.fn().mockImplementation(async () =>
      res({ ok: false, status: 404, body: 'Law not found' }),
    );
    const law = useLaw('nieuwe_wet2', null, 'tr-12345678');
    await waitForLoaded(law);
    expect(law.seedFromYaml('niet: [valide yaml')).toBe(false);
    expect(law.error.value).toBeTruthy();
  });

  it('createLaw POST naar het create-endpoint en ketent de ETag voor vervolg-saves', async () => {
    const calls = [];
    globalThis.fetch = vi.fn().mockImplementation(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (opts?.method === 'POST') {
        return res({ etag: '"created"', json: { pr: null, etag: '"created"' } });
      }
      return res({ ok: false, status: 404, body: 'Law not found' });
    });
    const law = useLaw('nieuwe_wet3', null, 'tr-12345678');
    await waitForLoaded(law);
    law.seedFromYaml('$id: nieuwe_wet3\narticles:\n  - number: "1"\n    text: t\n');

    await law.createLaw('$id: nieuwe_wet3\narticles:\n  - number: "1"\n    text: t\n');

    const post = calls.find((c) => c.opts?.method === 'POST');
    expect(post).toBeTruthy();
    expect(post.url).toBe('/api/trajects/tr-12345678/corpus/laws');
    // Geen If-Match op een create: er is nog niets om mee te racen.
    expect(post.opts.headers['If-Match']).toBeUndefined();
    expect(law.currentEtag.value).toBe('"created"');
    expect(law.lawId.value).toBe('nieuwe_wet3');
  });

  it('createLaw geeft de servermelding door (bijv. een 409-slugconflict)', async () => {
    globalThis.fetch = vi.fn().mockImplementation(async (url, opts) => {
      if (opts?.method === 'POST') {
        return res({ ok: false, status: 409, body: 'Er bestaat al een wet met dit $id in dit traject; pas het $id in de YAML aan.' });
      }
      return res({ ok: false, status: 404, body: 'Law not found' });
    });
    const law = useLaw('nieuwe_wet4', null, 'tr-12345678');
    await waitForLoaded(law);
    await expect(law.createLaw('$id: nieuwe_wet4\narticles: []\n')).rejects.toThrow(
      /bestaat al een wet/i,
    );
    expect(law.saveError.value).toBeTruthy();
  });
});
