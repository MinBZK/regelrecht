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
  // When the table is shown one level deep in a drill-in sheet there's no
  // accordion: the title is a plain heading and the body is always visible.
  drilledIn: { type: Boolean, default: false },
  // Optional id put on the drilled-in heading so the sheet's top-title-bar
  // can use it as its `collapse-anchor` (full back button until scrolled).
  anchorId: { type: String, default: '' },
  // Secondary line under the drilled-in heading (e.g. the scenario name).
  subtitle: { type: String, default: '' },
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

// Drilled-in tables have no toggle, so the body is always shown.
const showBody = computed(() => props.drilledIn || expanded.value);
</script>

<template>
  <div class="ds-block">
    <!-- Header: accordion toggle, or a plain heading when drilled in -->
    <button v-if="!drilledIn" class="ds-block-toggle" :aria-expanded="expanded" @click="toggleExpand" type="button">
      <span class="ds-block-chevron" :class="{ 'ds-block-chevron--open': expanded }">&#9656;</span>
      <nldd-title size="5" style="flex: 1; text-align: left;">
        <span>{{ title }}</span>
      </nldd-title>
      <span class="ds-block-badge" v-if="rowCount > 0">{{ rowCount }}</span>
    </button>
    <template v-else>
      <nldd-title size="5" class="ds-block-heading" :id="anchorId || undefined">
        <span>{{ title }}</span>
      </nldd-title>
      <p v-if="subtitle" class="ds-block-subtitle">{{ subtitle }}</p>
      <nldd-spacer size="8"></nldd-spacer>
    </template>

    <div v-if="showBody" class="ds-block-body">
      <div v-if="rows.length === 0" class="ds-block-empty">
        Geen gegevens &mdash; vul in indien relevant
      </div>

      <!-- One box list per row — identical layout regardless of row count,
           with the delete button at the bottom of each list. -->
      <nldd-list
        v-for="(row, ri) in rows"
        :key="row._id ?? ri"
        variant="box"
        class="ds-datasource-list"
      >
        <nldd-list-item v-for="col in allColumns" :key="col.name" size="md">
          <nldd-text-cell :text="col.name" max-width="140px" :class="{ 'ds-key-label': col.isKey }"></nldd-text-cell>
          <nldd-spacer-cell v-if="!readonly" size="8"></nldd-spacer-cell>
          <template v-if="readonly">
            <nldd-text-cell :text="String(row[col.name] ?? '')"></nldd-text-cell>
          </template>
          <nldd-cell v-else-if="col.type === 'boolean'">
            <nldd-dropdown size="md">
              <select
                :aria-label="col.name"
                :value="String(row[col.name] || 'null')"
                @change="updateCell(ri, col.name, $event.target.value)"
              >
                <option value="true">true</option>
                <option value="false">false</option>
                <option value="null">null</option>
              </select>
            </nldd-dropdown>
          </nldd-cell>
          <nldd-cell v-else>
            <nldd-text-field
              size="md"
              :type="inputType(col.type)"
              :value="String(row[col.name] ?? '')"
              :placeholder="col.name"
              @input="updateCell(ri, col.name, $event.target?.value ?? $event.detail?.value ?? '')"
            ></nldd-text-field>
          </nldd-cell>
        </nldd-list-item>

        <nldd-list-item v-if="!readonly" size="md">
          <nldd-cell width="full">
            <nldd-button variant="destructive" size="md" width="full" start-icon="minus" @click="removeRow(ri)" text="Verwijder"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>

      <nldd-list v-if="!readonly" variant="box">
        <nldd-list-item size="md">
          <nldd-cell width="full">
            <nldd-button size="md" width="full" start-icon="plus-small" @click="addRow" text="Voeg toe"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
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

.ds-block-heading {
  margin: 0;
}

.ds-block-subtitle {
  margin: 2px 0 0 0;
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
  font-size: 13px;
  color: var(--semantics-text-color-secondary, #545D68);
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
  background: var(--color-primary);
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

.ds-key-label {
  font-weight: 700;
}
</style>

<style>
/* Unscoped: nldd web components need global selectors */
.ds-datasource-list nldd-text-cell {
  width: 140px;
  min-width: 140px;
  flex-shrink: 0;
}

.ds-datasource-list nldd-cell {
  flex: 1;
  min-width: 0;
}

.ds-datasource-list nldd-text-field,
.ds-datasource-list nldd-dropdown {
  width: 100%;
}
</style>
