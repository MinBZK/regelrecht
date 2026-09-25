import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '@regelrecht/frontend-shared/apiFetch.js';
import { foutTekst, inzageApi, procesApi } from './api.js';

function antwoord(status, body) {
  return new Response(body === undefined ? null : JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('foutTekst', () => {
  it('leest `fout` uit het antwoord', () => {
    expect(foutTekst(400, '{"fout":"een nummer heeft acht cijfers"}')).toBe(
      'een nummer heeft acht cijfers',
    );
  });

  it('valt terug op de status zonder `fout`', () => {
    expect(foutTekst(502, 'Bad Gateway')).toBe('HTTP 502');
    expect(foutTekst(500, '{}')).toBe('HTTP 500');
  });
});

describe('vraag', () => {
  it('geeft de JSON van een geslaagd antwoord', async () => {
    const fetch = vi.fn().mockResolvedValue(antwoord(200, { rol: 'aanvrager' }));
    vi.stubGlobal('fetch', fetch);
    await expect(procesApi('p 1').sessie()).resolves.toEqual({ rol: 'aanvrager' });
    expect(fetch).toHaveBeenCalledWith(
      '/processen/p%201/api/sessie',
      expect.objectContaining({ method: 'GET', credentials: 'same-origin' }),
    );
  });

  it('geeft null bij 204', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(antwoord(204)));
    await expect(procesApi('p').uitloggen('k')).resolves.toBeNull();
  });

  it('logt in langs het kanaal', async () => {
    const fetch = vi.fn().mockResolvedValue(antwoord(200, { rol: 'r' }));
    vi.stubGlobal('fetch', fetch);
    await procesApi('p').inloggen('k 1', { nummer: '1' });
    expect(fetch).toHaveBeenCalledWith(
      '/processen/p/api/kanalen/k%201/login',
      expect.objectContaining({ method: 'POST', body: '{"nummer":"1"}' }),
    );
  });

  it('stuurt een handeling naar haar eigen route', async () => {
    const fetch = vi.fn().mockResolvedValue(antwoord(200, { te_nemen: true }));
    vi.stubGlobal('fetch', fetch);
    await procesApi('p').proefhandeling('Z 1', 'betalen', { bedrag: 100 });
    expect(fetch).toHaveBeenCalledWith(
      '/processen/p/api/zaken/Z%201/handelingen/betalen/proef',
      expect.objectContaining({ method: 'POST', body: '{"formulier":{"bedrag":100}}' }),
    );
  });

  it('meldt een gebeurd feit met gebeurd: true', async () => {
    const fetch = vi.fn().mockResolvedValue(antwoord(201, { gram: {} }));
    vi.stubGlobal('fetch', fetch);
    await procesApi('p').handeling('Z', 'betalen', { bedrag: 1 }, true);
    expect(fetch).toHaveBeenCalledWith(
      '/processen/p/api/zaken/Z/handelingen/betalen',
      expect.objectContaining({ body: '{"formulier":{"bedrag":1},"gebeurd":true}' }),
    );
  });

  it('leest een cel via de inzage van een proces', async () => {
    const fetch = vi.fn().mockImplementation(async () => antwoord(200, []));
    vi.stubGlobal('fetch', fetch);
    await inzageApi('p 1', 'c').kroniek();
    expect(fetch).toHaveBeenCalledWith('/processen/p%201/api/inzage/c/kroniek', expect.anything());
    await inzageApi('p', 'c').lexostatus('zaakstand', { zaakkenmerk: 'Z 1' });
    expect(fetch).toHaveBeenLastCalledWith(
      '/processen/p/api/inzage/c/lexostatus/zaakstand?zaakkenmerk=Z+1',
      expect.anything(),
    );
  });

  it('gooit een ApiError met de tekst uit `fout` en de status', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(antwoord(409, { fout: 'al besloten' })));
    const fout = await procesApi('p').handeling('Z-1', 'besluit', {}).catch((e) => e);
    expect(fout).toBeInstanceOf(ApiError);
    expect(fout.message).toBe('al besloten');
    expect(fout.status).toBe(409);
  });
});
