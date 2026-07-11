import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref } from 'vue';

// Route the composable's network leg through a controllable spy. The
// factory closes over `apiFetchJson` so it survives `vi.resetModules()`.
const apiFetchJson = vi.fn();
vi.mock('../lib/apiFetch.js', () => ({
  apiFetchJson: (...args) => apiFetchJson(...args),
}));

// The composable keeps module-level per-scope caches; import a fresh
// copy per test so tests can't leak scope state into each other.
async function freshUseCorpusLaws() {
  vi.resetModules();
  const mod = await import('./useCorpusLaws.js');
  return mod.useCorpusLaws;
}

// Flush the microtask/watcher chain so the fetch commit lands.
async function flush() {
  await new Promise((resolve) => setTimeout(resolve));
}

beforeEach(() => {
  apiFetchJson.mockReset();
});

describe('useCorpusLaws', () => {
  it('shares one fetch between consumers of the same scope', async () => {
    const useCorpusLaws = await freshUseCorpusLaws();
    apiFetchJson.mockResolvedValue([{ law_id: 'wet_a', name: 'Wet A' }]);

    const first = useCorpusLaws(ref('tr-1'));
    const second = useCorpusLaws(ref('tr-1'));
    await flush();

    expect(apiFetchJson).toHaveBeenCalledTimes(1);
    expect(first.laws.value).toHaveLength(1);
    expect(second.displayName('wet_a')).toBe('Wet A');
  });

  it('refresh() busts the scope cache, re-fetches and commits the new list', async () => {
    const useCorpusLaws = await freshUseCorpusLaws();
    apiFetchJson.mockResolvedValueOnce([{ law_id: 'wet_a', name: 'Wet A' }]);

    const { laws, displayName, refresh } = useCorpusLaws(ref('tr-1'));
    await flush();
    expect(apiFetchJson).toHaveBeenCalledTimes(1);
    // Before the refresh the unknown law falls back to the humanized id.
    expect(displayName('wet_b')).toBe('Wet B');

    apiFetchJson.mockResolvedValueOnce([
      { law_id: 'wet_a', name: 'Wet A' },
      { law_id: 'wet_b', display_name: 'Wet B (officieel)' },
    ]);
    await refresh();

    expect(apiFetchJson).toHaveBeenCalledTimes(2);
    expect(laws.value).toHaveLength(2);
    expect(displayName('wet_b')).toBe('Wet B (officieel)');
  });

  it('joins a concurrent refresh() instead of firing a duplicate re-fetch', async () => {
    const useCorpusLaws = await freshUseCorpusLaws();
    apiFetchJson.mockResolvedValueOnce([{ law_id: 'wet_a', name: 'Wet A' }]);

    const { laws, refresh } = useCorpusLaws(ref('tr-1'));
    await flush();
    expect(apiFetchJson).toHaveBeenCalledTimes(1);

    apiFetchJson.mockResolvedValue([
      { law_id: 'wet_a', name: 'Wet A' },
      { law_id: 'wet_b', name: 'Wet B' },
    ]);
    // Two concurrent refreshes share one re-fetch (the second must not
    // orphan the first one's in-flight promise and fire a third GET).
    await Promise.all([refresh(), refresh()]);

    expect(apiFetchJson).toHaveBeenCalledTimes(2);
    expect(laws.value).toHaveLength(2);
  });

  it('refresh() during the initial in-flight fetch serializes instead of duplicating the GET', async () => {
    const useCorpusLaws = await freshUseCorpusLaws();
    const resolvers = [];
    apiFetchJson.mockImplementation(
      () => new Promise((resolve) => resolvers.push(resolve)),
    );

    const { laws, refresh } = useCorpusLaws(ref('tr-1'));
    expect(resolvers).toHaveLength(1);

    // Refresh while the initial fetch is still in flight: it must not
    // orphan that request and fire a concurrent duplicate GET.
    const refreshed = refresh();
    await flush();
    expect(resolvers).toHaveLength(1);

    // Once the initial fetch settles, the refresh busts the slot and
    // re-fetches — its result wins, never the pre-refresh list.
    resolvers[0]([{ law_id: 'wet_old' }]);
    await flush();
    expect(resolvers).toHaveLength(2);
    resolvers[1]([{ law_id: 'wet_old' }, { law_id: 'wet_new' }]);

    await refreshed;
    expect(laws.value.map((l) => l.law_id)).toEqual(['wet_old', 'wet_new']);
  });
});
