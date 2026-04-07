import { ref } from 'vue';

const authenticated = ref(false);
const person = ref(null);
const oidcConfigured = ref(false);
const loading = ref(true);
let fetched = false;

async function checkAuth() {
  try {
    const response = await fetch('/auth/status');
    if (!response.ok) return;
    const status = await response.json();
    authenticated.value = status.authenticated;
    oidcConfigured.value = status.oidc_configured;
    person.value = status.person || null;
  } catch {
    // Auth check failed — leave as unauthenticated
  } finally {
    loading.value = false;
  }
}

function logout() {
  window.location.href = '/auth/logout';
}

function redirectToLogin() {
  window.location.href = '/auth/login';
}

export function useAuth() {
  if (!fetched) {
    fetched = true;
    checkAuth();
  }

  return { authenticated, person, oidcConfigured, loading, logout, redirectToLogin };
}
