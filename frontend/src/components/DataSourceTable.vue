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
  <div class="ds-block">
    <!-- Header -->
    <button class="ds-block-toggle" :aria-expanded="expanded" @click="toggleExpand" type="button">
      <span class="ds-block-chevron" :class="{ 'ds-block-chevron--open': expanded }">&#9656;</span>
      <ndd-title size="5">
        <h5>{{ title }}</h5>
      </ndd-title>
      <span class="ds-block-badge" v-if="rowCount > 0">{{ rowCount }}</span>
    </button>

    <div v-if="expanded" class="ds-block-body">
      <div v-if="rows.length === 0" class="ds-block-empty">
        Geen gegevens &mdash; vul in indien relevant
      </div>

      <!-- One card per data row -->
      <div v-for="(row, ri) in rows" :key="row._id ?? ri" class="ds-row-card">
        <div v-if="rows.length > 1" class="ds-row-card-header">
          <span class="ds-row-card-label">Rij {{ ri + 1 }}</span>
          <ndd-icon-button
            v-if="!readonly"
            icon="minus"
            title="Rij verwijderen"
            @click="removeRow(ri)"
          ></ndd-icon-button>
        </div>

        <ndd-list variant="box" class="ds-settings-list">
          <ndd-list-item v-for="col in allColumns" :key="col.name" size="md">
            <ndd-text-cell :text="col.name" max-width="140" :class="{ 'ds-key-label': col.isKey }"></ndd-text-cell>
            <ndd-cell>
              <template v-if="readonly">
                <ndd-text-field size="md" :value="String(row[col.name] ?? '')" readonly></ndd-text-field>
              </template>
              <template v-else-if="col.type === 'boolean'">
                <ndd-dropdown size="md">
                  <select
                    :aria-label="col.name"
                    :value="String(row[col.name] || 'null')"
                    @change="updateCell(ri, col.name, $event.target.value)"
                  >
                    <option value="true">true</option>
                    <option value="false">false</option>
                    <option value="null">null</option>
                  </select>
                </ndd-dropdown>
              </template>
              <template v-else>
                <ndd-text-field
                  size="md"
                  :value="row[col.name] ?? ''"
                  :placeholder="col.name"
                  @input="updateCell(ri, col.name, $event.target?.value ?? $event.detail?.value ?? '')"
                ></ndd-text-field>
              </template>
            </ndd-cell>
          </ndd-list-item>

          <!-- Single-row inline delete -->
          <ndd-list-item v-if="!readonly && rows.length === 1" size="md">
            <ndd-button size="md" full-width start-icon="minus" @click="removeRow(ri)" text="Rij verwijderen"></ndd-button>
          </ndd-list-item>
        </ndd-list>
      </div>

      <ndd-button v-if="!readonly" size="md" full-width start-icon="plus-small" @click="addRow" text="Rij toevoegen"></ndd-button>
    </div>
  </div>
</template>

<style scoped>
.ds-block + .ds-block {
  margin-top: 12px;
}

.ds-block-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  background: none;
  border: none;
  cursor: pointer;
  padding: 0;
  width: 100%;
  margin-bottom: 4px;
}

.ds-block-chevron {
  display: inline-block;
  font-size: 12px;
  transition: transform 0.15s ease;
  flex-shrink: 0;
}

.ds-block-chevron--open {
  transform: rotate(90deg);
}

.ds-block-badge {
  font-size: 11px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 4px;
  background: #154273;
  color: white;
  flex-shrink: 0;
}

.ds-block-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ds-block-empty {
  padding: 12px;
  text-align: center;
  font-size: 14px;
  color: var(--semantics-text-color-secondary, #999);
  font-style: italic;
}

.ds-row-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 2px;
}

.ds-row-card-label {
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
  font-size: 13px;
  font-weight: 600;
  color: var(--semantics-text-color-secondary, #545D68);
}
</style>

<style>
/* Unscoped: ndd web components need global selectors */
.ds-settings-list ndd-text-cell {
  width: 140px;
  min-width: 140px;
  flex-shrink: 0;
}

.ds-settings-list ndd-cell {
  flex: 1;
  min-width: 0;
}

.ds-settings-list ndd-text-field,
.ds-settings-list ndd-dropdown {
  width: 100%;
}

.ds-key-label {
  font-weight: 700;
}
</style>
