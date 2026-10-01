import { describe, expect, it, vi } from 'vitest';
import { fragmentCache, fragmentPath, stepLabel } from './why.js';

describe('fragmentPath', () => {
  it('an article', () => {
    expect(fragmentPath({ law: 'wet_op_de_politieke_partijen#102' })).toBe(
      '/law/wet_op_de_politieke_partijen/102',
    );
  });
  it('an article with a colon in its number', () => {
    expect(fragmentPath({ law: 'algemene_wet_bestuursrecht#4:2' })).toBe('/law/algemene_wet_bestuursrecht/4%3A2');
  });
  it('a configuration with an anchor', () => {
    expect(fragmentPath({ config: 'stream/napp_aanvragen', anchor: 'aanvraag_ontvangen' })).toBe(
      '/config/stream/napp_aanvragen?anchor=aanvraag_ontvangen',
    );
  });
  it('nothing to open', () => {
    expect(fragmentPath({})).toBeNull();
  });
});

describe('stepLabel', () => {
  it('names the article or the configuration', () => {
    expect(stepLabel({ source: { law: 'uitvoering_napp#4b' } })).toBe('uitvoering_napp#4b');
    expect(stepLabel({ source: { config: 'process', anchor: 'portal' } })).toBe('process: portal');
  });
});

describe('fragmentCache', () => {
  it('fetches only when asked, once per source', async () => {
    const fetcher = vi.fn(async (path) => ({ path }));
    const cache = fragmentCache(fetcher);
    expect(fetcher).not.toHaveBeenCalled();
    const a = await cache({ law: 'x#1' });
    const b = await cache({ law: 'x#1' });
    expect(a).toEqual({ path: '/law/x/1' });
    expect(b).toBe(a);
    expect(fetcher).toHaveBeenCalledTimes(1);
  });
  it('forgets a failure, so a retry fetches again', async () => {
    const fetcher = vi.fn().mockRejectedValueOnce(new Error('weg')).mockResolvedValueOnce({ ok: 1 });
    const cache = fragmentCache(fetcher);
    await expect(cache({ law: 'x#1' })).rejects.toThrow('weg');
    await expect(cache({ law: 'x#1' })).resolves.toEqual({ ok: 1 });
  });
});
