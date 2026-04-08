<script setup>
import StatusBadge from './StatusBadge.vue';
import TableToolbar from './TableToolbar.vue';
import { formatDate, formatCoverageScore, truncateUuid } from '../formatters.js';

const props = defineProps({
  columns: { type: Array, required: true },
  data: { type: Array, required: true },
  loading: { type: Boolean, default: false },
  error: { type: String, default: null },
  sort: { type: String, default: '' },
  order: { type: String, default: 'desc' },
  filters: { type: Object, default: () => ({}) },
  clickableRows: { type: Boolean, default: false },
});

const emit = defineEmits(['sort', 'filter-change', 'row-click']);

function sortLabel(col) {
  if (!col.sortable) return col.label;
  if (props.sort !== col.key) return col.label;
  return `${col.label} ${props.order === 'asc' ? '\u2191' : '\u2193'}`;
}

function formatCellValue(value, key) {
  if (value === null || value === undefined || value === '') return null;
  if (key === 'id') return truncateUuid(value);
  if (key === 'coverage_score') return formatCoverageScore(value);
  if (key.endsWith('_at')) return formatDate(value);
  return String(value);
}
</script>

<template>
  <ndd-simple-section>
    <TableToolbar
      :columns="columns"
      :sort="sort"
      :order="order"
      :filters="filters"
      @sort="(key) => emit('sort', key)"
      @filter-change="(key, value) => emit('filter-change', key, value)"
    />

    <ndd-inline-dialog v-if="loading && data.length === 0" text="Loading…"></ndd-inline-dialog>
    <ndd-inline-dialog v-else-if="error && data.length === 0" :text="'Failed to load data: ' + error"></ndd-inline-dialog>
    <ndd-inline-dialog v-else-if="data.length === 0" text="No data found"></ndd-inline-dialog>

    <ndd-list v-else variant="simple">
      <!-- Header row -->
      <ndd-list-item size="sm">
        <ndd-title-cell
          v-for="col in columns"
          :key="col.key"
          :text="sortLabel(col)"
        ></ndd-title-cell>
        <slot name="extra-header" />
      </ndd-list-item>

      <!-- Data rows -->
      <slot name="rows" :data="data" :columns="columns">
        <ndd-list-item
          v-for="row in data"
          :key="row.id || row.law_id"
          size="md"
          :type="clickableRows ? 'button' : undefined"
          @click="clickableRows && emit('row-click', row)"
        >
          <template v-for="col in columns" :key="col.key">
            <ndd-cell width="stretch">
              <slot :name="'cell-' + col.key" :row="row" :value="row[col.key]">
                <StatusBadge v-if="col.key === 'status'" :status="row[col.key] || 'unknown'" />
                <span v-else-if="col.key === 'id'" class="cell-mono" :title="String(row[col.key])">
                  {{ formatCellValue(row[col.key], col.key) }}
                </span>
                <span v-else-if="col.key === 'law_id'" class="cell-mono">
                  {{ row[col.key] }}
                </span>
                <template v-else-if="formatCellValue(row[col.key], col.key) !== null">
                  {{ formatCellValue(row[col.key], col.key) }}
                </template>
                <span v-else class="cell-null">&mdash;</span>
              </slot>
            </ndd-cell>
          </template>
        </ndd-list-item>
      </slot>
    </ndd-list>
  </ndd-simple-section>
</template>
