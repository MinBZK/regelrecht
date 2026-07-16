/**
 * useTasks - de open taken van de ingelogde gebruiker (module-singleton).
 *
 * Poll-patroon van useTrajectDocumentJobs (keep-stale on failure), maar met
 * één gedeelde module-state zoals useBwbHarvest: het sidebar-item (badge) en
 * het taken-panel kijken naar dezelfde lijst. Interval bewust ruim (30s) - taken
 * zijn laagfrequent; na eigen acties (enrich-aanvraag, resolve) wordt direct
 * ge-refreshed.
 */
import { ref, computed, onUnmounted, getCurrentInstance } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';

const POLL_INTERVAL_MS = 30_000;

// Module-level shared state - all callers of useTasks() share these refs.
const tasks = ref([]);
const openCount = ref(0);
// Lopende taak-flow-aanvragen (pending/processing) - de "Bezig"-sectie.
// Zelfde keep-stale-gedrag als tasks/openCount: een poll-fout laat 'm staan.
const running = ref([]);
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
    running.value = Array.isArray(json?.running) ? json.running : [];
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
  // Increment/register-unmount are both gated on an active component
  // instance, and both need the SAME gate: a caller without one (e.g. a
  // plain module/test context) never decrements either, so counting it in
  // would leak a phantom consumer that keeps the poll interval alive
  // forever. Non-component callers therefore don't influence the poll
  // lifecycle at all - use `useTaskActions()` below if you don't want to
  // join it in the first place.
  const hasInstance = !!getCurrentInstance();
  if (hasInstance) consumers += 1;
  if (!timer) {
    // Defer the initial load a microtask tick so a caller that immediately
    // fires its own request (e.g. requestEnrich right after useTasks()) gets
    // to make its apiFetch call first - keeps this composable's eager load
    // from racing a caller's own synchronous follow-up call for the mock/
    // fetch queue.
    Promise.resolve().then(refresh);
    timer = setInterval(refresh, POLL_INTERVAL_MS);
  }
  if (hasInstance) {
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
    running,
    error,
    openCount: computed(() => openCount.value),
    refresh,
    fetchTask,
    resolveTask,
    requestEnrich,
  };
}

// useTaskActions - the non-polling half of useTasks(): action helpers only,
// no shared tasks/openCount state and no consumer/timer registration. For
// callers that only need to fetch/resolve/enrich a task (e.g. EditorView's
// review mode and "Verrijk deze wet" action) and would otherwise start the
// 30s poll unconditionally in setup() - including for anonymous visitors -
// which breaks the invariant that anonymous visitors never poll. Callers
// that DO want the shared, polled task list (the taken-lijst in Home:
// TasksSidebarItem/TasksPane) keep using useTasks().
export function useTaskActions() {
  return { fetchTask, resolveTask, requestEnrich, refresh };
}
