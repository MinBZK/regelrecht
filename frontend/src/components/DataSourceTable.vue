<script setup>
import { ref, computed } from 'vue';

let nextRowId = 0;

const props = defineProps({
  title: { type: String, required: true },
  keyField: { type: String, default: 'bsn' },
  fields: { type: Array, required: true },
  modelValue: { type: Array, default: () => [] },
  defaultExpanded: { type: Boolean, default: false },
  readonly: { type: Boolean, default: false },
});

const emit = defineEmits(['update:modelValue']);

const expanded = ref(props.defaultExpanded);

const rows = computed({
  get: () => props.modelValue,
  set: (val) => emit('update:modelValue', val),
});

function toggleExpand() {
  expanded.value = !expanded.value;
}

function addRow() {
  const newRow = { _id: ++nextRowId };
  // Pre-fill key field if there's a common value from existing rows
  newRow[props.keyField] = rows.value.length > 0
    ? rows.value[0][props.keyField] || ''
    : '';
  for (const field of props.fields) {
    if (!(field.name in newRow)) {
      newRow[field.name] = defaultForType(field.type);
    }
  }
  rows.value = [...rows.value, newRow];
}

function removeRow(index) {
  const updated = [...rows.value];
  updated.splice(index, 1);
  rows.value = updated;
}

function updateCell(rowIndex, fieldName, value) {
  const updated = rows.value.map((row, i) => {
    if (i !== rowIndex) return row;
    return { ...row, [fieldName]: value };
  });
  rows.value = updated;
}

function defaultForType(type) {
  switch (type) {
    case 'number':
    case 'amount':
      return '';
    case 'boolean':
      return 'false';
    default:
      return '';
  }
}

// All columns: key field + declared fields (deduplicated)
const allColumns = computed(() => {
  const cols = [];
  const seen = new Set();

  // Key field first
  seen.add(props.keyField);
  cols.push({ name: props.keyField, type: 'string', isKey: true });

  for (const field of props.fields) {
    if (!seen.has(field.name)) {
      seen.add(field.name);
      cols.push({ ...field, isKey: false });
    }
  }

  return cols;
});

const rowCount = computed(() => rows.value.length);

const PAGE_SIZE = 10;
const currentPage = ref(1);
const totalPages = computed(() => Math.max(1, Math.ceil(rowCount.value / PAGE_SIZE)));

const paginatedRows = computed(() => {
  const start = (currentPage.value - 1) * PAGE_SIZE;
  return rows.value.slice(start, start + PAGE_SIZE);
});

function actualIndex(paginatedIndex) {
  return (currentPage.value - 1) * PAGE_SIZE + paginatedIndex;
}

function onPageChange(event) {
  currentPage.value = event.detail.page;
}
</script>

<template>
  <div class="ds-table">
    <button class="ds-table-header" @click="toggleExpand" type="button">
      <span class="ds-table-toggle">{{ expanded ? '\u25BE' : '\u25B8' }}</span>
      <span class="ds-table-title">{{ title }}</span>
      <span class="ds-table-badge" v-if="rowCount > 0">{{ rowCount }}</span>
    </button>

    <div v-if="expanded" class="ds-table-body">
      <ndd-inline-dialog v-if="rows.length === 0" text="Geen gegevens — vul in indien relevant"></ndd-inline-dialog>

      <ndd-list v-else variant="simple">
        <!-- Header row with title cells -->
        <ndd-list-item size="sm">
          <ndd-title-cell v-for="col in allColumns" :key="col.name" :text="col.name"></ndd-title-cell>
          <ndd-cell v-if="!readonly" width="fit-content"></ndd-cell>
        </ndd-list-item>

        <!-- Data rows -->
        <ndd-list-item v-for="(row, ri) in paginatedRows" :key="row._id ?? ri" size="md">
          <template v-for="col in allColumns" :key="col.name">
            <!-- Readonly -->
            <ndd-text-cell v-if="readonly" :text="String(row[col.name] ?? '')"></ndd-text-cell>

            <!-- Boolean editable -->
            <ndd-cell v-else-if="col.type === 'boolean'">
              <ndd-dropdown size="md">
                <select
                  :value="String(row[col.name] || 'null')"
                  @change="updateCell(actualIndex(ri), col.name, $event.target.value)"
                  :aria-label="col.name"
                >
                  <option value="true">true</option>
                  <option value="false">false</option>
                  <option value="null">null</option>
                </select>
              </ndd-dropdown>
            </ndd-cell>

            <!-- Text/number editable -->
            <ndd-cell v-else>
              <ndd-text-field
                size="md"
                :value="row[col.name] ?? ''"
                :placeholder="col.name"
                @input="updateCell(actualIndex(ri), col.name, $event.target?.value ?? $event.detail?.value ?? '')"
              ></ndd-text-field>
            </ndd-cell>
          </template>

          <!-- Delete button -->
          <ndd-cell v-if="!readonly" width="fit-content" vertical-alignment="center">
            <ndd-icon-button icon="minus" @click="removeRow(actualIndex(ri))" title="Rij verwijderen"></ndd-icon-button>
          </ndd-cell>
        </ndd-list-item>
      </ndd-list>

      <ndd-pagination
        v-if="totalPages > 1"
        :current="currentPage"
        :total="totalPages"
        full-width
        @page-change="onPageChange"
      ></ndd-pagination>

      <ndd-spacer v-if="!readonly" size="4"></ndd-spacer>
      <ndd-button v-if="!readonly" start-icon="plus-small" @click="addRow" text="Rij toevoegen" style="width: 100%;"></ndd-button>
    </div>
  </div>
</template>

<style scoped>
.ds-table {
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 8px;
  overflow: hidden;
}

.ds-table + .ds-table {
  margin-top: 8px;
}

.ds-table-header {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 10px 12px;
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
  border: none;
  cursor: pointer;
  font-size: 13px;
  font-weight: 600;
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
  text-align: left;
  color: var(--semantics-text-color-primary, #1C2029);
}

.ds-table-header:hover {
  background: var(--semantics-surfaces-color-tertiary, #F0F1F3);
}

.ds-table-toggle {
  flex-shrink: 0;
  width: 12px;
  font-size: 11px;
  color: var(--semantics-text-color-secondary, #666);
}

.ds-table-title {
  flex: 1;
}

.ds-table-badge {
  font-size: 11px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 4px;
  background: #154273;
  color: white;
}

.ds-table-body {
  border-top: 1px solid var(--semantics-dividers-color, #E0E3E8);
  padding: 8px;
}
</style>
