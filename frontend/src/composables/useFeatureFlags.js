/**
 * useFeatureFlags - singleton feature flag store with API sync.
 *
 * Fetches flags from /api/feature-flags on first use, falls back to
 * hardcoded defaults when the API is unavailable (e.g. no database).
 */
import { ref, readonly } from 'vue';
import { apiFetch, apiFetchJson } from '../lib/apiFetch.js';
import { useAuth } from './useAuth.js';

const DEFAULTS = {
  'panel.article_text': true,
  'panel.scenario_form': true,
  'panel.yaml_editor': true,
  'panel.machine_readable': true,
  'panel.notes': true,
  // Per-user GitHub OAuth link (spike, PR #887): gates the "Koppel
  // GitHub-account" affordance in the account menu. Off by default so the
  // spike stays invisible until a user opts in; the backend is independently
  // gated on GITHUB_OAUTH_* env vars (unconfigured deployments never show it).
  'github.user_oauth': false,
  // Taken: verrijking op aanvraag met review-taken. Off by default; same
  // allow-list rule as the flags above — the backend key must exist or the
  // toggle PUT 400s and the switch silently reverts.
  'tasks.job_review': false,
};

// Local overrides survive refresh when the backend has no persistence (dev).
const STORAGE_KEY = 'regelrecht-feature-flags';

function loadLocal() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return saved ? JSON.parse(saved) : {};
  } catch { return {}; }
}

function saveLocal(flagMap) {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(flagMap)); } catch { /* ignore */ }
}

const flags = ref({ ...DEFAULTS, ...loadLocal() });
const loaded = ref(false);

let fetchPromise = null;

// useAuth() returns module-level refs; capture oidcConfigured once at load so
// the toggle() catch below reads .value without calling a composable at runtime
// outside a setup() context.
const { oidcConfigured } = useAuth();

async function loadFlags() {
  if (fetchPromise) return fetchPromise;
  fetchPromise = (async () => {
    try {
      const server = await apiFetchJson('/api/feature-flags', {
        errorMessage: (status) => `HTTP ${status}`,
      });
      // The server (DB) is authoritative for any flag it returns, so it wins
      // over a stale local override - otherwise a client toggle saved during a
      // no-DB session would silently beat a later DB-backed (e.g. admin-set)
      // value. A local override only fills in flags the server does not define,
      // which is the no-DB (503) survival path below.
      flags.value = { ...DEFAULTS, ...loadLocal(), ...server };
    } catch (e) {
      console.warn('Failed to load feature flags, using defaults:', e.message);
      flags.value = { ...DEFAULTS, ...loadLocal() };
    } finally {
      loaded.value = true;
    }
  })();
  return fetchPromise;
}

async function toggle(key) {
  const current = flags.value[key] ?? DEFAULTS[key] ?? true;
  const newValue = !current;

  // Optimistic update, persisted locally so the toggle survives refreshes
  // whenever the backend can't accept the write (503 no-DB, or a failed write
  // handled in the catch below).
  flags.value = { ...flags.value, [key]: newValue };

  try {
    const res = await apiFetch(`/api/feature-flags/${encodeURIComponent(key)}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled: newValue }),
      allowStatuses: [503],
      errorMessage: (status) => `HTTP ${status}`,
    });
    if (res.status === 503) {
      // Backend has no DB; keep the change local so it survives refresh.
      saveLocal({ ...loadLocal(), [key]: newValue });
      return;
    }
    // Clear any stale local override for this key - server is now authoritative.
    const local = loadLocal();
    if (key in local) {
      delete local[key];
      saveLocal(local);
    }
    const updated = await res.json();
    flags.value = { ...DEFAULTS, ...loadLocal(), ...updated };
  } catch (e) {
    // A write can fail two ways. In OIDC-off dev the PUT 401s because the dev
    // session has no auth, and there is no server state to contradict an
    // override, so keep the toggle local and let panes stay switchable. With
    // OIDC configured (prod) that same 401 means a transient/expired session
    // (or the failure is offline / a 5xx); persisting it would make the
    // override sticky in localStorage and silently win over the server even
    // after re-authentication, so revert the optimistic change instead.
    if (oidcConfigured.value) {
      console.warn('Feature flag write failed; reverting (server stays authoritative):', e.message);
      flags.value = { ...flags.value, [key]: current };
    } else {
      console.warn('Feature flag write failed in no-auth dev; keeping change locally:', e.message);
      saveLocal({ ...loadLocal(), [key]: newValue });
    }
  }
}

export function useFeatureFlags() {
  if (!loaded.value && !fetchPromise) {
    loadFlags();
  }
  return {
    flags: readonly(flags),
    loaded: readonly(loaded),
    isEnabled: (key) => flags.value[key] ?? DEFAULTS[key] ?? true,
    toggle,
  };
}
