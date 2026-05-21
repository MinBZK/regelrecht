import { computed, ref } from 'vue';
import { clearLawCache } from './useLaw.js';

const trajects = ref([]);
const activeTrajectId = ref(null);
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
    const [listResp, activeResp] = await Promise.all([
      fetch('/api/trajects'),
      fetch('/api/session/active-traject'),
    ]);
    if (listResp.ok) {
      trajects.value = await listResp.json();
    }
    if (activeResp.ok) {
      const body = await activeResp.json();
      activeTrajectId.value = body.traject_id || null;
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

export async function switchTraject(trajectId) {
  const resp = await fetch('/api/session/active-traject', {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ traject_id: trajectId }),
  });
  if (!resp.ok) throw new Error(`Failed to switch traject: ${resp.status}`);
  const body = await resp.json();
  activeTrajectId.value = body.traject_id || null;

  // After a successful switch the read scope on the server changed —
  // GET /api/corpus/laws/... now serves the new traject's branch
  // content (or the global view when the active id was cleared). Drop
  // every cached law-content entry so the next fetch hits the API.
  //
  // Stay on the current route: LibraryApp and EditorApp watch
  // `activeTrajectId` and re-fetch the open law (or surface a 404 when
  // the law isn't part of the new traject). Navigating away on every
  // switch destroyed that context — the user landed on the library
  // overview even when they were halfway through editing.
  clearLawCache();
  return activeTrajectId.value;
}

export async function createTraject(payload) {
  const resp = await fetch('/api/trajects', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(text || `Create failed: ${resp.status}`);
  }
  const created = await resp.json();
  await refreshTrajects();
  await switchTraject(created.id);
  return created;
}

export function useTrajects() {
  ensureTrajectsReady();
  const activeTraject = computed(() =>
    trajects.value.find((t) => t.id === activeTrajectId.value) || null,
  );
  return {
    trajects,
    activeTrajectId,
    activeTraject,
    loading,
    error,
    switchTraject,
    createTraject,
    refreshTrajects,
  };
}
