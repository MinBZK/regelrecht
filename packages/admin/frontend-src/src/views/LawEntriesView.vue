<script setup>
import { useRouter } from 'vue-router';
import { useLawEntries } from '../composables/useLawEntries.js';
import { LAW_ENTRY_COLUMNS } from '../constants.js';
import DataTable from '../components/DataTable.vue';
import StatusBadge from '../components/StatusBadge.vue';
import RowActions from '../components/RowActions.vue';
import PaginationControls from '../components/PaginationControls.vue';

const router = useRouter();

const {
  data, totalCount, loading, error,
  sort, order, filters,
  currentPage, totalPages,
  setSort, setFilter, goToPage, refresh,
} = useLawEntries();

function viewJobsForLaw(lawId) {
  router.push({ name: 'jobs', query: { law_id: lawId } });
}
</script>

<template>
  <DataTable
    :columns="LAW_ENTRY_COLUMNS"
    :data="data"
    :loading="loading"
    :error="error"
    :sort="sort"
    :order="order"
    :filters="filters"
    @sort="setSort"
    @filter-change="setFilter"
  >
    <template #cell-law_id="{ row }">
      <a class="cell-mono law-id-link" href="#" @click.prevent="viewJobsForLaw(row.law_id)">
        {{ row.law_id }}
      </a>
    </template>
    <template #cell-status="{ row }">
      <StatusBadge :status="row.status || 'unknown'" />
    </template>
    <template #cell-_actions="{ row }">
      <RowActions :row="row" @action-complete="refresh" />
    </template>
  </DataTable>

  <PaginationControls
    :current-page="currentPage"
    :total-pages="totalPages"
    :total-count="totalCount"
    @page-change="goToPage"
  />
</template>
