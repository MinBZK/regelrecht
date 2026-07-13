<script setup>
import { computed } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';
import { useDashboardStats } from '../composables/useDashboardStats.js';
import { useJobDetail } from '../composables/useJobDetail.js';
import { JOB_STATUSES } from '../constants.js';
import { formatNumber, formatStatus } from '../formatters.js';
import DataTable from '../components/DataTable.vue';
import DailyJobsChart from '../components/DailyJobsChart.vue';
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
// titled section: status breakdown + executed counts next to the daily chart.
// `daily` is absent while an older API is deployed - then chartEntries stays
// empty and the chart simply doesn't render.
const typePanels = computed(() => {
  const s = stats.value;
  if (!s) return [];
  const daily = s.daily ?? [];
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
    chartEntries: daily.map((d) => ({ date: d.date, ...d[key] })),
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
    <!-- Top-level totals: same half/half rhythm as the per-type sections
         below, so every card on the page shares one width -->
    <nldd-one-half-one-half-section>
      <nldd-card v-for="(kpi, i) in topKpis" :key="kpi.label" :slot="i === 0 ? 'left' : 'right'">
        <nldd-container padding="16">
          <nldd-title size="2">
            <span slot="overline">{{ kpi.label }}</span>
            {{ formatNumber(kpi.value) }}
          </nldd-title>
        </nldd-container>
      </nldd-card>
    </nldd-one-half-one-half-section>

    <!-- Per type (harvest/enrich) one titled section: details next to the
         daily chart -->
    <nldd-one-half-one-half-section v-for="panel in typePanels" :key="panel.key">
      <nldd-title slot="header" size="6"><h3>{{ panel.label }}</h3></nldd-title>

      <nldd-card slot="left">
        <nldd-container padding="16">
          <nldd-title size="3">
            {{ formatNumber(panel.total) }}
            <span slot="subtitle">jobs totaal</span>
          </nldd-title>

          <nldd-spacer size="16"></nldd-spacer>

          <nldd-list variant="simple">
            <nldd-list-item v-for="item in panel.statuses" :key="item.status">
              <nldd-cell width="fit-content">
                <StatusBadge :status="item.status" size="md" />
              </nldd-cell>
              <nldd-text-cell :text="formatNumber(item.count)" horizontal-alignment="right" />
            </nldd-list-item>
          </nldd-list>

          <nldd-spacer size="12"></nldd-spacer>
          <nldd-divider></nldd-divider>
          <nldd-spacer size="12"></nldd-spacer>

          <nldd-list variant="simple">
            <nldd-list-item>
              <nldd-text-cell text="Uitgevoerd vandaag" color="secondary" />
              <nldd-text-cell :text="formatNumber(panel.today)" horizontal-alignment="right" />
            </nldd-list-item>
            <nldd-list-item>
              <nldd-text-cell text="Afgelopen 7 dagen" color="secondary" />
              <nldd-text-cell :text="formatNumber(panel.last7d)" horizontal-alignment="right" />
            </nldd-list-item>
          </nldd-list>
        </nldd-container>
      </nldd-card>

      <DailyJobsChart
        v-if="panel.chartEntries.length"
        slot="right"
        :title="'Per dag'"
        :entries="panel.chartEntries"
      />
    </nldd-one-half-one-half-section>

    <!-- Recent failures -->
    <nldd-simple-section>
      <nldd-title slot="header" size="6"><h3>Gefaalde jobs</h3></nldd-title>
    </nldd-simple-section>
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
