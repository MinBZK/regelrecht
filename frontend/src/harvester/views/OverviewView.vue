<script setup>
import { computed } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';
import { useDashboardStats } from '../composables/useDashboardStats.js';
import { useJobDetail } from '../composables/useJobDetail.js';
import { JOB_STATUSES } from '../constants.js';
import { formatNumber, formatStatus } from '../formatters.js';
import DataTable from '../components/DataTable.vue';
import DetailPanel from '../components/DetailPanel.vue';
import StatusBadge from '../components/StatusBadge.vue';

const { stats, loading, error } = useDashboardStats();
const { job: detailJob, isOpen: detailOpen, open: openDetail, close: closeDetail } =
  useJobDetail();

// Columns for the recent-failures list. No `sortable`/`filter` on any column
// (and no sortOptions passed to DataTable) so the table toolbar renders empty.
const FAILED_COLUMNS = [
  { key: 'law_id', label: 'Law ID', minWidth: '160px' },
  { key: 'job_type', label: 'Type', width: 'fit-content', text: (row) => formatStatus(row.job_type) },
  { key: 'failed_at', label: 'Wanneer', width: 'fit-content' },
  { key: 'error', label: 'Reden', minWidth: '240px', text: (row) => row.error },
];

const kpiCards = computed(() => {
  const s = stats.value;
  if (!s) return [];
  return [
    { label: 'Jobs totaal', value: s.jobs.total },
    { label: 'Harvest', value: s.jobs.by_type.harvest },
    { label: 'Enrich', value: s.jobs.by_type.enrich },
    { label: 'Open untranslatables', value: s.open_untranslatables },
  ];
});

const statusItems = computed(() => {
  const byStatus = stats.value?.jobs.by_status || {};
  return JOB_STATUSES.map((status) => ({ status, count: byStatus[status] ?? 0 }));
});

const windowCards = computed(() => {
  const e = stats.value?.executed;
  if (!e) return [];
  return [
    { label: 'Vandaag', ...e.today },
    { label: 'Afgelopen 7 dagen', ...e.last_7d },
  ];
});

// Failed rows only carry a subset of the job; fetch the full job so the detail
// sheet shows the complete info. Fall back to the summary row (with the reason)
// if the fetch fails.
async function onFailureClick(row) {
  try {
    const resp = await apiFetch(`/api/harvest-admin/jobs/${encodeURIComponent(row.id)}`);
    openDetail(await resp.json());
  } catch {
    openDetail({
      ...row,
      status: 'failed',
      completed_at: row.failed_at,
      result: { error: row.error },
    });
  }
}
</script>

<template>
  <nldd-simple-section v-if="loading && !stats">
    <nldd-inline-dialog text="Laden…"></nldd-inline-dialog>
  </nldd-simple-section>
  <nldd-simple-section v-else-if="error && !stats">
    <nldd-inline-dialog variant="alert" :text="'Kan dashboard niet laden: ' + error"></nldd-inline-dialog>
  </nldd-simple-section>

  <template v-else-if="stats">
    <!-- KPI cards -->
    <nldd-simple-section>
      <div class="overview-kpis">
        <nldd-card v-for="kpi in kpiCards" :key="kpi.label" class="overview-kpi">
          <nldd-container padding="16">
            <div class="overview-kpi__value">{{ formatNumber(kpi.value) }}</div>
            <div class="overview-kpi__label">{{ kpi.label }}</div>
          </nldd-container>
        </nldd-card>
      </div>
    </nldd-simple-section>

    <!-- Jobs per status -->
    <nldd-simple-section>
      <nldd-title slot="header" size="6"><h3>Jobs per status</h3></nldd-title>
      <div class="overview-badges">
        <span v-for="item in statusItems" :key="item.status" class="overview-badge">
          <StatusBadge :status="item.status" size="md" />
          <span class="overview-badge__count">{{ formatNumber(item.count) }}</span>
        </span>
      </div>
    </nldd-simple-section>

    <!-- Executed windows -->
    <nldd-simple-section>
      <nldd-title slot="header" size="6"><h3>Uitgevoerd</h3></nldd-title>
      <div class="overview-kpis">
        <nldd-card v-for="win in windowCards" :key="win.label" class="overview-kpi">
          <nldd-container padding="16">
            <div class="overview-kpi__value">{{ formatNumber(win.total) }}</div>
            <div class="overview-kpi__label">{{ win.label }}</div>
            <div class="overview-kpi__sub">
              Harvest {{ formatNumber(win.harvest) }} · Enrich {{ formatNumber(win.enrich) }}
            </div>
          </nldd-container>
        </nldd-card>
      </div>
    </nldd-simple-section>

    <!-- Recent failures -->
    <nldd-title class="overview-failures-title" size="6"><h3>Gefaalde jobs</h3></nldd-title>
    <DataTable
      :columns="FAILED_COLUMNS"
      :data="stats.recent_failures"
      :clickable-rows="true"
      empty-text="Geen gefaalde jobs"
      empty-supporting-text="Er zijn geen recente mislukte jobs"
      @row-click="onFailureClick"
    />

    <DetailPanel :job="detailJob" :is-open="detailOpen" @close="closeDetail" />
  </template>
</template>
