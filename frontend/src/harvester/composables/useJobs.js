import { ref, reactive, computed } from 'vue';
import { usePollingFetch } from './usePollingFetch.js';
import { JOB_SORT_KEYS, GROUPED_SORT_KEYS } from '../constants.js';

export function useJobs(options = {}) {
  const initialView = options.initialViewMode === 'flat' ? 'flat' : 'grouped';
  const sort = ref(initialView === 'flat' ? 'created_at' : 'latest_created_at');
  const order = ref('desc');
  const limit = ref(50);
  const offset = ref(0);
  const filters = reactive({});
  const viewMode = ref(initialView);

  function buildUrl() {
    const allowedKeys = viewMode.value === 'grouped' ? GROUPED_SORT_KEYS : JOB_SORT_KEYS;
    const params = new URLSearchParams();
    if (allowedKeys.has(sort.value)) {
      params.set('sort', sort.value);
    }
    params.set('order', order.value === 'asc' ? 'asc' : 'desc');
    params.set('limit', String(limit.value));
    params.set('offset', String(offset.value));
    for (const [key, value] of Object.entries(filters)) {
      if (value) params.set(key, value);
    }
    const endpoint =
      viewMode.value === 'grouped'
        ? '/api/harvest-admin/jobs/summary'
        : '/api/harvest-admin/jobs';
    return `${endpoint}?${params.toString()}`;
  }

  const { data, totalCount, loading, error, refresh: rawRefresh, startPolling, stopPolling, reset } =
    usePollingFetch(buildUrl);

  async function refresh() {
    await rawRefresh();
  }

  async function toggleViewMode() {
    viewMode.value = viewMode.value === 'grouped' ? 'flat' : 'grouped';
    offset.value = 0;
    sort.value = viewMode.value === 'grouped' ? 'latest_created_at' : 'created_at';
    order.value = 'desc';
    if (viewMode.value === 'grouped') {
      delete filters.law_id;
    }
    reset();
    await refresh();
  }

  function setSort(key, newOrder) {
    const allowedKeys = viewMode.value === 'grouped' ? GROUPED_SORT_KEYS : JOB_SORT_KEYS;
    if (!allowedKeys.has(key)) return;
    if (newOrder === 'asc' || newOrder === 'desc') {
      sort.value = key;
      order.value = newOrder;
    } else if (sort.value === key) {
      order.value = order.value === 'asc' ? 'desc' : 'asc';
    } else {
      sort.value = key;
      order.value = 'desc';
    }
    offset.value = 0;
    refresh();
  }

  function setFilter(key, value) {
    if (value) {
      filters[key] = value;
    } else {
      delete filters[key];
    }
    offset.value = 0;
    refresh();
  }

  function goToPage(page) {
    const maxPage = Math.max(1, Math.ceil(totalCount.value / limit.value));
    const clamped = Math.max(1, Math.min(page, maxPage));
    offset.value = (clamped - 1) * limit.value;
    refresh();
  }

  function setLawIdFilter(lawId) {
    for (const key of Object.keys(filters)) delete filters[key];
    if (viewMode.value === 'flat') {
      filters.law_id = lawId;
    }
    offset.value = 0;
    refresh();
  }

  const currentPage = computed(() => Math.floor(offset.value / limit.value) + 1);
  const totalPages = computed(() => Math.max(1, Math.ceil(totalCount.value / limit.value)));

  // Initial load + start polling
  refresh();
  startPolling();

  return {
    data, totalCount, loading, error,
    sort, order, limit, offset, filters,
    viewMode,
    currentPage, totalPages,
    setSort, setFilter, goToPage,
    toggleViewMode, setLawIdFilter,
    refresh, startPolling, stopPolling,
  };
}
