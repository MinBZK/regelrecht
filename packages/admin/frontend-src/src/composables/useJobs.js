import { ref, reactive, computed } from 'vue';
import { usePollingFetch } from './usePollingFetch.js';
import { JOB_SORT_KEYS, GROUPED_SORT_KEYS } from '../constants.js';

export function useJobs() {
  const sort = ref('latest_created_at');
  const order = ref('desc');
  const limit = ref(50);
  const offset = ref(0);
  const filters = reactive({});
  const viewMode = ref('grouped');
  const expandedLawIds = reactive(new Set());
  const expandedJobsCache = reactive({});

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
    const endpoint = viewMode.value === 'grouped' ? 'api/jobs/summary' : 'api/jobs';
    return `${endpoint}?${params.toString()}`;
  }

  const { data, totalCount, loading, error, refresh: rawRefresh, startPolling, stopPolling, reset } =
    usePollingFetch(buildUrl);

  async function refresh() {
    await rawRefresh();
    // Re-fetch expanded groups after main data refresh
    if (viewMode.value === 'grouped' && expandedLawIds.size > 0) {
      await Promise.all([...expandedLawIds].map((lawId) => fetchJobsForLaw(lawId)));
    }
  }

  async function fetchJobsForLaw(lawId) {
    try {
      const params = new URLSearchParams();
      params.set('law_id', lawId);
      params.set('sort', 'created_at');
      params.set('order', 'desc');
      params.set('limit', '50');
      if (filters.status) params.set('status', filters.status);
      if (filters.job_type) params.set('job_type', filters.job_type);

      const response = await fetch(`api/jobs?${params.toString()}`);
      if (!response.ok) {
        expandedJobsCache[lawId] = [];
        return;
      }
      const json = await response.json();
      expandedJobsCache[lawId] = json.data || [];
    } catch {
      expandedJobsCache[lawId] = [];
    }
  }

  async function toggleGroupExpansion(lawId) {
    if (expandedLawIds.has(lawId)) {
      expandedLawIds.delete(lawId);
      delete expandedJobsCache[lawId];
    } else {
      expandedLawIds.add(lawId);
      await fetchJobsForLaw(lawId);
    }
  }

  async function toggleViewMode() {
    viewMode.value = viewMode.value === 'grouped' ? 'flat' : 'grouped';
    offset.value = 0;
    sort.value = viewMode.value === 'grouped' ? 'latest_created_at' : 'created_at';
    order.value = 'desc';
    expandedLawIds.clear();
    for (const key of Object.keys(expandedJobsCache)) {
      delete expandedJobsCache[key];
    }
    if (viewMode.value === 'grouped') {
      delete filters.law_id;
    }
    reset();
    await refresh();
  }

  function setSort(key) {
    const allowedKeys = viewMode.value === 'grouped' ? GROUPED_SORT_KEYS : JOB_SORT_KEYS;
    if (!allowedKeys.has(key)) return;
    if (sort.value === key) {
      order.value = order.value === 'asc' ? 'desc' : 'asc';
    } else {
      sort.value = key;
      order.value = 'asc';
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
    // Clear expanded jobs cache so child rows re-fetch with new filters
    for (const k of Object.keys(expandedJobsCache)) {
      delete expandedJobsCache[k];
    }
    refresh();
  }

  function goToPage(page) {
    const maxPage = Math.max(1, Math.ceil(totalCount.value / limit.value));
    const clamped = Math.max(1, Math.min(page, maxPage));
    offset.value = (clamped - 1) * limit.value;
    refresh();
  }

  function setLawIdFilter(lawId) {
    if (viewMode.value === 'grouped') {
      // In grouped view, auto-expand this law instead of filtering
      for (const key of Object.keys(filters)) delete filters[key];
      expandedLawIds.clear();
      for (const key of Object.keys(expandedJobsCache)) delete expandedJobsCache[key];
      expandedLawIds.add(lawId);
      offset.value = 0;
      reset();
      refresh();
    } else {
      for (const key of Object.keys(filters)) delete filters[key];
      filters.law_id = lawId;
      offset.value = 0;
      refresh();
    }
  }

  const currentPage = computed(() => Math.floor(offset.value / limit.value) + 1);
  const totalPages = computed(() => Math.max(1, Math.ceil(totalCount.value / limit.value)));

  // Initial load + start polling
  refresh();
  startPolling();

  return {
    data, totalCount, loading, error,
    sort, order, limit, offset, filters,
    viewMode, expandedLawIds, expandedJobsCache,
    currentPage, totalPages,
    setSort, setFilter, goToPage,
    toggleViewMode, toggleGroupExpansion, setLawIdFilter,
    refresh, startPolling, stopPolling,
  };
}
