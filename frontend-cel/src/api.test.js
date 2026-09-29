import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '@regelrecht/frontend-shared/apiFetch.js';
import { errorText, inspectionApi, processApi } from './api.js';

function response(status, body) {
  return new Response(body === undefined ? null : JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('errorText', () => {
  it('reads `error` from the response', () => {
    expect(errorText(400, '{"error":"een nummer heeft acht cijfers"}')).toBe(
      'een nummer heeft acht cijfers',
    );
  });

  it('falls back to the status without `error`', () => {
    expect(errorText(502, 'Bad Gateway')).toBe('HTTP 502');
    expect(errorText(500, '{}')).toBe('HTTP 500');
  });
});

describe('request', () => {
  it('returns the JSON of a successful response', async () => {
    const fetch = vi.fn().mockResolvedValue(response(200, { role: 'aanvrager' }));
    vi.stubGlobal('fetch', fetch);
    await expect(processApi('p 1').session()).resolves.toEqual({ role: 'aanvrager' });
    expect(fetch).toHaveBeenCalledWith(
      '/processes/p%201/api/session',
      expect.objectContaining({ method: 'GET', credentials: 'same-origin' }),
    );
  });

  it('returns null on 204', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(response(204)));
    await expect(processApi('p').logout('k')).resolves.toBeNull();
  });

  it('logs in through the channel', async () => {
    const fetch = vi.fn().mockResolvedValue(response(200, { role: 'r' }));
    vi.stubGlobal('fetch', fetch);
    await processApi('p').login('k 1', { nummer: '1' });
    expect(fetch).toHaveBeenCalledWith(
      '/processes/p/api/channels/k%201/login',
      expect.objectContaining({ method: 'POST', body: '{"nummer":"1"}' }),
    );
  });

  it('sends an action to its own route', async () => {
    const fetch = vi.fn().mockResolvedValue(response(200, { takeable: true }));
    vi.stubGlobal('fetch', fetch);
    await processApi('p').trialAction('Z 1', 'betalen', { bedrag: 100 });
    expect(fetch).toHaveBeenCalledWith(
      '/processes/p/api/cases/Z%201/actions/betalen/trial',
      expect.objectContaining({ method: 'POST', body: '{"form":{"bedrag":100}}' }),
    );
  });

  it('reports a fact that happened with happened: true', async () => {
    const fetch = vi.fn().mockResolvedValue(response(201, { gram: {} }));
    vi.stubGlobal('fetch', fetch);
    await processApi('p').takeAction('Z', 'betalen', { bedrag: 1 }, true);
    expect(fetch).toHaveBeenCalledWith(
      '/processes/p/api/cases/Z/actions/betalen',
      expect.objectContaining({ body: '{"form":{"bedrag":1},"happened":true}' }),
    );
  });

  it('reads a cell through the inspection of a process', async () => {
    const fetch = vi.fn().mockImplementation(async () => response(200, []));
    vi.stubGlobal('fetch', fetch);
    await inspectionApi('p 1', 'c').chronicle();
    expect(fetch).toHaveBeenCalledWith('/processes/p%201/api/inspection/c/chronicle', expect.anything());
    await inspectionApi('p', 'c').lexostatus('case_state', { root: 'Z 1' });
    expect(fetch).toHaveBeenLastCalledWith(
      '/processes/p/api/inspection/c/lexostatus/case_state?root=Z+1',
      expect.anything(),
    );
  });

  it('throws an ApiError with the text from `error` and the status', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(response(409, { error: 'al besloten' })));
    const error = await processApi('p').takeAction('Z-1', 'besluit', {}).catch((e) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect(error.message).toBe('al besloten');
    expect(error.status).toBe(409);
  });
});
