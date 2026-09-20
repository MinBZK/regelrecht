<script setup>
import { ref } from 'vue';
import { useMarkings } from '../composables/useMarkings.js';
import { useMarkingClusters } from '../composables/useMarkingClusters.js';
import { MARKING_COLUMNS, MARKING_SORT_OPTIONS } from '../constants.js';
import DataTable from '../components/DataTable.vue';
import StatusBadge from '../components/StatusBadge.vue';
import PaginationControls from '../components/PaginationControls.vue';
import MarkingDetailPanel from '../components/MarkingDetailPanel.vue';
import MarkingClusters from '../components/MarkingClusters.vue';

const {
  data, loading, error,
  sort, order, filters,
  currentPage, totalPages,
  setSort, setFilter, goToPage, filterParams,
} = useMarkings();

// The backlog reads the same filtered set as the list below it.
const {
  data: clusters,
  loading: clustersLoading,
  refresh: refreshClusters,
} = useMarkingClusters(filterParams);

// Read-only detail: no polling needed, unlike the jobs sheet.
const selected = ref(null);
const detailOpen = ref(false);

function openDetail(row) {
  selected.value = row;
  detailOpen.value = true;
}

function closeDetail() {
  detailOpen.value = false;
}

// Clicking a cluster filters the list to it, which is how a reader gets from
// "this change is wanted in four places" to the four articles themselves.
//
// Both fields, because `resolution` has two possible values: filtering on it
// alone lands the reader on half the corpus rather than on those four
// articles. `resolved_by` is what identifies the cluster.
//
// A cluster that names no change (`resolved_by` is nullable) can only be
// narrowed by resolution, which is honest: there is nothing else to match on.
function focusCluster(cluster) {
  setFilter('resolution', cluster.resolution);
  setFilter('resolved_by', cluster.resolved_by || '');
  refreshClusters();
}

// Every filter change has to reach the clusters too, or the backlog describes
// a different set than the rows under it until the next poll tick. A count
// that groups something other than the visible rows is worse than no count:
// nothing about the page looks wrong while it happens.
function onFilterChange(key, value) {
  setFilter(key, value);
  refreshClusters();
}
</script>

<template>
  <MarkingClusters
    :clusters="clusters"
    :loading="clustersLoading"
    @select="focusCluster"
  />

  <DataTable
    :columns="MARKING_COLUMNS"
    :sort-options="MARKING_SORT_OPTIONS"
    :data="data"
    :loading="loading"
    :error="error"
    :sort="sort"
    :order="order"
    :filters="filters"
    :clickable-rows="true"
    empty-text="Geen markeringen"
    empty-supporting-text="Ze verschijnen hier zodra een verrijking een constructie markeert die het formaat niet kan uitdrukken"
    @sort="setSort"
    @filter-change="onFilterChange"
    @row-click="openDetail"
  >
    <template #cell-resolution="{ row }">
      <nldd-text-cell
        :text="row.resolution === 'operation' ? 'bewerking' : 'formaat'"
        :supporting-text="
          row.resolution === 'operation'
            ? 'een bewerking ontbreekt'
            : 'het formaat mist een vorm'
        "
      />
    </template>
    <template #cell-target="{ row }">
      <!-- An empty target is a claim, not a blank: the article still runs. -->
      <nldd-text-cell
        :text="row.target && row.target.length ? String(row.target.length) : '—'"
        :supporting-text="
          row.target && row.target.length ? row.target.join(', ') : 'artikel blijft werken'
        "
      />
    </template>
    <template #cell-accepted="{ row }">
      <StatusBadge :status="row.accepted ? 'accepted' : 'open'" />
    </template>
    <template #pagination>
      <PaginationControls
        :current-page="currentPage"
        :total-pages="totalPages"
        @page-change="goToPage"
      />
    </template>
  </DataTable>

  <MarkingDetailPanel :row="selected" :is-open="detailOpen" @close="closeDetail" />
</template>
