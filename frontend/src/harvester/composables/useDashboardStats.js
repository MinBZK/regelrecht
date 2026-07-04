import { ref, onUnmounted } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';

// Polls the harvester dashboard-stats endpoint. Mirrors usePollingFetch's
// semantics (loading only on first load, keep stale data on poll failures, 401
// handled by the global apiAuthGuard) but returns a single stats object instead
// of the `{ data: [], total }` paginated shape usePollingFetch is built around.
export function useDashboardStats(options = {}) {
  const { interval = 20_000 } = options;

  const stats = ref(null);
  const loading = ref(true);
  const error = ref(null);

  let timer = null;
  let initialLoad = true;

  async function refresh() {
    if (initialLoad) loading.value = true;
    try {
      const response = await apiFetch('/api/harvest-admin/dashboard-stats', {
        allowStatuses: [401],
      });
      // 401 → redirect handled globally; return before flashing an error.
      if (response.status === 401) return;
      stats.value = await response.json();
      error.value = null;
    } catch (err) {
      console.error('Failed to fetch dashboard stats:', err);
      error.value = err.message;
      // Keep the last good stats on poll failures.
    } finally {
      loading.value = false;
      initialLoad = false;
    }
  }

  function startPolling() {
    stopPolling();
    timer = setInterval(refresh, interval);
  }

  function stopPolling() {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  }

  onUnmounted(stopPolling);

  // Initial load + start polling.
  refresh();
  startPolling();

  return { stats, loading, error, refresh, startPolling, stopPolling };
}
