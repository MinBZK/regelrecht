import { ref } from 'vue';
import { apiFetchJson } from './apiFetch.js';

const authenticated = ref(false);
const oidcConfigured = ref(false);
const person = ref(null);
const loading = ref(true);

let readyPromise = null;

async function checkAuth() {
  try {
    const status = await apiFetchJson('/auth/status');
    authenticated.value = status.authenticated;
    oidcConfigured.value = status.oidc_configured;
    person.value = status.person || null;
  } catch {
    // Auth endpoint not available or errored — treat as no auth configured
  } finally {
    loading.value = false;
  }
}

// Kick off the single shared /auth/status fetch and expose its promise so
// callers outside the Vue component tree (e.g. router guards) can await it
// without touching the reactive `loading` ref.
export function ensureAuthReady() {
  if (!readyPromise) {
    readyPromise = checkAuth();
  }
  return readyPromise;
}

export function useAuth() {
  ensureAuthReady();

  // Accepts an explicit return URL so callers that know the user's intended
  // destination (e.g. a router guard firing before navigation commits, where
  // `window.location` still points at the source route) can forward it to
  // SSO. Falls back to the current location for the common case.
  function login(returnUrl) {
    // Only accept an explicit string: a template that passes `login` as a
    // bare event handler (`@click="login"`) hands us a PointerEvent, which
    // would otherwise stringify into return_url=[object PointerEvent].
    const url = typeof returnUrl === 'string' && returnUrl
      ? returnUrl
      : window.location.pathname + window.location.search + window.location.hash;
    window.location.href = '/auth/login?return_url=' + encodeURIComponent(url);
  }

  function logout() {
    window.location.href = '/auth/logout';
  }

  return { authenticated, oidcConfigured, person, loading, login, logout };
}
