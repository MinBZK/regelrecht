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
  <div>
    <!-- Header: accordion toggle, or a plain heading when drilled in -->
    <button v-if="!drilledIn" class="ds-block-toggle" :aria-expanded="expanded" @click="toggleExpand" type="button">
      <span class="ds-block-chevron" :class="{ 'ds-block-chevron--open': expanded }">&#9656;</span>
      <nldd-title size="5" style="flex: 1; text-align: left;">
        <span>{{ title }}</span>
      </nldd-title>
      <span class="ds-block-badge" v-if="rowCount > 0">{{ rowCount }}</span>
    </button>
    <template v-else>
      <nldd-title size="3" :id="anchorId || undefined">
        <h2>{{ title }}</h2>
      </nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
    </template>

    <template v-if="showBody">
      <nldd-inline-dialog v-if="rows.length === 0" text="Geen gegevens — vul in indien relevant"></nldd-inline-dialog>

      <!-- One box list per row — identical layout regardless of row count,
           with the delete button at the bottom of each list. Spacers (not a
           flex-gap container) separate the stacked lists. -->
      <template v-for="(row, ri) in rows" :key="row._id ?? ri">
        <nldd-spacer v-if="ri > 0" size="12"></nldd-spacer>
        <nldd-list variant="box">
        <nldd-list-item v-for="col in allColumns" :key="col.name" size="md">
          <nldd-text-cell :text="col.name" min-width="120px" max-width="200px"></nldd-text-cell>
          <nldd-spacer-cell v-if="!readonly" size="8"></nldd-spacer-cell>
          <template v-if="readonly">
            <nldd-text-cell :text="String(row[col.name] ?? '')"></nldd-text-cell>
          </template>
          <nldd-cell v-else-if="col.type === 'boolean'" width="full" min-width="120px">
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
          <nldd-cell v-else width="full" min-width="120px">
            <nldd-text-field
              size="md"
              :type="inputType(col.type)"
              :value="String(row[col.name] ?? '')"
              @input="updateCell(ri, col.name, $event.target?.value ?? $event.detail?.value ?? '')"
            ></nldd-text-field>
          </nldd-cell>
        </nldd-list-item>

        <nldd-list-item v-if="!readonly" size="md">
          <nldd-cell width="full">
            <nldd-button variant="destructive" size="md" width="full" start-icon="delete" @click="removeRow(ri)" text="Verwijder"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
        </nldd-list>
      </template>

      <nldd-spacer v-if="!readonly" size="12"></nldd-spacer>
      <nldd-list v-if="!readonly" variant="box">
        <nldd-list-item size="md">
          <nldd-cell width="full">
            <nldd-button size="md" width="full" start-icon="plus-small" @click="addRow" text="Voeg toe"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </template>
  </div>
</template>

<style scoped>
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
  background: var(--color-primary);
  color: white;
  flex-shrink: 0;
}

</style>
