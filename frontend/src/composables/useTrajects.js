import { computed, ref } from 'vue';
import { useRoute } from 'vue-router';
import { apiFetch, apiFetchJson } from '../lib/apiFetch.js';

const trajects = ref([]);
const loading = ref(true);
const error = ref(null);

let readyPromise = null;

async function loadTrajects() {
  // Reset on each call so refresh-path consumers (createTraject →
  // refreshTrajects) see the menu flip back to its loading placeholder
  // while the new list is in flight, instead of holding the stale label
  // for the duration of the round-trip.
  loading.value = true;
  try {
    // Raw fetch + ok-branch on purpose: a non-ok status (e.g. 401 on a
    // public page) keeps the previous list without setting `error` —
    // only a network failure surfaces through the catch.
    const resp = await fetch('/api/trajects');
    if (resp.ok) {
      trajects.value = await resp.json();
    }
  } catch (e) {
    error.value = e;
  } finally {
    loading.value = false;
  }
}

export function ensureTrajectsReady() {
  if (!readyPromise) {
    readyPromise = loadTrajects();
  }
  return readyPromise;
}

export async function refreshTrajects() {
  readyPromise = loadTrajects();
  return readyPromise;
}

export async function createTraject(payload) {
  const created = await apiFetchJson('/api/trajects', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
    errorMessage: (status, body) => body || `Create failed: ${status}`,
  });
  await refreshTrajects();
  return created;
}

// Owner-only hard delete (backend: DELETE /api/trajects/:id → 204). The
// upstream branch on GitHub is deliberately left untouched by the backend.
export async function deleteTraject(trajectId) {
  await apiFetch(`/api/trajects/${encodeURIComponent(trajectId)}`, {
    method: 'DELETE',
    errorMessage: (status, body) => body || `Delete failed: ${status}`,
  });
  await refreshTrajects();
}

// A contributor leaves a traject (backend: POST /api/trajects/:id/leave). Mirror
// of deleteTraject from the member's side: refresh the list so the traject they
// just left drops out immediately.
export async function leaveTraject(trajectId) {
  await apiFetch(`/api/trajects/${encodeURIComponent(trajectId)}/leave`, {
    method: 'POST',
    errorMessage: (status, body) => body || `Verlaten mislukt: ${status}`,
  });
  await refreshTrajects();
}

// Active traject lives in `route.params.trajectRef` (per-tab state),
// derived here so consumers do not each repeat the lookup. Returns
// `null` for any route without a traject param — that's the "global
// browse" mode where edits are not available. The ref is the URL form
// `{slug}-{8hex}` returned on `t.ref`; the backend resolver looks up
// the matching traject by the trailing 8 hex chars of its UUID.
export function useTrajects() {
  ensureTrajectsReady();
  const route = useRoute();
  const activeTrajectRef = computed(() => route.params.trajectRef || null);
  // Guard against `t.ref` being null/undefined: the backend serialises
  // it as `null` when a `TrajectSummary` is built without calling
  // `fill_ref()` (defensive contract — see editor-api/trajects.rs).
  // Skip those rather than risk a `null === null` match against a
  // missing `activeTrajectRef`.
  const activeTraject = computed(
    () => trajects.value.find((t) => t.ref && t.ref === activeTrajectRef.value) || null,
  );
  return {
    trajects,
    activeTrajectRef,
    activeTraject,
    loading,
    error,
    createTraject,
    refreshTrajects,
  };
}
