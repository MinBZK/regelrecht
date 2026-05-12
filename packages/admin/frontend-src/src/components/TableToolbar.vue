<script setup>
import { computed, onUnmounted, useId } from 'vue';

const props = defineProps({
  columns: { type: Array, required: true },
  sortOptions: { type: Array, default: null },
  sort: { type: String, default: '' },
  order: { type: String, default: 'desc' },
  filters: { type: Object, default: () => ({}) },
});

const emit = defineEmits(['sort', 'filter-change']);

// Unique prefix for menu anchor IDs (avoid collisions when multiple toolbars exist)
const uid = useId();

const sortableColumns = computed(
  () => props.sortOptions || props.columns.filter((c) => c.sortable),
);
const filterColumns = computed(() => props.columns.filter((c) => c.filter));
const dropdownFilters = computed(() => filterColumns.value.filter((c) => c.filter.options));
const textFilters = computed(() => filterColumns.value.filter((c) => c.filter.type === 'text'));

const sortMenuItems = computed(() => {
  const items = [];
  for (const opt of sortableColumns.value) {
    if (opt.directionLabels) {
      for (const [dir, dirLabel] of Object.entries(opt.directionLabels)) {
        items.push({
          key: opt.key,
          order: dir,
          value: `${opt.key}:${dir}`,
          text: `${opt.label} (${dirLabel})`,
        });
      }
    } else {
      items.push({ key: opt.key, order: 'desc', value: opt.key, text: opt.label });
    }
  }
  return items;
});

const activeSortLabel = computed(() => {
  const opt = sortableColumns.value.find((c) => c.key === props.sort);
  if (!opt) return 'Sort';
  const dirLabel = opt.directionLabels?.[props.order];
  return dirLabel ? `Sort: ${opt.label} (${dirLabel})` : `Sort: ${opt.label}`;
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
  return `${label}: ${value || 'All'}`;
}

function onSortSelect(event) {
  const item = event.target.closest('nldd-menu-item');
  if (!item) return;
  const [key, order] = String(item.value).split(':');
  emit('sort', key, order || 'desc');
}

function onFilterSelect(col, event) {
  const item = event.target.closest('nldd-menu-item');
  if (!item) return;
  const key = getFilterKey(col);
  // Empty string value means "All" (clear filter)
  emit('filter-change', key, item.value);
}

// Text filter debounce
const debounceTimers = {};
function onTextFilter(col, event) {
  const key = getFilterKey(col);
  const raw = event.detail?.value ?? event.target?.value ?? '';
  const value = String(raw).trim();
  clearTimeout(debounceTimers[key]);
  debounceTimers[key] = setTimeout(() => {
    emit('filter-change', key, value);
  }, 300);
}

onUnmounted(() => Object.values(debounceTimers).forEach(clearTimeout));
</script>

<template>
  <div v-if="sortableColumns.length > 0 || filterColumns.length > 0" class="table-toolbar">
    <slot name="prefix" />

    <!-- Sort menu -->
    <nldd-button
      v-if="sortableColumns.length > 0"
      :id="`sort-btn-${uid}`"
      expandable
      variant="neutral-tinted"
      size="sm"
      :text="activeSortLabel"
    />
    <nldd-menu
      v-if="sortableColumns.length > 0"
      :anchor="`sort-btn-${uid}`"
      @select="onSortSelect"
    >
      <nldd-menu-item
        v-for="item in sortMenuItems"
        :key="item.value"
        type="radio"
        :text="item.text"
        :value="item.value"
        :selected="sort === item.key && order === item.order"
      />
    </nldd-menu>

    <!-- Dropdown filter menus -->
    <template v-for="col in dropdownFilters" :key="getFilterKey(col)">
      <nldd-button
        :id="`filter-btn-${uid}-${getFilterKey(col)}`"
        expandable
        variant="neutral-tinted"
        size="sm"
        :text="getFilterButtonLabel(col)"
      />
      <nldd-menu
        :anchor="`filter-btn-${uid}-${getFilterKey(col)}`"
        @select="onFilterSelect(col, $event)"
      >
        <nldd-menu-item
          type="radio"
          text="All"
          value=""
          :selected="!filters[getFilterKey(col)]"
        />
        <nldd-menu-item
          v-for="v in col.filter.options"
          :key="v"
          type="radio"
          :text="v"
          :value="v"
          :selected="filters[getFilterKey(col)] === v"
        />
      </nldd-menu>
    </template>

    <!-- Text filter inputs -->
    <nldd-search-field
      v-for="col in textFilters"
      :key="getFilterKey(col)"
      size="sm"
      width="200px"
      :placeholder="getFilterLabel(col)"
      :accessible-label="`Filter ${getFilterLabel(col)}`"
      :value="filters[getFilterKey(col)] || ''"
      @input="onTextFilter(col, $event)"
    />
  </div>
</template>
