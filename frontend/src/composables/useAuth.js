import { ref } from 'vue';

const authenticated = ref(false);
const oidcConfigured = ref(false);
const person = ref(null);
const loading = ref(true);

let readyPromise = null;

async function checkAuth() {
  try {
    const response = await fetch('/auth/status');
    if (!response.ok) return;
    const status = await response.json();
    authenticated.value = status.authenticated;
    oidcConfigured.value = status.oidc_configured;
    person.value = status.person || null;
  } catch {
    // Auth endpoint not available — treat as no auth configured
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

  function login() {
    const returnUrl = window.location.pathname + window.location.search + window.location.hash;
    window.location.href = '/auth/login?return_url=' + encodeURIComponent(returnUrl);
  }

  function logout() {
    window.location.href = '/auth/logout';
  }

  return { authenticated, oidcConfigured, person, loading, login, logout };
}
