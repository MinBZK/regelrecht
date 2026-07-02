import { ref, onUnmounted } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';

export function usePollingFetch(buildUrl, options = {}) {
  const { interval = 20_000 } = options;

  const data = ref([]);
  const totalCount = ref(0);
  const loading = ref(true);
  const error = ref(null);

  let timer = null;
  let initialLoad = true;

  async function fetchData() {
    const url = buildUrl();
    if (!url) return;

    // Only show loading state on initial load, not on poll refreshes
    if (initialLoad) loading.value = true;

    try {
      // 401 is handled by the editor's global apiAuthGuard (redirect to
      // login); apiFetch throws ApiError on other non-ok statuses, caught below.
      const response = await apiFetch(url);
      const json = await response.json();
      data.value = json.data || [];
      totalCount.value = json.total ?? data.value.length;
      error.value = null;
    } catch (err) {
      console.error('Failed to fetch data:', err);
      // Only clear data on initial load errors; keep stale data on poll failures
      if (initialLoad) {
        data.value = [];
        totalCount.value = 0;
      }
      error.value = err.message;
    } finally {
      loading.value = false;
      initialLoad = false;
    }
  }

  function startPolling() {
    stopPolling();
    timer = setInterval(fetchData, interval);
  }

  function stopPolling() {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  }

  async function refresh() {
    await fetchData();
  }

  function reset() {
    initialLoad = true;
    data.value = [];
    totalCount.value = 0;
    error.value = null;
    loading.value = true;
  }

  onUnmounted(stopPolling);

  return { data, totalCount, loading, error, refresh, startPolling, stopPolling, reset };
}
