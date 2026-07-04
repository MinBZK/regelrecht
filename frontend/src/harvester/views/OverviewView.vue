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

// Top-level totals shown as standalone KPI cards; the per-type detail lives in
// the panels below.
const topKpis = computed(() => {
  const s = stats.value;
  if (!s) return [];
  return [
    { label: 'Jobs totaal', value: s.jobs.total },
    { label: 'Open untranslatables', value: s.open_untranslatables },
  ];
});

// Harvest and enrich are treated as two separate kinds, each with its own
// status breakdown and executed counts.
const typePanels = computed(() => {
  const s = stats.value;
  if (!s) return [];
  return [
    { key: 'harvest', label: 'Harvest' },
    { key: 'enrich', label: 'Enrich' },
  ].map(({ key, label }) => ({
    key,
    label,
    total: s.jobs.by_type[key] ?? 0,
    statuses: JOB_STATUSES.map((status) => ({
      status,
      count: s.jobs.by_type_status[key]?.[status] ?? 0,
    })),
    today: s.executed.today[key] ?? 0,
    last7d: s.executed.last_7d[key] ?? 0,
  }));
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
    <!-- Top-level totals -->
    <nldd-simple-section>
      <div class="overview-kpis">
        <nldd-card v-for="kpi in topKpis" :key="kpi.label" class="overview-kpi">
          <nldd-container padding="16">
            <div class="overview-kpi__value">{{ formatNumber(kpi.value) }}</div>
            <div class="overview-kpi__label">{{ kpi.label }}</div>
          </nldd-container>
        </nldd-card>
      </div>
    </nldd-simple-section>

    <!-- Per-type detail: harvest and enrich as two separate kinds -->
    <nldd-simple-section>
      <div class="overview-types">
        <nldd-card v-for="panel in typePanels" :key="panel.key" class="overview-type">
          <nldd-container padding="16">
            <nldd-title size="6"><h3>{{ panel.label }}</h3></nldd-title>
            <div class="overview-type__total">
              {{ formatNumber(panel.total) }}
              <span class="overview-type__total-label">jobs totaal</span>
            </div>
            <div class="overview-badges">
              <span v-for="item in panel.statuses" :key="item.status" class="overview-badge">
                <StatusBadge :status="item.status" size="md" />
                <span class="overview-badge__count">{{ formatNumber(item.count) }}</span>
              </span>
            </div>
            <div class="overview-type__executed">
              Uitgevoerd — Vandaag {{ formatNumber(panel.today) }} ·
              Afgelopen 7 dagen {{ formatNumber(panel.last7d) }}
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
