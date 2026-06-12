import { describe, it, expect, vi } from 'vitest';
import { useEngine } from './useEngine.js';

describe('useEngine composable', () => {
  it('exposes expected API surface', () => {
    const api = useEngine();
    expect(typeof api.getDemoIndex).toBe('function');
    expect(typeof api.getProfile).toBe('function');
    expect(typeof api.evaluate).toBe('function');
    expect(api.loading.value).toBe(false);
    expect(api.error.value).toBeNull();
  });

  it('fetches demo-index from /demo-assets/demo-index.json', async () => {
    const mockIndex = { laws: [{ id: 'zt', path: 'x', output: 'y', label: 'Z', summary: 's' }] };
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve(mockIndex),
    });
    const { getDemoIndex } = useEngine();
    const idx = await getDemoIndex();
    expect(idx).toEqual(mockIndex);
    expect(fetchSpy).toHaveBeenCalledWith('/demo-assets/demo-index.json');
    fetchSpy.mockRestore();
  });
});
