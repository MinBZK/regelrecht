<script setup>
import { computed } from 'vue';
import StatusBadge from './StatusBadge.vue';
import { GROUPED_COLUMNS, JOB_COLUMNS, STATUS_BADGE_MAP } from '../constants.js';
import { formatDate, truncateUuid, truncateError } from '../formatters.js';

const props = defineProps({
  data: { type: Array, required: true },
  loading: { type: Boolean, default: false },
  error: { type: String, default: null },
  sort: { type: String, default: '' },
  order: { type: String, default: 'desc' },
  filters: { type: Object, default: () => ({}) },
  expandedLawIds: { type: Set, required: true },
  expandedJobsCache: { type: Object, required: true },
});

const emit = defineEmits(['sort', 'filter-change', 'toggle-expand', 'row-click']);

const columns = GROUPED_COLUMNS;
const childColumns = JOB_COLUMNS;
const colCount = computed(() => columns.length + 1);

const statusCountKeys = ['pending', 'processing', 'completed', 'failed'];

function onHeaderClick(col, event) {
  if (event.target.closest('.th-filter')) return;
  if (col.sortable) emit('sort', col.key);
}

function onSelectFilter(key, event) {
  emit('filter-change', key, event.target.value);
}

function getFilterKey(col) {
  return col.filter?.key || col.key;
}

function getFilterLabel(col) {
  return col.filter?.label || col.label;
}

function formatChildCell(value, key) {
  if (value === null || value === undefined || value === '') return null;
  if (key === 'id') return truncateUuid(value);
  if (key.endsWith('_at')) return formatDate(value);
  return String(value);
}
</script>

<template>
  <rr-simple-section>
    <div class="table-container">
      <table class="data-table">
        <thead>
          <tr>
            <th
              v-for="col in columns"
              :key="col.key"
              :class="{ sortable: col.sortable, 'sort-active': sort === col.key }"
              @click="onHeaderClick(col, $event)"
            >
              <span class="th-label">
                {{ col.label }}
                <span v-if="col.sortable" class="sort-indicator">
                  {{ sort === col.key ? (order === 'asc' ? '\u25B2' : '\u25BC') : '\u25BC' }}
                </span>
              </span>
              <div v-if="col.filter" class="th-filter" @click.stop>
                <select
                  v-if="col.filter.options"
                  :aria-label="'Filter ' + getFilterLabel(col)"
                  :value="filters[getFilterKey(col)] || ''"
                  @change="onSelectFilter(getFilterKey(col), $event)"
                >
                  <option value="">All {{ getFilterLabel(col) }}</option>
                  <option v-for="v in col.filter.options" :key="v" :value="v">{{ v }}</option>
                </select>
              </div>
            </th>
            <th style="width: 40px"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="loading && data.length === 0">
            <td :colspan="colCount" class="table-message">Loading&hellip;</td>
          </tr>
          <tr v-else-if="error && data.length === 0">
            <td :colspan="colCount" class="table-message table-message--error">
              Failed to load data: {{ error }}
            </td>
          </tr>
          <tr v-else-if="data.length === 0">
            <td :colspan="colCount" class="table-message">No data found</td>
          </tr>
          <template v-else>
            <template v-for="group in data" :key="group.law_id">
              <!-- Group header row -->
              <tr
                class="group-row"
                :class="{ 'group-row--expanded': expandedLawIds.has(group.law_id) }"
                @click="emit('toggle-expand', group.law_id)"
              >
                <td v-for="col in columns" :key="col.key">
                  <span v-if="col.key === 'law_id'" class="cell-mono">{{ group.law_id }}</span>
                  <template v-else-if="statusCountKeys.includes(col.key)">
                    <span v-if="group[col.key] === 0" class="cell-null">0</span>
                    <span v-else class="badge" :class="'badge--' + (STATUS_BADGE_MAP[col.key] || 'grey')">
                      {{ group[col.key] }}
                    </span>
                  </template>
                  <template v-else-if="col.key.endsWith('_at') && formatDate(group[col.key])">
                    {{ formatDate(group[col.key]) }}
                  </template>
                  <template v-else-if="group[col.key] != null">{{ group[col.key] }}</template>
                  <span v-else class="cell-null">&mdash;</span>
                </td>
                <td class="group-row__toggle">
                  {{ expandedLawIds.has(group.law_id) ? '\u25B2' : '\u25BC' }}
                </td>
              </tr>

              <!-- Expanded child rows -->
              <template v-if="expandedLawIds.has(group.law_id)">
                <tr v-if="!expandedJobsCache[group.law_id]" class="child-row">
                  <td :colspan="colCount" class="table-message">Loading jobs&hellip;</td>
                </tr>
                <tr v-else-if="expandedJobsCache[group.law_id].length === 0" class="child-row">
                  <td :colspan="colCount" class="table-message">No jobs found</td>
                </tr>
                <template v-else>
                  <!-- Child header -->
                  <tr class="child-header">
                    <td v-for="col in childColumns" :key="col.key" class="child-header__cell">
                      {{ col.label }}
                    </td>
                  </tr>
                  <!-- Child job rows -->
                  <tr
                    v-for="job in expandedJobsCache[group.law_id]"
                    :key="job.id"
                    class="child-row clickable-row"
                    @click.stop="emit('row-click', job)"
                  >
                    <td v-for="col in childColumns" :key="col.key">
                      <template v-if="col.key === '_error'">
                        <span
                          v-if="job.result && job.result.error"
                          class="cell-error"
                          :title="job.result.error"
                        >
                          {{ truncateError(job.result.error) }}
                        </span>
                        <span v-else class="cell-null">&mdash;</span>
                      </template>
                      <StatusBadge v-else-if="col.key === 'status'" :status="job[col.key] || 'unknown'" />
                      <span v-else-if="col.key === 'id'" class="cell-mono" :title="String(job[col.key])">
                        {{ formatChildCell(job[col.key], col.key) }}
                      </span>
                      <span v-else-if="col.key === 'law_id'" class="cell-mono">{{ job[col.key] }}</span>
                      <template v-else-if="formatChildCell(job[col.key], col.key) !== null">
                        {{ formatChildCell(job[col.key], col.key) }}
                      </template>
                      <span v-else class="cell-null">&mdash;</span>
                    </td>
                  </tr>
                </template>
              </template>
            </template>
          </template>
        </tbody>
      </table>
    </div>
  </rr-simple-section>
</template>
