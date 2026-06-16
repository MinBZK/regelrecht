import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// useFeatureFlags is a module singleton; reset modules per test for a clean
// store, and stub the global fetch that apiFetch ultimately calls.
async function freshFlags() {
  vi.resetModules();
  const mod = await import('./useFeatureFlags.js');
  return mod.useFeatureFlags();
}

function stubFetch(putResponse) {
  vi.stubGlobal(
    'fetch',
    vi.fn((input, init) => {
      const method = init?.method ?? 'GET';
      if (method === 'PUT') return Promise.resolve(putResponse());
      // GET /api/feature-flags — empty server set, so DEFAULTS apply.
      return Promise.resolve(
        new Response('{}', { status: 200, headers: { 'content-type': 'application/json' } }),
      );
    }),
  );
}

describe('useFeatureFlags', () => {
  beforeEach(() => {
    localStorage.clear();
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('keeps a toggle locally when the backend write is rejected (e.g. 401 in OIDC-off dev)', async () => {
    stubFetch(() => new Response('unauthorized', { status: 401 }));
    const { isEnabled, toggle, loaded } = await freshFlags();
    await vi.waitFor(() => expect(loaded.value).toBe(true));

    expect(isEnabled('panel.notes')).toBe(false); // default
    await toggle('panel.notes');

    // Kept on (not reverted) and persisted so it survives a refresh.
    expect(isEnabled('panel.notes')).toBe(true);
    const saved = JSON.parse(localStorage.getItem('regelrecht-feature-flags') || '{}');
    expect(saved['panel.notes']).toBe(true);
  });

  it('clears the local override when the server write succeeds', async () => {
    stubFetch(
      () =>
        new Response(JSON.stringify({ 'panel.notes': true }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
    );
    const { isEnabled, toggle, loaded } = await freshFlags();
    await vi.waitFor(() => expect(loaded.value).toBe(true));

    await toggle('panel.notes');

    expect(isEnabled('panel.notes')).toBe(true);
    // Server is authoritative — no lingering local override.
    const saved = JSON.parse(localStorage.getItem('regelrecht-feature-flags') || '{}');
    expect('panel.notes' in saved).toBe(false);
  });
});
