<script setup>
import TableToolbar from './TableToolbar.vue';
import { GROUPED_COLUMNS } from '../constants.js';
import { formatDate } from '../formatters.js';

defineProps({
  data: { type: Array, required: true },
  loading: { type: Boolean, default: false },
  error: { type: String, default: null },
  sort: { type: String, default: '' },
  order: { type: String, default: 'desc' },
  filters: { type: Object, default: () => ({}) },
  emptyText: { type: String, default: 'No data found' },
  sortOptions: { type: Array, default: null },
});

const emit = defineEmits(['sort', 'filter-change', 'view-jobs']);

const columns = GROUPED_COLUMNS;

const STATUS_BAR_KEYS = ['completed', 'failed', 'processing', 'pending'];

function statusSegments(group) {
  const total = group.total_jobs || 0;
  if (total === 0) return [];
  return STATUS_BAR_KEYS
    .map((key) => ({ key, count: group[key] || 0 }))
    .filter((seg) => seg.count > 0)
    .map((seg) => ({ ...seg, percent: (seg.count / total) * 100 }));
}

function statusBarTitle(group) {
  return STATUS_BAR_KEYS
    .filter((key) => group[key] > 0)
    .map((key) => `${group[key]} ${key}`)
    .join(' · ');
}
</script>

<template>
  <nldd-simple-section width="full">
    <TableToolbar
      :columns="columns"
      :sort-options="sortOptions"
      :sort="sort"
      :order="order"
      :filters="filters"
      @sort="(key, order) => emit('sort', key, order)"
      @filter-change="(key, value) => emit('filter-change', key, value)"
    >
      <template #prefix>
        <slot name="toolbar-prefix" />
      </template>
    </TableToolbar>
    <nldd-spacer size="16" />

    <nldd-inline-dialog v-if="loading && data.length === 0" text="Loading…"></nldd-inline-dialog>
    <nldd-inline-dialog v-else-if="error && data.length === 0" :text="'Failed to load data: ' + error"></nldd-inline-dialog>
    <nldd-inline-dialog v-else-if="data.length === 0" :text="emptyText">
      <slot name="empty-action" />
    </nldd-inline-dialog>

    <nldd-list v-else variant="simple">
      <!-- Group rows -->
      <nldd-list-item
        v-for="group in data"
        :key="group.law_id"
        size="md"
        type="button"
        @click="emit('view-jobs', group.law_id)"
      >
        <template v-for="(col, idx) in columns" :key="col.key">
          <nldd-spacer-cell v-if="idx > 0" size="12" />
          <nldd-cell
            v-if="col.key === 'status_bar'"
            :width="col.width || 'stretch'"
          >
            <nldd-tooltip :text="statusBarTitle(group)" placement="top">
              <div class="status-bar">
                <div
                  v-for="seg in statusSegments(group)"
                  :key="seg.key"
                  :class="['status-bar__segment', `status-bar__segment--${seg.key}`]"
                  :style="{ width: seg.percent + '%' }"
                />
              </div>
            </nldd-tooltip>
          </nldd-cell>
          <nldd-text-cell
            v-else-if="col.text"
            :text="col.text(group)"
            :overline="col.overline ? col.overline(group) : undefined"
            :supporting-text="col.supportingText ? col.supportingText(group) : undefined"
            :width="col.width || 'stretch'"
            :min-width="col.minWidth"
            :horizontal-alignment="col.align"
          />
          <nldd-text-cell
            v-else
            :text="group[col.key] != null ? String(group[col.key]) : '—'"
            :color="group[col.key] != null ? 'default' : 'secondary'"
            :width="col.width || 'stretch'"
            :min-width="col.minWidth"
            :horizontal-alignment="col.align"
          />
        </template>
        <nldd-spacer-cell size="12" />
        <nldd-icon-cell icon="chevron-right" size="20" />
      </nldd-list-item>
    </nldd-list>

    <slot name="pagination" />
  </nldd-simple-section>
</template>
