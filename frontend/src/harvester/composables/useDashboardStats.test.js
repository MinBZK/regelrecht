import { describe, it, expect, afterEach, vi } from 'vitest';

// useDashboardStats calls apiFetch, which ultimately calls the global fetch.
// Stub that (as useFeatureFlags.test.js does) rather than mocking the module.
function jsonResponse(body, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

const PAYLOAD = {
  jobs: {
    total: 3,
    by_type: { harvest: 2, enrich: 1 },
    by_status: { pending: 2, processing: 0, completed: 1, failed: 0 },
    by_type_status: {},
  },
  executed: {
    today: { total: 1, harvest: 1, enrich: 0 },
    last_7d: { total: 2, harvest: 1, enrich: 1 },
  },
  open_untranslatables: 5,
  recent_failures: [],
};

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('useDashboardStats', () => {
  it('loads and exposes the stats payload', async () => {
    vi.stubGlobal('fetch', vi.fn(() => Promise.resolve(jsonResponse(PAYLOAD))));
    const { useDashboardStats } = await import('./useDashboardStats.js');
    const { stats, loading, error, stopPolling } = useDashboardStats();

    await vi.waitFor(() => expect(stats.value).not.toBeNull());
    expect(stats.value.jobs.total).toBe(3);
    expect(stats.value.open_untranslatables).toBe(5);
    expect(loading.value).toBe(false);
    expect(error.value).toBeNull();
    stopPolling();
  });

  it('handles a 401 without throwing or setting stats', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.resolve(new Response('unauthorized', { status: 401 }))),
    );
    const { useDashboardStats } = await import('./useDashboardStats.js');
    const { stats, loading, error, stopPolling } = useDashboardStats();

    await vi.waitFor(() => expect(loading.value).toBe(false));
    expect(stats.value).toBeNull();
    expect(error.value).toBeNull();
    stopPolling();
  });
});
