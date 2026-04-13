import { ref } from 'vue';

const authenticated = ref(false);
const oidcConfigured = ref(false);
const person = ref(null);
const loading = ref(true);

let initialized = false;

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

export function useAuth() {
  if (!initialized) {
    initialized = true;
    checkAuth();
  }

  function login() {
    const returnUrl = window.location.pathname + window.location.search;
    window.location.href = '/auth/login?return_url=' + encodeURIComponent(returnUrl);
  }

  function logout() {
    window.location.href = '/auth/logout';
  }

  return { authenticated, oidcConfigured, person, loading, login, logout };
}
