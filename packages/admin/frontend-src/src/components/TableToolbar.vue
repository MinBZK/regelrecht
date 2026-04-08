<script setup>
import { computed, onUnmounted, useId } from 'vue';

const props = defineProps({
  columns: { type: Array, required: true },
  sort: { type: String, default: '' },
  order: { type: String, default: 'desc' },
  filters: { type: Object, default: () => ({}) },
});

const emit = defineEmits(['sort', 'filter-change']);

// Unique prefix for menu anchor IDs (avoid collisions when multiple toolbars exist)
const uid = useId();

const sortableColumns = computed(() => props.columns.filter((c) => c.sortable));
const filterColumns = computed(() => props.columns.filter((c) => c.filter));
const dropdownFilters = computed(() => filterColumns.value.filter((c) => c.filter.options));
const textFilters = computed(() => filterColumns.value.filter((c) => c.filter.type === 'text'));

const activeSortLabel = computed(() => {
  const col = sortableColumns.value.find((c) => c.key === props.sort);
  if (!col) return 'Sort';
  const arrow = props.order === 'asc' ? '\u2191' : '\u2193';
  return `${col.label} ${arrow}`;
});

function getFilterKey(col) {
  return col.filter?.key || col.key;
}

function getFilterLabel(col) {
  return col.filter?.label || col.label;
}

function getFilterButtonLabel(col) {
  const key = getFilterKey(col);
  const value = props.filters[key];
  const label = getFilterLabel(col);
  if (value) return `${label}: ${value}`;
  return label;
}

function onSortSelect(event) {
  const item = event.target.closest('ndd-menu-item');
  if (!item) return;
  emit('sort', item.value);
}

function onFilterSelect(col, event) {
  const item = event.target.closest('ndd-menu-item');
  if (!item) return;
  const key = getFilterKey(col);
  // Empty string value means "All" (clear filter)
  emit('filter-change', key, item.value);
}

// Text filter debounce
const debounceTimers = {};
function onTextFilter(col, event) {
  const key = getFilterKey(col);
  const value = event.target.value.trim();
  clearTimeout(debounceTimers[key]);
  debounceTimers[key] = setTimeout(() => {
    emit('filter-change', key, value);
  }, 300);
}

onUnmounted(() => Object.values(debounceTimers).forEach(clearTimeout));
</script>

<template>
  <div v-if="sortableColumns.length > 0 || filterColumns.length > 0" class="table-toolbar">
    <!-- Sort menu -->
    <ndd-button
      v-if="sortableColumns.length > 0"
      :id="`sort-btn-${uid}`"
      is-expandable
      variant="neutral-tinted"
      size="sm"
    >{{ activeSortLabel }}</ndd-button>
    <ndd-menu
      v-if="sortableColumns.length > 0"
      :anchor="`sort-btn-${uid}`"
      @select="onSortSelect"
    >
      <ndd-menu-item
        v-for="col in sortableColumns"
        :key="col.key"
        :text="col.label"
        :value="col.key"
        :selected="sort === col.key"
      />
    </ndd-menu>

    <!-- Dropdown filter menus -->
    <template v-for="col in dropdownFilters" :key="getFilterKey(col)">
      <ndd-button
        :id="`filter-btn-${uid}-${getFilterKey(col)}`"
        is-expandable
        variant="neutral-tinted"
        size="sm"
      >{{ getFilterButtonLabel(col) }}</ndd-button>
      <ndd-menu
        :anchor="`filter-btn-${uid}-${getFilterKey(col)}`"
        @select="onFilterSelect(col, $event)"
      >
        <ndd-menu-item
          text="All"
          value=""
          :selected="!filters[getFilterKey(col)]"
        />
        <ndd-menu-item
          v-for="v in col.filter.options"
          :key="v"
          :text="v"
          :value="v"
          :selected="filters[getFilterKey(col)] === v"
        />
      </ndd-menu>
    </template>

    <!-- Text filter inputs -->
    <ndd-text-field
      v-for="col in textFilters"
      :key="getFilterKey(col)"
      size="sm"
      :placeholder="`Filter ${getFilterLabel(col)}…`"
      :accessible-label="`Filter ${getFilterLabel(col)}`"
      :value="filters[getFilterKey(col)] || ''"
      @input="onTextFilter(col, $event)"
    />
  </div>
</template>
