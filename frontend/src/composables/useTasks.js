/**
 * useTasks - de open taken van de ingelogde gebruiker (module-singleton).
 *
 * Poll-patroon van useTrajectDocumentJobs (keep-stale on failure), maar met
 * één gedeelde module-state zoals useBwbHarvest: de topbar-badge en de
 * taken-sheet kijken naar dezelfde lijst. Interval bewust ruim (30s) - taken
 * zijn laagfrequent; na eigen acties (enrich-aanvraag, resolve) wordt direct
 * ge-refreshed.
 */
import { ref, computed, onUnmounted, getCurrentInstance } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';

const POLL_INTERVAL_MS = 30_000;

// Module-level shared state - all callers of useTasks() share these refs.
const tasks = ref([]);
const openCount = ref(0);
const error = ref(null);

let timer = null;
let consumers = 0;

async function refresh() {
  try {
    const res = await apiFetch('/api/tasks', { allowStatuses: [401] });
    if (res.status === 401) return;
    const json = await res.json();
    tasks.value = Array.isArray(json?.tasks) ? json.tasks : [];
    openCount.value = Number.isFinite(json?.open_count) ? json.open_count : tasks.value.length;
    error.value = null;
  } catch (e) {
    // Keep-stale: een poll-fout wist de badge/lijst niet.
    error.value = e;
  }
}

async function fetchTask(taskId) {
  const res = await apiFetch(`/api/tasks/${encodeURIComponent(taskId)}`);
  return res.json();
}

async function resolveTask(taskId, action) {
  await apiFetch(`/api/tasks/${encodeURIComponent(taskId)}/resolve`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ action }),
  });
  await refresh();
}

async function requestEnrich(trajectRef, lawId) {
  const res = await apiFetch(
    `/api/trajects/${encodeURIComponent(trajectRef)}/corpus/laws/${encodeURIComponent(lawId)}/enrich`,
    { method: 'POST', allowStatuses: [409, 429] }
  );
  return { alreadyRunning: res.status === 409, tooMany: res.status === 429 };
}

export function useTasks() {
  consumers += 1;
  if (!timer) {
    // Defer the initial load a microtask tick so a caller that immediately
    // fires its own request (e.g. requestEnrich right after useTasks()) gets
    // to make its apiFetch call first - keeps this composable's eager load
    // from racing a caller's own synchronous follow-up call for the mock/
    // fetch queue.
    Promise.resolve().then(refresh);
    timer = setInterval(refresh, POLL_INTERVAL_MS);
  }
  // Only register unmount cleanup when called during a component's setup();
  // useTasks() is also called from plain module/test contexts (no active
  // component instance) where onUnmounted would otherwise warn and no-op.
  if (getCurrentInstance()) {
    onUnmounted(() => {
      consumers -= 1;
      if (consumers <= 0 && timer) {
        clearInterval(timer);
        timer = null;
      }
    });
  }
  return {
    tasks,
    error,
    openCount: computed(() => openCount.value),
    refresh,
    fetchTask,
    resolveTask,
    requestEnrich,
  };
}
