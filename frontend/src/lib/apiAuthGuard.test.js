import { describe, it, expect, beforeEach, vi } from 'vitest';

// Hoisted shared state: the login spy and the mutable auth refs the guard reads.
const { loginSpy, authState } = vi.hoisted(() => ({
  loginSpy: vi.fn(),
  authState: { authenticated: true, oidcConfigured: true },
}));

vi.mock('../composables/useAuth.js', () => ({
  ensureAuthReady: vi.fn().mockResolvedValue(undefined),
  useAuth: () => ({
    // Read authState at call time so a test can set it before the 401 fires.
    authenticated: { value: authState.authenticated },
    oidcConfigured: { value: authState.oidcConfigured },
    login: loginSpy,
  }),
}));

import { isApiUrl } from './apiAuthGuard.js';

// happy-dom serves window.location.origin === 'http://localhost' by default.
const ORIGIN = window.location.origin;

/**
 * Install the guard over a stubbed original fetch that resolves to `status`.
 * `auth` overrides the mocked authenticated/oidcConfigured refs (default: an
 * authenticated session against a configured IdP). A fresh module instance per
 * call resets the redirect latch.
 */
async function freshGuard(status, auth = {}) {
  vi.resetModules();
  loginSpy.mockClear();
  authState.authenticated = auth.authenticated ?? true;
  authState.oidcConfigured = auth.oidcConfigured ?? true;
  const original = vi.fn().mockResolvedValue({ status });
  window.fetch = original;
  const { installApiAuthGuard: install } = await import('./apiAuthGuard.js');
  install();
  return { wrapped: window.fetch, original };
}

describe('isApiUrl', () => {
  it('matches same-origin /api/ paths (string, URL, Request)', () => {
    expect(isApiUrl('/api/trajects')).toBe(true);
    expect(isApiUrl(`${ORIGIN}/api/trajects`)).toBe(true);
    expect(isApiUrl(new URL('/api/foo', ORIGIN))).toBe(true);
    expect(isApiUrl(new Request(`${ORIGIN}/api/foo`))).toBe(true);
  });

  it('rejects /auth/*, /data/*, /wasm/* and other same-origin paths', () => {
    expect(isApiUrl('/auth/status')).toBe(false);
    expect(isApiUrl('/auth/login')).toBe(false);
    expect(isApiUrl('/data/annotations/x.yaml')).toBe(false);
    expect(isApiUrl('/wasm/pkg/engine.js')).toBe(false);
    expect(isApiUrl('/library')).toBe(false);
  });

  it('rejects cross-origin /api/ paths', () => {
    expect(isApiUrl('https://evil.example/api/x')).toBe(false);
  });
});

describe('installApiAuthGuard', () => {
  beforeEach(() => {
    loginSpy.mockClear();
  });

  it('redirects on a 401 from an /api/ call for an authenticated session', async () => {
    const { wrapped } = await freshGuard(401);
    await wrapped('/api/trajects');
    expect(loginSpy).toHaveBeenCalledTimes(1);
  });

  it('does NOT redirect an unauthenticated session (anonymous public-page 401)', async () => {
    // e.g. /library loading /api/favorites without a session — tolerated, no bounce.
    const { wrapped } = await freshGuard(401, { authenticated: false });
    await wrapped('/api/favorites');
    expect(loginSpy).not.toHaveBeenCalled();
  });

  it('does NOT redirect when OIDC is disabled (local dev)', async () => {
    const { wrapped } = await freshGuard(401, { oidcConfigured: false, authenticated: false });
    await wrapped('/api/trajects');
    expect(loginSpy).not.toHaveBeenCalled();
  });

  it('does NOT redirect on a 403 (missing role, not a re-login case)', async () => {
    const { wrapped } = await freshGuard(403);
    await wrapped('/api/trajects');
    expect(loginSpy).not.toHaveBeenCalled();
  });

  it('does NOT redirect on a 401 from /auth/status', async () => {
    const { wrapped } = await freshGuard(401);
    await wrapped('/auth/status');
    expect(loginSpy).not.toHaveBeenCalled();
  });

  it('does NOT redirect on a 401 from a cross-origin /api/ call', async () => {
    const { wrapped } = await freshGuard(401);
    await wrapped('https://other.example/api/x');
    expect(loginSpy).not.toHaveBeenCalled();
  });

  it('redirects only once for multiple concurrent 401s (loop guard)', async () => {
    const { wrapped } = await freshGuard(401);
    await Promise.all([wrapped('/api/a'), wrapped('/api/b'), wrapped('/api/c')]);
    expect(loginSpy).toHaveBeenCalledTimes(1);
  });

  it('passes the original response through unchanged on success', async () => {
    const { wrapped, original } = await freshGuard(200);
    const res = await wrapped('/api/trajects', { method: 'POST' });
    expect(res.status).toBe(200);
    expect(original).toHaveBeenCalledWith('/api/trajects', { method: 'POST' });
    expect(loginSpy).not.toHaveBeenCalled();
  });
});
