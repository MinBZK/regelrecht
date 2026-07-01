import { ref } from 'vue';
import { apiFetch, apiFetchJson } from './apiFetch.js';

// Per-user GitHub link status (spike). Shape mirrors the editor-api
// `/auth/github/status` response:
//   { connected, configured, github_login?, scopes?, expired, required }
// `configured: false` means the deployment has no GitHub OAuth App wired up,
// so the UI should hide the whole affordance.
const status = ref(null);
const loading = ref(true);

let readyPromise = null;

async function fetchStatus() {
  try {
    status.value = await apiFetchJson('/auth/github/status');
  } catch {
    // Endpoint missing/errored (e.g. auth-disabled dev mode) — treat as
    // unconfigured so the UI simply hides the GitHub controls.
    status.value = { connected: false, configured: false, expired: false, required: false };
  } finally {
    loading.value = false;
  }
}

export function ensureGithubReady() {
  if (!readyPromise) {
    readyPromise = fetchStatus();
  }
  return readyPromise;
}

export function useGithubAuth() {
  ensureGithubReady();

  // Redirect the browser into the GitHub consent flow. `returnUrl` is a
  // same-origin relative path the backend validates and bounces back to with
  // a `?github=connected|error|denied` marker. Guards against being passed a
  // bare event object (`@click="connect"`).
  function connect(returnUrl) {
    const url =
      typeof returnUrl === 'string' && returnUrl
        ? returnUrl
        : window.location.pathname + window.location.search + window.location.hash;
    window.location.href = '/auth/github/login?return_url=' + encodeURIComponent(url);
  }

  async function disconnect() {
    await apiFetch('/auth/github/disconnect', { method: 'POST' });
    await refresh();
  }

  // Force a re-fetch of the shared status (after connect/disconnect, or when a
  // save reported a 428 that may have been resolved in another tab).
  async function refresh() {
    readyPromise = null;
    return ensureGithubReady();
  }

  return { status, loading, connect, disconnect, refresh };
}
