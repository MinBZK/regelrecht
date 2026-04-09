import { ref, reactive, computed } from 'vue';
import { usePollingFetch } from './usePollingFetch.js';
import { LAW_ENTRY_SORT_KEYS } from '../constants.js';

export function useLawEntries() {
  const sort = ref('updated_at');
  const order = ref('desc');
  const limit = ref(50);
  const offset = ref(0);
  const filters = reactive({});

  function buildUrl() {
    const params = new URLSearchParams();
    if (LAW_ENTRY_SORT_KEYS.has(sort.value)) {
      params.set('sort', sort.value);
    }
    params.set('order', order.value === 'asc' ? 'asc' : 'desc');
    params.set('limit', String(limit.value));
    params.set('offset', String(offset.value));
    for (const [key, value] of Object.entries(filters)) {
      if (value) params.set(key, value);
    }
    return `api/law_entries?${params.toString()}`;
  }

  const { data, totalCount, loading, error, refresh, startPolling, stopPolling } =
    usePollingFetch(buildUrl);

  function setSort(key) {
    if (!LAW_ENTRY_SORT_KEYS.has(key)) return;
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
    refresh();
  }

  function goToPage(page) {
    const maxPage = Math.max(1, Math.ceil(totalCount.value / limit.value));
    const clamped = Math.max(1, Math.min(page, maxPage));
    offset.value = (clamped - 1) * limit.value;
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
    currentPage, totalPages,
    setSort, setFilter, goToPage,
    refresh, startPolling, stopPolling,
  };
}
