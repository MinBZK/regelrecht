import { ref, reactive, computed } from 'vue';
import { usePollingFetch } from './usePollingFetch.js';
import { MARKING_SORT_KEYS } from '../constants.js';

// Mirrors useUntranslatables: server-side paginated/sorted/filtered list,
// polled. Reads GET /api/harvest-admin/markings (editor-api proxies to the
// harvester-admin service).
//
// The filters are shared with the cluster view through `filterParams`, so the
// backlog always describes the same set of markings as the rows above it. A
// cluster count that counts something else than the list is worse than no
// count at all: nothing about the page looks wrong while it happens.
export function useMarkings() {
  const sort = ref('created_at');
  const order = ref('desc');
  const limit = ref(50);
  const offset = ref(0);
  const filters = reactive({});

  // Just the filters, without paging or sorting. The cluster endpoint groups
  // over the whole filtered set, so it takes these and nothing else.
  function filterParams() {
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(filters)) {
      if (value) params.set(key, value);
    }
    return params;
  }

  function buildUrl() {
    const params = filterParams();
    if (MARKING_SORT_KEYS.has(sort.value)) {
      params.set('sort', sort.value);
    }
    params.set('order', order.value === 'asc' ? 'asc' : 'desc');
    params.set('limit', String(limit.value));
    params.set('offset', String(offset.value));
    return `/api/harvest-admin/markings?${params.toString()}`;
  }

  const { data, totalCount, loading, error, refresh, startPolling, stopPolling } =
    usePollingFetch(buildUrl);

  function setSort(key, newOrder) {
    if (!MARKING_SORT_KEYS.has(key)) return;
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

  // Several filters in one update, and one request.
  //
  // Calling `setFilter` twice fires two un-awaited refreshes. `buildUrl()` runs
  // synchronously at the start of each fetch, so the first goes out with only
  // the filter set so far, and the two race for the same `data` ref with no
  // sequencing or cancellation in `usePollingFetch`. If the broader first
  // response lands last it overwrites the narrower one, which is the bug this
  // filter exists to avoid, now intermittent rather than constant.
  function setFilters(next) {
    for (const [key, value] of Object.entries(next)) {
      if (value) {
        filters[key] = value;
      } else {
        delete filters[key];
      }
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
    setSort, setFilter, setFilters, goToPage, filterParams,
    refresh, startPolling, stopPolling,
  };
}
