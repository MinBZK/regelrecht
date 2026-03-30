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

function inputType(fieldType) {
  switch (fieldType) {
    case 'number':
    case 'amount':
      return 'number';
    default:
      return 'text';
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
</script>

<template>
  <div class="ds-table">
    <button class="ds-table-header" @click="toggleExpand" type="button">
      <span class="ds-table-toggle">{{ expanded ? '\u25BE' : '\u25B8' }}</span>
      <span class="ds-table-title">{{ title }}</span>
      <span class="ds-table-badge" v-if="rowCount > 0">{{ rowCount }}</span>
    </button>

    <div v-if="expanded" class="ds-table-body">
      <div v-if="rows.length === 0" class="ds-table-empty">
        Geen gegevens &mdash; vul in indien relevant
      </div>

      <div v-else class="ds-table-scroll">
        <table class="ds-table-grid">
          <thead>
            <tr>
              <th v-for="col in allColumns" :key="col.name" :class="{ 'ds-key-col': col.isKey }">
                {{ col.name }}
              </th>
              <th v-if="!readonly" class="ds-action-col"></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, ri) in rows" :key="row._id ?? ri">
              <td v-for="col in allColumns" :key="col.name">
                <template v-if="readonly">
                  <span class="ds-cell-readonly" :class="{ 'ds-key-col': col.isKey }">{{ row[col.name] ?? '' }}</span>
                </template>
                <template v-else-if="col.type === 'boolean'">
                  <select
                    class="ds-cell-input ds-cell-select"
                    :value="String(row[col.name] || 'null')"
                    @change="updateCell(ri, col.name, $event.target.value)"
                  >
                    <option value="true">true</option>
                    <option value="false">false</option>
                    <option value="null">null</option>
                  </select>
                </template>
                <template v-else>
                  <input
                    class="ds-cell-input"
                    :type="inputType(col.type)"
                    :value="row[col.name] ?? ''"
                    @input="updateCell(ri, col.name, $event.target.value)"
                    :placeholder="col.name"
                  />
                </template>
              </td>
              <td v-if="!readonly" class="ds-action-col">
                <button class="ds-remove-btn" @click="removeRow(ri)" type="button" title="Rij verwijderen">&times;</button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <button v-if="!readonly" class="ds-add-btn" @click="addRow" type="button">
        + Rij toevoegen
      </button>
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
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
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

.ds-table-empty {
  padding: 12px;
  text-align: center;
  font-size: 12px;
  color: var(--semantics-text-color-secondary, #999);
  font-style: italic;
}

.ds-table-scroll {
  overflow-x: auto;
}

.ds-table-grid {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.ds-table-grid th {
  padding: 4px 6px;
  text-align: left;
  font-weight: 600;
  font-size: 11px;
  color: var(--semantics-text-color-secondary, #666);
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
  white-space: nowrap;
}

.ds-table-grid td {
  padding: 2px 4px;
}

.ds-key-col {
  color: #154273 !important;
}

.ds-cell-input {
  width: 100%;
  min-width: 60px;
  padding: 4px 6px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 4px;
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  background: white;
}

.ds-cell-input:focus {
  outline: none;
  border-color: #154273;
  box-shadow: 0 0 0 1px #154273;
}

.ds-cell-select {
  min-width: 70px;
}

.ds-cell-readonly {
  display: block;
  padding: 4px 6px;
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  color: var(--semantics-text-color-primary, #1C2029);
  white-space: nowrap;
}

.ds-action-col {
  width: 28px;
  text-align: center;
}

.ds-remove-btn {
  background: none;
  border: none;
  color: #c00;
  font-size: 16px;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
  opacity: 0.5;
}

.ds-remove-btn:hover {
  opacity: 1;
}

.ds-add-btn {
  display: block;
  width: 100%;
  margin-top: 4px;
  padding: 6px;
  background: none;
  border: 1px dashed var(--semantics-dividers-color, #D0D3D8);
  border-radius: 4px;
  font-size: 12px;
  color: #154273;
  cursor: pointer;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.ds-add-btn:hover {
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
  border-color: #154273;
}
</style>
