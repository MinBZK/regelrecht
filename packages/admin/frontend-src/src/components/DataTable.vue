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

function onHeaderClick(col) {
  if (col.sortable) emit('sort', col.key);
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
  <TableToolbar
    :columns="columns"
    :sort="sort"
    :order="order"
    :filters="filters"
    @sort="(key) => emit('sort', key)"
    @filter-change="(key, value) => emit('filter-change', key, value)"
  />
  <ndd-simple-section>
    <div class="table-container">
      <table class="data-table">
        <thead>
          <tr>
            <th
              v-for="col in columns"
              :key="col.key"
              :class="{
                sortable: col.sortable,
                'sort-active': sort === col.key,
              }"
              @click="onHeaderClick(col)"
            >
              <span class="th-label">
                {{ col.label }}
                <span v-if="col.sortable" class="sort-indicator">
                  {{ sort === col.key ? (order === 'asc' ? '\u25B2' : '\u25BC') : '\u25BC' }}
                </span>
              </span>
            </th>
            <slot name="extra-header" />
          </tr>
        </thead>
        <tbody>
          <tr v-if="loading && data.length === 0">
            <td :colspan="columns.length" class="table-message">Loading&hellip;</td>
          </tr>
          <tr v-else-if="error && data.length === 0">
            <td :colspan="columns.length" class="table-message table-message--error">
              Failed to load data: {{ error }}
            </td>
          </tr>
          <tr v-else-if="data.length === 0">
            <td :colspan="columns.length" class="table-message">No data found</td>
          </tr>
          <template v-else>
            <slot name="rows" :data="data" :columns="columns">
              <tr
                v-for="row in data"
                :key="row.id || row.law_id"
                :class="{ 'clickable-row': clickableRows }"
                @click="clickableRows && emit('row-click', row)"
              >
                <td v-for="col in columns" :key="col.key">
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
                </td>
              </tr>
            </slot>
          </template>
        </tbody>
      </table>
    </div>
  </ndd-simple-section>
</template>
