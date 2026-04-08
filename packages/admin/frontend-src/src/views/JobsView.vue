<script setup>
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { useJobs } from '../composables/useJobs.js';
import { useJobDetail } from '../composables/useJobDetail.js';
import { JOB_COLUMNS } from '../constants.js';
import DataTable from '../components/DataTable.vue';
import GroupedTable from '../components/GroupedTable.vue';
import DetailPanel from '../components/DetailPanel.vue';
import JobCreation from '../components/JobCreation.vue';
import StatusBadge from '../components/StatusBadge.vue';
import PaginationControls from '../components/PaginationControls.vue';
import { truncateError } from '../formatters.js';

const route = useRoute();

const {
  data, totalCount, loading, error,
  sort, order, filters,
  viewMode, expandedLawIds, expandedJobsCache,
  currentPage, totalPages,
  setSort, setFilter, prevPage, nextPage,
  toggleViewMode, toggleGroupExpansion, setLawIdFilter,
  refresh,
} = useJobs();

const { job: detailJob, isOpen: detailOpen, open: openDetail, close: closeDetail } = useJobDetail();

const paginationUnit = computed(() => viewMode.value === 'grouped' ? 'laws' : 'results');

// Handle incoming law_id query param (runs synchronously during setup)
if (route.query.law_id) {
  setLawIdFilter(route.query.law_id);
}
</script>

<template>
  <Teleport to="#view-toggle-target" defer>
    <ndd-button
      variant="neutral-tinted"
      size="md"
      :title="viewMode === 'grouped' ? 'Show individual jobs' : 'Group jobs by law'"
      @click="toggleViewMode"
    >{{ viewMode === 'grouped' ? 'Flat view' : 'Grouped view' }}</ndd-button>
  </Teleport>

  <Teleport to="#pagination-target" defer>
    <PaginationControls
      :current-page="currentPage"
      :total-pages="totalPages"
      :total-count="totalCount"
      :unit="paginationUnit"
      @prev="prevPage"
      @next="nextPage"
    />
  </Teleport>

  <JobCreation @job-created="refresh" />

  <!-- Flat view -->
  <DataTable
    v-if="viewMode === 'flat'"
    :columns="JOB_COLUMNS"
    :data="data"
    :loading="loading"
    :error="error"
    :sort="sort"
    :order="order"
    :filters="filters"
    :clickable-rows="true"
    @sort="setSort"
    @filter-change="setFilter"
    @row-click="openDetail"
  >
    <template #cell-status="{ row }">
      <StatusBadge :status="row.status || 'unknown'" />
    </template>
    <template #cell-_error="{ row }">
      <span
        v-if="row.result && row.result.error"
        class="cell-error"
        :title="row.result.error"
      >{{ truncateError(row.result.error) }}</span>
      <span v-else class="cell-null">&mdash;</span>
    </template>
  </DataTable>

  <!-- Grouped view -->
  <GroupedTable
    v-else
    :data="data"
    :loading="loading"
    :error="error"
    :sort="sort"
    :order="order"
    :filters="filters"
    :expanded-law-ids="expandedLawIds"
    :expanded-jobs-cache="expandedJobsCache"
    @sort="setSort"
    @filter-change="setFilter"
    @toggle-expand="toggleGroupExpansion"
    @row-click="openDetail"
  />

  <DetailPanel :job="detailJob" :is-open="detailOpen" @close="closeDetail" />
</template>
