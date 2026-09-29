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
  it('leest `error` uit het antwoord', () => {
    expect(foutTekst(400, '{"error":"een nummer heeft acht cijfers"}')).toBe(
      'een nummer heeft acht cijfers',
    );
  });

  it('valt terug op de status zonder `error`', () => {
    expect(foutTekst(502, 'Bad Gateway')).toBe('HTTP 502');
    expect(foutTekst(500, '{}')).toBe('HTTP 500');
  });
});

describe('vraag', () => {
  it('geeft de JSON van een geslaagd antwoord', async () => {
    const fetch = vi.fn().mockResolvedValue(antwoord(200, { role: 'aanvrager' }));
    vi.stubGlobal('fetch', fetch);
    await expect(procesApi('p 1').sessie()).resolves.toEqual({ role: 'aanvrager' });
    expect(fetch).toHaveBeenCalledWith(
      '/processes/p%201/api/session',
      expect.objectContaining({ method: 'GET', credentials: 'same-origin' }),
    );
  });

  it('geeft null bij 204', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(antwoord(204)));
    await expect(procesApi('p').uitloggen('k')).resolves.toBeNull();
  });

  it('logt in langs het kanaal', async () => {
    const fetch = vi.fn().mockResolvedValue(antwoord(200, { role: 'r' }));
    vi.stubGlobal('fetch', fetch);
    await procesApi('p').inloggen('k 1', { nummer: '1' });
    expect(fetch).toHaveBeenCalledWith(
      '/processes/p/api/channels/k%201/login',
      expect.objectContaining({ method: 'POST', body: '{"nummer":"1"}' }),
    );
  });

  it('stuurt een handeling naar haar eigen route', async () => {
    const fetch = vi.fn().mockResolvedValue(antwoord(200, { takeable: true }));
    vi.stubGlobal('fetch', fetch);
    await procesApi('p').proefhandeling('Z 1', 'betalen', { bedrag: 100 });
    expect(fetch).toHaveBeenCalledWith(
      '/processes/p/api/cases/Z%201/actions/betalen/trial',
      expect.objectContaining({ method: 'POST', body: '{"form":{"bedrag":100}}' }),
    );
  });

  it('meldt een gebeurd feit met happened: true', async () => {
    const fetch = vi.fn().mockResolvedValue(antwoord(201, { gram: {} }));
    vi.stubGlobal('fetch', fetch);
    await procesApi('p').handeling('Z', 'betalen', { bedrag: 1 }, true);
    expect(fetch).toHaveBeenCalledWith(
      '/processes/p/api/cases/Z/actions/betalen',
      expect.objectContaining({ body: '{"form":{"bedrag":1},"happened":true}' }),
    );
  });

  it('leest een cel via de inzage van een proces', async () => {
    const fetch = vi.fn().mockImplementation(async () => antwoord(200, []));
    vi.stubGlobal('fetch', fetch);
    await inzageApi('p 1', 'c').kroniek();
    expect(fetch).toHaveBeenCalledWith('/processes/p%201/api/inspection/c/chronicle', expect.anything());
    await inzageApi('p', 'c').lexostatus('case_state', { root: 'Z 1' });
    expect(fetch).toHaveBeenLastCalledWith(
      '/processes/p/api/inspection/c/lexostatus/case_state?root=Z+1',
      expect.anything(),
    );
  });

  it('gooit een ApiError met de tekst uit `error` en de status', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(antwoord(409, { error: 'al besloten' })));
    const fout = await procesApi('p').handeling('Z-1', 'besluit', {}).catch((e) => e);
    expect(fout).toBeInstanceOf(ApiError);
    expect(fout.message).toBe('al besloten');
    expect(fout.status).toBe(409);
  });
});
