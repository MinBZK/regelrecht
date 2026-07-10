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
});
