import { ensureAuthReady, useAuth } from '../composables/useAuth.js';

// Global 401 interceptor for the editor SPA.
//
// The backend runs a BFF/session model: the browser holds an opaque HttpOnly
// session cookie, never a token. When the session expires the backend answers
// API calls with 401. Without this guard each call site surfaces a raw "401"
// error; with it, a 401 on an `/api/*` call bounces the user through
// `/auth/login?return_url=<current path>` so they re-authenticate (silently if
// the Keycloak SSO session is still alive) and land back where they were.
//
// The router already does this for *navigation* (router.js `beforeEach`); this
// closes the gap for fetch calls fired while a page is open. We reuse the same
// `useAuth().login()` redirect so there is one auth path, not two.
//
// Deliberately NOT handled here:
// - 403 (authenticated but missing role) is not a re-login case — redirecting
//   would loop. Call sites show their own "no access" message.
// - Only same-origin `/api/*` responses are intercepted, so `/auth/*` (incl.
//   `/auth/status`), `/data/*`, `/wasm/*` and cross-origin fetches pass through.
// - 401s on sessions that were never authenticated. Public pages legitimately
//   call `/api/*` endpoints that 401 without a session (e.g. `/library` loads
//   `/api/favorites`, which the editor tolerates as "no favorites"), and local
//   dev runs with OIDC disabled. We only redirect a session that loaded
//   authenticated against a configured IdP and then got a 401 — i.e. its
//   session expired. This mirrors the router guard's `oidcConfigured` gate.

/**
 * True when the fetch target is a same-origin `/api/...` URL.
 * Accepts the same input shapes as `fetch`: string, URL, or Request.
 * @param {RequestInfo | URL} input
 * @returns {boolean}
 */
export function isApiUrl(input) {
  const raw = input instanceof Request ? input.url : input;
  let url;
  try {
    url = new URL(raw, window.location.origin);
  } catch {
    return false;
  }
  return url.origin === window.location.origin && url.pathname.startsWith('/api/');
}

// Module-level latch: once a 401 triggers the login redirect we are leaving the
// page, so further 401s from in-flight calls must not fire a second redirect.
let redirecting = false;

/**
 * Wrap `window.fetch` so a 401 on a same-origin `/api/*` call redirects to the
 * OIDC login. Idempotent-safe to call once at app start (before mount).
 */
export function installApiAuthGuard() {
  const originalFetch = window.fetch.bind(window);

  window.fetch = async (input, init) => {
    const response = await originalFetch(input, init);
    if (response.status === 401 && !redirecting && isApiUrl(input)) {
      // Resolve auth status before deciding: an early `/api/*` 401 can race the
      // `/auth/status` check, leaving the refs at their defaults otherwise.
      await ensureAuthReady();
      const { authenticated, oidcConfigured, login } = useAuth();
      // Only an expired session (loaded authenticated, IdP configured) redirects.
      // Anonymous visitors on public pages and OIDC-off dev fall through.
      if (oidcConfigured.value && authenticated.value && !redirecting) {
        redirecting = true;
        // No explicit returnUrl: the navigation has already committed, so
        // window.location reflects the page the user is actually on — exactly
        // where they should return after re-auth.
        login();
      }
    }
    return response;
  };
}
