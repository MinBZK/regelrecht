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
  it('a configuration without an anchor', () => {
    expect(fragmentPath({ config: 'form' })).toBe('/config/form');
  });
  it('nothing to open', () => {
    expect(fragmentPath({})).toBeNull();
    expect(fragmentPath(undefined)).toBeNull();
  });
  it('the whole file of an article or a configuration', () => {
    expect(fragmentPath({ law: 'algemene_wet_bestuursrecht#4:2', whole: true })).toBe(
      '/law/algemene_wet_bestuursrecht',
    );
    expect(fragmentPath({ config: 'stream/napp_aanvragen', anchor: 'aanvraag_ontvangen', whole: true })).toBe(
      '/config/stream/napp_aanvragen',
    );
  });
  it('a law without an article is nothing to open', () => {
    expect(fragmentPath({ law: 'uitvoering_napp' })).toBeNull();
    expect(fragmentPath({ law: 'uitvoering_napp#' })).toBeNull();
  });
});

describe('stepLabel', () => {
  it('names the article or the configuration', () => {
    expect(stepLabel({ source: { law: 'uitvoering_napp#4b' } })).toBe('uitvoering_napp#4b');
    expect(stepLabel({ source: { config: 'process', anchor: 'portal' } })).toBe('process: portal');
    expect(stepLabel({ source: { config: 'form' } })).toBe('form');
  });
  it('a step without a source has no label', () => {
    expect(stepLabel({})).toBe('');
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
  it('shares one fetch between concurrent calls', async () => {
    let resolve;
    const fetcher = vi.fn(() => new Promise((r) => (resolve = r)));
    const cache = fragmentCache(fetcher);
    const a = cache({ law: 'x#1' });
    const b = cache({ law: 'x#1' });
    resolve({ ok: 1 });
    expect(await a).toBe(await b);
    expect(fetcher).toHaveBeenCalledTimes(1);
  });
  it('fetches nothing for a source without a fragment', async () => {
    const fetcher = vi.fn();
    await expect(fragmentCache(fetcher)({ law: 'x' })).resolves.toBeNull();
    expect(fetcher).not.toHaveBeenCalled();
  });
  it('forgets a failure, so a retry fetches again', async () => {
    const fetcher = vi.fn().mockRejectedValueOnce(new Error('weg')).mockResolvedValueOnce({ ok: 1 });
    const cache = fragmentCache(fetcher);
    await expect(cache({ law: 'x#1' })).rejects.toThrow('weg');
    await expect(cache({ law: 'x#1' })).resolves.toEqual({ ok: 1 });
  });
});
