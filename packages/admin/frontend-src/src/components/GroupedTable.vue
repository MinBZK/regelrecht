<script setup>
import StatusBadge from './StatusBadge.vue';
import TableToolbar from './TableToolbar.vue';
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

const statusCountKeys = ['pending', 'processing', 'completed', 'failed'];

function sortLabel(col) {
  if (!col.sortable) return col.label;
  if (props.sort !== col.key) return col.label;
  return `${col.label} ${props.order === 'asc' ? '\u2191' : '\u2193'}`;
}

function formatChildCell(value, key) {
  if (value === null || value === undefined || value === '') return null;
  if (key === 'id') return truncateUuid(value);
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
          :width="col.width || 'stretch'"
        ></ndd-title-cell>
        <ndd-cell width="fit-content"></ndd-cell>
      </ndd-list-item>

      <!-- Group rows -->
      <template v-for="group in data" :key="group.law_id">
        <!-- Group header -->
        <ndd-list-item
          size="md"
          type="button"
          class="group-row"
          @click="emit('toggle-expand', group.law_id)"
        >
          <template v-for="col in columns" :key="col.key">
            <ndd-cell :width="col.width || 'stretch'">
              <div class="cell-wrap">
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
              </div>
            </ndd-cell>
          </template>
          <ndd-cell width="fit-content">
            {{ expandedLawIds.has(group.law_id) ? '\u25B2' : '\u25BC' }}
          </ndd-cell>
        </ndd-list-item>

        <!-- Expanded child rows -->
        <template v-if="expandedLawIds.has(group.law_id)">
          <ndd-list-item v-if="!expandedJobsCache[group.law_id]" size="sm">
            <ndd-text-cell text="Loading jobs…"></ndd-text-cell>
          </ndd-list-item>
          <ndd-list-item v-else-if="expandedJobsCache[group.law_id].length === 0" size="sm">
            <ndd-text-cell text="No jobs found"></ndd-text-cell>
          </ndd-list-item>
          <template v-else>
            <!-- Child header -->
            <ndd-list-item size="sm" class="child-header">
              <ndd-title-cell
                v-for="col in childColumns"
                :key="col.key"
                :text="col.label"
                :width="col.width || 'stretch'"
              ></ndd-title-cell>
            </ndd-list-item>
            <!-- Child job rows -->
            <ndd-list-item
              v-for="job in expandedJobsCache[group.law_id]"
              :key="job.id"
              size="md"
              type="button"
              class="child-row"
              @click.stop="emit('row-click', job)"
            >
              <template v-for="col in childColumns" :key="col.key">
                <ndd-cell :width="col.width || 'stretch'">
                  <div class="cell-wrap">
                    <template v-if="col.key === '_error'">
                      <span
                        v-if="job.result && job.result.error"
                        class="cell-error"
                        :title="job.result.error"
                      >{{ truncateError(job.result.error) }}</span>
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
                  </div>
                </ndd-cell>
              </template>
            </ndd-list-item>
          </template>
        </template>
      </template>
    </ndd-list>
  </ndd-simple-section>
</template>
